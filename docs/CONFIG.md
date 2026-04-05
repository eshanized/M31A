# Configuration Reference

M31A reads configuration from `~/.m31a/config.toml`. Override with `M31A_CONFIG` environment variable.

Config loading order (later overrides earlier):
1. **Defaults** — built-in sane defaults
2. **Global TOML** — `~/.m31a/config.toml`
3. **Environment variables** — `M31A_*` vars
4. **Project TOML** — `m31a.toml` in cwd (walks up 3 levels)

---

## Full Config Structure

```toml
[provider]
default = "openrouter"              # "openrouter" or "zen"
auto_fallback = true                 # Auto-switch on 429/503 errors

[provider.openrouter]
api_key = ""                         # OpenRouter API key

[provider.zen]
api_key = ""                         # Zen API key

openrouter_base_url = ""             # Custom base URL (default: https://openrouter.ai/api/v1)
zen_base_url = ""                    # Custom base URL (default: https://api.zen.com/v1)
openrouter_referer = ""              # HTTP-Referer header (default: https://github.com/eshanized/M31A)
openrouter_title = ""                # X-Title header (default: M31A)

[model]
default = ""                         # Default model ID
context_warning_threshold = 0.8      # Warning at 80% context usage (0.0–1.0)
show_thinking_by_default = false     # Show reasoning/thinking blocks
auto_collapse_tools = true           # Auto-collapse tool call cards
auto_arbitrage = false               # Enable model cost comparison
arbitrage_threshold = 0.1            # Suggest cheaper if savings > 10%
default_context_length = 0           # Fallback context length (default: 128000)
token_ema_alpha = 0.0                # Token estimator calibration (default: 0.3)

[ui]
theme = "dark"                       # "dark", "light", or "auto"
compact_mode = false                 # Compact UI mode
show_token_usage = true              # Show token usage in header
show_cost_estimate = true            # Show cost estimates
max_iterations = 100                 # Max conversation turns
leader_key = "ctrl+x"                # Leader key for chords
leader_timeout_ms = 1000             # Leader key timeout
sidebar_width_threshold = 120        # Min width for sidebar auto-show
discuss_timeout = 300                # Q&A timeout in seconds
thinking_max_lines = 20              # Max thinking block lines
permission_modal_width = 60          # Permission modal width
sidebar_width = 42                   # Sidebar width in columns
max_message_history = 1000           # Max stored messages
fallback_banner_timeout_secs = 15    # Fallback banner duration
default_log_lines = 20               # Default log lines shown
session_list_limit = 20              # Max sessions in resume list
thinking_opacity = 0.6               # Thinking block opacity
frecent_history_size = 100           # Frecent history entries

# Theme & Colors
accent_color = ""                    # Accent color override
custom_background = ""               # Custom background color
border_style = "rounded"             # "rounded", "double", "hidden", "thick"

# Typography
bold_headers = true
italic_thinking = true
tab_width = 4

# Layout
sidebar_position = "right"           # "left" or "right"
sidebar_auto_show = true
card_padding = 1
welcome_screen = true               # Show welcome screen on startup
zen_mode_key = "ctrl+z"             # Toggle zen mode

# Animation
animation_speed = "normal"           # "fast", "normal", "slow", "none"
spinner_style = "dots"               # "dots", "line", "pulse", "arc"
transition_style = "fade"            # "fade", "slide", "none"
breathing_effects = true
logo_animation = true

# Status Bar
status_bar_style = "default"         # "default", "minimal", "hidden"
status_bar_position = "bottom"       # "bottom" or "top"
show_spinner_in_status = true

# Tool Cards
tool_card_style = "minimal"          # "minimal", "detailed", "compact"
tool_output_max_lines = 50
syntax_highlight = true

# Toasts
toast_position = "bottom-right"      # "top-right", "bottom-right", "top-left", "bottom-left"
toast_duration_secs = 4
toast_max_visible = 3

[permissions]
default_mode = "prompt"              # "prompt", "allow", or "deny"
timeout_seconds = 300                # Permission prompt timeout

[[permissions.rules]]                # Custom permission rules
tool = "Bash"
pattern = "rm -rf"
risk_level = "destructive"
action = "deny"

[permissions.agents]                 # Per-agent permission profiles
[permissions.agents.build]
default_action = "allow"
rules = []

[features]
auto_backup = true                   # Auto-backup files before edits
resume_on_startup = false            # Resume last session on startup
model_cache_ttl_minutes = 5          # Model cache refresh interval
model_cache_stale_hours = 24         # Stale cache threshold
healthcheck_live_ms = 2000           # "live" latency threshold (ms)
healthcheck_slow_ms = 5000           # "slow" latency threshold (ms)
session_id_length = 8                # Session ID hex chars (4–16)
max_recent_models = 10               # Max recent models tracked
session_retention_days = 30          # Session retention period
health_check_timeout_secs = 10       # Health check timeout
rate_limit_backoff_secs = 120        # Rate limit backoff
budget_limit_usd = 0.0               # Per-session budget (0 = unlimited)

[tools]
max_glob_results = 1000              # Max glob search results
max_grep_results = 100               # Max grep search results
bash_kill_grace_secs = 5             # Grace period before killing bash
max_backups_per_file = 5             # Max backup copies per file
webfetch_max_redirects = 3           # Max web fetch redirects
webfetch_user_agent = "M31A/dev"     # User-Agent for web fetches
skip_dirs = [                        # Directories to skip in searches
  ".git", "node_modules", "vendor", "target", "dist", "build"
]

[ledger]
enabled = true                       # Cross-session learning ledger
max_entries = 100                    # Max ledger entries

[git]
commit_prefix = "feat"               # Git commit message prefix
fix_prefix = "fix"                   # Fix commit prefix
ship_prefix = "chore"                # Ship commit prefix
user_name = "M31A"                   # Git user name
user_email = "m31a@local"            # Git user email

[verify]
build_command = ""                   # Build verification command (auto-detect)
test_command = ""                    # Test verification command (auto-detect)

[agents]
default = ""                         # Default agent model
plan = ""                            # Plan phase agent model
execute = ""                         # Execute phase agent model
verify = ""                          # Verify phase agent model
ship = ""                            # Ship phase agent model
discuss = ""                         # Discuss phase agent model
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `M31A_CONFIG` | Override config file path |
| `M31A_OPENROUTER_API_KEY` | OpenRouter API key (highest priority) |
| `OPENROUTER_API_KEY` | OpenRouter API key (fallback) |
| `M31A_ZEN_API_KEY` | Zen API key (highest priority) |
| `ZEN_API_KEY` | Zen API key (fallback) |
| `M31A_LOG_FORMAT` | Log format: `json` or `text` |
| `M31A_LOG_LEVEL` | Log level: `debug`, `info`, `warn`, `error` |
| `M31A_THEME` | Override UI theme |
| `M31A_DEFAULT_MODEL` | Override default model |
| `M31A_PROVIDER` | Override default provider |
| `M31A_PERMISSION_MODE` | Override permission mode |
| `M31A_COMPACT` | Enable compact mode (`true`/`1`) |

---

## API Key Resolution Order

1. **Environment variable** — `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`
2. **Standard fallback** — `OPENROUTER_API_KEY` / `ZEN_API_KEY`
3. **OS keychain** — `m31a/openrouter` or `m31a/zen` service entries
4. **Config file** — `provider.openrouter.api_key` / `provider.zen.api_key`

---

## Keychain Backends

| Platform | Backend |
|----------|---------|
| Linux | D-Bus Secret Service, `pass` CLI (fallback) |
| macOS | `/usr/bin/security` (Keychain Access) |
| Windows | Windows Credential Manager |

Service names: `m31a/openrouter`, `m31a/zen`

---

## Variable Substitution

Config values support `${VAR_NAME}` syntax. Unresolved variables cause a validation error:

```toml
[provider]
default = "${M31A_PROVIDER:-openrouter}"
```

---

## CLI Flags

| Flag | Description |
|------|-------------|
| `--version` | Print version, platform, Go version, then exit |

---

## Default Config

If no config file exists, M31A uses built-in defaults equivalent to:

```toml
[provider]
default = ""
auto_fallback = true

