# External Integrations

**Analysis Date:** 2026-07-11

## APIs & External Services

**LLM Providers (3):**

| Provider | Purpose | SDK/Client | Auth |
|----------|---------|------------|------|
| **OpenRouter** | Access 100+ models via unified API | Custom client: `internal/provider/openrouter/client.go` | `OPENROUTER_API_KEY` (env/keychain) |
| **Zen** (opencode.ai) | OpenAI-compatible API gateway | Custom client: `internal/provider/zen/client.go` | `ZEN_API_KEY` (env/keychain) |
| **NVIDIA NIM** | NVIDIA-hosted models (Llama, Nemotron, etc.) | Custom client: `internal/provider/nvidia/client.go` | `NVIDIA_API_KEY` (env/keychain) |

**Provider Details:**

### OpenRouter (`internal/provider/openrouter/client.go`)
- **Base URL:** `https://openrouter.ai/api/v1` (`types.DefaultOpenRouterBaseURL`)
- **Auth:** Bearer token via `Authorization` header
- **Headers:** `HTTP-Referer` (default: `https://github.com/eshanized/M31A`), `X-Title` (default: `M31A`)
- **Models:** Fetched dynamically from `/models` endpoint; cached 5 min (`ModelCacheTTL`)
- **Streaming:** SSE via `/chat/completions`
- **Health check:** `GET /auth/key`
- **Config:** `ProviderConfig.OpenRouter` (`internal/config/types.go:136`)

### Zen (`internal/provider/zen/client.go`)
- **Base URL:** `https://opencode.ai/zen/v1` (`types.DefaultZenBaseURL`)
- **Auth:** Bearer token via `Authorization` header
- **Models:** Fetched from `/models`; enriched with pricing/context from OpenRouter data
- **Streaming:** SSE via `/chat/completions`
- **Special handling:** Detects "CreditsError"/billing issues on 401
- **Health check:** `GET /models`
- **Config:** `ProviderConfig.Zen` (`internal/config/types.go:137`)

### NVIDIA NIM (`internal/provider/nvidia/client.go`)
- **Base URL:** `https://integrate.api.nvidia.com/v1` (`types.DefaultNvidiaBaseURL`)
- **Auth:** Bearer token via `Authorization` header
- **Models:** Fetched from `/models`; filters non-chat & known-broken models
- **Multimodal handling:** Adds `force_text` to `extra_body.chat_template_kwargs` for vision models with text-only input
- **Reasoning config:** Supports `extra_body` parameters per-model
- **Health check:** `GET /models`
- **Config:** `ProviderConfig.Nvidia` (`internal/config/types.go:138`)

**Provider Registration:**
- All three registered in `cmd/m31a/main.go:243-261` via `tui.RegisterProvider()`
- Fallback priority configurable: `ProviderConfig.FallbackPriority` (default: `["nvidia", "zen", "openrouter"]`)
- Auto-fallback: `ProviderConfig.AutoFallback` (default: `true`)

## Data Storage

**Databases:**
- **None** — No SQL/NoSQL database used
- All state stored as files in `~/.m31a/` and `<project>/.m31a/`

**File Storage:**
- **Local filesystem only**
- Config: `~/.m31a/config.toml` (TOML)
- Sessions: `<project>/.m31a/sessions/<id>/` (JSON + markdown)
- Backups: `<project>/.m31a/backups/` (file backups before edits)
- Checkpoints: `<project>/.m31a/checkpoints/` (git-based rollback points)
- Ledger: `~/.m31a/LEDGER.md` (Markdown transaction log)
- Metrics: `<session>/METRICS.json` (observability)

**Caching:**
- **In-memory only** — Model catalogs cached per-provider (5 min TTL, 24h stale fallback)
- No Redis/Memcached/remote cache

## Authentication & Identity

