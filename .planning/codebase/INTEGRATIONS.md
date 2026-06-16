# External Integrations

**Analysis Date:** 2026-06-16

## APIs & External Services

**LLM Providers:**
- **OpenRouter** - Primary LLM gateway for chat completions
  - Endpoint: `https://openrouter.ai/api/v1`
  - SDK/Client: `internal/provider/openrouter/client.go`
  - Auth: `M31A_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY` env var or OS keychain
  - Features: Model catalog, streaming completions, health checks, rate limit handling

- **Zen** - Secondary LLM provider
  - Endpoint: `https://opencode.ai/zen/v1`
  - SDK/Client: `internal/provider/zen/client.go`
  - Auth: `M31A_ZEN_API_KEY` / `ZEN_API_KEY` env var or OS keychain
  - Features: Model catalog, streaming completions, health checks

**Web Search:**
- **SearXNG** - Privacy-respecting meta-search engine
  - Endpoint: Configurable via `tools.websearch_base_url` (default: `https://search.sagibo.net`)
  - SDK/Client: `internal/tools/websearch.go`
  - Auth: None (public instance)
  - Features: General web search, configurable engines (google, bing, duckduckgo)

**Web Fetch:**
- Generic HTTP client for fetching web content
  - SDK/Client: `internal/tools/webfetch.go`
  - Auth: None (public URLs)
  - Features: HTML to markdown conversion, SSRF protection, DNS pinning

## Data Storage

**Databases:**
- Not applicable - No database used

**File Storage:**
- Local filesystem only
  - Session data: `<workDir>/.m31a/session.json`, `messages.json`, `checkpoint.json`
  - Configuration: `~/.m31a/config.toml`, `m31a.toml`
  - Logs: `~/.m31a/m31a.log` (daily rotation, 7-day retention)
  - Ledger: `~/.m31a/LEDGER.md` (cross-session learning records)
  - Backups: `<workDir>/.m31a/backups/` (auto-backup before file edits)

**Caching:**
- In-memory model catalog cache with TTL (5 min active, 24h stale fallback)
  - Implementation: `internal/provider/cache.go`
- DNS cache for WebFetch (5-minute TTL, prevents TOCTOU attacks)
  - Implementation: `internal/tools/webfetch.go`

## Authentication & Identity

**Auth Provider:**
- OS-native keychain integration (no external auth provider)
  - Implementation: `pkg/keychain/`
  - Linux: D-Bus Secret Service + `pass` CLI fallback
  - macOS: `/usr/bin/security` CLI
  - Windows: Windows Credential Manager
  - Service names: `m31a/openrouter`, `m31a/zen`

**API Key Resolution Order:**
1. Environment variable: `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`
2. Standard fallback: `OPENROUTER_API_KEY` / `ZEN_API_KEY`
3. OS keychain: `m31a/openrouter` or `m31a/zen`
4. Config file: `provider.openrouter.api_key` / `provider.zen.api_key`

## Monitoring & Observability

**Error Tracking:**
- Structured logging via `log/slog`
  - Implementation: `internal/log/log.go`
  - Format: JSON (default) or text
  - Level: Configurable via `M31A_LOG_LEVEL`

**Logs:**
- Daily log rotation with 7-day retention
  - Location: `~/.m31a/m31a.log`
  - Rotation: Automatic (files named `m31a.log.YYYY-MM-DD`)
  - Cleanup: Files older than 7 days automatically removed

**Health Checks:**
- Provider health checks with latency thresholds
  - Live: < 500ms
  - Slow: < 2000ms
  - Offline: Connection failed or error response
  - Implementation: `internal/provider/interface.go`

## CI/CD & Deployment

**Hosting:**
- GitHub (repository: `github.com/eshanized/M31A`)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Jobs: lint, test, security, build, release
  - Triggers: Push to `master`, PRs to `master`, manual dispatch
  - Build matrix: linux/darwin/windows × amd64/arm64

**Release:**
- GoReleaser v2
  - Formats: tar.gz, deb, rpm, apk, archlinux, Windows scoop
  - Checksums: SHA-256
  - Release type: Draft with auto-prerelease detection

## Environment Configuration

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` - OpenRouter API access
- `M31A_ZEN_API_KEY` or `ZEN_API_KEY` - Zen API access (optional)

**Optional env vars:**
- `M31A_CONFIG` - Custom config file path
- `M31A_LOG_LEVEL` - Log verbosity (debug/info/warn/error)
- `M31A_LOG_FORMAT` - Log format (json/text)
- `M31A_THEME` - UI theme override
- `M31A_DEFAULT_MODEL` - Default model override
- `M31A_PROVIDER` - Default provider override
- `M31A_PERMISSION_MODE` - Permission mode override

**Secrets location:**
- Primary: OS keychain (Linux/macOS/Windows)
- Fallback: Environment variables or config file

## Webhooks & Callbacks

**Incoming:**
- None - M31A is a terminal application, not a server

**Outgoing:**
- HTTP requests to LLM providers (OpenRouter, Zen)
- HTTP requests to web search (SearXNG)
- HTTP requests to web URLs (WebFetch)

## Git Integration

**Local Git Operations:**
- Shell-based git wrapper: `internal/git/`
- Operations: init, add, commit, status, diff, log, branch, checkout, stash
- Used for: Session management, rollback, bisect, ship phase

**GitHub Integration:**
- GitHub Actions for CI/CD
- GoReleaser for automated releases
- No direct API integration (CLI-based)

## Security Considerations

**SSRF Protection:**
- DNS pinning with 5-minute TTL
- Private IP blocking (RFC1918, link-local, cloud metadata)
- Redirect validation
- Implementation: `internal/tools/webfetch.go`

**Permission Gating:**
- Per-request channels with timeout (default 300s)
- Risk levels: safe, medium, dangerous, destructive
- Rate limiting via token bucket
- Implementation: `internal/tools/permissions.go`

**Process Lifecycle:**
- SIGINT/SIGKILL grace period
- Pipe cleanup
- 5-second force exit timeout
- Implementation: `cmd/m31a/main.go`

---

*Integration audit: 2026-06-16*
