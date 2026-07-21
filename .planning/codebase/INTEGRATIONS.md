# External Integrations

**Analysis Date:** 2026-07-21

## LLM Providers

M31A integrates with three LLM providers through a unified provider interface (`internal/provider/interface.go:15`). All providers implement `LLMProvider` and are registered via `internal/provider/registry.go`.

### OpenRouter
- **Purpose**: Access to 100+ models via unified OpenAI-compatible API
- **Package**: `internal/provider/openrouter/`
- **Client**: `internal/provider/openrouter/client.go`
- **Base URL**: `https://openrouter.ai/api/v1` (`internal/types/constants.go:48`)
- **Auth**: API key via `OPENROUTER_API_KEY` or `M31A_OPENROUTER_API_KEY` env var, or OS keychain (`internal/config/loader.go:568-581`)
- **Headers**: `HTTP-Referer`, `X-Title` (configurable via `openrouter_referer`, `openrouter_title` in config)
- **Model Discovery**: Dynamic via `/models` endpoint (`client.go:86-133`)
- **Streaming**: SSE via `/chat/completions` (`client.go:158-188`)
- **Health Check**: `GET /auth/key` (`client.go:190-192`)
- **Integration Tests**: `internal/provider/openrouter/integration_test.go`

### Zen (OpenCode)
- **Purpose**: OpenCode's hosted model gateway
- **Package**: `internal/provider/zen/`
- **Client**: `internal/provider/zen/client.go`
- **Base URL**: `https://opencode.ai/zen/v1` (`internal/types/constants.go:51`)
- **Auth**: API key via `ZEN_API_KEY` or `M31A_ZEN_API_KEY` env var, or OS keychain (`internal/config/loader.go:583-594`)
- **Model Discovery**: Dynamic via `/models` endpoint (`client.go:73-123`)
- **Streaming**: SSE via `/chat/completions` (`client.go:125-163`)
- **Health Check**: `GET /models` (`client.go:165-167`)
- **Special Handling**: Credit detection on 401 (`client.go:149-156`)
- **Integration Tests**: `internal/provider/zen/integration_test.go`

### NVIDIA NIM
- **Purpose**: NVIDIA-hosted optimized models (Llama, Nemotron, etc.)
- **Package**: `internal/provider/nvidia/`
- **Client**: `internal/provider/nvidia/client.go`
- **Base URL**: `https://integrate.api.nvidia.com/v1` (`internal/types/constants.go:54`)
- **Auth**: API key via `NVIDIA_API_KEY` or `M31A_NVIDIA_API_KEY` env var, or OS keychain (`internal/config/loader.go:596-607`)
- **Model Discovery**: Dynamic via `/models` endpoint with filtering (`client.go:71-128`)
  - Filters out non-chat models (`provider.IsNonChatModel`)
  - Filters known broken models (`provider.IsLikelyBrokenOnNvidia`)
- **Streaming**: SSE via `/chat/completions` with NVIDIA-specific extensions (`client.go:130-260`)
  - `extra_body` for reasoning params
  - `force_text` hint for multimodal models with text-only input
- **Health Check**: `GET /models` (`client.go:262-264`)
- **Special Handling**: 404 evicts model from cache; 400 with context error evicts incompatible model (`client.go:236-252`)
- **Integration Tests**: `internal/provider/nvidia/integration_test.go`

## Provider Abstraction Layer

| File | Purpose |
|------|---------|
| `internal/provider/interface.go` | `LLMProvider` interface definition |
| `internal/provider/registry.go` | Thread-safe provider registry with active provider tracking |
| `internal/provider/base_client.go` | Shared HTTP client, caching, health check logic |
| `internal/provider/common.go` | Shared request/response builders, headers, SSE parsing |
| `internal/provider/capabilities.go` | Model capability detection (tools, reasoning, vision) |
| `internal/provider/fallback.go` | Automatic fallback on provider failure |
| `internal/provider/reasoning.go` | Reasoning config per model |

## External APIs

### Web Search
- **Service**: Sagibo Search (`https://search.sagibo.net`)
- **Config**: `tools.websearch_base_url` (`internal/config/types.go:139`)
- **Enabled by default**: `tools.websearch_enabled = true` (`types.go:140`)
- **Implementation**: `internal/tools/search/websearch.go`

### Web Fetch
- **Service**: Generic HTTP fetch with HTML extraction
- **Implementation**: `internal/tools/search/webfetch.go`, `internal/tools/search/webfetch_html.go`
- **Config**: `tools.webfetch_max_redirects`, `tools.webfetch_user_agent` (`types.go:136`)

### DNS Cache
- **Implementation**: Custom in-memory DNS cache (`internal/tools/search/dns_cache.go`)
- **Config**: `tools.dns_cache_ttl_secs` (default 300s, `types.go:152`)

