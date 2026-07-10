# INTEGRATIONS.md — External Integrations

> Last mapped: 2026-07-10

## Overview

| Integration | Type | Purpose | Files |
|-------------|------|---------|-------|
| **OpenRouter** | LLM API Provider | Chat completions, model discovery, pricing | `internal/provider/openrouter/client.go` |
| **Zen** | LLM API Provider | Chat completions, model discovery | `internal/provider/zen/client.go` |
| **NVIDIA NIM** | LLM API Provider | Chat completions, model discovery | `internal/provider/nvidia/client.go` |
| **OS Keychain** | Secrets Storage | Secure API key storage (macOS/Windows/Linux) | `pkg/keychain/` |
| **SearXNG** | Web Search | Privacy-respecting web search | `internal/tools/websearch.go` |
| **Git CLI** | Version Control | Git operations, worktrees | `internal/git/git.go` |
| **Goreleaser** | Release Automation | Cross-compile, package, release | `.goreleaser.yaml` |
| **golangci-lint** | Code Quality | Linting pipeline | `.golangci.yml`, `Makefile` |

---

## API Providers

### OpenRouter (`internal/provider/openrouter/`)

- **Purpose**: Primary LLM provider — chat completions, model discovery with pricing, context length, capabilities
- **SDK/Client**: Custom Go client using `net/http` (no third-party SDK)
- **Auth**: API key via environment variable or OS keychain
  - Env vars: `OPENROUTER_API_KEY` or `M31A_OPENROUTER_API_KEY`
  - Keychain key: `openrouter`
  - Config fallback: `provider.openrouter.api_key`
- **Endpoints**:
  - `GET /models` — fetch available models with pricing & metadata
  - `POST /chat/completions` — streaming chat completions (SSE)
  - `GET /auth/key` — health check
- **Base URL**: `https://openrouter.ai/api/v1` (`types.DefaultOpenRouterBaseURL`)
- **Configuration**: `internal/config/types.go` → `ProviderConfig.OpenRouter`
- **Error Handling**:
  - Retry logic: 2 retries with exponential backoff (1s, 2s) in `ChatCompletionStream`
  - Shared error handling via `BaseClient.HandleChatHTTPErrorWithCredits` for 429 (rate limit), 401 (invalid key), 402 (no credits), 503 (unavailable)
  - Context exceeded detection via `IsContextExceeded`
  - Model eviction on 404/400 (model unavailable)
  - Stale cache fallback on fetch failure
- **Files**:
  - `internal/provider/openrouter/client.go` — main client
  - `internal/provider/openrouter/client_test.go` — unit tests
  - `internal/provider/openrouter/integration_test.go` — real API tests (requires `OPENROUTER_API_KEY`)

### Zen (`internal/provider/zen/`)

- **Purpose**: Alternative LLM provider — chat completions, model discovery
- **SDK/Client**: Custom Go client using `net/http`
- **Auth**: API key via environment variable or OS keychain
  - Env vars: `ZEN_API_KEY` or `M31A_ZEN_API_KEY`
  - Keychain key: `zen`
  - Config fallback: `provider.zen.api_key`
- **Endpoints**:
  - `GET /models` — fetch available models
  - `POST /chat/completions` — streaming chat completions (SSE)
  - `GET /models` — health check (same as models endpoint)
- **Base URL**: `https://opencode.ai/zen/v1` (`types.DefaultZenBaseURL`)
- **Configuration**: `internal/config/types.go` → `ProviderConfig.Zen`
- **Special Handling**:
  - Zen API does not return pricing or per-model context length — enriched via `provider.EnrichModelInfo(models, "zen")`
  - Credits error detection on 401: checks response body for "CreditsError", "payment", "billing", "credit"
  - Reasoning config via `provider.GetReasoningConfig` for `extra_body` params
- **Files**:
  - `internal/provider/zen/client.go` — main client
  - `internal/provider/zen/client_test.go` — unit tests
  - `internal/provider/zen/integration_test.go` — real API tests (requires `ZEN_API_KEY`)

