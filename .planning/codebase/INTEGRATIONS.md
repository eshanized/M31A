# External Integrations

**Analysis Date:** 2026-08-03

## APIs & External Services

**LLM Providers (Chat Completions + Model Discovery):**
- OpenRouter - Multi-model LLM gateway (OpenAI-compatible API)
  - Client: `internal/integrations/provider/openrouter/client.go`
  - Base URL: `https://openrouter.ai/api/v1` (configurable via `provider.openrouter_base_url`)
  - Auth: `OPENROUTER_API_KEY` env var or OS keychain
  - Endpoints: `GET /models`, `POST /chat/completions`, `GET /auth/key` (health check)
  - Headers: `HTTP-Referer`, `X-Title` (configurable)
  - Model catalog cached with 5-minute TTL + 24-hour stale fallback

- Zen - OpenCode's hosted LLM API (OpenAI-compatible)
  - Client: `internal/integrations/provider/zen/client.go`
  - Base URL: `https://opencode.ai/zen/v1` (configurable via `provider.zen_base_url`)
  - Auth: `ZEN_API_KEY` env var or OS keychain
  - Endpoints: `GET /models`, `POST /chat/completions`, `GET /models` (health check)
  - Pricing/context enrichment from OpenRouter or local database

- NVIDIA NIM - NVIDIA's Inference Microservices (OpenAI-compatible)
  - Client: `internal/integrations/provider/nvidia/client.go`
  - Base URL: `https://integrate.api.nvidia.com/v1` (configurable via `provider.nvidia_base_url`)
  - Auth: `NVIDIA_API_KEY` env var or OS keychain
  - Endpoints: `GET /models`, `POST /chat/completions`, `GET /models` (health check)
  - Special handling: multimodal model detection, reasoning config `extra_body` params
  - Filters out non-chat and broken models from catalog

**Web Search:**
- SearXNG (privacy-respecting meta-search engine)
  - Client: `internal/tools/search/websearch.go`
  - Default URL: `https://search.sagibo.net` (configurable via `tools.websearch_base_url`)
  - Endpoint: `GET /search?q=...&format=json&categories=general`
  - No authentication required
  - DNS pinning with private IP blocking for SSRF protection

**Web Fetch:**
- Built-in HTTP client (no external service)
  - Client: `internal/tools/search/webfetch.go`
  - Features: DNS caching, SSRF protection (private/reserved IP blocking), max 5 redirects
  - Configurable: retries, redirect limits, user agent

**HTTP Health Check:**
- Built-in HTTP checker tool
  - Client: `internal/tools/network/httpcheck.go`
  - Purpose: Verify endpoint availability (used by tools, not for provider health)

## Data Storage

**Databases:**
- None (file-based storage only)

**File Storage:**
- Local filesystem only
  - Config: `~/.m31a/config.toml` (global), `m31a.toml` (project-level)
  - Sessions: `<workDir>/.m31a/` (project-local session data)
  - Backups: `<workDir>/.m31a/backups/` (file change backups)
  - Tool output: `~/.m31a/tool-output/` (bounded output store with TTL cleanup)
  - Ledgers: `~/.m31a/LEDGER.md` (activity tracking)
  - Planning: `<workDir>/.m31a/planning/` (phase workflow data)

**Caching:**
- Model catalog cache (in-memory, per-provider)
  - TTL: 5 minutes (configurable via `features.model_cache_ttl_minutes`)
  - Stale TTL: 24 hours (configurable via `features.model_cache_stale_hours`)
  - Implementation: `internal/integrations/provider/cache.go`
- DNS cache (in-memory)
  - TTL: 300 seconds (configurable via `tools.dns_cache_ttl_secs`)
  - Implementation: `internal/tools/search/dns_cache.go`
- Keychain availability cache
  - Blacklist TTL: 5 minutes after transient failure
  - Implementation: `internal/integrations/keychain/keychain.go`

## Authentication & Identity

