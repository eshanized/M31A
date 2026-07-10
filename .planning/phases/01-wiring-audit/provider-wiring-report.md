# Provider Layer Wiring Report

**Phase:** 01-wiring-audit  
**Plan:** 01-02  
**Task:** 2 — Provider Layer (3 providers, registry, fallback, health, discovery)  
**Generated:** 2026-07-10

---

## 1. Registry Operations

### 1.1 Registry Structure (`internal/provider/registry.go`)

```go
type Registry struct {
    mu        sync.RWMutex
    providers map[string]LLMProvider
    active    string
}
```

### 1.2 Registry Methods & Wiring

| Method | Purpose | Key Logic |
|--------|---------|-----------|
| `Register(name, p)` | Add provider | Locks, stores in map, sets as active if first (`registry.go:23-33`) |
| `Get(name)` | Retrieve by name | RLock, returns provider or `ErrProviderUnreachable` (`registry.go:85-93`) |
| `Active()` | Get active name | RLock, returns `rLock read (`registry.go:36-40`) |
| `SetActive(name)` | Change active | Lock, validates exists, updates active (`registry.go:42-53`) |
| `TrySetActive(name)` | Atomic check+set | Lock, returns provider + sets active in one op (`registry.go:58-67`) |
| `RollbackActive(from, to)` | Revert on failure | Lock, only reverts if current==from (`registry.go:72-83`) |
| `List()` | All names sorted | RLock, returns sorted slice (`registry.go:95-104`) |
| `ActiveProvider()` | Get active instance | RLock, returns provider directly (`registry.go:112-116`) |

---

## 2. Registration Sequence

### 2.1 Main.go Registration Flow (`cmd/m31a/main.go:219-267`)

```go
// 1. Create registry
registry := provider.NewRegistry()

// 2. Set capability config (reasoning patterns, tool-capable patterns, etc.)
provider.SetCapabilityConfig(provider.CapabilityConfig{
    ReasoningPatterns:        cfg.ModelCapabilities.ExtraReasoningPatterns,
    ToolCapablePatterns:      cfg.ModelCapabilities.ExtraToolCapablePatterns,
    CompletionOnlyPatterns:   cfg.ModelCapabilities.ExtraCompletionOnlyPatterns,
    NonChatPatterns:          cfg.ModelCapabilities.ExtraNonChatPatterns,
    KnownCapabilities:        cfg.ModelCapabilities.KnownCapabilities,
})

// 3. Registration order from config.Provider.RegistrationOrder (default: ["openrouter", "zen", "nvidia"])
for _, id := range cfg.Provider.RegistrationOrder {
    apiKey := cfg.Provider.GetAPIKey(id)  // Resolved via config.ResolveAPIKeys()
    tui.RegisterProvider(registry, cfg, id, apiKey, Version)
}