### NVIDIA NIM (`internal/provider/nvidia/`)

- **Purpose**: NVIDIA-hosted models (Nemotron, Llama, etc.) — chat completions, model discovery
- **SDK/Client**: Custom Go client using `net/http`
- **Auth**: API key via environment variable or OS keychain
  - Env vars: `NVIDIA_API_KEY` or `M31A_NVIDIA_API_KEY`
  - Keychain key: `nvidia`
  - Config fallback: `provider.nvidia.api_key`
- **Endpoints**:
  - `GET /models` — fetch available models
  - `POST /chat/completions` — streaming chat completions (SSE)
  - `GET /models` — health check
- **Base URL**: `https://integrate.api.nvidia.com/v1` (`types.DefaultNvidiaBaseURL`)
- **Configuration**: `internal/config/types.go` → `ProviderConfig.Nvidia`
- **Special Handling**:
  - Filters non-chat models via `IsNonChatModel`
  - Filters known broken models via `IsLikelyBrokenOnNvidia`
  - Multimodal model detection (`isMultimodalModel`) — adds `force_text` hint for text-only requests
  - Reasoning config via `provider.GetReasoningConfig` for `extra_body` params
  - Model eviction on 404 (unavailable/deprecated) and 400 (incompatible)
  - Retry logic: 2 retries with exponential backoff in `ChatCompletionStream`
  - Credit detection on 402 (payment required)
- **Files**:
  - `internal/provider/nvidia/client.go` — main client
  - `internal/provider/nvidia/integration_test.go` — real API tests (requires `NVIDIA_API_KEY`)

### Provider Abstraction Layer (`internal/provider/`)

- **Registry**: `internal/provider/registry.go` — registers and manages active provider
- **Interface**: `internal/provider/interface.go` — `LLMProvider` interface
- **Base Client**: `internal/provider/base_client.go` — shared HTTP clients, caching, health checks, error handling
  - Shared `http.Transport` across all providers (connection pooling)
  - Dual HTTP clients: `HTTPClient` (no timeout, for SSE streaming) and `CatalogClient` (hard timeout for model fetch/health)
  - Model cache with TTL and stale fallback
  - Health status classification: Live (< `HealthLiveMs`), Slow (< `HealthSlowMs`), Degraded
- **SSE Parser**: `internal/provider/sse.go` — Server-Sent Events parsing for streaming
- **Resilience**: `internal/provider/resilience.go` — circuit breaker, retry policies
- **Fallback**: `internal/provider/fallback.go` — provider fallback chain

---

## Authentication & Secrets

### OS Keychain (`pkg/keychain/`)

- **Backend**: Platform-native secure storage
  - **macOS** (`keychain_darwin.go`): `/usr/bin/security` CLI — Keychain Access
  - **Windows** (`keychain_windows.go`): Windows Credential Manager via `CredReadW`/`CredWriteW`/`CredDeleteW` (advapi32.dll)
  - **Linux** (`keychain_linux.go`): D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
- **Keys Stored**: API keys for providers
  - Service names: `openrouter`, `zen`, `nvidia` (prefixed with `m31a/`)
  - Account: `m31a` (constant `AccountName`)
- **Interface**: `pkg/keychain/keychain.go` — `Keychain` interface with `Get`, `Set`, `Delete`
- **Availability Caching**: `cachedKeychain` wrapper — once any operation returns `ErrKeychainUnavailable`, all subsequent operations fail fast without retrying D-Bus/pass
- **Error Types**:
  - `ErrKeyNotFound` — key doesn't exist
  - `ErrKeychainUnavailable` — backend not accessible
  - `ErrKeychainDecrypt` — GPG decryption failed (Linux `pass` fallback)
  - `ErrNotImplemented` — platform not supported
- **Integration**: `internal/config/loader.go` → `Config.ResolveAPIKeys(kc)` and `Config.SaveWithKeychain(path, kc)`
- **Priority Order** (resolution):
  1. `M31A_<PROVIDER>_API_KEY` env var
  2. `<PROVIDER>_API_KEY` env var (e.g., `OPENROUTER_API_KEY`)
  3. OS Keychain (`keychain.Get("openrouter")`)
  4. Config file field (`cfg.Provider.OpenRouter.APIKey`)
