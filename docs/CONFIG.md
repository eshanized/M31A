# Configuration Reference

M31 Autonomous reads configuration from `~/.m31a/config.toml`. Override with `M31A_CONFIG` environment variable.

## Config Loading Order

Later overrides earlier:
1. **Defaults** — built-in sane defaults (`DefaultConfig()`)
2. **Global TOML** — `~/.m31a/config.toml`
3. **`.env` file** — auto-loaded from cwd
4. **Environment variables** — `M31A_*` vars
5. **Project TOML** — `m31a.toml` in cwd (walks up 3 levels)
6. **Variable substitution** — `${VAR}` → env value

---

## Full Config Structure

### Provider

```toml
[provider]
default = ""                    # Default provider: "openrouter", "zen", or "nvidia"
auto_fallback = true            # Auto-switch on 429/503 errors

[provider.openrouter]
api_key = ""                    # OpenRouter API key

[provider.zen]
api_key = ""                    # Zen API key

[provider.nvidia]
api_key = ""                    # Nvidia NIM API key

openrouter_base_url = ""        # Custom base URL (default: https://openrouter.ai/api/v1)
zen_base_url = ""               # Custom base URL (default: https://opencode.ai/zen/v1)
nvidia_base_url = ""            # Custom base URL (default: https://integrate.api.nvidia.com/v1)
openrouter_referer = ""         # HTTP-Referer header (default: https://github.com/eshanized/M31A)
openrouter_title = ""           # X-Title header (default: M31A)
```

### Model

```toml
[model]
default = ""                         # Default model ID
context_warning_threshold = 0.8      # Warning at 80% context usage (0.0–1.0)
show_thinking_by_default = false     # Show reasoning/thinking blocks
auto_collapse_tools = true           # Auto-collapse tool call cards
auto_arbitrage = false               # Enable model cost comparison
arbitrage_threshold = 0.1            # Suggest cheaper if savings > 10%
default_context_length = 128000      # Fallback context length
token_ema_alpha = 0.3                # Token estimator EMA calibration (0.0–1.0)
```

### UI

```toml
[ui]
theme = "dark"                       # Theme: "dark", "light", or "auto"
compact_mode = false                 # Compact UI mode
show_token_usage = true              # Show token usage in header
show_cost_estimate = true            # Show cost estimates
max_iterations = 100                 # Max conversation turns
leader_key = "ctrl+x"                # Leader key for chords
leader_timeout_ms = 1000             # Leader key timeout (ms)
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
```

#### Theme & Colors (Reserved)

```toml
[ui]
accent_color = ""                    # Reserved for future customization
custom_background = ""               # Reserved for future customization
border_style = "rounded"             # Reserved: "rounded", "double", "hidden", "thick"
bold_headers = true                  # Reserved
italic_thinking = true               # Reserved
tab_width = 4                        # Reserved
```

#### Layout (Reserved)

```toml
[ui]
sidebar_position = "right"           # Reserved: "left" or "right"
sidebar_auto_show = true             # Reserved
card_padding = 1                     # Reserved
welcome_screen = true                # Reserved
zen_mode_key = "ctrl+z"             # Reserved
```

#### Animation (Reserved)

```toml
[ui]
animation_speed = "normal"           # Reserved: "fast", "normal", "slow", "none"
spinner_style = "dots"               # Reserved: "dots", "line", "pulse", "arc"
transition_style = "fade"            # Reserved: "fade", "slide", "none"
breathing_effects = true             # Reserved
logo_animation = true                # Reserved
```

#### Status Bar (Reserved)

```toml
[ui]
status_bar_style = "default"         # Reserved: "default", "minimal", "hidden"
status_bar_position = "bottom"       # Reserved: "bottom" or "top"
show_spinner_in_status = true        # Reserved
```

#### Tool Cards (Reserved)

```toml
[ui]
tool_card_style = "minimal"          # Reserved: "minimal", "detailed", "compact"
tool_output_max_lines = 50           # Reserved
syntax_highlight = true              # Reserved
```

#### Toasts (Reserved)

```toml
[ui]
toast_position = "bottom-right"      # Reserved: "top-right", "bottom-right", "top-left", "bottom-left"
toast_duration_secs = 4              # Reserved
toast_max_visible = 3                # Reserved
```

### Permissions

```toml
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
```

### Features

```toml
[features]
auto_backup = true                   # Auto-backup files before edits
resume_on_startup = false            # Resume last session on startup
workflow_mode = "auto"               # "auto", "full", "fast", "direct"
budget_limit_usd = 0.0               # Per-session budget (0 = unlimited)
metrics_enabled = true               # Enable metrics collection

# Model cache
model_cache_ttl_minutes = 5          # Model cache refresh interval
model_cache_stale_hours = 24         # Stale cache threshold

# Health checks
healthcheck_live_ms = 500            # "live" latency threshold (ms)
healthcheck_slow_ms = 2000           # "slow" latency threshold (ms)
health_check_timeout_secs = 10       # Health check timeout

# Session
session_id_length = 8                # Session ID hex chars (4–16)
max_recent_models = 10               # Max recent models tracked
session_retention_days = 30          # Session retention period

# Rate limiting
rate_limit_backoff_secs = 120        # Rate limit backoff

# Workflow quality gates
plan_research = true                 # Pre-plan web research
plan_check = true                    # LLM-based plan quality review
plan_check_max_iter = 3              # Max plan revision iterations
plan_security_gate = true            # Security file awareness check
plan_coverage_gate = true            # Goal phrase coverage check
plan_gap_analysis = true             # Missing file detection
plan_chunked = true                  # Auto-chunk large plans
plan_chunk_threshold = 10            # Task count threshold for chunking
discuss_quality_check = true         # Filter low-quality questions
discuss_completeness = true          # Score answer completeness
discuss_follow_ups = true            # Generate follow-up questions
execute_preflight = true             # Pre-execution dependency validation
execute_quality_gate = true          # Per-task acceptance criteria verification
execute_loop_detect = true           # Tool-call loop detection
verify_report = true                 # Generate verification report
verify_security = true               # Security file scanning
ship_preflight = true                # Pre-ship checklist
ship_changelog = true                # Auto-generated changelog
init_deep_analysis = true            # Deep project analysis
init_preflight = true                # Environment preflight checks
intent_classification = true         # LLM-based intent classification
```

