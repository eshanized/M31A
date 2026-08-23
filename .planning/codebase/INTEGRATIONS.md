# External Integrations

**Analysis Date:** 2026-08-23

## APIs & External Services

**LLM Providers (3):**
- **OpenRouter** — Primary provider for diverse model access
  - SDK/Client: Custom HTTP client in `internal/integrations/provider/openrouter/client.go`
  - Base URL: `https://openrouter.ai/api/v1` (configurable via `openrouter_base_url`)
  - Auth: Bearer token via `OPENROUTER_API_KEY` or `M31A_OPENROUTER_API_KEY`
  - Model discovery: Dynamic via `/models` endpoint
  - Headers: `HTTP-Referer`, `X-Title` (configurable)

- **Zen** — Secondary provider
  - SDK/Client: Custom HTTP client in `internal/integrations/provider/zen/client.go`
  - Base URL: `https://api.zenai.xyz/v1` (configurable via `zen_base_url`)
  - Auth: Bearer token via `ZEN_API_KEY` or `M31A_ZEN_API_KEY`
  - Model discovery: Dynamic via `/models` endpoint
  - Special handling: Credits error detection on 401

- **NVIDIA NIM** — Tertiary provider
  - SDK/Client: Custom HTTP client in `internal/integrations/provider/nvidia/client.go`
  - Base URL: `https://integrate.api.nvidia.com/v1` (configurable via `nvidia_base_url`)
  - Auth: Bearer token via `NVIDIA_API_KEY` or `M31A_NVIDIA_API_KEY`
  - Model discovery: Dynamic via `/models` endpoint
  - Special handling: Multimodal model detection, model deprecation eviction on 404, context exceeded detection

**Provider Abstraction:**
- Interface: `internal/integrations/provider/interface.go` — `LLMProvider`
- Registry: `internal/integrations/provider/registry.go` — Lazy registration, fallback priority
- Base client: `internal/integrations/provider/base_client.go` — Shared HTTP, caching, health checks
- Capabilities: `internal/integrations/provider/capabilities.go` — Model capability parsing (tools, reasoning, vision, chat)
- Resilience: Retry logic (max 2 attempts, exponential backoff), stale cache fallback, circuit breaker patterns

**Web Search:**
- **Sagibo Search** (`https://search.sagibo.net`) — Default web search backend
  - Configurable via `tools.websearch_base_url` in config
  - Enabled by default (`tools.webfetch_enabled: true`)
  - Client: `internal/tools/search/websearch.go`

**Web Fetch:**
- Custom HTTP client with redirect handling, retry logic, DNS caching
- User agent: `M31A/dev` (configurable)
- Max redirects: 10 (configurable)
- Client: `internal/tools/search/webfetch.go`, `internal/tools/search/webfetch_html.go`

## Data Storage

**Databases:**
- None — No traditional database used

**File Storage:**
- **Local filesystem only** — All data stored in project-local `.m31a/` directories:
  - Sessions: `<workDir>/.m31a/sessions/`
  - Backups: `<workDir>/.m31a/backups/`
  - Planning: `<workDir>/.m31a/planning/`
  - Config: `~/.m31a/config.toml` (global), `.m31a/workspace.toml` (workspace), `m31a.json` (project)
  - Ledger: `~/.m31a/LEDGER.md` (global), `<workDir>/.m31a/LEDGER.md` (project)
  - Extensions: Compiled binaries in `~/.m31a/extensions/`

**Caching:**
- **In-memory model cache** (`internal/integrations/provider/cache.go`) — TTL-based (default 5 min fresh, 24 hr stale)
- **DNS cache** (`internal/tools/search/dns_cache.go`) — TTL-based (default 300s)
- **No external cache service** (Redis, Memcached, etc.)

## Authentication & Identity

**Auth Provider:**
- **Custom** — API key based authentication via OS keychain
- Implementation: `internal/integrations/keychain/keychain.go` with platform-specific backends:
  - Linux: `keychain_linux.go` — D-Bus Secret Service (libsecret) + `pass` CLI fallback
  - macOS: `keychain_darwin.go` — `/usr/bin/security` keychain API
  - Windows: `keychain_windows.go` — Windows Credential Manager (cmdkey)