## Keychain / Secrets

### OS Keychain Integration
- **Package**: `internal/keychain/`
- **Interface**: `keychain.Keychain` (`keychain.go:13-28`)
- **Platform Implementations** (build tags):
  - **Linux**: `keychain_linux.go` — D-Bus Secret Service (`org.freedesktop.secrets`) with `pass` CLI fallback
  - **macOS**: `keychain_darwin.go` — `/usr/bin/security` CLI
  - **Windows**: `keychain_windows.go` — Windows Credential Manager (`cmdkey`, `CredWrite`)
- **Service Prefix**: `m31a/` (`keychain.go:6`)
- **Account Name**: `m31a` (`keychain.go:8`)
- **Availability Caching**: `keychain.NewCached()` wraps backend to avoid repeated failures (`keychain.go:42-87`)

### Environment Variables
Priority order for API keys (`internal/config/loader.go:566-607`):
1. `M31A_<PROVIDER>_API_KEY` (e.g., `M31A_OPENROUTER_API_KEY`)
2. `<PROVIDER>_API_KEY` (e.g., `OPENROUTER_API_KEY`)
3. OS Keychain (service = provider name: "openrouter", "zen", "nvidia")
4. Config file field (`provider.<provider>.api_key`)

### .env File Support
- **File**: `.env` in working directory (gitignored)
- **Loader**: `internal/config/loader.go:739-785` (`LoadDotEnv()`)
- **Security**: Skips group/world-writable files (`loader.go:750-753`)
- **Example**: `.env.example` with three provider keys
- **Called**: Before config load in `main.go:276` and `loader.go:263`

## Configuration

### Config File Locations (load order, later overrides earlier)
1. **Defaults** — `internal/config/loader.go:26` (`DefaultConfig()`)
2. **Global TOML** — `~/.m31a/config.toml` (or `$M31A_CONFIG`) (`main.go:298-304`, `loader.go:237-259`)
3. **Environment Variables** — `M31A_*` prefix (`loader.go:265-280`)
4. **Project TOML** — `m31a.toml` in cwd or up to 3 parents (`loader.go:282-302`, `types.MaxProjectConfigDepth=3`)
5. **Variable Substitution** — `${VAR}` → env value (`loader.go:306`)

### Config Structure (`internal/config/types.go`)
- **Provider**: API keys, base URLs, fallback priority, registration order, health check timeout
- **Model**: Default model, context warnings, arbitration settings
- **UI**: Theme, layout, sidebar, toasts, animations, accessibility
- **Permissions**: Default mode, timeout, per-tool rules with TTL
- **Tools**: Rate limits, output bounds, DNS cache, bash timeout, webfetch retries
- **Features**: Workflow mode, cache TTLs, health thresholds, quality gates, retry policy
- **Agents**: Per-phase model assignments, subagent profiles
- **Git**: Commit prefixes, user identity
- **Compaction**: Auto, proactive, thresholds, custom templates
- **Narrative**: Template/classification overrides
- **Templates**: External dir, website framework, custom palettes
- **Prompts**: 4-level override chain (config → project → global → embedded)

## E2E Test Integration

Real API integration tests in `e2e_test.go`:
- `TestBinary_Prompt_NvidiaRealAPI` — Requires `NVIDIA_API_KEY`, uses `meta/llama-3.1-8b-instruct`
- `TestBinary_Prompt_ZenRealAPI` — Requires `ZEN_API_KEY`, uses `openai/gpt-3.5-turbo`
- `TestBinary_Prompt_OpenRouterRealAPI` — Requires `OPENROUTER_API_KEY`, uses `openai/gpt-3.5-turbo`
- All skip gracefully when env var not set
- Binary built fresh per test (`e2e_test.go:151-161`)

## Key Files Summary

| Integration | Key Files |
|-------------|-----------|
| OpenRouter | `internal/provider/openrouter/client.go`, `internal/provider/openrouter/integration_test.go` |
| Zen | `internal/provider/zen/client.go`, `internal/provider/zen/integration_test.go` |
| NVIDIA | `internal/provider/nvidia/client.go`, `internal/provider/nvidia/integration_test.go` |
| Provider Interface | `internal/provider/interface.go`, `internal/provider/registry.go` |
| Keychain | `internal/keychain/keychain.go`, `internal/keychain/keychain_{linux,darwin,windows}.go` |
| Config Loading | `internal/config/loader.go`, `internal/config/types.go` |
| Env File | `internal/config/loader.go:739` (LoadDotEnv), `.env.example` |
| Web Search | `internal/tools/search/websearch.go` |
| Web Fetch | `internal/tools/search/webfetch.go`, `internal/tools/search/webfetch_html.go` |
| E2E Tests | `e2e_test.go` |

---

*Integration audit: 2026-07-21*