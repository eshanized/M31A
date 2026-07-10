# External Integrations

**Analysis Date:** 2026-07-10

## APIs & External Services

**LLM Providers (Chat Completions + Model Catalog):**

- **OpenRouter** - Multi-model gateway, primary LLM provider
  - Base URL: `https://openrouter.ai/api/v1` (default, configurable via `openrouter_base_url` in config)
  - Endpoints: `GET /models`, `POST /chat/completions`, `GET /auth/key`
  - SDK/Client: `internal/provider/openrouter/client.go` - Custom HTTP client with SSE streaming
  - Auth: `OPENROUTER_API_KEY` env var, stored in OS keychain
  - Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`
  - Retry: exponential backoff, max 2 retries, retries on 500/502/503

- **Zen (OpenCode)** - Secondary LLM provider
  - Base URL: `https://opencode.ai/zen/v1` (default, configurable via `zen_base_url`)
  - Endpoints: `GET /models`, `POST /chat/completions`
  - SDK/Client: `internal/provider/zen/client.go`
  - Auth: `ZEN_API_KEY` env var, stored in OS keychain
  - Special: credit detection on 401 responses

- **NVIDIA NIM** - Third LLM provider
  - Base URL: `https://integrate.api.nvidia.com/v1` (default, configurable via `nvidia_base_url`)
  - Endpoints: `GET /models`, `POST /chat/completions`
  - SDK/Client: `internal/provider/nvidia/client.go`
  - Auth: `NVIDIA_API_KEY` env var, stored in OS keychain
  - Special: multimodal model handling (`force_text` hints), model eviction on 404/400

**Web Services (Tools):**

- **Web Search** - HTTP search API
  - Base URL: `https://search.sagibo.net` (default, configurable via `websearch_base_url`)
  - SDK/Client: `internal/tools/websearch.go`
  - Auth: None (public API)
  - SSRF protection: DNS resolution + private IP blocking

- **Web Fetch** - URL content retrieval
  - Implementation: `internal/tools/webfetch.go`
  - Features: SSRF protection, redirect limiting, DNS caching, retry with backoff
  - No external SDK - raw `net/http`

- **HTTP Check** - URL health checking
  - Implementation: `internal/tools/httpcheck.go`
  - SSRF protection with DNS pinning

## Data Storage

**Databases:**
- None - all data is file-based

**File Storage:**
- Session data: `.m31a/` directory (per-project)
  - `session.json` - Session metadata
  - `messages.json` - Conversation history
  - `session.lock` - Lock file for concurrent access
- Backups: `.m31a/backups/` - File versioning before edits
- Tool output: `~/.m31a/tool-output/` - Bounded output storage (7-day retention)
- Logs: `~/.m31a/m31a.log` - JSON-structured application logs
- Config: `~/.m31a/config.toml` - User configuration
- LEDGER: `~/.m31a/LEDGER.md` - Cross-session learning ledger

**Caching:**
- Model catalog cache: In-memory `ModelCache` per provider (`internal/provider/cache.go`)
  - TTL: 5 minutes (configurable)
  - Stale fallback: 24 hours
- DNS cache: In-memory (`internal/tools/dns_cache.go`)
  - TTL: 5 minutes
- Token estimation: EMA-calibrated tiktoken estimation (`internal/tokens/estimator.go`)

## Authentication & Identity

**Auth Provider:**
- OS-native keychain (never writes API keys to disk)
  - Implementation: `pkg/keychain/keychain.go` (interface)
  - Linux: `pkg/keychain/keychain_linux.go` - D-Bus Secret Service + `pass` CLI fallback
  - macOS: `pkg/keychain/keychain_darwin.go` - `/usr/bin/security` CLI
  - Windows: `pkg/keychain/keychain_windows.go` - Windows Credential Manager
  - Caching: `pkg/keychain/keychain.go` - `cachedKeychain` wraps with availability caching
- First-run prompt: On launch, if no provider is configured, TUI prompts for API key entry
- Config resolution: `config.Config.ResolveAPIKeys(keychain)` loads keys from keychain into config

## Monitoring & Observability

**Error Tracking:**
- Structured logging via `log/slog` (`internal/log/log.go`)
  - JSON format (default) or text (via `M31A_LOG_FORMAT`)
  - Output: `~/.m31a/m31a.log` with rotation
  - Log level: configurable via `M31A_LOG_LEVEL`

**Metrics:**
- Metrics collection: `pkg/metrics/` - Records tool execution, LLM token usage, workflow phase metrics
  - Output: `METRICS.json` in session directory
  - Enabled by default (`features.metrics_enabled`)

**Health Checks:**
- Provider health checks: `internal/provider/base_client.go`
  - Endpoints: `/auth/key` (OpenRouter), `/models` (Zen, NVIDIA)
  - Thresholds: <500ms = "live", <2000ms = "slow", else "offline"
  - Interval: 60 seconds

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (primary distribution)
- Homebrew: `brew install eshanized/tap/m31a`
- Scoop: `scoop install m31a` (Windows)
- Linux packages: .deb, .rpm, .apk, .archlinux via GoReleaser nfpm
- Install script: `curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash`

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`)
  - Jobs: lint, test, security, build, release
  - Lint: golangci-lint + gofmt check
  - Test: `go test -race -coverprofile=coverage.out`
  - Security: `govulncheck`
  - Build: matrix (ubuntu/macos/windows x amd64/arm64, excluding windows/arm64)
  - Release: GoReleaser on tag push (`v*`)
  - Go version: 1.25

**Dependency Management:**
- Dependabot (`.github/dependabot.yml`)
  - gomod: weekly, max 10 open PRs
  - github-actions: weekly, max 5 open PRs

## Environment Configuration

**Required env vars:**
- `OPENROUTER_API_KEY` - OpenRouter API access
- `ZEN_API_KEY` - Zen API access
- `NVIDIA_API_KEY` - NVIDIA NIM API access

**Optional env vars:**
- `M31A_CONFIG` - Custom config file path (default: `~/.m31a/config.toml`)
- `M31A_LOG_FORMAT` - Log format: json (default) or text
- `M31A_LOG_LEVEL` - Log level: debug/info/warn/error

**Secrets location:**
- OS keychain (primary) - accessed via `pkg/keychain/`
- `.env` file (development convenience) - gitignored, loaded at startup
- Config TOML `provider.<name>.api_key` - fallback if keychain unavailable

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- LLM API requests (streaming SSE):
  - OpenRouter: `POST https://openrouter.ai/api/v1/chat/completions`
  - Zen: `POST https://opencode.ai/zen/v1/chat/completions`
  - NVIDIA: `POST https://integrate.api.nvidia.com/v1/chat/completions`
- Web search: `GET https://search.sagibo.net/...`
- Web fetch: `GET <user-provided URL>` (SSRF-protected)

## External Tool Dependencies

- `git` - Git operations (worktree management, commits, rollbacks)
  - Client: `internal/git/git.go`
  - Used for: session management, rollback chains, subagent worktrees
- `pass` - Password store CLI (Linux keychain fallback)
  - Client: `pkg/keychain/keychain_linux.go`
  - Used when D-Bus Secret Service unavailable
- `/usr/bin/security` - macOS keychain
  - Client: `pkg/keychain/keychain_darwin.go`

---

*Integration audit: 2026-07-10*