// 4. Set default active provider
registry.SetActive(cfg.Provider.Default)
```

### 2.2 TUI.RegisterProvider (`internal/tui/provider_registry.go`)

```go
func RegisterProvider(reg *provider.Registry, cfg *config.Config, id, apiKey, version string) error {
    var p provider.LLMProvider
    var err error
    switch id {
    case "openrouter":
        p, err = openrouter.New(apiKey, openrouter.Options{
            BaseURL:           cfg.Provider.OpenRouter.BaseURL,
            CacheTTL:          time.Duration(cfg.Features.ModelCacheTTLMinutes) * time.Minute,
            CacheStaleTTL:     time.Duration(cfg.Features.ModelCacheStaleHours) * time.Hour,
            Referer:           cfg.Provider.OpenRouter.Referer,
            Title:             cfg.Provider.OpenRouter.Title,
            HealthCheckLiveMs: cfg.Features.HealthCheckLiveMs,
            HealthCheckSlowMs: cfg.Features.HealthCheckSlowMs,
            Version:           version,
        })
    case "zen":
        p, err = zen.New(apiKey, zen.Options{...})
    case "nvidia":
        p, err = nvidia.New(apiKey, nvidia.Options{...})
    }
    if err != nil { return err }
    return reg.Register(id, p)
}
```

### 2.3 Config Resolution Priority (per provider)

**File:** `internal/config/loader.go:935-976` — `ResolveAPIKeys()`

1. `M31A_<PROVIDER>_API_KEY` env var
2. `<PROVIDER>_API_KEY` env var (legacy)
3. Keychain: `kc.Get("<provider>")`
4. Config file field (already loaded in `cfg.Provider.<Provider>.APIKey`)

---

## 3. Model Discovery

### 3.1 FetchModels() at Registration

Each provider implements `FetchModels(ctx)` called during TUI startup:

| Provider | Method | Cache Behavior |
|----------|--------|----------------|
| OpenRouter | `internal/provider/openrouter/client.go:86` | Returns cached if not expired; else `Cache.Refresh()` with singleflight |
| Zen | `internal/provider/zen/client.go:73` | Same pattern; enriches with `provider.EnrichModelInfo()` |
| NVIDIA | `internal/provider/nvidia/client.go:71` | Filters non-chat & broken models; `EnrichModelInfo()`; stale fallback |

### 3.2 Cache TTL Configuration

**Config:** `internal/config/loader.go:85-86`
```go
ModelCacheTTLMinutes:      5,    // Fresh TTL
ModelCacheStaleHours:      24,   // Stale TTL (fallback window)
```

**Cache Implementation:** `internal/provider/cache.go`
- `ModelCache.Refresh()` uses `singleflight.Group` — only one HTTP request even with concurrent callers
- `IsExpired()` checks fresh TTL (5 min)
- `IsStale()` checks stale TTL (24 hr) — used for fallback
- `Get(id)` returns nil if stale TTL exceeded

### 3.3 CachedModels() for TUI Model Picker

```go
// BaseClient.CachedModels() → provider.CachedModels(cache) → cache.Models()
// Returns snapshot of map (shallow copy, ~300 models for OpenRouter)
```

**TUI Usage:** `internal/tui/app.go:48-61` — `syncReplProvider()` fetches and sets `activeModel` from cache

---

## 4. Fallback Chain

### 4.1 Fallback Priority Source

**Config:** `internal/config/loader.go:30` — `ProviderConfig.FallbackPriority` (default: `["nvidia", "zen", "openrouter"]`)

### 4.2 Fallback Logic (`internal/provider/fallback.go:27-135`)

```go
func FindFallbackProvider(registry, current, fallbackPriority, healthCheckTimeoutSecs) {
    // 1. Build candidates in priority order (skip current)
    // 2. Parallel health checks (10s timeout default, configurable)
    // 3. Collect results, short-circuit on first "live" in priority order
    // 4. Fallback to first "slow" if no "live"
    // 5. TrySetActive() atomically; RollbackActive() on health check failure
}
```

### 4.3 Health Check Integration

```go
// Triggered on ChatCompletionStream error (engine.go:1413 retryChatStream)
class, reason := retry.ClassifyError(err)
if !retry.IsRetryable(class) {
    // Non-retryable → attempt fallback
    event := FindFallbackProvider(...)
}
```

### 4.4 Retry-After Awareness (`fallback.go:176-203`)

```go
func FindFallbackWithRetryAfter(registry, current, retryAfterHeader, fallbackPriority, timeout) FallbackAfterWait {
    wait := parseRetryAfter(retryAfterHeader)  // capped at 120s
    _, event, err := FindFallbackProvider(...)
    if event != nil && wait > 0 {
        event.Reason = "rate_limited"
    }
    return FallbackAfterWait{Event: event, Wait: wait}
}
```

**TUI Handling:** Returns `FallbackAfterWait` with `Wait` duration; caller schedules as `tea.Tick(wait)` before applying switch.

---

## 5. Health Checks

### 5.1 Health Check Implementation

| Provider | Endpoint | Method |
|----------|----------|--------|
| OpenRouter | `/auth/key` | `BaseClient.HealthCheck(ctx, "/auth/key")` |
| Zen | `/models` | `BaseClient.HealthCheck(ctx, "/models")` |
| NVIDIA | `/models` | `BaseClient.HealthCheck(ctx, "/models")` |

**Base Implementation:** `internal/provider/base_client.go:148-177`
```go
func (b *BaseClient) HealthCheck(ctx, endpoint) types.HealthStatus {
    req, _ := http.NewRequestWithContext(ctx, "GET", b.BaseURLField+endpoint, nil)
    SetCommonHeaders(req, b.APIKeyField, b.Version)
    resp, err := b.CatalogClient.Do(req)  // CatalogClient has hard Timeout (FetchModelsTimeout)
    latency := time.Since(start).Milliseconds()
    
    // Classify by latency thresholds
    switch {
    case latency < b.HealthLiveMs:   return HealthStatus{Status: "live", LatencyMs: latency}
    case latency < b.HealthSlowMs:   return HealthStatus{Status: "slow", LatencyMs: latency}
    default:                         return HealthStatus{Status: "degraded", LatencyMs: latency}
    }
}
```

### 5.2 Health Thresholds Config

**File:** `internal/config/loader.go:89-90`
```go
HealthCheckLiveMs:     2000,   // < 2s = live
HealthCheckSlowMs:     5000,   // 2-5s = slow, > 5s = degraded
```

### 5.3 Background Health Ticker (TUI)

**File:** `internal/tui/app.go:24-26` (Init)
```go
baseCmds = append(baseCmds, NextHealthTick(ctx, types.HealthCheckInterval))
```

**Interval:** `internal/types/constants.go` — `HealthCheckInterval = 30 * time.Second`

**Handler:** `internal/tui/app_handlers.go` — `handleHealthCheckTickMsg` → calls `provider.HealthCheck()` on active → emits `HealthCheckResultMsg`

---

## 6. Streaming Lifecycle

### 6.1 Stream Iterator Pattern

```go
// Engine.streamLLMWithTools() → engine.go:1380
iterator, err := e.provider.ChatCompletionStream(ctx, req)
// Returns *types.StreamIterator with Next() and Close()
```

### 6.2 Provider Stream Implementation

**OpenRouter:** `internal/provider/openrouter/client.go:135-188`
```go
func (c *Client) ChatCompletionStream(ctx, req) (*types.StreamIterator, error) {
    // Retries up to 2x on retryable errors (local retry loop)
    for attempt := 0; attempt <= maxRetries; attempt++ {
        iter, err := c.doChatStream(ctx, req)
        if err == nil { return iter, nil }
        if attempt < maxRetries && provider.IsRetryable(err) {
            delay := time.Duration(1<<uint(attempt)) * time.Second
            select { case <-ctx.Done(): return nil, ctx.Err(); case <-time.After(delay): }
            continue
        }
        return nil, err
    }
}
```

**NVIDIA:** `internal/provider/nvidia/client.go:130-151` — Similar with `maxRetries=2`

**Zen:** `internal/provider/zen/client.go:125-163` — Single attempt, special 401 credit handling

### 6.3 SSE Parsing

**File:** `internal/provider/sse.go`
- `NewSSEParserWithContext(resp, ctx)` — wraps response body with context cancellation
- `Next()` returns `(event, data, error)` — parses SSE format
- `ParseSSEChunk(data, modelID)` → `*types.StreamChunk` with Delta, ToolCalls, Type

### 6.4 Iterator Contract

```go
type StreamIterator struct {
    Next func() (*StreamChunk, error)  // Returns chunk or io.EOF
    Close func() error                  // Must be called to release HTTP response
}
```

**Engine consumes:** `engine.go:1189-1215` `consumeStream()` — loops `Next()`, enforces `MaxLLMResponseBytes`, calls `defer iterator.Close()`

---

## 7. Retry Policy Mapping

### 7.1 Classification (`pkg/retry/classify.go`)

```go
func ClassifyError(err error) (ErrorClass, string) {
    switch {
    case errors.Is(err, ErrRateLimited):      return ClassRateLimited, "rate_limited"
    case errors.Is(err, ErrProviderUnreachable): return ClassUnavailable, "unavailable"
    case errors.Is(err, ErrContextExceeded):  return ClassContextExceeded, "context_exceeded"
    case errors.Is(err, ErrInvalidKey):       return ClassAuth, "auth"
    case errors.Is(err, ErrNoCredits):        return ClassNoCredits, "no_credits"
    case errors.Is(err, ErrModelNotFound):    return ClassModelNotFound, "model_not_found"
    default:                                   return ClassUnknown, "unknown"
    }
}
```

### 7.2 Retry Policy Configuration

| Parameter | Config Key | Default |
|-----------|------------|---------|
| Max Attempts | `Features.RetryMaxAttempts` | 3 |
| Base Delay (ms) | `Features.RetryBaseDelayMs` | 1000 |
| Max Delay (ms) | `Features.RetryMaxDelayMs` | 30000 |
| Backoff Multiplier | `Features.RetryBackoffMultiplier` | 2.0 |

**Engine usage:** `engine.go:1420-1427` — `retry.ConfiguredPolicy()` from config

---

## 8. Capabilities Detection

### 8.1 Capability Config (`internal/config/loader.go:189-195`)

```go
ModelCapabilities: ModelCapabilitiesConfig{
    ExtraReasoningPatterns:      []string{},  // e.g. "o1", "reasoning"
    ExtraToolCapablePatterns:    []string{},  // e.g. "claude", "gpt-4"
    ExtraCompletionOnlyPatterns: []string{},  // e.g. "instruct"
    ExtraNonChatPatterns:        []string{},  // e.g. "embedding"
    KnownCapabilities:           map[string]ModelCapabilityOverride{},
}
```

### 8.2 Capability Resolution (`internal/provider/capabilities.go`)

```go
func ParseModelCapabilities(modelID string, reasoningSuffixes ...string) ModelCapabilities {
    caps := ModelCapabilities{Chat: true}  // Default: chat capable
    
    // Reasoning: matches ExtraReasoningPatterns or suffixes ("-r1", "o1")
    // Tool-capable: matches ExtraToolCapablePatterns
    // Completion-only: matches ExtraCompletionOnlyPatterns
    // Non-chat: matches ExtraNonChatPatterns
    
    // KnownCapabilities override (explicit map)
    if override, ok := KnownCapabilities[modelID]; ok {
        return override
    }
    return caps
}
```

### 8.3 Provider Integration

- **OpenRouter:** `provider.ParseModelCapabilities(m.ID)` at fetch time (`openrouter/client.go:122`)
- **Zen:** `provider.ParseModelCapabilities(m.ID, "-r1")` (`zen/client.go:109`)
- **NVIDIA:** `provider.ParseModelCapabilities(m.ID)` + filters `IsNonChatModel()` and `IsLikelyBrokenOnNvidia()` (`nvidia/client.go:96-101`)

### 8.4 Capability Interface

```go
// LLMProvider interface extension
Capabilities() ModelCapabilities
SupportsReasoning() bool
MaxContextTokens(model) int
```

**Used by:** Context builder for token budgeting, TUI model picker for filtering

---

## 9. Cost Estimation

### 9.1 Pricing Model

**ModelInfo.Pricing:** `internal/types/types.go`
```go
type Pricing struct {
    InputPerMToken  float64  // $ per 1M input tokens
    OutputPerMToken float64  // $ per 1M output tokens
}
```

### 9.2 Estimation Flow

```go
// BaseClient.EstimateCost() → internal/provider/base_client.go:102
func (b *BaseClient) EstimateCost(modelID string, usage types.Usage) float64 {
    return EstimateCost(modelID, usage, b.Cache)  // Uses cached ModelInfo.Pricing
}

