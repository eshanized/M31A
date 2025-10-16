# External Integrations

**Analysis Date:** 2026-06-07

## APIs & External Services

**LLM Providers:**
- OpenRouter — Primary LLM gateway (routes to OpenAI, Anthropic, DeepSeek, etc.)
  - SDK/Client: `internal/provider/openrouter/client.go`
  - Base URL: `https://openrouter.ai/api/v1`
  - Auth: `M31A_OPENROUTER_API_KEY` env var → OS keychain → config file
  - Endpoints: `GET /models`, `POST /chat/completions`, `GET /auth/key`
  - Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`, `User-Agent: M31A/<version>`

- OpenCode Zen — Secondary LLM gateway
  - SDK/Client: `internal/provider/zen/client.go`
  - Base URL: `https://opencode.ai/zen/v1`
  - Auth: `M31A_ZEN_API_KEY` env var → OS keychain → config file
  - Endpoints: `GET /models`, `POST /chat/completions`
  - OpenAI-compatible API format

**Streaming Protocol:**
- Both providers use Server-Sent Events (SSE) for streaming chat completions
- SSE parser: `internal/provider/sse.go`
- Supports both `event: message` and unnamed events
- Handles `[DONE]` sentinel, `\r\n` line endings, multi-line `data:` payloads

**Reasoning/Thinking Normalization:**
- DeepSeek models: `reasoning_content` field in SSE delta
- OpenAI o-series: `reasoning` field in SSE delta
- Anthropic: `type: "thinking"` segments in delta
- Qwen: `reasoning_content` field in SSE delta
- Config: `internal/provider/reasoning.go`

## Data Storage

**Databases:**
- None — All state is file-based

**File Storage:**
- Local filesystem only
- Session data: `~/.m31a/sessions/<id>/` (8-char hex ID)
  - `session.json` — Metadata (model, provider, phase, timestamps)
  - `messages.json` — Message history
  - `planning/` — Human-readable state files (PROJECT.md, TASKS.md, STATE.md)
  - `checkpoints/` — State snapshots for undo
  - `backups/` — Pre-overwrite file backups
  - `archives/` — Post-ship archived sessions
- Config: `~/.m31a/config.toml`
- Ledger: `~/.m31a/LEDGER.md`
- Logs: `~/.m31a/m31a.log` (daily rotation, 7-day retention)

**Caching:**
- Model cache: In-memory with TTL (default 5 min fresh, 24 hr stale)
  - Location: `internal/provider/cache.go`
  - Deduplication via `golang.org/x/sync/singleflight`
  - Thread-safe: `sync.RWMutex` + `atomic.Bool`
- DNS cache: `sync.Map` with 5-minute TTL for SSRF protection
  - Location: `internal/tools/webfetch.go`

## Authentication & Identity

**Auth Provider:**
- API Key authentication only (no OAuth, no user accounts)
  - OpenRouter: Bearer token in `Authorization` header
  - Zen: Bearer token in `Authorization` header

**Key Resolution Order (per provider):**
1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
2. OS keychain (`pkg/keychain/`)
3. Config file (`~/.m31a/config.toml` — last resort fallback)

**OS Keychain Implementations:**
- Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
  - Location: `pkg/keychain/keychain_linux.go`
- macOS: `/usr/bin/security` CLI (Keychain Services)
  - Location: `pkg/keychain/keychain_darwin.go`
- Windows: Credential Manager via `advapi32.dll` (`CredReadW`/`CredWriteW`/`CredDeleteW`)
  - Location: `pkg/keychain/keychain_windows.go`

## Git Integration

**Git Operations:**
- Wrapper: `internal/git/git.go`
- Operations: init, add, commit, log, diff, status, branch, reset (soft/hard), stash
- Used by: Workflow engine (commits per task), rollback (soft/hard reset), bisect

**Git Bisect:**
- Location: `pkg/bisect/`
- Programmatic bisect for regression detection in self-healing workflow

