# External Integrations

**Analysis Date:** 2026-08-04

## APIs & External Services

**LLM Providers (Primary Integration):**
- **OpenRouter** - Multi-model LLM API gateway
  - SDK/Client: Custom HTTP client (`internal/integrations/provider/openrouter/client.go`)
  - Base URL: `https://openrouter.ai/api/v1` (`internal/core/types/constants.go:47`)
  - Auth: `OPENROUTER_API_KEY` env var / OS keychain
  - Endpoints: `/models` (catalog), `/chat/completions` (streaming), `/auth/key` (health)
  - Features: Model catalog with pricing, SSE streaming, retry with exponential backoff

- **Zen** - OpenCode AI API
  - SDK/Client: Custom HTTP client (`internal/integrations/provider/zen/client.go`)
  - Base URL: `https://opencode.ai/zen/v1` (`internal/core/types/constants.go:50`)
  - Auth: `ZEN_API_KEY` env var / OS keychain
  - Endpoints: `/models` (catalog), `/chat/completions` (streaming)
  - Features: Credit-based billing detection, model enrichment from OpenRouter data

- **NVIDIA NIM** - NVIDIA Inference Microservices
  - SDK/Client: Custom HTTP client (`internal/integrations/provider/nvidia/client.go`)
  - Base URL: `https://integrate.api.nvidia.com/v1` (`internal/core/types/constants.go:53`)
  - Auth: `NVIDIA_API_KEY` env var / OS keychain
  - Endpoints: `/models` (catalog), `/chat/completions` (streaming)
  - Features: Multimodal model support, model eviction for unavailable/deprecated models

**Web Services:**
- **Web Search API** - Custom search endpoint
  - SDK/Client: Custom HTTP client (`internal/tools/search/websearch.go`)
  - Base URL: `https://search.sagibo.net` (configurable via `ToolsConfig.WebSearchBaseURL`)
  - Auth: None required
  - Features: SSRF protection, DNS caching, private IP blocking

- **Web Fetch** - URL content retrieval
  - SDK/Client: Custom HTTP client (`internal/tools/search/webfetch.go`)
  - Auth: None required
  - Features: HTML-to-markdown conversion, SSRF protection, configurable timeout

## Data Storage

**Databases:**
- Not applicable - M31A is a stateless CLI tool with file-based persistence

**File Storage:**
- **Session Data**: JSON files in `.m31a/sessions/` per project
- **Backups**: `.m31a/backups/` directory for file backups before edits
- **Tool Output**: `~/.m31a/tool-output/` for bounded tool execution output
- **Configuration**: `~/.m31a/config.toml` (global) or `m31a.json` (project)

**Caching:**
- **Model Cache**: In-memory cache with TTL (`internal/integrations/provider/cache.go`)
  - TTL: 5 minutes (`types.ModelCacheTTL`)
  - Stale fallback: 24 hours (`types.StaleCacheTTL`)
- **DNS Cache**: In-memory cache for web fetch/search (`internal/tools/search/dns_cache.go`)
  - TTL: 5 minutes (`DNSCacheTTL`)
  - Capacity: 64 entries

## Authentication & Identity

**Auth Provider:**
- OS Keychain Integration (custom implementation)
  - Implementation: Platform-specific keychain backends
  - Linux: D-Bus Secret Service API with `pass` CLI fallback (`internal/integrations/keychain/keychain_linux.go`)
  - macOS: `/usr/bin/security` CLI (`internal/integrations/keychain/keychain_darwin.go`)
  - Windows: Windows Credential Manager (`internal/integrations/keychain/keychain_windows.go`)
  - Account name: `m31a` (constant `AccountName`)
  - Service prefix: `m31a/` (constant `servicePrefix`)
  - Availability caching: 5-minute blacklist on backend failure (`blacklistTTL`)

**API Key Management:**
- Keys stored in OS keychain (never written to disk in plaintext)
- `.env` files are gitignored (only `.env.example` committed)
- Keys resolved at startup via `cfg.ResolveAPIKeys(kc)` (`cmd/m31a/main.go:335`)
- Masked display: last 4 characters shown (`base_client.go:94-98`)

## Monitoring & Observability

**Error Tracking:**
- Structured logging via `log/slog` (Go standard library)
- Log file: `~/.m31a/m31a.log` (managed by `internal/integrations/log/log.go`)
- Log rotation: Automatic via the logger package

**Metrics:**
- Session-level metrics collection (`internal/integrations/metrics/collector.go`)
- Persisted to `METRICS.json` per session
- Tracks: tool calls, LLM interactions, phase durations, costs
- Configurable via `FeaturesConfig.MetricsEnabled`

**Health Checks:**
- Provider health checks with latency classification (`internal/integrations/provider/base_client.go:148-177`)
  - Live: < 500ms
  - Slow: < 2000ms
  - Degraded: >= 2000ms
  - Offline: connection failure or non-200 response

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (via GoReleaser)
- Package formats: tar.gz, deb, rpm, apk, archlinux, scoop (Windows)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
- Jobs:
  - `lint`: gofmt check + golangci-lint + GoReleaser config validation
  - `test`: Race-enabled tests with coverage + go vet
  - `security`: govulncheck vulnerability scanning
  - `build`: Cross-platform matrix build (linux/darwin/windows x amd64/arm64)
  - `release`: GoReleaser on version tags (depends on all other jobs)
- Go version: 1.25.12 (pinned across all jobs)
- Build constraint: `CGO_ENABLED=0`

**Release Process:**
- Tag-based: Push `v*` tag triggers release workflow
- GoReleaser builds binaries for all platforms
- Generates checksums (SHA256)
- Creates draft GitHub release
- Package managers: deb, rpm, apk, archlinux, scoop

## Environment Configuration

**Required env vars:**
- `OPENROUTER_API_KEY` - OpenRouter API access
- `ZEN_API_KEY` - Zen API access
- `NVIDIA_API_KEY` - NVIDIA NIM API access

**Optional env vars:**
- `M31A_CONFIG` - Override default config path (`~/.m31a/config.toml`)

**Secrets location:**
- OS Keychain (primary, preferred)
- `.env` file (fallback, gitignored)
- Never stored in plaintext on disk

## Webhooks & Callbacks

**Incoming:**
- None - M31A is a client-side CLI tool, not a server

**Outgoing:**
- SSE (Server-Sent Events) streaming to LLM providers
- HTTP webhooks not used

## System Integrations

**Git:**
- Direct shell execution of `git` commands (`internal/integrations/git/git.go`)
- Operations: init, add, commit, diff, log, status, branch, stash, checkout, reset
- Security: Sensitive file detection prevents accidental credential commits
- Ref validation: Rejects shell metacharacters, path traversal, injection attempts

**Shell Execution:**
- Bash command execution with sandboxing (`internal/tools/exec/bash.go`)
- Platform-specific sandboxing:
  - Linux: Landlock/SELinux/AppArmor (`bash_sandbox_linux.go`)
  - macOS: Seatbelt sandbox (`bash_sandbox_darwin.go`)
  - Windows: Restricted token (`bash_sandbox_windows.go`)
- Process management: Graceful kill with configurable grace period
- Concurrency control: Configurable parallel task execution

**Code Intelligence:**
- Tree-sitter parsing for code analysis (`internal/integrations/codeintel/`)
- Supports multiple languages via tree-sitter grammars
- Features: Symbol indexing, call graphs, relevance scoring

---

*Integration audit: 2026-08-04*
