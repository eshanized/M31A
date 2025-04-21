# Configuration Reference

M31A reads configuration from `~/.m31a/config.toml`. The config path can be overridden with the `M31A_CONFIG` environment variable.

## Config File

```toml
[provider]
default = "openrouter"        # Active provider: "openrouter" or "zen"
auto_fallback = true          # Automatically switch providers on 429/503 errors

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

[ui]
theme = "dark"                # Theme: "dark" or "light"
compact_mode = false          # Compact UI mode
show_token_usage = true       # Show token usage in header
show_cost_estimate = true     # Show cost estimates
max_iterations = 100          # Max conversation iterations

[permissions]
default_mode = "ask"          # Permission mode: "ask", "allow", "deny"
timeout_seconds = 300         # Permission prompt timeout (seconds)

[[permissions.rules]]         # Custom permission rules
tool = "Bash"
pattern = "rm -rf"
risk_level = "destructive"
action = "deny"

[features]
autodream_enabled = true      # Enable AutoDream context consolidation
subagent_enabled = false      # Enable subagent delegation (V1.1)
auto_backup = true            # Auto-backup files before modifications
resume_on_startup = false     # Resume last session on startup

[ledger]
enabled = true                # Enable cross-session learning ledger
max_entries = 100             # Maximum ledger entries

[ghost]
enabled = false               # Enable ghost mode (V1.1)
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `M31A_CONFIG` | Override config file path (default: `~/.m31a/config.toml`) |
| `OPENROUTER_API_KEY` | OpenRouter API key (highest priority) |
| `ZEN_API_KEY` | Zen API key (highest priority) |
| `M31A_LOG_FORMAT` | Log format: `json` or `text` (default: `json`) |
| `M31A_LOG_LEVEL` | Log level: `debug`, `info`, `warn`, `error` (default: `info`) |

## API Key Resolution Order

API keys are resolved in the following order (first match wins):

1. **Environment variable** (`OPENROUTER_API_KEY` or `ZEN_API_KEY`)
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

[model]
default = ""
context_warning_threshold = 0.8
show_thinking_by_default = false
auto_collapse_tools = true
auto_arbitrage = false
arbitrage_threshold = 0.1

[ui]
theme = "dark"
compact_mode = false
show_token_usage = true
show_cost_estimate = true
max_iterations = 100

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
autodream_enabled = true
subagent_enabled = false
auto_backup = true
resume_on_startup = false

[ledger]
enabled = true
max_entries = 100

[ghost]
enabled = false
```
