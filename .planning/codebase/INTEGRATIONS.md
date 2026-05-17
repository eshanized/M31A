# External Integrations

**Analysis Date:** 2026-06-14

## APIs & External Services

**LLM Provider - OpenRouter:**
- Service: OpenRouter AI gateway for LLM access
- Base URL: `https://openrouter.ai/api/v1` (`internal/types/constants.go:48`)
- Endpoints:
  - `GET /models` - Fetch available models (`internal/provider/openrouter/client.go:93`)
  - `POST /chat/completions` - Chat completions with streaming SSE (`internal/provider/openrouter/client.go:181`)
  - `GET /auth/key` - Health check / API key validation (`internal/provider/openrouter/client.go:225`)
- SDK/Client: Custom HTTP client (`internal/provider/openrouter/client.go`)
- Auth: Bearer token via `Authorization` header (`internal/provider/common.go:22`)
- Env vars: `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` (`internal/config/loader.go:813-815`)
- Custom headers: `HTTP-Referer`, `X-Title` for attribution (`internal/provider/openrouter/client.go:189-190`)
- Retry logic: Up to 2 retries on 500/502/503/connection errors with exponential backoff (`internal/provider/openrouter/client.go:136-156`)

**LLM Provider - Zen:**
- Service: Zen/OpenCode AI gateway
- Base URL: `https://opencode.ai/zen/v1` (`internal/types/constants.go:51`)
- Endpoints:
  - `GET /models` - Fetch available models (`internal/provider/zen/client.go:79`)
  - `POST /chat/completions` - Chat completions with streaming SSE (`internal/provider/zen/client.go:129`)
- SDK/Client: Custom HTTP client (`internal/provider/zen/client.go`)
- Auth: Bearer token via `Authorization` header (`internal/provider/common.go:22`)
- Env vars: `M31A_ZEN_API_KEY` or `ZEN_API_KEY` (`internal/config/loader.go:829-831`)

**Web Fetch:**
- Service: General HTTP content fetching for the WebFetch tool
- Implementation: Custom HTTP client with SSRF protection (`internal/tools/webfetch.go`)
- Features:
  - DNS pinning to prevent TOCTOU rebinding attacks (`internal/tools/webfetch.go:219-268`)
  - Private IP blocking (RFC1918, link-local, loopback, cloud metadata) (`internal/tools/webfetch.go:153-174`)
  - HTML-to-Markdown conversion (pure Go, no external deps) (`internal/tools/webfetch.go:402-449`)
  - 5MB response size limit (`internal/types/constants.go:11`)
  - Configurable redirect limit (default 5) (`internal/types/constants.go:69`)

## Data Storage

**File System - Sessions:**
- Location: `~/.m31a/sessions/` (`cmd/m31a/main.go:149`)
- Format: JSON files (`session.json`, `messages.json`, `checkpoint.json`)
- Max file size: 50MB per session file (`internal/types/constants.go:27`)
- Schema versioning: `CurrentSchemaVersion = 1` (`pkg/session/session.go:13`)
- Retention: Configurable, default 30 days (`internal/config/types.go:207`)

**File System - Configuration:**
- Global config: `~/.m31a/config.toml` (`cmd/m31a/main.go:93`)
- Project config: `m31a.toml` (walked up from cwd, max 3 levels) (`internal/config/loader.go:199-213`)
- Environment: `.env` in cwd (auto-loaded) (`internal/config/loader.go:971-1016`)

**File System - Backups:**
- Location: `~/.m31a/backups/` (`cmd/m31a/main.go:160`)
- Atomic writes: temp file + rename pattern (`internal/fileutil/atomic.go:13-57`)
- Max backups per file: 10 (configurable) (`internal/types/constants.go:68`)

**File System - Ledger:**
- Location: `~/.m31a/LEDGER.md` (`cmd/m31a/main.go:171`)
- Format: Markdown file for cost tracking

**File System - Git Worktrees:**
- Subagent worktrees: Created for parallel agent execution (`internal/tools/subagent/worktree.go`)
- Cleanup: Best-effort sweep on startup for stale worktrees (`cmd/m31a/main.go:244`)

## Authentication & Identity

**OS Keychain - Secure Credential Storage:**
- Interface: `pkg/keychain/keychain.go:11-26`
- Implementation: Platform-specific via build tags

  **Linux (`pkg/keychain/keychain_linux.go`):**
  - Primary: D-Bus Secret Service API (`org.freedesktop.secrets`)
  - Fallback: `pass` CLI (GPG-based password store)
  - D-Bus operations: `OpenSession`, `SearchItems`, `GetSecret`, `SetSecret`, `CreateItem`

  **macOS (`pkg/keychain/keychain_darwin.go`):**
  - Backend: `/usr/bin/security` CLI (macOS Keychain)
  - Operations: `find-generic-password`, `add-generic-password`, `delete-generic-password`
  - Account: `m31a` (hardcoded)

  **Windows (`pkg/keychain/keychain_windows.go`):**
  - Backend: Windows Credential Manager (advapi32.dll)
  - API calls: `CredReadW`, `CredWriteW`, `CredDeleteW`, `CredFree`
  - Uses unsafe pointer arithmetic for Win32 interop

- Stored services: `openrouter`, `zen` (API keys)
- Service prefix: `m31a/` (`pkg/keychain/keychain.go:4`)

**API Key Resolution Priority:**
1. Environment variable (`M31A_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY`)
2. OS Keychain (`keychain.Get("openrouter")`)
3. Config file field (`cfg.Provider.OpenRouter.APIKey`)
(`internal/config/loader.go:811-841`)

