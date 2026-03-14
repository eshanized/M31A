# External Integrations

**Analysis Date:** 2026-06-12

## APIs & External Services

**LLM Provider — OpenRouter:**
- Service: OpenRouter AI gateway — Routes requests to multiple LLM providers
- Endpoint: `https://openrouter.ai/api/v1` (configurable via `openrouter_base_url` in config)
- SDK/Client: Custom HTTP client (`internal/provider/openrouter/client.go`)
- Auth: Bearer token via `M31A_OPENROUTER_API_KEY` env var or OS keychain
- Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`, `User-Agent: M31A/<version>`
- Endpoints used:
  - `GET /models` — Fetch model catalog with pricing info
  - `POST /chat/completions` — Streaming chat completions (SSE)
  - `GET /auth/key` — Health check / key validation
- Retry logic: Exponential backoff (2 retries) for 500/502/503/connection errors (`internal/provider/openrouter/client.go:136-156`)

**LLM Provider — OpenCode Zen:**
- Service: OpenCode Zen gateway — Alternative LLM provider
- Endpoint: `https://opencode.ai/zen/v1` (configurable via `zen_base_url` in config)
- SDK/Client: Custom HTTP client (`internal/provider/zen/client.go`)
- Auth: Bearer token via `M31A_ZEN_API_KEY` env var or OS keychain
- Headers: `Authorization: Bearer <key>`, `User-Agent: M31A/<version>`
- Endpoints used:
  - `GET /models` — Fetch model catalog
  - `POST /chat/completions` — Streaming chat completions (SSE)

**Provider Fallback:**
- Automatic fallback between providers on failure (`internal/provider/fallback.go`)
- Health checks every 60 seconds (`internal/types/constants.go:11`)
- Health status: live (< 500ms), slow (< 2000ms), degraded, offline (`internal/types/constants.go:34-42`)

## Data Storage

**Databases:**
- None — All data stored as local files

**File Storage:**
- Session data: `~/.m31a/sessions/<session-id>/` directory
  - `session.json` — Session metadata (model, provider, phase, timestamps)
  - `messages.json` — Full message history
  - `checkpoints/` — State snapshots for undo
  - `planning/` — Human-readable Markdown files (PROJECT.md, TASKS.md, STATE.md)
  - `backups/` — Pre-overwrite file backups
  - `archives/` — Post-ship archived sessions
- Config: `~/.m31a/config.toml` (TOML format)
- Ledger: `~/.m31a/LEDGER.md` — Cross-session learning records (Markdown table)
- Logs: `~/.m31a/m31a.log` — Structured logs with daily rotation, 7-day retention

**Caching:**
- Model cache: In-memory with 5-minute TTL (`internal/provider/cache.go`)
- Stale cache fallback: 24-hour TTL for offline fallback (`internal/types/constants.go:32`)
- DNS cache: 5-minute TTL for SSRF protection (`internal/tools/webfetch.go:214`)

## Authentication & Identity

**Auth Provider:**
- OS Keychain — Platform-native secure storage for API keys
  - Implementation: `pkg/keychain/keychain.go` (interface), platform-specific files
  - Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback (`pkg/keychain/keychain_linux.go`)
  - macOS: `/usr/bin/security` CLI (`pkg/keychain/keychain_darwin.go`)
  - Windows: Windows Credential Manager (`pkg/keychain/keychain_windows.go`)
  - Account name: `m31a`
  - Service prefix: `m31a/`
  - Keys stored: `openrouter`, `zen`

**API Key Resolution Order:**
1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
2. Alternative env var (`OPENROUTER_API_KEY` / `ZEN_API_KEY`)
3. OS keychain (`keychain.Get("openrouter")` / `keychain.Get("zen")`)
4. Config file field (`provider.openrouter.api_key` / `provider.zen.api_key`)
- Implemented in `internal/config/loader.go:798-829`

## Monitoring & Observability