- Keychain interface: `Get(service)`, `Set(service, value)`, `Delete(service)`
- Service prefix: `m31a/` (e.g., `m31a/openrouter`)
- Account name: `m31a`
- Cached availability: `NewCached()` wrapper blacklists failing backends for 5 minutes

**Key Resolution Priority (per provider):**
1. `M31A_<PROVIDER>_API_KEY` env var
2. `<PROVIDER>_API_KEY` env var (standard names)
3. OS keychain (`keychain.Get("provider")`)
4. Config file field (`provider.<provider>.api_key`)

## Monitoring & Observability

**Error Tracking:**
- None — No external error tracking service (Sentry, etc.)

**Logs:**
- Structured logging via `log/slog` (standard library)
- Output: stderr (JSON or text based on config)
- Levels: debug, info, warn, error (configurable via `--log-level`, `M31A_LOG_LEVEL`, or `--debug`)
- File: `internal/integrations/log/log.go` — Logger initialization with version tagging

**Profiling:**
- **pprof** — Built-in Go runtime profiling
- Endpoint: `http://localhost:6060/debug/pprof/` (only when `--debug` or `M31A_DEBUG=1`)
- Available profiles: CPU (30s), heap, allocs, goroutine, threadcreate, block, mutex
- Custom endpoint: `/debug/memstats` for memory summary

**Metrics:**
- Session-level metrics collection (enabled by default: `features.metrics_enabled: true`)
- Output: `METRICS.json` in session directory
- Captures: Tool execution, LLM token usage, workflow phase metrics
- Implementation: `internal/observability/`

## CI/CD & Deployment

**Hosting:**
- GitHub Releases (via goreleaser)
- Binary distribution: Linux (deb, rpm, apk, archlinux), macOS (Homebrew via scoop), Windows (scoop)
- Package managers: Homebrew (tap), Scoop, AUR

**CI Pipeline:**
- GitHub Actions (implied by goreleaser config and release workflow)
- Build: `CGO_ENABLED=0`, cross-compile all targets
- Test: `go test -race -cover -timeout 5m ./...`
- Lint: `golangci-lint run --timeout 5m`
- Validate: `scripts/validate-release.sh`

## Environment Configuration

**Required env vars:**
- At least one provider API key:
  - `OPENROUTER_API_KEY` or `M31A_OPENROUTER_API_KEY`
  - `ZEN_API_KEY` or `M31A_ZEN_API_KEY`
  - `NVIDIA_API_KEY` or `M31A_NVIDIA_API_KEY`
- Optional overrides:
  - `M31A_CONFIG` — Config file path
  - `M31A_THEME` — UI theme
  - `M31A_DEFAULT_MODEL` — Default model ID
  - `M31A_PROVIDER` — Default provider
  - `M31A_PERMISSION_MODE` — Permission default
  - `M31A_COMPACT` — Compact mode
  - `M31A_LOG_LEVEL` — Log level
  - `M31A_DEBUG=1` — Enable debug/pprof

**Secrets location:**
- **Primary:** OS keychain (secure, encrypted at rest)
- **Fallback:** Config file (`~/.m31a/config.toml`) — Only if keychain unavailable
- **Never:** `.env` committed to git (`.env` is gitignored, only `.env.example` committed)
- **Never:** Project config (`m31a.json`) — API keys stripped on save (`config.SaveProject()`)

## Webhooks & Callbacks

**Incoming:**
- None — No webhook endpoints exposed

**Outgoing:**
- None — No outbound webhooks

**Extension Points (subprocess-based):**
- **External Tools** — Configurable via `extensions.tools` in config
  - Protocol: JSON-RPC over stdin/stdout (`pkg/extensions/protocol.go`)
  - Executable: User-defined command with args/env/timeout
  - Implementation: `pkg/extensions/adapter_tool.go`

- **External Providers** — Configurable via `extensions.providers` in config
  - Protocol: JSON-RPC over stdin/stdout
  - Implements `LLMProvider` interface via adapter
  - Implementation: `pkg/extensions/adapter_provider.go`

- **Phase Hooks** — Configurable via `extensions.hooks` in config
  - Pre/post hooks for workflow phases
  - Protocol: JSON-RPC over stdin/stdout
  - Implementation: `pkg/extensions/adapter_hook.go`

---

*Integration audit: 2026-08-23*