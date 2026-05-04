# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.2.6] - 2026-05-02

### Added
- **代理配置**：新增 `[proxy]` 配置项，支持 HTTP/HTTPS/SOCKS5 代理。美国地区用户可通过配置非美国
  出口节点绕过 CloudFront WAF 限制，无需依赖环境变量。
  ```toml
  [proxy]
  url = "http://127.0.0.1:7890"
  ```
- **WAF 友好提示**：当检测到 AWS WAF Challenge 时，输出清晰的双语提示和解决方案说明，
  替代原有的 `error decoding response body` 无意义错误
- **账号自动去重**：启动时自动按 email（优先）或 mobile 去重，重复配置的账号只会生效一次
- **重试全链路日志**：`try_chat()` 每次 Overloaded 退避重试输出 WARN 日志（含尝试次数和等待时间），
  重试成功输出 INFO 日志，全部失败输出 WARN 终结日志，便于诊断限流问题
- **适配器入口日志**：`chat_completions()` 处理开始时输出模型和 stream 标志的 DEBUG 日志
- **转换器提前结束检测**：DeepSeek 流在内容输出完毕前断开时输出 WARN 日志并附 usage 快照

### Changed
- **依赖升级**：wasmtime 43.0.0 → 44.0.0，修复安全通告 RUSTSEC-2026-0114
- **inline prompt 瘦身**：`split_history_prompt` 改为只保留最后一个带 `<think>` 的 `<｜Assistant｜>` 块
  作为 inline，其余全部进入 history 文件上传。避免工具调用结果超长导致输入超限
- **工具调用标签模糊匹配**：新增 `norm_tag_char` / `fuzzy_match_tag`，全角 `｜`(U+FF5C)↔`|`、
  `▁`(U+2581)↔`_` 自动模糊匹配，覆盖所有字符级幻觉变体。默认 extra 列表精简为格式完全不同
  的回退标签：`<|tool_call_begin|>`、`<tool_calls>`、`<tool_call>`

### Fixed
- **CI 幂等性**：`cargo install` 步骤添加 `command -v` 前置检查，避免缓存恢复后重复安装失败
- **client.rs 日志违规**：`print_waf_hint()` 中 11 条 `warn!` 补全 `target: "ds_core::client"` 参数
- **导入顺序合规**：`openai_adapter.rs`、`response.rs`、`accounts.rs`、`server.rs`、
  `anthropic_compat/response/stream.rs` 中的导入分组按规范重排（std → 第三方 → crate → super）

### Docs
- **Prompt injection 策略**：更新 README 中 DeepSeek 原生标签的注入策略说明，
  补充 `<｜Tool｜>` 标签的使用方式
- **CLAUDE.md / AGENTS.md**：精简架构描述，新增故障排除表、请求追踪 grep 示例、`#[allow]` 仅在
  `client.rs` 出现的策略说明
- **logging-spec.md**：新增 adapter 层（入口、重试、转换器提前结束）和 ds_core 编排层代码示例，
  补齐全管道日志级别映射
- **code-style.md**：修复 `docs/logging.md` 断链；补充错误消息语言约定（用户界面中文/内部英文）
  和枚举变体 PascalCase 约定


### Added
- **`DS_CONFIG_PATH` 环境变量**：Docker 部署时通过环境变量指定配置文件路径，
  优先级：`-c` > `DS_CONFIG_PATH` > 默认 `config.toml`
- **`Config::save()`**：Config 结构体新增 `save()` 方法，原子写入 TOML 文件（write tmp + rename），
  unix 权限 0600。管理面板的变更自动写回 `config.toml`
- **`OpenAIAdapter::sync_accounts()`**：批量对比新旧账号列表，差异化增删。
  替代原来 70 行内联 diff 逻辑
- **`auth::setup_admin()` / `auth::login_admin()`**：管理员密码设置和登录的高层编排函数，
  替代原来 handler 内联的 50 行校验逻辑

### Changed
- **配置归并**：`admin.json` 和 `api_keys.json` 删除，AdminConfig（密码 hash + JWT 密钥）和
  ApiKeyEntry（API Key 列表）合并到 `config.toml` 的 `[admin]` / `[[api_keys]]` 节。
  `stats.json` 保留为运行数据，不属于配置范畴