**Error Tracking:**
- Custom sentinel errors in `internal/errors/errors.go`
- Provider error normalization with friendly messages (`internal/provider/common.go:166-212`)
- API key masking in error messages to prevent leakage (`internal/provider/common.go:150-160`)

**Logs:**
- Structured logging via `log/slog` (Go standard library)
- Log file: `~/.m31a/m31a.log` with daily rotation, 7-day retention (`internal/log/`)
- Log levels: Debug, Info, Warn, Error
- Sensitive data filtering (API keys masked)

## CI/CD & Deployment

**Hosting:**
- GitHub Releases — Binary distribution via GoReleaser
- Homebrew tap: `eshanized/tap/m31a` (macOS install)
- Install script: `install.sh` (Linux/macOS)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
- Jobs:
  1. `lint` — gofmt check + golangci-lint + GoReleaser config validation
  2. `test` — `go test -race -coverprofile=coverage.out -covermode=atomic ./...`
  3. `security` — `govulncheck ./...`
  4. `build` — Cross-platform build matrix (linux/darwin/windows × amd64/arm64, excluding windows/arm64)
  5. `release` — GoReleaser on tag push (`refs/tags/v*`)
- Triggers: Push to `master`, PRs to `master`, manual dispatch

**Release Process:**
- GoReleaser creates draft releases with:
  - Binaries for linux/darwin/windows (amd64/arm64)
  - Archives: `.tar.gz` (linux/darwin), `.zip` (windows)
  - SHA256 checksums
- Tag-triggered: `git tag vX.Y.Z && git push --tags`

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` — OpenRouter API access
- `M31A_ZEN_API_KEY` or `ZEN_API_KEY` — Zen API access

**Optional env vars:**
- `M31A_CONFIG` — Custom config file path
- `M31A_THEME` — UI theme (dark/light/auto)
- `M31A_DEFAULT_MODEL` — Default model ID
- `M31A_PROVIDER` — Default provider (openrouter/zen)
- `M31A_PERMISSION_MODE` — Permission mode (prompt/allow/deny)
- `M31A_COMPACT` — Compact mode (true/1)

**Secrets location:**
- Primary: OS keychain (encrypted, platform-native)
- Fallback: `~/.m31a/config.toml` (plaintext, only when keychain unavailable)
- Never committed to git (`.gitignore` excludes `/.m31a/`)

## Webhooks & Callbacks

**Incoming:**
- None — M31A is a client application, not a server

**Outgoing:**
- LLM API requests (streaming SSE) to OpenRouter/Zen endpoints
- HTTP requests via WebFetch tool (`internal/tools/webfetch.go`) with SSRF protection:
  - DNS rebinding protection via pinned DNS cache
  - Private IP blocking (RFC1918, link-local, cloud metadata)
  - Redirect validation (max 5 redirects)
  - Response size limit (5MB)

## OS Integrations

**Keychain:**
- Linux: D-Bus Secret Service API (`org.freedesktop.secrets`) via `github.com/godbus/dbus/v5`
- macOS: `/usr/bin/security` CLI (Keychain Services)
- Windows: Windows Credential Manager API

**Clipboard:**
- `github.com/atotto/clipboard` — Copy/paste support in TUI

**File System:**
- Atomic writes: temp file + rename pattern (`internal/fileutil/atomic.go`)
- Config file watching: 5-second polling interval (`internal/config/loader.go:841-879`)
- Git operations: Shell execution via `os/exec` (`internal/git/git.go`)

## External Tool Dependencies

**Optional (enhances performance):**
- `ripgrep` (`rg`) — Used by Glob and Grep tools for faster file searching
  - Falls back to pure-Go doublestar if not available
  - Gitignore-aware file listing when `.gitignore` exists
- `git` CLI — Required for version control operations
  - Used by: `internal/git/git.go`, `pkg/bisect/bisect.go`, `pkg/rollback/rollback.go`

---

*Integration audit: 2026-06-12*