// Engine uses: result.Cost = e.provider.EstimateCost(model, usage)
```

**OpenRouter:** Pricing from `/models` API (`PromptToken`, `CompletionToken` × 1M)  
**Zen:** No pricing from API → `EnrichModelInfo()` adds estimates  
**NVIDIA:** No pricing → defaults to 0

---

## 10. Interface Compliance Verification

### 10.1 LLMProvider Interface (`internal/provider/interface.go:9-18`)

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx) ([]ModelInfo, error)
    CachedModels() []ModelInfo
    ChatCompletionStream(ctx, ChatRequest) (*StreamIterator, error)
    EstimateCost(model, Usage) float64
    HealthCheck(ctx) HealthStatus
    GetModel(id) (*ModelInfo, error)
}
```

### 10.2 Compile-Time Checks

Each provider has: `var _ provider.LLMProvider = (*Client)(nil)` 
- OpenRouter: `openrouter/client.go:18`
- Zen: `zen/client.go:19`
- NVIDIA: `nvidia/client.go:18`

### 10.3 Method Coverage

| Method | OpenRouter | Zen | NVIDIA |
|--------|------------|-----|--------|
| Name() | ✅ | ✅ | ✅ |
| APIKey() | ✅ (BaseClient) | ✅ | ✅ |
| FetchModels() | ✅ | ✅ | ✅ |
| CachedModels() | ✅ (BaseClient) | ✅ | ✅ |
| ChatCompletionStream() | ✅ | ✅ | ✅ |
| EstimateCost() | ✅ (BaseClient) | ✅ | ✅ |
| HealthCheck() | ✅ | ✅ | ✅ |
| GetModel() | ✅ (BaseClient) | ✅ | ✅ |

