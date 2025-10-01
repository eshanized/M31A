---
phase: provider-layer-review
reviewed: 2026-06-06T12:00:00Z
depth: deep
files_reviewed: 18
files_reviewed_list:
  - internal/provider/interface.go
  - internal/provider/registry.go
  - internal/provider/cache.go
  - internal/provider/common.go
  - internal/provider/fallback.go
  - internal/provider/reasoning.go
  - internal/provider/sse.go
  - internal/provider/capabilities.go
  - internal/provider/openrouter/client.go
  - internal/provider/zen/client.go
  - internal/provider/openrouter/client_test.go
  - internal/provider/zen/client_test.go
  - internal/provider/cache_refresh_test.go
  - internal/provider/resilience_test.go
  - internal/provider/registry_test.go
  - internal/provider/sse_test.go
  - internal/provider/reasoning_test.go
  - internal/provider/cache_test.go
findings:
  critical: 0
  warning: 8
  info: 4
  total: 12
status: issues_found
---

# Provider Layer: Deep Code Review Report

**Reviewed:** 2026-06-06T12:00:00Z
**Depth:** deep
**Files Reviewed:** 18 (10 source, 8 test)
**Status:** issues_found

## Summary

Deep review of the entire provider layer covering 10 source files and 8 test files. Cross-file analysis traced every method from the `LLMProvider` interface through the registry, cache, SSE parser, reasoning normalizer, fallback logic, and both OpenRouter/Zen client implementations. Callers in the TUI (`streaming.go`, `app_update.go`, `health.go`, `cache_refresh.go`, `modelselector.go`, `repl.go`, `commands_*.go`), workflow engine (`engine.go`), and entry point (`cmd/m31a/main.go`) were all audited.

**Key findings:**
- Both clients compile-time verify interface compliance (`var _ provider.LLMProvider = (*Client)(nil)`) and implement all 7 methods correctly.
- Registry is properly initialized in `main.go` with both providers registered conditionally on API key presence. Both providers go through the registry.
- Two significant correctness issues: (1) `Registry.Get()` returns `ErrProviderUnreachable` instead of `ErrProviderNotFound` for missing providers, conflating lookup failures with network errors, and (2) `cache.Refresh()` with singleflight deduplication is never called — both clients bypass it with direct `cache.Set()`.
- The SSE parser's `context.Background()` usage is not critical because stream cancellation propagates through `iterator.Close()` → body close, but the `NewSSEParserWithContext` defense-in-depth layer is unused.
- Health check, fallback, and cache refresh are all properly wired through the TUI event loop.

## Critical Issues

No critical issues found. All findings below are Warning or Info severity.

## Warnings

### WR-01: Registry.Get() Returns ErrProviderUnreachable Instead of ErrProviderNotFound

**File:** `internal/provider/registry.go:74`
**Issue:** When a provider name is not found in the registry, `Get()` returns `m31errors.ErrProviderUnreachable`. This conflates two distinct failure modes: "provider name not registered in the registry" vs "provider registered but network unreachable". The `FindFallbackProvider` function (fallback.go:30-31) correctly calls `TrySetActive` which wraps `ErrProviderNotFound`, but `Get()` does not.

Current callers (`cache_refresh.go:50`, `modelselector.go:112`, `app.go:684`) all check `err != nil` as a boolean and don't inspect the sentinel, so there's no incorrect behavior today. However, any future caller checking `errors.Is(err, ErrProviderUnreachable)` to decide whether to trigger auto-fallback would get a false positive from a simple registry miss.

**Fix:**
```go
// registry.go:69-77
func (r *Registry) Get(name string) (LLMProvider, error) {
    r.mu.RLock()
    defer r.mu.RUnlock()
    p, ok := r.providers[name]
    if !ok {
        return nil, fmt.Errorf("provider %q not registered: %w", name, m31errors.ErrProviderNotFound)
    }
    return p, nil
}
```

### WR-02: Cache Refresh Singleflight Deduplication Is Dead Code

**File:** `internal/provider/cache.go:46-63`, `internal/provider/openrouter/client.go:165`, `internal/provider/zen/client.go:152`
**Issue:** `ModelCache.Refresh()` uses `singleflight.Group` to deduplicate concurrent HTTP calls. However, both OpenRouter and Zen `FetchModels()` methods bypass this entirely — they make their own HTTP request and call `c.cache.Set(models)` directly. The `Refresh()` method, its singleflight guard, and the `refreshing` atomic flag are all dead code in production.