**Auth Provider:** **None (BYO API Keys)**
- No OAuth, OIDC, or SSO integration
- API keys sourced from:
  1. Environment variables (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
  2. OS keychain (preferred) — `pkg/keychain/`

**Keychain Implementation (`pkg/keychain/`):**
| Platform | Backend | Implementation |
|----------|---------|----------------|
| Linux | D-Bus Secret Service (`org.freedesktop.secrets`) + `pass` CLI fallback | `keychain_linux.go` (367 lines) |
| macOS | `/usr/bin/security` CLI (Keychain Access) | `keychain_darwin.go` |
| Windows | Windows Credential Manager (`advapi32.dll` via `syscall`) | `keychain_windows.go` (159 lines) |

**Keychain Interface (`pkg/keychain/keychain.go:13-28`):**
```go
type Keychain interface {
    Get(service string) (string, error)
    Set(service, value string) error
    Delete(service string) error
}
```
- Service prefix: `m31a/` (e.g., `m31a/openrouter`)
- Account name: `m31a` (constant)
- Availability caching: `NewCached()` wraps to avoid repeated D-Bus failures

**Config Resolution (`cmd/m31a/main.go:214-217`):**
```go
cfg.ResolveAPIKeys(kc)  // Reads from keychain into config struct
```

## Monitoring & Observability

**Error Tracking:** **None** — No Sentry, Rollbar, etc.

**Logs:**
- **Structured logging:** `log/slog` (stdlib) via `internal/log/log.go`
- Output: JSON to `~/.m31a/m31a.log` (rotating, 10MB max)
- Levels: Debug, Info, Warn, Error
- TUI integrates logger via `app.SetLogger()`

**Metrics (F-077):**
- **Enabled by default** (`FeaturesConfig.MetricsEnabled = true`)
- Written to `<session>/METRICS.json`
- Captures: tool executions, LLM token usage, workflow phase timings
- Collector: `pkg/metrics/collector.go`
- No external metrics backend (Prometheus, Datadog, etc.)

**Health Checks:**
- Per-provider: `HealthCheck(ctx)` hits `/models` or `/auth/key`
- Statuses: `live`, `slow`, `offline`, `degraded`
- Configurable thresholds: `HealthCheckLiveMs` (500ms), `HealthCheckSlowMs` (2000ms)

## CI/CD & Deployment

**Hosting:** **None** — CLI binary distributed via GitHub Releases

**CI Pipeline:**
- **GitHub Actions** (implied by goreleaser config)
- `make check` = `fmt → tidy → vet → lint → test` (race detector on)
- Cross-compilation: `make cross` (6 targets)
- Release: `goreleaser release --snapshot --clean`

**Build Artifacts:**
- `m31a` (current platform)
- `dist/m31a-{os}-{arch}` (all platforms)
- Checksums, signatures via goreleaser

## Environment Configuration

**Required Env Vars (`.env.example`):**
```
OPENROUTER_API_KEY=    # OpenRouter API key
ZEN_API_KEY=           # Zen (opencode.ai) API key
NVIDIA_API_KEY=        # NVIDIA NIM API key
```

**Optional/Implicit:**
- `M31A_CONFIG` — Custom config file path (default: `~/.m31a/config.toml`)
- `HOME` / `USERPROFILE` — Config directory base

**Secrets Location:**
1. **OS Keychain** (preferred) — `m31a/openrouter`, `m31a/zen`, `m31a/nvidia`
2. **Environment variables** — Fallback / CI
3. **Config file** (`config.toml`) — Plaintext fallback (not recommended)

**Key Management:**
- Never written to disk in plaintext by application
- `pass` CLI (Linux) stores GPG-encrypted
- macOS Keychain / Windows Credential Manager use OS encryption
- `.env` files gitignored (only `.env.example` committed)

## Webhooks & Callbacks

**Incoming:** **None** — No webhook endpoints exposed

**Outgoing:** **None** — No webhook dispatching

**External HTTP Calls (Outbound):**
| Target | Purpose | Endpoint |
|--------|---------|----------|
| OpenRouter | Model catalog, chat completions | `https://openrouter.ai/api/v1/models`, `/chat/completions` |
| Zen | Model catalog, chat completions | `https://opencode.ai/zen/v1/models`, `/chat/completions` |
| NVIDIA NIM | Model catalog, chat completions | `https://integrate.api.nvidia.com/v1/models`, `/chat/completions` |
| GitHub (optional) | Release checks, goreleaser | `api.github.com` (via goreleaser) |

**Network Requirements:**
- HTTPS (TLS 1.2+) to all three provider APIs
- No inbound connections required
- Proxy support: Respects `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY` (via Go stdlib)

---

*Integration audit: 2026-07-11*