## OS Integrations

**Shell Execution (`internal/tools/bash.go`):**
- Executes shell commands with:
  - Configurable timeout (default 30min, max 30min) (`internal/types/constants.go:20`)
  - Output capping (50,000 chars) (`internal/types/constants.go:21`)
  - Non-interactive environment injection (`CI=true`, `DEBIAN_FRONTEND=noninteractive`, etc.)
  - Process group management for cleanup
  - Platform-specific shell detection (`internal/tools/bash_unix.go`, `internal/tools/bash_windows.go`)

**Git Integration (`internal/git/git.go`):**
- Direct `git` CLI invocation (no library)
- Operations: `init`, `add`, `commit`, `push`, `pull`, `status`, `diff`, `log`, `bisect`, `worktree`
- Commit message sanitization: removes newlines, caps at 200 chars (`internal/git/git.go:84-91`)
- User identity: Configurable via `GitConfig` (`internal/config/types.go:19-25`)

**File System Watching (`internal/config/loader.go:876-924`):**
- Primary: fsnotify for config file hot-reload
- Fallback: Polling every 5 seconds (`internal/types/constants.go:94`)
- Debounce: 50ms for rapid file changes

**Clipboard (`internal/tui/repl_clipboard.go`):**
- Library: `atotto/clipboard` v0.1.4
- Used for: Copy/paste in TUI

**Signal Handling (`cmd/m31a/main.go:262-287`):**
- Catches: `SIGTERM`, `SIGINT`
- Graceful shutdown with 5-second timeout
- Sentinel file for unclean shutdown detection

## Network Protocols

**HTTP/HTTPS:**
- Used for: All LLM provider communication
- Transport: Shared `http.Transport` with connection pooling (`internal/provider/base_client.go:15-30`)
- Timeouts: 30s dial, 90s idle, 10s TLS handshake
- TLS: Standard Go crypto/tls

**SSE (Server-Sent Events):**
- Used for: Streaming LLM responses
- Parser: Custom SSE parser (`internal/provider/sse.go`)
- Watchdog: Timeout for stalled streams (`internal/provider/sse.go:34`)

**D-Bus:**
- Used for: Linux Secret Service API (keychain)
- Library: `godbus/dbus/v5`
- Bus: Session bus (`dbus.ConnectSessionBus()`)

## Embedded Resources

**Prompt Templates (`internal/workflow/engine.go:29-30`):**
- Method: `//go:embed prompts/*.md`
- Files:
  - `prompts/base.md` - Base system prompt
  - `prompts/tool-use.md` - Tool usage instructions
  - `prompts/plan-format.md` - Plan output format
  - `prompts/execute-task.md` - Task execution instructions
  - `prompts/discuss-questions.md` - Discussion Q&A format
  - `prompts/self-heal.md` - Self-healing instructions
  - `prompts/demonstration-format.md` - Demonstration format
  - `prompts/autonomous.md` - Autonomous mode instructions

## Cross-Platform Support

**Build Targets (`.goreleaser.yaml:8-16`):**
- linux/amd64, linux/arm64
- darwin/amd64, darwin/arm64
- windows/amd64 (arm64 excluded)

**Platform-Specific Files:**
- `internal/tools/bash_unix.go` / `bash_windows.go` - Shell execution
- `pkg/keychain/keychain_linux.go` / `keychain_darwin.go` / `keychain_windows.go` - Credential storage
- `internal/tui/commands_config_diskusage_unix.go` / `commands_config_diskusage_windows.go` - Disk usage

## Environment Configuration

**Required env vars (for LLM features):**
- `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` - OpenRouter access
- `M31A_ZEN_API_KEY` or `ZEN_API_KEY` - Zen access

**Optional env vars:**
- `M31A_CONFIG` - Custom config path
- `M31A_PROVIDER` - Default provider name
- `M31A_DEFAULT_MODEL` - Default model ID
- `M31A_THEME` - Theme override (dark/light/auto)
- `M31A_PERMISSION_MODE` - Permission mode (prompt/allow/deny)
- `M31A_COMPACT` - Compact mode toggle (true/1)

**Secrets location:**
- Primary: OS Keychain (platform-specific secure storage)
- Fallback: `~/.m31a/config.toml` (encrypted at rest only if OS provides it)
- Project config never stores API keys (`internal/config/loader.go:236-237`)

## Webhooks & Callbacks

**Incoming:**
- None (desktop CLI application)

**Outgoing:**
- LLM API calls to OpenRouter and Zen
- User-Agent header: `M31A/{version}` (`internal/provider/common.go:17`)

## Provider Fallback & Resilience

**Auto-fallback (`internal/provider/fallback.go`):**
- When primary provider fails, falls back to secondary
- Configurable via `provider.auto_fallback` in config
- Health checks: Periodic latency monitoring (`internal/provider/interface.go:16`)

**Model Cache (`internal/provider/cache.go`):**
- TTL: 5 minutes for fresh data (`internal/types/constants.go:9`)
- Stale TTL: 24 hours for offline fallback (`internal/types/constants.go:32`)
- Stores: Model metadata, pricing, capabilities

**Rate Limiting:**
- Client-side backoff: Configurable, default 120 seconds (`internal/config/types.go:211`)
- Retry-After header support: Up to 120 seconds wait (`internal/types/constants.go:83`)

---

*Integration audit: 2026-06-14*
