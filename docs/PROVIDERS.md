# Providers

M31 Autonomous supports three LLM providers with automatic fallback when a provider degrades.

---

## Supported Providers

| Provider | Package | Models | Base URL |
|----------|---------|--------|----------|
| **OpenRouter** | `internal/provider/openrouter/` | 300+ models | `https://openrouter.ai/api/v1` |
| **Zen** | `internal/provider/zen/` | OpenCode catalog | `https://opencode.ai/zen/v1` |
| **Nvidia** | `internal/provider/nvidia/` | NIM models | `https://integrate.api.nvidia.com/v1` |

---

## Authentication

Each provider requires an API key. Resolution order:

1. Environment variable (`M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`, `M31A_NVIDIA_API_KEY`)
2. Standard fallback (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
3. OS keychain (`m31a/openrouter`, `m31a/zen`, `m31a/nvidia`)
4. Config file (`provider.openrouter.api_key`, `provider.zen.api_key`, `provider.nvidia.api_key`)

Set the active provider in config:
```toml
[provider]
default = "openrouter"  # "openrouter", "zen", or "nvidia"
```

Switch at runtime with `/provider` command.

---

## Configuration

```toml
[provider]
default = "openrouter"
auto_fallback = true            # Auto-switch on 429/503 errors

[provider.openrouter]
api_key = "sk-or-v1-..."

[provider.zen]
api_key = "..."

[provider.nvidia]
api_key = "nvapi-..."

# Custom base URLs (for self-hosted or proxied gateways)
openrouter_base_url = "https://openrouter.ai/api/v1"
zen_base_url = "https://opencode.ai/zen/v1"
nvidia_base_url = "https://integrate.api.nvidia.com/v1"

# OpenRouter-specific headers
openrouter_referer = "https://github.com/eshanized/M31A"
openrouter_title = "M31A"
```

---

## Streaming

All providers use Server-Sent Events (SSE) for streaming responses. The SSE parser (`internal/provider/sse.go`) handles:

- Event/data line parsing
- Watchdog timer for stalled streams
- Context cancellation support
- Graceful handling of malformed events

---

## Auto-Fallback

When `auto_fallback = true`, M31 Autonomous automatically switches providers on failure:

1. **Rate limit (429)** — Extracts `Retry-After` header, switches to fallback provider
2. **Server error (503)** — Immediate fallback to next healthy provider
3. **Health check failure** — Parallel health checks across all providers (10s timeout)
4. **Priority-based selection** — Live > Slow > Degraded providers preferred

Provider switching is atomic — concurrent requests are handled safely via the provider registry.

---

## Model Cache

Models are cached in memory with:

- **Fresh TTL:** 5 minutes (configurable via `features.model_cache_ttl_minutes`)
- **Stale TTL:** 24 hours (served as fallback when API fails)
- **Singleflight:** Only one HTTP request made even under concurrent access

---

## Capability Detection

Model capabilities are inferred heuristically from the model ID:

| Capability | Detection Pattern |
|------------|-------------------|
| **Tool use** | claude, gpt, gemini, deepseek, qwen, llama, mistral, command-r, command-a |
| **Reasoning** | /o1, /o3, /o4 patterns + "reason" / "thinking" in ID |
| **Vision** | "vision" or "multimodal" in ID |

---

## Provider-Specific Behavior

### OpenRouter

- Aggregates 300+ models from multiple providers
- Configurable `HTTP-Referer` and `X-Title` headers
- Handles 402 credit exhaustion errors
- Retry with exponential backoff (up to 2 retries)

### Zen

- Model enrichment from Zen catalog
- Credit detection on 401 errors
- Default context length support

### Nvidia

- Model filtering for NIM-compatible models
- Multimodal handling with `extra_body` nesting
- Retry with exponential backoff (up to 2 retries)

---

## Per-Phase Model Assignment

Use different models for different workflow phases:

```toml
[agents]
default = ""
initialize = ""
research = ""
plan = ""
execute = ""
verify = ""
ship = ""
discuss = ""
```

Empty values fall back to the default model. Set via `/phase-model` command or config.

---

## Model Arbitrage

When `model.auto_arbitrage = true`, M31 Autonomous:

1. Classifies task complexity (simple/moderate/complex)
2. Estimates token usage per model
3. Recommends the cheapest model meeting quality requirements
4. Complex tasks require 64K+ context windows

Enable with `/optimize` command or config.
