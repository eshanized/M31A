# External Integrations

**Analysis Date:** 2026-06-04

## APIs & External Services

**LLM Provider Gateway (OpenRouter):**
- Service: OpenRouter API — LLM model gateway (access to multiple model providers)
  - Base URL: `https://openrouter.ai/api/v1`
  - Endpoints used: `GET /models`, `POST /chat/completions` (streaming SSE), `GET /auth/key` (health check)
  - SDK/Client: Custom HTTP client (`internal/provider/openrouter/client.go`)
  - Auth: `M31A_OPENROUTER_API_KEY` env var → OS keychain → config file
  - Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`, `User-Agent: M31A/<version>`
  - Streaming: SSE (Server-Sent Events) with line-by-line parsing
  - Rate limiting: HTTP 429 → auto-fallback to other provider
  - Model cache: 5-minute TTL, 24-hour stale fallback

**LLM Provider Gateway (Zen):**
- Service: OpenCode Zen API — Alternative LLM gateway
  - Base URL: `https://opencode.ai/zen/v1`
  - Endpoints used: `GET /models`, `POST /chat/completions` (streaming SSE), `GET /models` (health check)
  - SDK/Client: Custom HTTP client (`internal/provider/zen/client.go`)
  - Auth: `M31A_ZEN_API_KEY` env var → OS keychain → config file
  - Headers: `Authorization: Bearer <key>`, `User-Agent: M31A/<version>`
  - Streaming: SSE (Server-Sent Events) with line-by-line parsing
  - No HTTP-Referer/X-Title headers (unlike OpenRouter)

**Web Fetch (HTTP client):**
- Service: Arbitrary HTTP/HTTPS URLs
  - SDK/Client: Custom HTTP client with SSRF protection (`internal/tools/webfetch.go`)
  - Auth: None (public URLs only)
  - Rate limiting: None (tool-level timeout, max 120 seconds)
  - SSRF protection: Blocks private/loopback/link-local IPs, DNS rebinding protection

## Data Storage

**Databases:**
- None — File-based storage only

**File Storage:**
- Local filesystem:
  - `~/.m31a/config.toml` — User configuration
  - `~/.m31a/m31a.log` — Structured log (daily rotation, 7-day retention)
  - `~/.m31a/LEDGER.md` — Cross-session learning ledger
  - `~/.m31a/sessions/<id>/` — Session data directory
    - `session.json` — Session metadata
    - `messages.json` — Message history
    - `planning/PROJECT.md` — Project state
    - `planning/TASKS.md` — Task list
    - `planning/STATE.md` — Workflow state
    - `checkpoints/` — State snapshots for undo
    - `backups/` — Pre-overwrite file backups
    - `archives/` — Post-ship archived sessions

**Caching:**
- Model cache (`internal/provider/cache.go`):
  - Per-provider `ModelCache` with `sync.RWMutex` protection
  - TTL: 5 minutes (configurable via `features.model_cache_ttl_minutes`)
  - Stale TTL: 24 hours (configurable via `features.model_cache_stale_hours`)
  - Deduplication: `singleflight.Group` prevents concurrent refresh requests
  - Offline resilience: Returns stale cache on network failure

## Authentication & Identity

**Auth Provider:**
- OS-native keychain (`pkg/keychain/`):
  - Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
  - macOS: `/usr/bin/security` CLI (Keychain Services)
  - Windows: `advapi32.dll` (`CredReadW`, `CredWriteW`, `CredDeleteW`)
  - Interface: `Keychain` with `Get()`, `Set()`, `Delete()` methods

**Key Resolution Order:**
1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
2. OS keychain (`keychain.Get("openrouter")` / `keychain.Get("zen")`)
3. Config file (`provider.openrouter.api_key` / `provider.zen.api_key`)

**Security:**
- API keys never persisted to config file (`config.Save()` strips keys)
- Service names validated with `^[a-z]+$` regex (prevents command injection)
- Atomic writes for all config/session files (temp file + rename)
- File path safety: Symlink resolution, workDir containment checks
- SSRF protection: Private IP blocking in WebFetch tool

## Monitoring & Observability

**Error Tracking:**
- Structured logger (`internal/log/`):
  - Framework: `log/slog` (Go standard library)
  - Output: `~/.m31a/m31a.log` (JSON format)
  - Rotation: Daily, 7-day retention
  - Levels: `debug`, `info`, `warn`, `error`
  - No telemetry, no analytics, no external calls

**Logs:**
- Application logs to `~/.m31a/m31a.log` only
- Never logs to stdout/stderr during TUI operation
- User-friendly error messages via `errors.UserMessage()` (`internal/errors/errors.go`)

## CI/CD & Deployment