### Tools

```toml
[tools]
max_glob_results = 1000              # Max glob search results
max_grep_results = 100               # Max grep search results
bash_kill_grace_secs = 5             # Grace period before killing bash
max_backups_per_file = 10            # Max backup copies per file
webfetch_max_redirects = 5           # Max web fetch redirects
webfetch_user_agent = "M31A/dev"     # User-Agent for web fetches
skip_dirs = [".git", "node_modules", "vendor", "target", "dist", "build"]
websearch_base_url = "https://search.sagibo.net"  # SearXNG instance URL
websearch_enabled = true             # Enable/disable web search tool
output_max_lines = 2000              # Max tool output lines
output_max_bytes = 51200             # Max tool output bytes
```

### Ledger

```toml
[ledger]
enabled = true                       # Cross-session learning ledger
max_entries = 100                    # Max ledger entries
```

### Git

```toml
[git]
commit_prefix = "feat"               # Git commit message prefix
fix_prefix = "fix"                   # Fix commit prefix
ship_prefix = "chore"                # Ship commit prefix
user_name = "M31A"                   # Git user name
user_email = "m31a@local"            # Git user email
```

### Verify

```toml
[verify]
build_command = ""                   # Build verification command (auto-detect if empty)
test_command = ""                    # Test verification command (auto-detect if empty)
```

### Compaction

```toml
[compaction]
auto = true                          # Auto-compact context on overflow
buffer = 20000                       # Buffer tokens before compaction
keep_tokens = 8000                   # Tokens to keep after compaction
proactive = true                     # Proactive compaction
tool_calls_threshold = 15            # Tool calls before compaction
phase_transition_pct = 60            # Phase transition compaction threshold
```

### Instructions

```toml
[instructions]
enabled = true                       # Enable project instructions
disable_project = false              # Disable project-specific instructions
```

### Skills

```toml
[skills]
sources = []                         # Reserved for future skill source configuration
```

### Agents

```toml
[agents]
default = ""                         # Default agent model
initialize = ""                      # Initialize phase agent model
research = ""                        # Research phase agent model
plan = ""                            # Plan phase agent model
execute = ""                         # Execute phase agent model
verify = ""                          # Verify phase agent model
ship = ""                            # Ship phase agent model
discuss = ""                         # Discuss phase agent model

[agents.profiles]                    # Custom subagent profiles
[agents.profiles.build]
description = "Build and compile"
mode = "general"
model = ""
max_tools = 50
max_tokens = 50000
max_turns = 25
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
| `M31A_NVIDIA_API_KEY` | Nvidia API key (highest priority) |
| `NVIDIA_API_KEY` | Nvidia API key (fallback) |
| `M31A_LOG_FORMAT` | Log format: `json` or `text` |
| `M31A_LOG_LEVEL` | Log level: `debug`, `info`, `warn`, `error` |
| `M31A_THEME` | Override UI theme |
| `M31A_DEFAULT_MODEL` | Override default model |
| `M31A_PROVIDER` | Override default provider |
| `M31A_PERMISSION_MODE` | Override permission mode |
| `M31A_COMPACT` | Enable compact mode (`true`/`1`) |

---

## API Key Resolution Order

1. **Environment variable** — `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY` / `M31A_NVIDIA_API_KEY`
2. **Standard fallback** — `OPENROUTER_API_KEY` / `ZEN_API_KEY` / `NVIDIA_API_KEY`
3. **OS keychain** — `m31a/openrouter`, `m31a/zen`, `m31a/nvidia` service entries
4. **Config file** — `provider.openrouter.api_key` / `provider.zen.api_key` / `provider.nvidia.api_key`

---

## Keychain Backends

| Platform | Backend |
|----------|---------|
| Linux | D-Bus Secret Service, `pass` CLI (fallback) |
| macOS | `/usr/bin/security` (Keychain Access) |
| Windows | Windows Credential Manager |

Service names: `m31a/openrouter`, `m31a/zen`, `m31a/nvidia`

---

## Variable Substitution

Config values support `${VAR_NAME}` syntax. Unresolved variables cause a validation error:

```toml
[provider]
default = "${M31A_PROVIDER:-openrouter}"
```

---

## Config Hot-Reload

M31 Autonomous watches `~/.m31a/config.toml` for changes using `fsnotify` (falls back to polling every 5 seconds). Changes are applied without restarting.