- **Config 运行时可变**：`Config` 由启动时冻结的 `Arc<Config>` 改为 `Arc<RwLock<Config>>`，
  `config.toml` 时刻反映运行状态，不重启即可通过管理面板改配置并持久化
- **sse_stream_with_callback 删除**：OpenAI 流式响应路径改用 `inspect`/`map`/`TokenGuardStream`
  （与 Anthropic 路径完全对称），不再通过回调逃逸统计逻辑
- **handler 瘦身**：
  - `chat_completions` / `anthropic_messages` 的统计日志代码提取为 `AppState::record_request()`
  - `admin_setup` / `admin_login` 从各 ~50 行压缩到 ~12 行（委托 `auth::*`）
  - `admin_reload_config` 从 ~70 行压缩到 ~10 行（委托 `OpenAIAdapter::sync_accounts()`）
- **store.rs 重构**：`StoreManager` 从读写独立 JSON 文件改为委托共享 `Arc<RwLock<Config>>`，
  `admin.json` / `api_keys.json` 读写全部通过 `config.save()` 落盘
- **`save_admin` 参数优化**：从 `&AdminStore` 结构体改为三个独立参数 `(String, String, u64)`，
  避免多余的 `to_string()` clone
- **Dockerfile 更新**：`config.example.toml` → `/app/config.toml`，设置 `DS_CONFIG_PATH` 环境变量

### Removed
- `DS_CONFIG` 环境变量：配置路径现在通过 `-c` 或 `DS_CONFIG_PATH` 指定，启动后 `config_path`
  通过 `AppState.config_path` 传递，reload 时使用同一路径
- `admin.json` 和 `api_keys.json`：合并入 `config.toml`
- 启动时的 `accounts.is_empty()` 验证：无账号启动后通过管理面板添加
- `sse_stream_with_callback()` / `sse_stream()` / `SseSerializer` struct：改用 `TokenGuardStream`

### Fixed
- **管理面板 reload 路径一致**：`admin_reload_config` 不再独立读 `DS_CONFIG` 环境变量，
  而是使用 `AppState.config_path`，确保启动 `-c` 指定的路径与 reload 路径一致
## [0.2.5] - 2026-04-30

### Added
- **文件上传**：支持通过 API 上传文件/图片到 DeepSeek。OpenAI 端点的 `file` / `image_url` content part
  和 Anthropic 端点的 `document` / `image` content block 均可使用。内联 data URL 自动上传，
  HTTP URL 触发搜索模式，由模型自行访问
- **XML `<invoke>` 格式原生解析**：直接解析 `<invoke name="..."><parameter>` 格式的工具调用，
  无需触发修复管道，响应更快
- **流式工具调用保活**：模型生成工具调用期间（通常 2–10s），每 1s 发送空增量块防止客户端超时。
  OpenAI 端点为空 `tool_calls` delta，Anthropic 端点为 `"tool_calls..."` thinking 块
- **工具调用标签用户自维护**：`config.toml` 新增 `[deepseek.tool_call]` 配置项，
  用户可随时追加新发现的模型幻觉标签，无需等待代码更新

### Changed
- **Prompt 格式升级**：从 ChatML（`<|im_start|>` / `<|im_end|>`）全面迁移到 DeepSeek 原生标签格式。
  每次 `<｜User｜>` 前插入 `<｜end▁of▁sentence｜>` 闭合上一轮；工具结果改用 `<｜tool▁outputs▁begin｜>` 包裹；
  reminder 嵌入 `<think>` 块。与 DeepSeek 官方 chat_template 对齐后，模型遵循度明显提升
- **工具调用主标签变更**：从 `<|tool_calls_begin|>` 改为 `<|tool▁calls▁begin|>` / `<|tool▁calls▁end|>`
  （使用 ASCII `|` + `▁`）。模型输出这个标签的概率大幅高于旧标签，幻觉变体明显减少。
  默认回退标签覆盖已知变体：`<|tool_calls_begin|>`、`<|tool▁calls_begin|>`、`<|tool_calls▁begin|>`、`<tool_call>`
- **智能搜索默认开启**：搜索模式下 DeepSeek 注入的系统提示词更强，能提升工具调用遵循度