---

## 11. Unused Registration Detection

**Check:** Any provider registered but never `ActiveProvider()`?

**Current wiring:**
- All 3 registered in `main.go` loop
- `registry.SetActive(cfg.Provider.Default)` sets one active
- Fallback chain uses `registry.Get(name)` for candidates
- TUI model picker uses `registry.ActiveProvider().CachedModels()`
- Subagent manager uses `registry.ActiveProvider()` for model resolution

**Result:** No unused registrations — all three participate in fallback chain and are accessible via registry.

---

## 12. Cross-Reference Summary

| Connection | Producer | Consumer |
|------------|----------|----------|
| Config → Provider keys | `config.ResolveAPIKeys()` | `main.go` registration |
| Registry → Fallback | `registry.Get/List/TrySetActive` | `fallback.go:FindFallbackProvider` |
| Registry → TUI | `ActiveProvider()`, `CachedModels()` | `app.go:syncReplProvider`, model picker |
| Registry → Engine | `ActiveProvider()` | `engine.go:NewEngine` provider param |
| Cache → Discovery | `ModelCache.Refresh/Get` | All provider `FetchModels()` |
| Health → Fallback | `HealthCheck()` | `fallback.go` parallel checks |
| Stream → Engine | `ChatCompletionStream()` | `engine.go:streamLLM*` |
| Cost → Engine | `EstimateCost()` | `engine.go:RunPhase` result.Cost |
| Capabilities → Context | `ParseModelCapabilities()` | `context_builder.go` token budgeting |

