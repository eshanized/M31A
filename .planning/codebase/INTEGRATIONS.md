# External Integrations

**Analysis Date:** 2026-06-11

## APIs & External Services

**LLM Providers (OpenAI-compatible endpoints):**
- OpenRouter — Primary LLM gateway, proxies to multiple AI providers
  - Base URL: `https://openrouter.ai/api/v1`
  - Auth: `M31A_OPENROUTER_API_KEY` env var → OS keychain → config.toml
  - Client: `internal/provider/openrouter/client.go`
  - Endpoints: `/models`, `/chat/completions`
  - Headers: `Authorization: Bearer <key>`, `User-Agent: M31A/<version>`, `HTTP-Referer`, `X-Title`

- Zen (OpenCode) — Secondary LLM gateway
  - Base URL: `https://opencode.ai/zen/v1`
  - Auth: `M31A_ZEN_API_KEY` env var → OS keychain → config.toml
  - Client: `internal/provider/zen/client.go`
  - Endpoints: `/models`, `/chat/completions`
  - Headers: `Authorization: Bearer <key>`, `User-Agent: M31A/<version>`

**Note:** No direct Anthropic or OpenAI connections. Only OpenRouter and Zen gateways are supported.

## Data Storage

**Databases:**
- None (no SQL/NoSQL database)

**File Storage:**
- Local filesystem only
  - Sessions: `~/.m31a/sessions/<id>/`
  - Config: `~/.m31a/config.toml`
  - Ledger: `~/.m31a/LEDGER.md`
  - Logs: `~/.m31a/m31a.log` (daily rotation, 7-day retention)

**Caching:**
- In-memory model cache with TTL (5 minutes) and stale fallback (24 hours)
  - Implementation: `internal/provider/cache.go`
  - Pattern: singleflight for concurrent fetch deduplication

## Authentication & Identity

**Auth Provider:**
- No external auth provider (not a web service)
- API key management via three-tier resolution:
  1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
  2. OS keychain (platform-specific)
     - Linux: D-Bus secret-service (`pkg/keychain/keychain_linux.go`)
     - macOS: Keychain (`pkg/keychain/keychain_darwin.go`)
     - Windows: Credential Manager (`pkg/keychain/keychain_windows.go`)
  3. Config file fallback (`~/.m31a/config.toml`)

**Key Storage:**
- Keys never stored in plaintext
- Config file `api_key` field is last resort fallback
- Implementation: `pkg/keychain/`

## Monitoring & Observability

**Error Tracking:**
- None (local terminal application)

**Logs:**
- Structured logging via Go standard library `log/slog`
- Implementation: `internal/log/`
- Output: `~/.m31a/m31a.log`
- Rotation: Daily, 7-day retention
- Format: JSON (configurable via `M31A_LOG_FORMAT`)

**Health Checks:**
- Provider health polling every 60 seconds
- Latency thresholds: <500ms = live, <2000ms = slow, >2000ms = offline
- Implementation: `internal/provider/common.go` (HealthCheck method)

## CI/CD & Deployment

**Hosting:**
- GitHub (source repository)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Jobs: lint, test, security, build, release
  - Linting: gofmt + golangci-lint
  - Testing: `go test -race -coverprofile=coverage.out -covermode=atomic ./...`
  - Security: govulncheck
  - Build: Cross-platform matrix (linux/darwin/windows × amd64/arm64)
  - Release: GoReleaser on tag push (v*)

**Release:**
- GoReleaser (`.goreleaser.yaml`)
  - Archives: tar.gz (linux/darwin), zip (windows)
  - Checksums: SHA256
  - Draft releases on GitHub

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` — OpenRouter API key (required for OpenRouter provider)
- `M31A_ZEN_API_KEY` — Zen API key (required for Zen provider)
- `M31A_LOG_FORMAT` — Log format (optional, default: json)
- `M31A_LOG_LEVEL` — Log level (optional, default: info)

**Secrets location:**
- Environment variables (highest priority)
- OS keychain (platform-specific secret storage)
- Config file (`~/.m31a/config.toml`) — last resort fallback

## Webhooks & Callbacks

**Incoming:**
- None (terminal application, not a server)

**Outgoing:**
- HTTP requests to LLM providers only
  - POST `/chat/completions` — Streaming chat completions
  - GET `/models` — Fetch available models

## External Tool Dependencies

**System tools used by Bash tool:**
- Shell: User's default shell ($SHELL or /bin/sh)
- Git: Required for version control operations
- Standard Unix utilities: Used in Bash tool execution

## Network Configuration

**Timeouts:**
- HTTP dial timeout: 30 seconds (`types.HTTPDialTimeout`)
- No body read timeout (streaming responses)
- Bash tool timeout: 30 minutes (`types.BashTimeout`)

**Security:**
- SSRF protection: Private IP access blocked (`ErrPrivateIPBlocked`)
- Request size limits: Max LLM response bytes = 1MB (`types.MaxLLMResponseBytes`)
- Tool output truncation: Max 10,000 chars (`types.MaxToolOutputChars`)

---

*Integration audit: 2026-06-11*
