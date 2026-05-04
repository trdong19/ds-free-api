# Justfile for ai-free-api

set positional-arguments

# Run all checks: type check, lint, format, audit, unused deps
# 前置: cargo install cargo-audit && cargo install cargo-machete && cargo install cargo-outdated
check:
  cargo fmt --check      
  cargo check            
  cargo clippy -- -D warnings  
  cargo audit
  cargo outdated --root-deps-only
  cargo machete          

# Run unified protocol debug CLI (replaces ds-core-cli / openai-adapter-cli)
# 默认使用 py-e2e-tests/config.toml，可通过 -c <path> 覆盖
adapter-cli *ARGS:
  cargo run --example adapter_cli -- -c py-e2e-tests/config.toml "$@"

# Run openai_adapter/request submodule tests
test-adapter-request *ARGS:
  cargo test openai_adapter::request -- "$@"

# Run openai_adapter/response submodule tests
test-adapter-response *ARGS:
  cargo test openai_adapter::response -- "$@"

# Run HTTP server
serve *ARGS:
  cargo run -- "$@"

# Basic: 基础功能测试（两端点）
e2e-basic *ARGS:
  cd py-e2e-tests && uv run python runner.py scenarios/basic "$@"

# Repair: 工具调用损坏修复专项测试
e2e-repair *ARGS:
  cd py-e2e-tests && uv run python runner.py scenarios/repair "$@"

# Stress: 多迭代并发压测（basic + repair 全部场景）
e2e-stress *ARGS:
  cd py-e2e-tests && uv run python stress_runner.py "$@"

# Start server with e2e test config
e2e-serve:
  cargo run -- -c py-e2e-tests/config.toml