---

## 13. Summary: Verified Wiring

✅ **Registry operations** — Register, Get, Active, SetActive, TrySetActive, RollbackActive, List, ActiveProvider all implemented with mutex protection  
✅ **Registration sequence** — Config-driven order, capability config set before registration, API keys resolved via env→keychain→config priority  
✅ **Model discovery** — FetchModels at registration, cached with TTL (5m fresh/24h stale), singleflight dedup, stale fallback, CachedModels for TUI  
✅ **Fallback chain** — Config priority list, parallel health checks (10s timeout), live→slow priority, TrySetActive atomic, RollbackActive on failure, Retry-After awareness  
✅ **Health checks** — Per-provider endpoint, CatalogClient with hard timeout, latency classification (live/slow/degraded), background ticker (30s)  
✅ **Streaming** — SSE parser, StreamIterator with Next/Close, engine consumes with MaxLLMResponseBytes limit, provider-level retries (OR/NV: 2, Zen: 1)  
✅ **Retry policy** — ClassifyError → IsRetryable → ConfiguredPolicy from config (maxAttempts, baseDelay, maxDelay, multiplier)  
✅ **Capabilities** — Config-driven patterns, ParseModelCapabilities at fetch, KnownCapabilities override, MaxContextTokens per model  
✅ **Cost estimation** — ModelInfo.Pricing from API, EstimateCost uses cache, OpenRouter has real pricing, Zen/NV enriched/zero  
✅ **Interface compliance** — All 3 providers implement LLMProvider fully, compile-time checked