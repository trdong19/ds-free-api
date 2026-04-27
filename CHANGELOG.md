# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.4] - 2026-04-26

### Added
- **限流自动检测与指数退避重试**：ds_core 在发送流给 adapter 前先消费前两个 SSE 事件，
  检测到 `hint` + `rate_limit_reached` 时返回 `CoreError::Overloaded`，
  `try_chat()` 以 1s→2s→4s→8s→16s 指数间隔自动重试（最多 6 次），session 状态不受污染
- **`stop_stream` 端点集成**：添加 `DsClient::stop_stream()` 方法，在 `GuardedStream` 的
  `PinnedDrop` 中当流被提前丢弃时自动调用，通知 DeepSeek 端停止生成
- **动态 message_id 追踪**：从 SSE `ready` 事件中解析 `request_message_id` 和
  `response_message_id`，支持同一 session 内多次编辑，替代硬编码值
- **SessionInfo 结构**：将 `sessions` 从 `HashMap<String, String>` 升级为
  `RwLock<HashMap<String, SessionInfo>>`，每个 session 独立追踪 `next_message_id`
- **`finished` 标记**：`GuardedStream` 仅在流未自然完成时触发 `stop_stream`，
  正常完成不再发送停止信号
- **工具调用自修复（live model fallback）**：当 tool_calls XML 解析失败时，
  使用 DeepSeek 模型自身修复损坏的 JSON，提升工具调用稳定性
- **`aggregate()` 透传修复管道**：非流式请求现在也走完整的 tool_calls 修复流程，
  不再跳过 `repair_fn`，流式与非流式行为一致
- **`parse_tool_calls` arguments 类型归一**：当 arguments 是 JSON 字符串而非对象时，
  自动解析为对象后重新序列化，避免双重转义导致客户端解析错误
- **request_id 跨层日志追踪**：在 handler 入口生成 `req-{n}` 标识，沿 adapter → ds_core
  逐层传递，关键日志带 `req=` 前缀，`grep req=xxx` 可追踪单次请求全链路
- **x-ds-account 响应头**：通过 `ChatResult<T>` / `ChatResponse` 将 `account_id` 从
  ds_core 回流到 handler，注入 HTTP 响应头，方便客户端识别处理请求的账号
- **SseBody `with_header()` 构建器**：支持流式响应中注入自定义 HTTP 响应头
- **TRACE 日志贯穿流管道**：state → converter → repair → stop 各层增加 `>>> stage: ...`
  格式的 TRACE 日志，配合 SSE 层的 `<<< event data` 可观察字节在管道中的完整转换过程
- **pre-commit 钩子**：`.git/hooks/pre-commit`，顺序执行与 CI 一致的全量检查
  （check → clippy → fmt → audit → machete → test），工具未安装时友好跳过
- **`Account::display_id()`**：新公开方法返回账号标识（email 优先，mobile 兜底）
- **账号初始化并发限流**：`AccountPool::init()` 通过 `tokio::sync::Semaphore` 限制
  并发初始化数为 13，避免对 DeepSeek 端和本地连接池造成压力
- **e2e 测试重构**：从 pytest 迁移为 JSON 场景驱动框架（`runner.py` + `stress_runner.py`），
  场景文件独立存放，配置从 `config.toml` 动态读取账号数和 api_key

### Changed
- `Account::session_id()` 返回 `Option<String>` 替代 `Option<&str>`，适配内部锁
- 账号分配日志从 `info` 降为 `debug` 级别
- SSE trace 日志精简为单行 `<event> <data>` 格式
- 空内容警告不再误报工具调用场景（`has_tool_calls=true` 时跳过）
- `justfile` 精简：移除旧 pytest 靶子，保留正交的 `e2e-basic` / `e2e-repair` / `e2e-stress`
- 更新中英文 README，增加限流处理与并发策略说明
- 更新 `docs/deepseek-api-reference.md`：添加 `stop_stream` 端点及 message_id 实际模式
- **日志级别规范化**：全面审计并修正各级别使用
  - 账号池耗尽 `INFO` → `WARN`
  - SSE 流错误 `DEBUG` → `WARN`
  - tool_parser 修复触发 `DEBUG` → `WARN`
  - tool_calls 修复成功 `DEBUG` → `INFO`
  - 新增 ERROR：所有账号初始化失败、PoW 计算失败
- `health_check` 日志增加 `account=` 字段，区分并发初始化时的多账号输出
- `stream_tool_calls_repair_with_live_ds` 测试标记为 `#[ignore]`，仅手动调用
- 同步更新 `docs/logging-spec.md`，补充 `anthropic_compat::*`、`http::*` 等实际 target
- 默认并行数改为 2，推荐并行数 = 账号数 ÷ 2

