# INTEGRATIONS.md — External Services & Integrations

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## LLM Providers

### OpenRouter
- **Package:** `internal/provider/openrouter/client.go`
- **Base URL:** `https://openrouter.ai/api/v1` (configurable via `provider.openrouter_base_url`)
- **Headers:** Custom `HTTP-Referer` and `X-Title` headers sent with each request
- **Authentication:** API key via config `provider.openrouter.api_key`
- **Endpoints consumed:**
  - Model catalog listing (`/models`)
  - Streaming chat completions (`/chat/completions` with SSE)
- **Model metadata:** Fetched on startup, cached with configurable TTL (default 5 min live, 24h stale)
- **Health checks:** Periodic latency checks (default 60s interval)

### Zen (OpenCode)
- **Package:** `internal/provider/zen/client.go`
- **Base URL:** `https://opencode.ai/zen/v1` (configurable via `provider.zen_base_url`)
- **Authentication:** API key via config `provider.zen.api_key`
- **Endpoints consumed:**
  - Model catalog listing
  - Streaming chat completions (SSE)
- **Same interface:** Implements `provider.LLMProvider` interface

### Provider Registry
- **Package:** `internal/provider/registry.go`
- Thread-safe provider registry with `sync.RWMutex`
- Automatic fallback between providers (`provider.auto_fallback`)
- Model cost arbitrage between providers (`model.auto_arbitrage`)
- Active provider health monitoring and automatic rollback on failure

## OS Keychain Integration

### macOS
- **Package:** `pkg/keychain/keychain_darwin.go`
- Uses `/usr/bin/security` CLI tool
- Stores/retrieves API keys in the system keychain

### Linux
- **Package:** `pkg/keychain/keychain_linux.go`
- Uses D-Bus (`github.com/godbus/dbus/v5`) to talk to Secret Service (freedesktop.org)
- Fallback to `secret-tool` CLI

### Windows
- **Package:** `pkg/keychain/keychain_windows.go`
- Uses Windows Credential Manager via `kernel32.dll` / `advapi32.dll`

## Git Integration

- **Package:** `internal/git/git.go`
- Thin wrapper around `git` CLI commands
- **Capabilities:** commit, diff, log, status, branches, checkouts, bisect, worktrees
- **Subagent worktrees:** Parallel subagents use `git worktree add` for isolated working directories
- **Rollback:** `pkg/rollback/rollback.go` — git-based rollback with commit history
- **Bisect:** `pkg/bisect/bisect.go` — automated git bisect for bug hunting
- **Configurable commit prefixes:** `git.commit_prefix`, `git.fix_prefix`, `git.ship_prefix`

## File System Operations

### Atomic File Operations
- **Package:** `internal/fileutil/atomic.go`
- Atomic writes via temp file + rename pattern
- Backup creation before edits (configurable via `tools.max_backups_per_file`)

### Session Storage
- **Package:** `pkg/session/`
- JSON-based session files stored in `~/.m31a/sessions/`
- Session structure: `session.json`, `messages.json`, `checkpoint.json`
- Max file size: 50MB per session file (OOM protection)
- Session retention: configurable (default 30 days)

### Decision Ledger
- **Package:** `pkg/ledger/`
- Writes decisions to `~/.m31a/LEDGER.md`
- Tracks phase decisions, model selections, rollback events

## Clipboard

- **Package:** `github.com/atotto/clipboard`
- System clipboard access for copy/paste operations

## File Watching

- **Package:** `github.com/fsnotify/fsnotify`
- Config file hot-reload monitoring (`internal/config/loader.go`)
- Watches `~/.m31a/config.toml` for changes at 5s intervals

## Web Access (LLM)

- **WebFetch tool** (`internal/tools/webfetch.go`) — HTTP(S) GET with SSRF protection
  - Blocks private IP ranges, loopback, link-local
  - Configurable redirect limit, user-agent
- **WebSearch tool** — web search integration

## External CLI Dependencies

The following CLI tools are expected at runtime:
- `git` — version control operations
- `golangci-lint` — code linting (development)
- `goreleaser` — release automation (development)

## Build & Release Pipeline

- **CI/CD:** GitHub Actions (`.github/workflows/`)
- **Release tooling:** `goreleaser` (`.goreleaser.yaml`)
  - Binary builds: linux/darwin/windows × amd64/arm64
  - Archive formats: tar.gz (Linux/macOS), zip (Windows)
  - SHA256 checksums
  - Draft GitHub releases
