# External Integrations

**Analysis Date:** 2026-07-13

## APIs & External Services

**LLM Providers (Primary Integration):**
- OpenRouter - Multi-model LLM gateway
  - Base URL: `https://openrouter.ai/api/v1` (`internal/types/constants.go` line 48)
  - Client: `internal/provider/openrouter/client.go`
  - Auth: `OPENROUTER_API_KEY` env var, stored in OS keychain
  - Features: Model catalog, streaming chat completion, cost estimation
  - Headers: HTTP-Referer, X-Title for attribution

- Zen (OpenCode) - LLM provider
  - Base URL: `https://opencode.ai/zen/v1` (`internal/types/constants.go` line 51)
  - Client: `internal/provider/zen/client.go`
  - Auth: `ZEN_API_KEY` env var, stored in OS keychain
  - Features: Model catalog, streaming chat completion

- NVIDIA NIM - LLM provider
  - Base URL: `https://integrate.api.nvidia.com/v1` (`internal/types/constants.go` line 54)
  - Client: `internal/provider/nvidia/client.go`
  - Auth: `NVIDIA_API_KEY` env var, stored in OS keychain
  - Features: Model catalog, streaming chat completion

**Web Services:**
- Web Search - External search API
  - Default URL: `https://search.sagibo.net` (`internal/tools/constants.go` line 46)
  - Client: `internal/tools/websearch.go`
  - Auth: None (public endpoint)
  - Configurable via `tools.websearch_base_url` in config

- Web Fetch - HTTP content retrieval
  - Client: `internal/tools/webfetch.go`
  - Features: HTML to text conversion, DNS caching, retry logic, private IP blocking

## Data Storage

**Databases:**
- None - file-based storage only

**File Storage:**
- Session data: `~/.m31a/sessions/` (JSON files per session)
- Configuration: `~/.m31a/config.toml` (global), `.m31a/config.toml` (project)
- History: JSON files for frecency tracking
- Ledger: Markdown-based session records
- Checkpoints: JSON for workflow state persistence
- Code intelligence index: In-memory with disk cache

**Caching:**
- Model catalog cache with configurable TTL (`provider/cache.go`)
- DNS cache for web tools (`internal/tools/dns_cache.go`)
- Tool definition cache (`internal/workflow/engine.go` line 1156)
- Token estimation cache with EMA calibration

## Authentication & Identity

**Auth Provider:**
- OS Keychain - Native credential storage
  - Linux: D-Bus Secret Service + `pass` CLI fallback (`pkg/keychain/keychain_linux.go`)
  - macOS: `/usr/bin/security` CLI (`pkg/keychain/keychain_darwin.go`)
  - Windows: Windows Credential Manager (`pkg/keychain/keychain_windows.go`)
  - Interface: `pkg/keychain/keychain.go`
  - Account: `m31a`
  - Service prefix: `m31a/`
  - Cached wrapper prevents repeated unavailable backend attempts

**API Key Management:**
- Keys never written to disk in plaintext
- Stored via `keychain.Set("openrouter", apiKey)` pattern
- Retrieved via `keychain.Get("openrouter")` pattern
- First-launch prompt for API key entry

## Monitoring & Observability

**Error Tracking:**
- Internal error classification (`internal/errors/errors.go`)
- Retry policy with error classification (`pkg/retry/policy.go`)
- Structured logging via `log/slog`

**Logs:**
- `log/slog` standard library structured logging
- Audit trail (`internal/logging/audit.go`)
- Decision logging (`internal/decision/logger.go`)
- Session metrics collection (`pkg/metrics/collector.go`)

**Metrics:**
- Session-level metrics: tool calls, LLM usage, phase durations, heals
- Stored in `METRICS.json` per session
- Cost tracking per LLM request
- Token usage estimation with EMA calibration

## CI/CD & Deployment

**Hosting:**
- GitHub Releases - Binary distribution
- Homebrew tap: `eshanized/tap/m31a`
- Scoop: Windows package manager
- Linux packages: deb, rpm, apk, archlinux via GoReleaser

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Jobs: lint, test, security, build, release
  - Go version: 1.25
  - Cross-platform builds: ubuntu, macos, windows
  - Security scanning: govulncheck
  - Release: GoReleaser on tag push

**Release Process:**
- GoReleaser (`.goreleaser.yaml`)
- Cross-compilation: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64
- Checksums: SHA256
- Draft releases with auto-prerelease detection

## Environment Configuration

**Required env vars:**
- `OPENROUTER_API_KEY` - OpenRouter API access (optional if not using OpenRouter)
- `ZEN_API_KEY` - Zen API access (optional if not using Zen)
- `NVIDIA_API_KEY` - NVIDIA NIM API access (optional if not using Nvidia)

**Optional env vars:**
- `HOME` - User home directory (for config/session paths)
- Standard Go env vars for cross-compilation

**Secrets location:**
- OS keychain (primary)
- `.env` file (gitignored, for development only)
- `.env.example` - Template with placeholders

## Webhooks & Callbacks

**Incoming:**
- None - CLI application, no server mode

**Outgoing:**
- LLM API requests (streaming HTTP/SSE)
- Web search API requests
- Web fetch HTTP requests
- OpenRouter attribution headers (Referer, X-Title)

## Provider Fallback

**Auto-fallback mechanism:**
- Configurable priority order (`provider.fallback_priority`)
- Default: nvidia → zen → openrouter (alphabetical)
- Health check before fallback (`provider/health_check.go`)
- Configurable timeout (`provider.health_check_timeout_secs`)
- SSE stream error detection and recovery

**Model Discovery:**
- Dynamic model catalog from each provider API
- Cached with configurable TTL (`features.model_cache_ttl_minutes`)
- Stale cache fallback (`features.model_cache_stale_hours`)
- Model capability detection and override system

---

*Integration audit: 2026-07-13*