### Ref
- [#19](https://github.com/NIyueeE/ds-free-api/pull/19) — 参考 `x-ds-account` 设计思路

### Removed
- 移除 `examples/adapter_cli/` 中冗余的 allow 注释
- 移除旧 pytest e2e 测试目录及遗留脚本

### Stress Test Results
- **4 账号 + 3 并发 + 3 迭代**：17 场景（7 basic + 10 repair）× 2 模型 × 3 次迭代 = 102 次请求全部通过，成功率 100%，总耗时 5.9 分钟
- 覆盖场景：基础对话、深度思考、流式、标准工具调用，以及 10 种 tool_calls 损坏格式
  （XML 风格、XML+JSON 混合、字段名不一致、arguments 为字符串、括号不匹配、
  括号缺失、name/arguments 互换、参数外溢等），修复管道全部正确兜底
- 验证结论：tool_calls 自修复 + `aggregate()` 修复透传 + `parse_tool_calls` 类型归一
  三层保障下，模型输出的各类非标准格式均能被正确修复，不产生 500 错误

## [0.2.3] - 2026-04-24

### Added
- Tool call XML 解析增强：增加 `repair_invalid_backslashes` 与 `repair_unquoted_keys`
  宽松修复，当模型输出的 JSON 包含未引号 key 或无效转义时自动修复后重试
- 增加 `is_inside_code_fence` 检查：跳过 markdown 代码块中的工具示例，防止误解析
- 新增 Anthropic 协议压测脚本 `stress_test_tools_anthropic.py`，与 OpenAI 版对称
- 示例文件正交化：`examples/adapter_cli/` 下按功能拆分为
  `basic_chat`/`stream`/`stop`/`reasoning`/`web_search`/`reasoning_search`/`tool_call` 等独立文件
- 默认 adapter-cli 配置文件路径指向 `py-e2e-tests/config.toml`

### Changed
- 账号池选择策略：从**轮询线性探测**改为**空闲最久优先**，最大化账号复用间隔
- 移除固定的冷却时间常量，选择算法天然避免账号被过快重用
- 同步更新中英文 README，增加并发经验说明

### Stress Test Results

针对 4 账号池的 70 请求压测（7 场景 × 2 模型 × 5 迭代）：

| 策略 | 并发 | 成功率 | 平均耗时 |
|------|------|--------|----------|
| 轮询 + 无冷却 | 3 | 25.7% | 2.57s |
| 轮询 + 2s 冷却 | 3 | 97.1% | 10.46s |
| **空闲最久优先 + 无冷却** | **2** | **100%** | **10.14s** |
| **空闲最久优先 + 无冷却 (Anthropic)** | **2** | **100%** | **11.31s** |

结论：稳定安全并发 ≈ 账号数 ÷ 2，空闲最久优先策略可在不设冷却的前提下实现 100% 成功率。

## [0.2.2] - 2026-04-22

### Added
- Anthropic Messages API 兼容层：
  - `/anthropic/v1/messages` streaming + non-streaming 端点
  - `/anthropic/v1/models` list/get 端点（Anthropic 格式）
  - 请求映射：Anthropic JSON → OpenAI ChatCompletion
  - 响应映射：OpenAI SSE/JSON → Anthropic Message SSE/JSON
- OpenAI adapter 向后兼容：
  - 已弃用的 `functions`/`function_call` 自动映射为 `tools`/`tool_choice`
  - `response_format` 降级：在 ChatML prompt 中注入 JSON/Schema 约束（`text` 类型为 no-op）
- CI 发布流程改进：
  - tag 触发 release（`push.tags v*`）
  - CHANGELOG 自动提取版本说明
  - 发布前校验 Cargo.toml 版本与 tag 一致

### Changed
- Rust toolchain 升级到 1.95.0，CI workflow 同步更新
- justfile 添加 `set positional-arguments`，安全传递带空格的参数
- Python E2E 测试套件重组为 `openai_endpoint/` 和 `anthropic_endpoint/`
- 启动日志显示 OpenAI 和 Anthropic base URLs
- README/README.en.md 添加 SVG 图标、GitHub badges、同步文档
- LICENSE 添加版权声明 `Copyright 2026 NIyueeE`
- CLAUDE.md/AGENTS.md 同步更新

### Fixed
- Anthropic 流式工具调用协议：使用 `input_json_delta` 事件逐步传输工具参数
- Tool use ID 映射一致性：`call_{suffix}` → `toolu_{suffix}`
- Anthropic 工具定义兼容：处理缺少 `type` 字段的情况（Claude Code 客户端）

## [0.2.1] - 2026-04-15

### Added
- 默认开启深度思考：`reasoning_effort` 默认设为 `high`，搜索默认关闭。
- WASM 动态探测：`pow.rs` 改为基于签名的动态 export 探测，不再硬编码 `__wbindgen_export_0`，降低 DeepSeek 更新 WASM 后启动失败的风险。
- 新增 Python E2E 测试套件：覆盖 auth、models、chat completions、tool calling 等场景。
- 新增 `tiktoken-rs` 依赖，用于服务端 prompt token 计算。
- CI 新增 `cargo audit` 与 `cargo machete` 检查。

### Changed
- 账号初始化优化：日志在手机号为空时自动回退显示邮箱。
- 更新 `axum`、`cranelift` 等核心依赖至最新 patch 版本。
- Client Version 保持与网页端一致的 `1.8.0`。

### Removed
- 移除未使用的 `tower` 依赖。

## [0.2.0] - 2026-04-13

### Added
- 项目从 Python 全面重构到 Rust，带来原生高性能和跨平台支持。
- OpenAI 兼容 API（`/v1/chat/completions`、`/v1/models`）。
- 账号池轮转 + PoW 求解 + SSE 流式响应。
- 深度思考和智能搜索支持。
- Tool calling（XML 解析）。
- GitHub CI + 多平台 Release（8 目标平台）。
- 兼容最新 DeepSeek Web 后端接口。