**Auth Provider:**
- OS Keychain (platform-native secure storage)
  - Interface: `internal/integrations/keychain/keychain.go`
  - Linux: D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
    - Implementation: `internal/integrations/keychain/keychain_linux.go`
  - macOS: `/usr/bin/security` CLI (Keychain.app)
    - Implementation: `internal/integrations/keychain/keychain_darwin.go`
  - Windows: Windows Credential Manager
    - Implementation: `internal/integrations/keychain/keychain_windows.go`
  - Account name: `m31a`
  - Services: `openrouter`, `zen`, `nvidia` (prefixed with `m31a/`)
  - Availability caching with 5-minute blacklist on transient failures
  - Used for: API key storage (never written to disk in plaintext)

**API Key Resolution Priority:**
1. Environment variable (`M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY`)
2. OS keychain (`keychain.Get("openrouter")`)
3. Config file field (`provider.openrouter.api_key`)
  - Implementation: `internal/core/config/loader.go:ResolveAPIKeys()`

## Monitoring & Observability

**Error Tracking:**
- Structured logging via `log/slog` (Go standard library)
  - Logger initialization: `internal/integrations/log/`
  - Log level: configurable (default: info)
  - All provider failures, config errors, and keychain issues logged with context

**Metrics:**
- Session-level metrics collection (when `features.metrics_enabled = true`)
  - Records: tool execution metrics, LLM token usage, workflow phase metrics
  - Output: `METRICS.json` in session directory
  - Implementation: `internal/integrations/metrics/`

**Logs:**
- File-based logging to `~/.m31a/logs/` (application logs)
- Structured slog output to stderr (during startup)
- Session logs within session directories

## CI/CD & Deployment

**Hosting:**
- GitHub (source code + releases)

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Jobs: lint, test, security (govulncheck), build (matrix: linux/darwin/windows x amd64/arm64), release
  - Go version: 1.25.12 (pinned)
  - Release: GoReleaser on tag push (`v*`)
  - Packages: deb, rpm, apk, archlinux (via GoReleaser nfpms), Windows scoop

**Release Process:**
- GoReleaser v2 (`.goreleaser.yaml`)
  - Cross-compiled binaries (CGO_ENABLED=0)
  - Archives, checksums (SHA256), changelogs
  - Draft GitHub releases with auto-prerelease detection

## Environment Configuration

**Required env vars:**
- `OPENROUTER_API_KEY` - OpenRouter API authentication (or `M31A_OPENROUTER_API_KEY`)
- `ZEN_API_KEY` - Zen API authentication (or `M31A_ZEN_API_KEY`)
- `NVIDIA_API_KEY` - NVIDIA NIM API authentication (or `M31A_NVIDIA_API_KEY`)

**Optional env vars:**
- `M31A_CONFIG` - Override config file path
- `M31A_THEME` - Override theme
- `M31A_DEFAULT_MODEL` - Override default model
- `M31A_PROVIDER` - Override default provider
- `M31A_PERMISSION_MODE` - Override permission mode
- `M31A_COMPACT` - Enable compact mode

**Secrets location:**
- API keys stored in OS keychain (preferred) or config file (fallback)
- `.env` file in working directory (auto-loaded, never committed per `.gitignore`)
- `.env.example` provided as template

## Webhooks & Callbacks

**Incoming:**
- None (TUI application, no server mode)

**Outgoing:**
- LLM API requests (OpenRouter, Zen, NVIDIA NIM) via HTTP POST
- Web search requests to SearXNG instance
- Web fetch requests to arbitrary URLs (with SSRF protection)

## External Tool Dependencies

**Git:**
- Git CLI (required for git operations)
  - Integration: `internal/integrations/git/`
  - Used for: commit, branch, diff, worktree operations
  - User identity configurable via `git.user_name` and `git.user_email`

**pass (Linux optional):**
- `pass` CLI - Password store (fallback for D-Bus Secret Service)
  - Used when: D-Bus Secret Service unavailable
  - Requires: GPG key initialized (`pass init <gpg-id>`)

---

*Integration audit: 2026-08-03*
