# Configuration Reference

M31A reads configuration from `~/.m31a/config.toml`. The config path can be overridden with the `M31A_CONFIG` environment variable.

## Config File

```toml
[provider]
default = "openrouter"        # Active provider: "openrouter" or "zen"
auto_fallback = true          # Automatically switch providers on 429/503 errors
openrouter_base_url = ""      # Custom OpenRouter URL (empty = default)
zen_base_url = ""             # Custom Zen URL (empty = default)
openrouter_referer = ""       # Custom HTTP-Referer header (empty = default)
openrouter_title = ""         # Custom X-Title header (empty = default)

[provider.openrouter]
api_key = ""                  # OpenRouter API key (or env/keychain)

[provider.zen]
api_key = ""                  # Zen API key (or env/keychain)

[model]
default = ""                  # Default model ID (empty = provider default)
context_warning_threshold = 0.8  # Warning at 80% context usage
show_thinking_by_default = false # Show thinking blocks by default
auto_collapse_tools = true    # Auto-collapse tool call cards
auto_arbitrage = false        # Enable model cost comparison
arbitrage_threshold = 0.1     # Suggest cheaper model if savings > 10%
default_context_length = 0    # Fallback context length (0 = 128000)
token_ema_alpha = 0.0         # Token estimator calibration (0 = 0.3)

[ui]
theme = "dark"                # Theme: "dark" or "light"
compact_mode = false          # Compact UI mode
show_token_usage = true       # Show token usage in header
show_cost_estimate = true     # Show cost estimates
max_iterations = 100          # Max conversation iterations
leader_key = "ctrl+x"         # Leader key for chord shortcuts
leader_timeout_ms = 1000      # Leader key timeout (milliseconds)

[permissions]
default_mode = "ask"          # Permission mode: "ask", "allow", "deny"
timeout_seconds = 300         # Permission prompt timeout (seconds)

[[permissions.rules]]         # Custom permission rules
tool = "Bash"
pattern = "rm -rf"
risk_level = "destructive"
action = "deny"

[features]
auto_backup = true            # Auto-backup files before modifications
resume_on_startup = false     # Resume last session on startup
model_cache_ttl_minutes = 0   # Model cache refresh interval (0 = 5 min)
model_cache_stale_hours = 0   # Stale cache threshold (0 = 24 hours)
healthcheck_live_ms = 0       # "live" latency threshold (0 = 2000ms)
healthcheck_slow_ms = 0       # "slow" latency threshold (0 = 5000ms)
session_id_length = 0         # Session ID hex length (0 = 8, range 4-16)
max_recent_models = 0         # Max recent models to track (0 = 10)

[ledger]
enabled = true                # Enable cross-session learning ledger
max_entries = 100             # Maximum ledger entries
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `M31A_CONFIG` | Override config file path (default: `~/.m31a/config.toml`) |
| `M31A_OPENROUTER_API_KEY` | OpenRouter API key (highest priority) |
| `OPENROUTER_API_KEY` | OpenRouter API key (fallback if M31A_ prefix not set) |
| `M31A_ZEN_API_KEY` | Zen API key (highest priority) |
| `ZEN_API_KEY` | Zen API key (fallback if M31A_ prefix not set) |
| `M31A_LOG_FORMAT` | Log format: `json` or `text` (default: `json`) |
| `M31A_LOG_LEVEL` | Log level: `debug`, `info`, `warn`, `error` (default: `info`) |

## API Key Resolution Order

API keys are resolved in the following order (first match wins):

1. **Environment variable** — `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY` (preferred), or `OPENROUTER_API_KEY` / `ZEN_API_KEY` (standard fallback)
2. **OS keychain** (`m31a/openrouter` or `m31a/zen` service entries)
3. **Config file** (`provider.openrouter.api_key` or `provider.zen.api_key`)

Keys are **never stored in plaintext** in the config file. When entered via the first-run wizard, keys are saved to the OS keychain if available.

## Keychain Integration

| Platform | Backend |
|----------|---------|
| Linux | D-Bus Secret Service (primary), `pass` CLI (fallback) |
| macOS | `/usr/bin/security` (Keychain Access) |
| Windows | Windows Credential Manager (`advapi32.dll`) |

Service names follow the pattern `m31a/<provider>` (e.g., `m31a/openrouter`, `m31a/zen`).

## CLI Flags

| Flag | Description |
|------|-------------|
| `--version` | Print version, platform, and Go version, then exit |

## Default Config

If no config file exists, M31A uses these defaults:

```toml
[provider]
default = ""
auto_fallback = true
openrouter_base_url = ""
zen_base_url = ""
openrouter_referer = ""
openrouter_title = ""

[model]
default = ""
context_warning_threshold = 0.8
show_thinking_by_default = false
auto_collapse_tools = true
auto_arbitrage = false
arbitrage_threshold = 0.1
default_context_length = 0
token_ema_alpha = 0.0

[ui]
theme = "dark"
compact_mode = false
show_token_usage = true
show_cost_estimate = true
max_iterations = 100
leader_key = "ctrl+x"
leader_timeout_ms = 1000

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
auto_backup = true
resume_on_startup = false
model_cache_ttl_minutes = 0
model_cache_stale_hours = 0
healthcheck_live_ms = 0
healthcheck_slow_ms = 0
session_id_length = 0
max_recent_models = 0

[ledger]
enabled = true
max_entries = 100
```

## Configurable Defaults Reference

When a config field is set to `0` or empty string, the following defaults are used:

| Setting | Default | Description |
|---------|---------|-------------|
| `openrouter_base_url` | `https://openrouter.ai/api/v1` | OpenRouter API endpoint |
| `zen_base_url` | `https://opencode.ai/zen/v1` | Zen API endpoint |
| `openrouter_referer` | `https://github.com/eshanized/M31A` | HTTP-Referer header |
| `openrouter_title` | `M31A` | X-Title header |
| `default_context_length` | `128000` | Fallback model context length |
| `token_ema_alpha` | `0.3` | Token estimator EMA calibration rate |
| `leader_key` | `ctrl+x` | Leader key for chord shortcuts |
| `leader_timeout_ms` | `1000` | Leader key timeout (1 second) |
| `model_cache_ttl_minutes` | `5` | Model cache refresh interval |
| `model_cache_stale_hours` | `24` | Stale cache threshold |
| `healthcheck_live_ms` | `2000` | "live" health check latency |
| `healthcheck_slow_ms` | `5000` | "slow" health check latency |
| `session_id_length` | `8` | Session ID hex character length |
| `max_recent_models` | `10` | Max recent models to track |