- **Persistence**: On save, keys written to keychain; cleared from config file if keychain succeeds. If keychain unavailable, keys persisted to config file as fallback (with warning).

---

## Configuration Sources

### Environment Variables

| Variable | Purpose | Fallback |
|----------|---------|----------|
| `OPENROUTER_API_KEY` | OpenRouter API key | `M31A_OPENROUTER_API_KEY` |
| `ZEN_API_KEY` | Zen API key | `M31A_ZEN_API_KEY` |
| `NVIDIA_API_KEY` | NVIDIA API key | `M31A_NVIDIA_API_KEY` |
| `M31A_CONFIG` | Config file path override | — |
| `M31A_THEME` | UI theme override | — |
| `M31A_DEFAULT_MODEL` | Default model ID | — |
| `M31A_PROVIDER` | Default provider | — |
| `M31A_PERMISSION_MODE` | Permission mode | — |
| `M31A_COMPACT` | Compact mode | — |

### Config Files (TOML)

**Loading Order** (later overrides earlier):
1. `DefaultConfig()` — zero-valued defaults (`internal/config/loader.go:27`)
2. Global config: `~/.m31a/config.toml` (or `M31A_CONFIG` env)
3. Environment variables (`M31A_*`)
4. Project config: `m31a.toml` (walked up 3 parent dirs from CWD)
5. Variable substitution: `${VAR}` → environment value

**Key Config Sections** (`internal/config/types.go`):
- `Provider` — API keys, fallback priority, registration order, health check timeout
- `UI` — theme, sidebar, toasts, welcome screen, layout thresholds
- `Model` — context warnings, EMA alpha, default context length
- `Features` — cache TTL, health thresholds, retry policy, quality gates
- `Tools` — rate limits, output bounds, DNS cache, WebSearch URL
- `Permissions` — default mode, rules, agents
- `Git` — commit prefixes, user info
- `Compaction` — auto-compaction thresholds, templates

**Variable Substitution**: `${VAR}` patterns in string fields resolved against environment. Unresolved vars preserved with warning.

**Files**:
- `internal/config/loader.go` — load, merge, validate, watch
- `internal/config/types.go` — config structs
- `internal/config/merge.go` — deep merge logic
- `.env.example` — example environment file

### .env File Support

- Auto-loaded from CWD via `config.LoadDotEnv()` (called in `main.go` and `config.Load`)
- Parsed line-by-line, `KEY=VALUE` format
- Skips group/world-writable files (security)
- Does not override existing env vars

---

## Third-Party SDKs & Libraries

| Package | Purpose | Version | File Reference |
|---------|---------|---------|----------------|
| `github.com/charmbracelet/bubbletea` | TUI framework (Elm architecture) | v1.3.0 | `go.mod` |
| `github.com/charmbracelet/bubbles` | TUI components | v0.20.0 | `go.mod` |
| `github.com/charmbracelet/lipgloss` | Terminal styling | v1.1.0 | `go.mod` |
| `github.com/charmbracelet/glamour` | Markdown rendering | v0.6.0 | `go.mod` |
| `github.com/BurntSushi/toml` | TOML parsing | v1.6.0 | `go.mod` |
| `github.com/fsnotify/fsnotify` | File watching | v1.10.1 | `go.mod` |
| `github.com/godbus/dbus/v5` | Linux D-Bus (keychain) | v5.2.2 | `go.mod` |
| `github.com/pkoukk/tiktoken-go` | Token counting | v0.1.8 | `go.mod` |
| `golang.org/x/sync` | Singleflight, errgroup | v0.21.0 | `go.mod` |
| `github.com/bmatcuk/doublestar/v4` | Glob matching | v4.10.0 | `go.mod` |

**Standard Library Only** for HTTP/API clients — no provider-specific SDKs.

---

## External Services

### SearXNG Web Search (`internal/tools/websearch.go`)

