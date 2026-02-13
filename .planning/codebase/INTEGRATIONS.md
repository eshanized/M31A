# External Integrations

**Analysis Date:** 2026-06-11

## APIs & External Services

**LLM Provider — OpenRouter:**
- Service: OpenRouter API (LLM gateway/aggregator)
  - Base URL: `https://openrouter.ai/api/v1`
  - Custom base URL: configurable via `openrouter_base_url` in config
  - Endpoints consumed:
    - `GET /models` — Fetch model catalog (cached 5 min TTL, stale fallback 24h)
    - `POST /chat/completions` — Streaming chat completion (SSE)
    - `GET /auth/key` — Health check / auth validation
  - Auth: Bearer token via `Authorization` header
  - Headers: `HTTP-Referer`, `X-Title` (configurable, defaults to GitHub repo URL and "M31A")
  - Client: `internal/provider/openrouter/client.go`
  - Retry: Up to 2 retries on 5xx and connection errors with exponential backoff

**LLM Provider — OpenCode Zen:**
- Service: OpenCode Zen API (LLM gateway)
  - Base URL: `https://opencode.ai/zen/v1`
  - Custom base URL: configurable via `zen_base_url` in config
  - Endpoints consumed:
    - `GET /models` — Fetch model catalog (cached 5 min TTL, stale fallback 24h)
    - `POST /chat/completions` — Streaming chat completion (SSE)
    - `GET /models` — Health check (doubles as auth validation)
  - Auth: Bearer token via `Authorization` header
  - Client: `internal/provider/zen/client.go`
  - SSE field mapping differences from OpenRouter (reasoning fields)

**Web Content Fetching:**
- Service: Any HTTP/HTTPS URL (user-initiated via WebFetch tool)
  - Client: `internal/tools/webfetch.go`
  - Auth: None (public HTTP GET)
  - SSRF Protection: DNS caching with TOCTOU prevention, private IP blocking, redirect validation
  - Response size limit: 5MB
  - Timeout: Configurable (default 30s, max 120s)

## Data Storage

**Databases:**
- Not applicable — no database. All state is file-based.

**File Storage:**
- Local filesystem only
  - Config: `~/.m31a/config.toml`
  - Sessions: `~/.m31a/sessions/<id>/`
    - `session.json` — Session metadata (JSON)
    - `messages.json` — Message history (JSON)
    - `checkpoints/` — State snapshots for undo
    - `planning/` — Human-readable state files (Markdown)
      - `PROJECT.md` — Goal, project type, framework, discuss Q&A
      - `TASKS.md` — Task list with status, dependencies, files
      - `STATE.md` — Current phase, progress, last action
    - `backups/` — Pre-overwrite file backups
    - `archives/` — Post-ship archived sessions
  - Ledger: `~/.m31a/LEDGER.md` — Cross-session learning ledger (append-only Markdown)
  - Log: `~/.m31a/m31a.log` — Structured log with daily rotation, 7-day retention

**Caching:**
- In-memory model cache (`internal/provider/cache.go`)
  - TTL: 5 minutes (configurable via `features.model_cache_ttl_minutes`)
  - Stale TTL: 24 hours (configurable via `features.model_cache_stale_hours`)
  - Deduplication: `golang.org/x/sync/singleflight` prevents concurrent refreshes
  - Thread-safe: `sync.RWMutex`

## Authentication & Identity

**Auth Provider:**
- Custom — API key-based authentication for LLM providers
  - OpenRouter: API key stored in OS keychain or config
  - Zen: API key stored in OS keychain or config
  - No OAuth, no SSO, no user accounts

**OS Keychain Integration:**
- Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
  - Client: `pkg/keychain/keychain_linux.go`
  - D-Bus: `github.com/godbus/dbus/v5`
  - Fallback: `pass` (password-store) CLI
  - Service prefix: `m31a/`
  - Account: `m31a`
- macOS: `/usr/bin/security` CLI (Keychain Services)
  - Client: `pkg/keychain/keychain_darwin.go`
- Windows: Windows Credential Manager
  - Client: `pkg/keychain/keychain_windows.go`

## System Integrations