**Hosting:**
- Binary distribution via GitHub Releases
- Cross-compilation: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- Static binary (CGO_ENABLED=0) — no runtime dependencies

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`):
  - `lint` job: gofmt check, golangci-lint
  - `test` job: `go test -race -coverprofile=coverage.out`
  - `build` job: Matrix build (linux/macos/windows × amd64/arm64)
  - `release` job: Tag-triggered goreleaser build
  - Triggers: push to main/dev, PR to main, manual dispatch

**Release Process:**
- goreleaser (`.goreleaser.yaml`):
  - Archives: `.tar.gz` (linux/darwin), `.zip` (windows)
  - Checksums: SHA256
  - Draft releases: Yes
  - Pre-release: Auto-detect
- Makefile targets: `build`, `cross`, `release`, `release-dry`

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` — OpenRouter API key (or use keychain/config)
- `M31A_ZEN_API_KEY` — Zen API key (or use keychain/config)
- `M31A_CONFIG` — Override config file path (optional)
- `M31A_THEME` — Override theme setting (optional)
- `M31A_DEFAULT_MODEL` — Override default model (optional)
- `M31A_LOG_FORMAT` — Log format: `json` (default) or `text` (optional)
- `M31A_LOG_LEVEL` — Log verbosity: `debug`, `info`, `warn`, `error` (optional)

**Secrets location:**
- Environment variables (highest priority)
- OS keychain (Linux: D-Bus Secret Service / pass; macOS: Keychain; Windows: Credential Manager)
- Config file (lowest priority, API keys stripped on save)

## Webhooks & Callbacks

**Incoming:**
- None — CLI tool, no server component

**Outgoing:**
- HTTP requests to OpenRouter API (`https://openrouter.ai/api/v1/*`)
- HTTP requests to Zen API (`https://opencode.ai/zen/v1/*`)
- HTTP requests to arbitrary URLs (WebFetch tool, with SSRF protection)
- Health check polling: Active provider polled every 60 seconds

## Reasoning Model Integration

**Model-specific parameter injection (`internal/provider/reasoning.go`):**
- DeepSeek: No extra params; SSE field `choices.0.delta.reasoning_content`
- OpenAI o-series: `reasoning_effort: "medium"`; SSE field `choices.0.delta.reasoning`
- Anthropic: `thinking: {type: "enabled", budget_tokens: 1024}`; SSE field `choices.0.delta.content`
- Qwen: No extra params; SSE field `choices.0.delta.reasoning_content`

**Reasoning detection patterns:**
- Pre-content reasoning (DeepSeek R1, OpenAI o-series): All thinking tokens before content
- Interleaved reasoning (Claude extended thinking): Thinking and content segments alternate
- Segment boundaries detected via SSE event type or special markers

## Known Model Capabilities

**OpenRouter models with explicit capability flags (`internal/provider/openrouter/client.go`):**
- `anthropic/claude-sonnet-4`, `anthropic/claude-opus-4`, `anthropic/claude-3.5-sonnet`: Tools=yes, Reasoning=no
- `openai/gpt-4o`: Tools=yes, Reasoning=no
- `openai/o1`, `openai/o3`: Tools=yes, Reasoning=yes
- `deepseek/deepseek-r1`: Tools=no, Reasoning=yes
- `deepseek/deepseek-v3`: Tools=yes, Reasoning=no
- `google/gemini-2.5-pro`, `google/gemini-2.5-pro-exp`: Tools=yes, Reasoning=no
- `meta-llama/llama-3.3-70b-instruct`: Tools=yes, Reasoning=no

**Zen models with explicit capability flags (`internal/provider/zen/client.go`):**
- `deepseek-v3`, `deepseek-v3-free`: Tools=yes, Reasoning=no
- `deepseek-r1`, `deepseek-r1-free`: Tools=no, Reasoning=yes
- `claude-sonnet-4`, `claude-opus-4`: Tools=yes, Reasoning=no
- `gpt-4o`: Tools=yes, Reasoning=no
- `gemini-2.5-pro`: Tools=yes, Reasoning=no

## External Tool Dependencies

**Optional system tools:**
- `rg` (ripgrep) — Used by Glob and Grep tools when available; pure-Go fallback if missing
- `git` — Required for workflow engine (commits, bisect, rollback)
- `pass` CLI — Linux keychain fallback (D-Bus Secret Service is primary)
- `/usr/bin/security` — macOS keychain operations
- `xdg-open` (Linux), `open` (macOS), `start` (Windows) — Browser launch (for dev server URL)

## Provider Error Normalization

| HTTP Status | Normalized Error | Action |
|-------------|-----------------|--------|
| 401 | `ErrInvalidKey` | Show "Invalid API key" modal |
| 402 | `ErrProviderUnreachable` | Log, suggest checking key status |
| 429 | `ErrRateLimited` | Auto-fallback if enabled; respect Retry-After header (max 60s) |
| 503 | `ErrProviderUnreachable` | Auto-fallback if enabled |
| 400 (other) | `ErrToolExecution` | Return to LLM for correction |

## Auto-Fallback Logic

- Trigger: HTTP 429 (rate limited) or HTTP 503 (service unavailable)
- Action: Switch `Registry.active` to the other healthy provider
- Pre-check: Verify other provider is healthy via `HealthCheck()` (10s timeout)
- Event: Emit `FallbackEvent{From, To, Reason}` for TUI banner display
- Resume: Rebuild `ChatRequest` from current message history on new provider
- Config: `provider.auto_fallback` must be enabled

---

*Integration audit: 2026-06-04*