### Fixed
- **Anthropic 协议兼容性**：`message_start` 补回 `stop_reason: null` / `stop_sequence: null`；
  `message_delta` 始终携带 `usage.output_tokens`；usage 不再始终为 0。
  以上修复解决 Claude Code 等标准 Anthropic 客户端的兼容性问题
- **文件上传错误处理**：历史对话文件上传失败时自动回退为内联 prompt，不再静默丢失上下文；
  外部文件上传失败直接返回明确错误，不再静默跳过
- **修复模型准确度**：自修复请求现在自动携带工具定义列表和 JSON 转义提示，
  模型从破碎文本推测正确参数的能力明显提升

## [0.2.4] - 2026-04-27

### Added
- **历史对话文件化**：多轮对话历史自动拆分上传为独立文件，绕过 DeepSeek 单次输入长度限制。
  对适配器层完全透明，上传失败不影响主流程，自动退化为纯文本发送
- **临时 Session 生命周期**：每次请求创建独立 session，请求结束自动清理（stop_stream + delete_session），
  彻底杜绝 session 泄漏和 TTL 过期残留
- **工具调用自修复**：当模型输出的 tool_calls 格式异常时，使用 DeepSeek 自身修复损坏的 JSON/XML，
  流式和非流式路径均覆盖，大幅提升工具调用成功率
- **arguments 类型归一**：自动处理 arguments 为 JSON 字符串的异常情况，避免客户端双重转义解析失败
- **`input_exceeds_limit` 检测**：识别输入超长错误并返回明确错误信息，不再静默失败
- **全链路日志追踪**：`req-{n}` 标识贯穿 handler → adapter → ds_core 全层，
  `x-ds-account` 响应头标识处理账号，单次请求可完整 grep 追踪
- **TRACE 级别字节追踪**：流管道各层 TRACE 日志，可观察字节在 SSE 管道中的完整转换过程
- **`/` 端点**：免鉴权返回可用端点列表和项目地址
- **e2e 测试重构**：从 pytest 迁移为 JSON 场景驱动框架，场景独立存放，配置动态读取

### Changed
- **请求流程重构**：从"持久 session + edit_message"升级为"临时 session + completion + 文件上传"，
  每次请求独立生命周期，不再依赖预创建的持久 session
- **限流自动重试**：检测到 rate_limit 时以 1s→2s→4s→8s→16s 指数退避自动重试（最多 6 次），
  对用户透明，大幅降低限流导致的请求失败
- **Prompt 构建优化**：reminder 插入位置调整到最后一轮对话之前，确保模型优先遵循指令；
  工具描述的代码块格式化；工具调用结果的 Markdown 结构化展示
- **推理控制语义修正**：禁用思考时使用 `"none"` 替代 `"minimal"`，语义更明确
- **日志级别规范化**：账号池耗尽提升为 `WARN`，常规分配降为 `DEBUG`，
  新增 session/上传/PoW 等 debug 日志，health_check 合并为单条带耗时日志

### Removed
- 账号初始化不再按 model_type 管理 session，移除 session 持久化和 update_title 逻辑
- 移除旧 pytest e2e 测试目录（被 JSON 场景驱动框架替代）

### Test Results

#### py-e2e-tests
- **4 账号 + 3 并发 + 3 迭代**：17 场景 × 2 模型 × 3 次 = 102 次请求，成功率 100%，总耗时 5.5 分钟
- 覆盖场景：基础对话、深度思考、流式、标准工具调用，以及 10 种 tool_calls 损坏格式
  （XML/JSON 混合、字段名不一致、arguments 字符串、括号不匹配/缺失、
  name/arguments 互换、参数外溢等），修复管道全部正确兜底

#### claude-code 测试
```bash
export ANTHROPIC_BASE_URL=http://127.0.0.1:5317/anthropic
export ANTHROPIC_AUTH_TOKEN=sk-test
export ANTHROPIC_DEFAULT_OPUS_MODEL=deepseek-expert
export ANTHROPIC_DEFAULT_SONNET_MODEL=deepseek-expert
export ANTHROPIC_DEFAULT_HAIKU_MODEL=deepseek-default
claude
```
- 基本稳定, 工具解析时会使得claude-code暂时卡住是正常现象, 部分情况可能出现模型不遵循指令导致工具调用指令泄漏
- 其他编程工具没有大量测试, 希望大家积极反馈

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