**Git Rollback:**
- Location: `pkg/rollback/`
- Commit chain browser with soft/hard reset and backup branch creation

## Monitoring & Observability

**Error Tracking:**
- Structured logging via `log/slog`
  - Location: `internal/log/`
  - Output: `~/.m31a/m31a.log` (JSON format)
  - Rotation: Daily, 7-day retention
  - Never writes to stdout/stderr during TUI operation

**Health Checks:**
- Provider health polling every 60s (configurable)
  - Location: `internal/provider/fallback.go`
  - Status levels: live (<500ms), slow (<2000ms), degraded (>2000ms), offline
  - Adaptive: increases to 120s on rate-limit headers; stops on 401

**No External Monitoring:**
- No telemetry, no analytics, no phone-home (mandated by FOSS requirements)

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (binary distribution)
- Install script: `install.sh` (curl-based)

**CI Pipeline:**
- GitHub Actions: `.github/workflows/ci.yml`
  - Lint: gofmt + golangci-lint
  - Test: `go test -race -coverprofile=coverage.out`
  - Build: Matrix (ubuntu/macos/windows × amd64/arm64, excluding windows/arm64)
  - Release: GoReleaser on tag push (`refs/tags/v*`)

**Release Pipeline:**
- GoReleaser v2 (`.goreleaser.yaml`)
- Builds: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- Archives: tar.gz (unix), zip (windows)
- Checksums: SHA256
- Draft releases on GitHub

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` OR `M31A_ZEN_API_KEY` — At least one provider key needed for LLM access
- Optional: `M31A_CONFIG`, `M31A_THEME`, `M31A_DEFAULT_MODEL`, `M31A_LOG_FORMAT`, `M31A_LOG_LEVEL`

**Secrets location:**
- Environment variables (highest priority)
- OS keychain (Linux: D-Bus/pass, macOS: Keychain, Windows: Credential Manager)
- Config file (last resort, never persisted via `config.Save()`)

## Webhooks & Callbacks

**Incoming:**
- None — M31A is a CLI tool, not a server

**Outgoing:**
- HTTP POST to OpenRouter/Zen APIs for chat completions (streaming SSE)
- HTTP GET to provider endpoints for model catalog and health checks
- WebFetch tool: HTTP GET to arbitrary URLs (with SSRF protection)

## LLM API Integration Details

**Streaming Chat Completion Flow:**
1. Build request body via `provider.BuildChatBody()` (`internal/provider/common.go`)
2. Send POST to `/chat/completions` with `stream: true`
3. Parse SSE stream via `provider.NewSSEParserWithContext()` (`internal/provider/sse.go`)
4. Convert SSE chunks to `types.StreamChunk` via `provider.ParseSSEChunk()` (`internal/provider/reasoning.go`)
5. Emit chunks as `tea.Msg` for Bubble Tea rendering

**Reasoning Parameter Injection:**
- DeepSeek: No extra params (reasoning via SSE field)
- OpenAI o-series: `reasoning_effort: "medium"`
- Anthropic: `thinking: {type: "enabled", budget_tokens: 1024}`
- Qwen: No extra params (reasoning via SSE field)
- Applied via `provider.ApplyReasoningParams()` (`internal/provider/reasoning.go`)

**Error Normalization:**
- HTTP 401 → `ErrInvalidKey`
- HTTP 402 → `ErrNoCredits`
- HTTP 429 → `ErrRateLimited` (with Retry-After header support)
- HTTP 503 → `ErrProviderUnreachable`
- HTTP 400 + context patterns → `ErrContextExceeded`

**Auto-Fallback:**
- On rate limit (429) or provider unreachable (503): switch to healthy provider
- Location: `internal/provider/fallback.go`
- Respects Retry-After header (capped at 120s)
- Non-blocking: returns `FallbackAfterWait` for async scheduling

---

*Integration audit: 2026-06-07*