**Git:**
- Service: Local git installation (via `os/exec`)
  - Client: `internal/git/git.go`
  - Operations: init, commit, log, diff, branch, worktree, bisect, reset
  - Used by: workflow phases (commit after task execution), rollback, bisect, ship

**Terminal/PTY:**
- Service: PTY allocation for Bash tool execution
  - Linux/macOS: `creack/pty` (referenced in docs but not in go.mod — likely stdlib)
  - Windows: Plain pipe fallback
  - Client: `internal/tools/bash_unix.go`, `internal/tools/bash_windows.go`

**Clipboard:**
- Service: System clipboard
  - Client: `github.com/atotto/clipboard`
  - Used by: `internal/tui/repl_clipboard.go`

## Monitoring & Observability

**Error Tracking:**
- None — no external error tracking service

**Logs:**
- Local structured logging via `log/slog`
  - Format: JSON (default) or text (`M31A_LOG_FORMAT`)
  - Level: configurable (`M31A_LOG_LEVEL`)
  - Output: `~/.m31a/m31a.log` (never stdout/stderr during TUI operation)
  - Rotation: daily, 7-day retention
  - Client: `internal/log/log.go`

## CI/CD & Deployment

**Hosting:**
- GitHub Releases
  - GoReleaser v2: builds for linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
  - Archives: tar.gz (unix), zip (windows)
  - Checksums: SHA256
  - Release: draft mode

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Lint: `golangci-lint run ./... --timeout=5m`
  - Test: `go test -race -cover ./...`
  - Build matrix: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
  - Enforced: `CGO_ENABLED=0` in all build steps

**Installation:**
- Install script: `install.sh`
  - Usage: `curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/main/install.sh | bash`
  - Supports: `--version`, `--bin-dir`
  - Auto-detects OS and architecture
  - Verifies SHA256 checksum

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` — For OpenRouter provider
- `M31A_ZEN_API_KEY` or `ZEN_API_KEY` — For Zen provider
- At least one provider key is required for LLM features

**Optional env vars:**
- `M31A_CONFIG` — Custom config file path
- `M31A_THEME` — Theme override
- `M31A_DEFAULT_MODEL` — Default model override
- `M31A_PROVIDER` — Default provider override
- `M31A_PERMISSION_MODE` — Permission mode override
- `M31A_COMPACT` — Compact mode toggle
- `M31A_LOG_LEVEL` — Log level
- `M31A_LOG_FORMAT` — Log format

**Secrets location:**
- OS keychain (preferred): D-Bus Secret Service / pass / Keychain / Credential Manager
- Config file (fallback): `~/.m31a/config.toml` (API key fields)
- Environment variables (highest priority): `M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`
- `.env` file (auto-loaded from cwd, lowest priority for existing vars)

## Webhooks & Callbacks

**Incoming:**
- None — this is a CLI tool, not a server

**Outgoing:**
- SSE (Server-Sent Events) stream to OpenRouter/Zen APIs for chat completions
  - Parsed line-by-line via custom SSE parser (`internal/provider/sse.go`)
  - [DONE] sentinel marks stream end

## Third-Party SDKs

- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation for GPT/Claude model families
  - Used by: `internal/tokens/estimator.go`
  - Note: unmaintained since 2024; new tokenizers (e.g. o200k_base for GPT-4o) may not be recognized
  - Fallback: rune-based estimation (`len([]rune) / 4 * 1.3`) with EMA calibration

## API Rate Limiting & Resilience

**Client-side protections:**
- Model cache: 5 min TTL prevents excessive `/models` calls
- singleflight: Deduplicates concurrent model refresh requests
- Stale fallback: Serves cached models for up to 24h on network failure
- Auto-retry: Up to 2 retries on 5xx/connection errors with exponential backoff
- HTTP dial timeout: 30 seconds (no body read timeout for streaming)
- Health check polling: Every 60s (adaptive based on rate-limit headers)
- DNS cache: 5 min TTL for WebFetch SSRF prevention

**Provider-side (observed):**
- OpenRouter: 429 → `ErrRateLimited`, auto-fallback if enabled
- Zen: 429 → `ErrRateLimited`
- Both: 401 → `ErrInvalidKey`, 402 → `ErrNoCredits`, 503 → `ErrProviderUnreachable`

---

*Integration audit: 2026-06-11*
