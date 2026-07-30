# INTEGRATIONS.md — M31A External Integrations

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## LLM Providers

### OpenRouter (`internal/integrations/provider/openrouter/`)
- API: `https://openrouter.ai/api/v1`
- Auth: `OPENROUTER_API_KEY` env var
- Capabilities: Chat completions (streaming), model listing
- SSE parsing: Custom streaming implementation in `sse.go`

### Zen (`internal/integrations/provider/zen/`)
- API: Custom Zen API
- Auth: `ZEN_API_KEY` env var
- Capabilities: Chat completions (streaming), model listing

### NVIDIA NIM (`internal/integrations/provider/nvidia/`)
- API: `https://integrate.api.nvidia.com/v1`
- Auth: `NVIDIA_API_KEY` env var
- Capabilities: Chat completions (streaming), model listing

### Provider Registry (`internal/integrations/provider/registry.go`)
- Dynamic model discovery from APIs
- Fallback chains for provider resilience
- Model capability detection (reasoning patterns, tool support)
- Cost estimation per model

## Keychain / Secret Storage

### Platform-specific implementations:
- **Linux**: `internal/integrations/keychain/keychain_linux.go` — D-Bus Secret Service API (GNOME Keyring / KWallet)
- **macOS**: `internal/integrations/keychain/keychain_darwin.go` — macOS Keychain
- **Windows**: `internal/integrations/keychain/keychain_windows.go` — Windows Credential Manager

### Usage pattern:
- API keys stored in OS keychain via `keychain.Save()`
- Cached via `keychain.NewCached()` to avoid repeated D-Bus calls
- Graceful fallback if keychain unavailable (warns but continues)

## Git Integration (`internal/integrations/git/`)

- Native `git` CLI wrapper
- Operations: status, diff, commit, branch, stash, log
- Used by: rollback engine, ship phase, session management

## External APIs (via Tools)

### Web Search (`internal/tools/network/`)
- Custom search API at configurable base URL
- Default: `https://search.sagibo.net`
- DNS caching for performance (`search.NewDNSCache`)

### HTTP Check (`internal/tools/network/`)
- HTTP health checks for URLs
- Used by `http_check` tool

## System Integrations

### File System Watching (`fsnotify`)
- File change detection for session persistence
- Used by file explorer and hot-reload features

### Clipboard (`atotto/clipboard`)
- System clipboard read/write via `repl_clipboard.go`

### Tree-sitter (`odvcencio/gotreesitter`)
- Code analysis for `code_analysis` and `code_complexity` tools
- Syntax-aware code understanding

## Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `OPENROUTER_API_KEY` | No* | OpenRouter API authentication |
| `ZEN_API_KEY` | No* | Zen API authentication |
| `NVIDIA_API_KEY` | No* | NVIDIA NIM API authentication |
| `M31A_CONFIG` | No | Custom config path (default: `~/.m31a/config.toml`) |

*At least one provider key required for LLM features.