This means 10 concurrent `FetchModels()` calls (e.g., from model selector UI + health check + cache refresh ticker + command execution) will each make separate HTTP requests to the provider API instead of being coalesced into one.

**Fix:** Refactor `FetchModels` in both clients to use `c.cache.Refresh(ctx, fetchFn)`:
```go
// openrouter/client.go — FetchModels
func (c *Client) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
    if !c.cache.IsExpired() && c.cache.Len() > 0 {
        return provider.CachedModels(c.cache), nil
    }
    return c.cache.Refresh(ctx, func(ctx context.Context) ([]types.ModelInfo, error) {
        req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
        if err != nil {
            return nil, err
        }
        provider.SetCommonHeaders(req, c.apiKey, Version)
        resp, err := c.httpClient.Do(req)
        // ... rest of fetch + parse logic
    })
}
```

### WR-03: HealthCheckTicker Accepts Unused Parameters

**File:** `internal/tui/health.go:12-13`
**Issue:** `HealthCheckTicker(ctx, registry, activeProvider, interval)` accepts `registry *provider.Registry` and `activeProvider string` but never uses them in the function body. The function just returns `tea.Tick(interval, ...)`. The actual provider lookup happens in `AppState.Update()` when `HealthCheckTickMsg` arrives (app_update.go:459). The parameters are misleading — callers pass them expecting them to influence the ticker, but they have no effect.

**Fix:** Remove the unused parameters:
```go
func HealthCheckTicker(ctx context.Context, interval time.Duration) tea.Cmd {
    if ctx == nil {
        return nil
    }
    if interval <= 0 {
        interval = types.HealthCheckInterval
    }
    return tea.Tick(interval, func(t time.Time) tea.Msg {
        return HealthCheckTickMsg{Time: t}
    })
}
```
Update callers at `app.go:604` and `app_update.go:788`.

### WR-04: SetActive("") Wraps ErrProviderNotFound — Wrong Sentinel for Validation Error

**File:** `internal/provider/registry.go:46`
**Issue:** `SetActive("")` returns `fmt.Errorf("provider name cannot be empty: %w", m31errors.ErrProviderNotFound)`. An empty name is a validation error (invalid argument), not a "provider not found" condition. Using `ErrProviderNotFound` conflates two distinct failure modes. Any caller checking `errors.Is(err, ErrProviderNotFound)` cannot distinguish between "empty name" and "name not registered".

**Fix:** Use a plain error without wrapping the sentinel:
```go
if name == "" {
    return fmt.Errorf("provider name cannot be empty")
}
```

### WR-05: maxRetryAfter Comment Says 60s But Constant Is 120s

**File:** `internal/provider/fallback.go:19` and `internal/provider/fallback.go:105`
**Issue:** Two comments say "(60s cap)" but `maxRetryAfter = types.MaxRetryAfterWait` which is `120 * time.Second` (types/constants.go:77). This is misleading documentation that could cause incorrect assumptions about retry behavior.

**Fix:**
```go
// Line 19: Update comment
// maxRetryAfter is the maximum duration to wait for Retry-After header (120s cap).

// Line 105: Update comment
// Wait is capped at maxRetryAfter (120s).
```

### WR-06: IsContextExceeded Has Implicit Operator Precedence Dependency

**File:** `internal/provider/common.go:30-34`
**Issue:** The expression on line 34:
```go
strings.Contains(lower, "context_length") && strings.Contains(lower, "exceed")
```
relies on `&&` having higher precedence than `||` to work correctly as `A || B || C || D || (E && F)`. While this IS correct in Go, the lack of explicit parentheses makes the logic fragile. If someone adds a new term and misplaces a parenthesis, the semantics change silently.

**Fix:** Add explicit parentheses:
```go
return strings.Contains(lower, "context_length_exceeded") ||
    strings.Contains(lower, "maximum context length") ||
    strings.Contains(lower, "request too large") ||
    strings.Contains(lower, "context window exceeded") ||
    (strings.Contains(lower, "context_length") && strings.Contains(lower, "exceed"))
```

### WR-07: ResponseHeaderTimeout Set to 30s May Prematurely Abort Slow-Starting Streams

**File:** `internal/provider/openrouter/client.go:79` and `internal/provider/zen/client.go:75`
**Issue:** `ResponseHeaderTimeout` is set to `types.HTTPDialTimeout` (30s). For streaming SSE, this means if the model takes >30s to send the first response byte (common for complex reasoning models like DeepSeek R1 or large-context requests), the HTTP transport will abort the connection with a timeout error. The architecture docs state "NO body read timeout (streaming)" (ARCHITECTURE.md), but `ResponseHeaderTimeout` effectively acts as a first-byte timeout for the response headers.