- **Purpose**: Privacy-respecting meta-search engine for web search tool
- **Endpoint**: Configurable base URL, default `https://search.sagibo.net` (`tools.WebSearchBaseURL`)
- **API**: `GET /search?q={query}&format=json&categories=general`
- **Auth**: None (public instance)
- **Client**: Custom `http.Client` with:
  - DNS caching (`DNSCache` with TTL 300s)
  - Private IP blocking (SSRF protection)
  - Redirect validation (max 5, blocks private IPs on redirect)
  - 30s request timeout
- **Configuration**: `config.Tools.WebSearchBaseURL`, `config.Tools.WebSearchEnabled`
- **Rate Limits**: `config.Tools.RateLimitBurst` (20), `config.Tools.RateLimitPerSec` (10)

### Git CLI (`internal/git/git.go`)

- **Purpose**: Git operations via `git` binary
- **Operations**: status, diff, log, commit, branch, worktree, push, remote
- **Worktrees**: Used for subagent isolation (`internal/tools/subagent/worktree.go`)
- **No libgit2** — pure CLI wrapper for portability

---

## CI/CD & Release

### Goreleaser (`.goreleaser.yaml`)

- **Build**: `CGO_ENABLED=0`, `-trimpath`, `-ldflags "-s -w -X main.Version=..."`
- **Targets**:
  - linux/amd64, linux/arm64
  - darwin/amd64, darwin/arm64
  - windows/amd64 (windows/arm64 excluded)
- **Artifacts**: Binary archives, checksums (SHA256)
- **Packages**: 
  - nfpm: deb, rpm, apk, archlinux
  - scoop: Windows package manager
- **Changelog**: Conventional commits, excludes docs/test/chore/ci/build/merge commits
- **Release**: Draft by default, prerelease auto-detected

### Makefile Targets

| Target | Purpose |
|--------|---------|
| `build` | Optimized static binary (current platform) |
| `debug` | Debug symbols, no strip |
| `test` | Race detector + coverage |
| `test-fast` | No race detector |
| `lint` | golangci-lint (5m timeout) |
| `check` | fmt → tidy → vet → lint → test |
| `cover` | HTML coverage report |
| `cross` | Cross-compile all targets |

### Linting (`.golangci.yml`)

- **Enabled**: govet (with shadow), staticcheck, errcheck, ineffassign, unused
- **Test file exclusions**: errcheck, unused disabled for `_test.go` files
- **Timeout**: 5 minutes

---

## File References

### Provider Integrations
- `internal/provider/openrouter/client.go`
- `internal/provider/openrouter/client_test.go`
- `internal/provider/openrouter/integration_test.go`
- `internal/provider/zen/client.go`
- `internal/provider/zen/client_test.go`
- `internal/provider/zen/integration_test.go`
- `internal/provider/nvidia/client.go`
- `internal/provider/nvidia/integration_test.go`
- `internal/provider/registry.go`
- `internal/provider/interface.go`
- `internal/provider/base_client.go`
- `internal/provider/sse.go`
- `internal/provider/fallback.go`
- `internal/provider/resilience.go`
- `internal/provider/common.go`
- `internal/provider/capabilities.go`
- `internal/provider/model_metadata.go`

### Authentication & Secrets
- `pkg/keychain/keychain.go`
- `pkg/keychain/keychain_darwin.go`
- `pkg/keychain/keychain_linux.go`
- `pkg/keychain/keychain_windows.go`
- `pkg/keychain/errors.go`
- `internal/config/loader.go` (ResolveAPIKeys, SaveWithKeychain)

### Configuration
- `internal/config/loader.go`
- `internal/config/types.go`
- `internal/config/merge.go`
- `internal/config/validation.go`
- `.env.example`

### External Services
- `internal/tools/websearch.go`
- `internal/tools/websearch_test.go`
- `internal/git/git.go`

### CI/CD
- `.goreleaser.yaml`
- `Makefile`
- `.golangci.yml`
- `go.mod`
- `go.sum`

---

*Integration audit: 2026-07-10*