[model]
context_warning_threshold = 0.8
auto_collapse_tools = true
default_context_length = 0
token_ema_alpha = 0.0

[ui]
sidebar_width_threshold = 120
max_iterations = 100
discuss_timeout = 300
leader_timeout_ms = 1000
thinking_max_lines = 20
permission_modal_width = 60
sidebar_width = 42
max_message_history = 1000
fallback_banner_timeout_secs = 15
default_log_lines = 20
session_list_limit = 20
thinking_opacity = 0.6
frecent_history_size = 100

[features]
model_cache_ttl_minutes = 5
model_cache_stale_hours = 24
session_id_length = 8
max_recent_models = 10
session_retention_days = 30
health_check_timeout_secs = 10
auto_backup = true
rate_limit_backoff_secs = 120

[tools]
max_glob_results = 1000
max_grep_results = 100
bash_kill_grace_secs = 5
max_backups_per_file = 5
webfetch_max_redirects = 3
webfetch_user_agent = "M31A/dev"
skip_dirs = [".git", "node_modules", "vendor", "target", "dist", "build"]

[git]
commit_prefix = "feat"
fix_prefix = "fix"
ship_prefix = "chore"
user_name = "M31A"
user_email = "m31a@local"
```

---

## Config Hot-Reload

M31A watches `~/.m31a/config.toml` for changes using `fsnotify` (falls back to polling every 5 seconds). Changes are applied without restarting.