**Fix:** Set `ResponseHeaderTimeout` to 0 (no timeout) for streaming endpoints, relying on `HTTPDialTimeout` only for connection establishment:
```go
httpClient: &http.Client{
    Transport: &http.Transport{
        DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
        ResponseHeaderTimeout: 0, // No timeout — first token may take minutes for reasoning models
    },
},
```

### WR-08: SSE Parser Created With context.Background() — Unused Defense-in-Depth

**File:** `internal/provider/openrouter/client.go:212` and `internal/provider/zen/client.go:201`
**Issue:** Both clients call `provider.NewSSEParser(resp)` which wraps `context.Background()` instead of using `provider.NewSSEParserWithContext(resp, ctx)`. The `NewSSEParserWithContext` function exists with a context-cancellation check between SSE lines (sse.go:42-49, the "H-7" fix), but is never called in production.

Stream cancellation DOES work through an alternative path: `streaming.go:91-98` spawns a goroutine that calls `iterator.Close()` on `ctx.Done()`, which closes `resp.Body`, which unblocks `bufio.Scanner.Scan()`. However, the H-7 context check between lines was specifically added as defense-in-depth for faster cancellation when the server is slow to send the next SSE event. Without it, cancellation relies entirely on the body-close propagation chain.

**Fix:**
```go
// openrouter/client.go — ChatCompletionStream, after resp.StatusCode check
sse := provider.NewSSEParserWithContext(resp, ctx)

// zen/client.go — same change
sse := provider.NewSSEParserWithContext(resp, ctx)
```

## Info

### IN-01: provider Variable Shadows Package Import in cycleRecentModel

**File:** `internal/tui/app.go:684-688`
**Issue:** `provider, err := m.registry.Get(m.activeProvider)` creates a local variable named `provider` which shadows the `provider` package import. On line 688, `provider.GetModel(data.Recent[targetIdx])` calls the `LLMProvider.GetModel()` method (correct behavior) but reads like it might be calling the package-level `provider.GetModel(id, cache)` function from common.go (which takes 2 args). This is not a bug — Go resolves it to the method call — but it's a readability hazard.

**Fix:** Rename the local variable:
```go
p, err := m.registry.Get(m.activeProvider)
if err != nil {
    return nil
}
model, err := p.GetModel(data.Recent[targetIdx])
```

### IN-02: FindFallbackProvider Has Undocumented Side Effect on Registry State

**File:** `internal/provider/fallback.go:22-31`
**Issue:** `FindFallbackProvider` calls `registry.TrySetActive(name)` inside its search loop, which atomically sets the found provider as the registry's active provider. The function name implies a read-only query, but it mutates registry state. The caller in `app_update.go:547` relies on this side effect (it doesn't call `SetActive` separately), making the coupling implicit.

**Fix:** Document the side effect clearly in the godoc:
```go
// FindFallbackProvider searches for an alternative provider that is healthy
// and passes a health check. As a side effect, it atomically sets the found
// provider as the registry's active provider via TrySetActive to prevent
// TOCTOU races between finding and switching.
func FindFallbackProvider(registry *Registry, currentProvider string) (string, *FallbackEvent, error) {
```

### IN-03: handleFallbackEvent Doesn't Explicitly Call registry.SetActive

**File:** `internal/tui/app_update_workflow.go:519-527`
**Issue:** `handleFallbackEvent` sets `m.activeProvider = msg.To` but doesn't call `m.registry.SetActive(msg.To)`. This works because `FindFallbackProvider` already called `TrySetActive` (which updated the registry). However, the asymmetry with other code paths that do call `SetActive` (app_update_workflow.go:465, 540) makes the code harder to follow.

**Fix:** Add an explicit `SetActive` call for clarity and defensive coding:
```go
func (m *AppState) handleFallbackEvent(msg FallbackEventMsg) (tea.Model, tea.Cmd) {
    _ = m.registry.SetActive(msg.To) // Already set by FindFallbackProvider; explicit for safety
    m.activeProvider = msg.To
    // ...
}
```

### IN-04: Interface Compliance Verified at Compile Time — Good Practice

**File:** `internal/provider/openrouter/client.go:22` and `internal/provider/zen/client.go:23`
**Issue:** Both clients have `var _ provider.LLMProvider = (*Client)(nil)` compile-time checks. The test mock at `resilience_test.go:21` also verifies compliance. No action needed — noting for completeness.

---

_Reviewed: 2026-06-06T12:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
