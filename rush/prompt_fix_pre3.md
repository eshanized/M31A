# ROLE

You are the **Senior Go Engineer** performing pre-Phase 3 maintenance on M31A.
Phases 0, 1, and 2 are complete but an audit identified 6 items that must be fixed
before Phase 3 (Message Rendering Pipeline) can begin safely.

Your job is to fix EXACTLY these 6 items. Do NOT implement Phase 3 features.
Do NOT refactor unrelated code. Do NOT add new features. Fix the 6 items, run
verification, and stop.

---

# CONTEXT

An audit report (`rush/audit_0_2.md`) identified the following issues:

## Blockers (MUST fix)

1. **CRIT-01**: `go.mod` says `go 1.24.2`, spec requires `go 1.22`
2. **MED-02**: `EstimateCost(usage types.Usage) float64` always returns 0 — needs `modelID string` parameter

## Recommended (SHOULD fix before Phase 3)

3. **MED-01**: Provider badges show `[OP]`/`[ZE]` instead of spec-mandated `[OR]`/`[ZEN]`
4. **MED-03**: `ModelCache.Get()` returns `nil, false` on TTL expiry even when stale data is available
5. **MED-04**: `FetchModels` always hits API first — should check cache before HTTP call
6. **MED-06/MED-07**: Walkthrough_2.md has inaccurate claims (screen enum, terminal guard threshold, backoff)

---

# FIXES TO IMPLEMENT

## Fix 1: Restore Go 1.22 in go.mod

**File**: `go.mod`

Change:
```
go 1.24.2
```
To:
```
go 1.22
```

Then run: `go mod tidy -go=1.22`

Verify: `go build ./...` still passes. If it fails, the code uses Go 1.23+ features
and must be adjusted. Currently no such features are known to exist.

---

## Fix 2: Add `modelID` to `EstimateCost` Interface

**Files to modify:**

### `internal/provider/interface.go`

Change the interface method signature:
```go
// Before
EstimateCost(usage types.Usage) float64

// After
EstimateCost(modelID string, usage types.Usage) float64
```

### `internal/provider/openrouter/client.go`

Update the method signature and implementation:
```go
// Before
func (c *Client) EstimateCost(usage types.Usage) float64 {
    // Returns 0 — no model ID to look up pricing
}

// After
func (c *Client) EstimateCost(modelID string, usage types.Usage) float64 {
    model, ok := c.cache.Get(modelID)
    if !ok {
        return 0
    }
    return (float64(usage.PromptTokens) / 1_000_000) * model.Pricing.InputPerMToken +
        (float64(usage.CompletionTokens) / 1_000_000) * model.Pricing.OutputPerMToken
}
```

### `internal/provider/zen/client.go`

Same update — add `modelID string` parameter and implement pricing lookup from cache.

### Test files to update:

- `internal/provider/openrouter/client_test.go` — update `TestEstimateCost` to pass modelID
- `internal/provider/zen/client_test.go` — update `TestEstimateCost` to pass modelID

The tests should verify:
- Returns 0 for unknown modelID
- Returns correct cost for known model with pricing data

---

## Fix 3: Provider Badge Format — `[OR]`/`[ZEN]`

**File**: `internal/tui/header.go`

Replace the `provider[:2]` uppercase logic with an explicit mapping:

```go
// Before (line ~23):
prefix := strings.ToUpper(provider[:2])  // "openrouter" → "OP", "zen" → "ZE"

// After:
var badge string
switch provider {
case "openrouter":
    badge = "OR"
case "zen":
    badge = "ZEN"
default:
    badge = strings.ToUpper(provider[:min(len(provider), 3)])
}
```

**File**: `internal/tui/header_test.go`

Update all tests that check for `[OP]` or `[ZE]` badges to expect `[OR]` and `[ZEN]` respectively.
Specifically:
- `TestRenderHeader_BrandAndProviderBadge` — expect `[OR]` for openrouter
- Any test that checks provider badge format for zen — expect `[ZEN]`

---

## Fix 4: ModelCache.Get() Stale Fallback

**File**: `internal/provider/cache.go`

Modify `Get()` to serve stale data when TTL has expired but stale TTL has not:

```go
// Before:
func (c *ModelCache) Get(id string) (*types.ModelInfo, bool) {
    c.mu.RLock()
    defer c.mu.RUnlock()
    if c.IsExpired() {
        return nil, false  // Stale data available but not served
    }
    model, ok := c.models[id]
    return model, ok
}

// After:
func (c *ModelCache) Get(id string) (*types.ModelInfo, bool) {
    c.mu.RLock()
    defer c.mu.RUnlock()
    model, ok := c.models[id]
    if !ok {
        return nil, false
    }
    if c.IsExpired() && !c.IsStale() {
        // TTL expired but data is still within stale window — serve it
        return model, true
    }
    if c.IsExpired() && c.IsStale() {
        // Data is beyond stale window — do not serve
        return nil, false
    }
    return model, true
}
```

The logic is:
- Cache not expired → serve normally
- Cache expired but not stale (< 24h old) → serve stale data (return true)
- Cache stale (> 24h old) → do not serve (return false)

**Add a test** in `internal/provider/cache_test.go` (create if it doesn't exist):

```go
func TestModelCache_Get_StaleFallback(t *testing.T)
```

This test should:
1. Populate cache with a model
2. Manually set `c.fetched` to a time older than TTL but less than 24h ago
3. Call `Get()` — should return the model (stale read)
4. Set `c.fetched` to > 24h ago
5. Call `Get()` — should return nil, false

---

## Fix 5: FetchModels Cache-First

**File**: `internal/provider/openrouter/client.go`

Add a cache check at the top of `FetchModels`:

```go
func (c *Client) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
    // Check cache first
    if !c.cache.IsExpired() && c.cache.Len() > 0 {
        return c.cachedModels(), nil
    }

    // Cache expired or empty — fetch from API
    // ... existing HTTP logic ...
}
```

Add a helper method:
```go
func (c *Client) cachedModels() []types.ModelInfo {
    c.cache.mu.RLock()
    defer c.cache.mu.RUnlock()
    models := make([]types.ModelInfo, 0, len(c.cache.models))
    for _, m := range c.cache.models {
        models = append(models, *m)
    }
    return models
}
```

**File**: `internal/provider/zen/client.go`

Same change — add cache check at top of `FetchModels`, add `cachedModels()` helper.

**Test updates:**

Add tests to verify cache-first behavior:
- `TestFetchModels_CacheHit` — populate cache, call FetchModels, verify no HTTP call
- `TestFetchModels_CacheExpired` — populate cache, set fetched time to > TTL ago, call FetchModels, verify HTTP call made

---

## Fix 6: Update walkthrough_2.md

**File**: `walkthrough_2.md`

Correct these inaccurate claims:

1. **Line ~31**: Change "exponential backoff with jitter (60s base, 5 min max, uniform)" to "two-state interval: 60s normal, 120s on rate-limit/offline"

2. **Line ~38**: Change "terminal-too-small guard (80×24 threshold)" to "terminal-too-small guard (40×10 threshold)"

3. **Line ~6**: Change screen enum from "ScreenREPL, ScreenFirstRun, ScreenHelp, ScreenExit" to "ScreenFirstRun, ScreenREPL, ScreenModelSelector, ScreenSettings, ScreenResume, ScreenPermission"

4. **Line ~27**: Remove or correct the "pressing `ctrl+w` at Welcome jumps to Complete" claim — no ctrl+w handler exists.

---

# EXECUTION ORDER

1. Fix 1: Update `go.mod` to `go 1.22`, run `go mod tidy -go=1.22`
2. Fix 2: Update `EstimateCost` interface and both client implementations + tests
3. Fix 3: Update provider badge logic in `header.go` + tests
4. Fix 4: Update `ModelCache.Get()` stale fallback + add test
5. Fix 5: Update `FetchModels` cache-first in both clients + add tests
6. Fix 6: Update `walkthrough_2.md` with accurate claims
7. Run: `go build ./...` — MUST succeed
8. Run: `go vet ./...` — MUST pass
9. Run: `go test -race -count=1 ./...` — MUST pass ALL tests (all packages)
10. Run: `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` — MUST produce static binary
11. Verify: `file m31a` shows "statically linked"
12. Verify: `go.mod` contains `go 1.22` (not 1.23, 1.24, etc.)
13. Create `rush/fix_report.md`

---

# HARD CONSTRAINTS

- Do NOT implement any Phase 3 features (streaming renderer, thinking blocks, tool cards, permission modal)
- Do NOT refactor unrelated code
- Do NOT change any file not listed in the fixes above
- `go build ./...` MUST succeed
- `go vet ./...` MUST pass with zero warnings
- `go test -race ./...` MUST pass all tests
- `go.mod` MUST contain `go 1.22`
- `EstimateCost` MUST accept `modelID string` as first parameter
- Provider badges MUST be `[OR]` and `[ZEN]`
- `walkthrough_2.md` MUST be updated with accurate claims

---

# FIX REPORT TEMPLATE

Create `rush/fix_report.md` with this structure:

```markdown
# Pre-Phase 3 Fix Report

## Fixes Applied

| # | ID | Description | Status |
|---|----|-------------|--------|
| 1 | CRIT-01 | go.mod restored to go 1.22 | ✅ |
| 2 | MED-02 | EstimateCost now accepts modelID string | ✅ |
| 3 | MED-01 | Provider badges corrected to [OR]/[ZEN] | ✅ |
| 4 | MED-03 | ModelCache.Get() serves stale data within 24h | ✅ |
| 5 | MED-04 | FetchModels checks cache before API call | ✅ |
| 6 | MED-06/07 | walkthrough_2.md claims corrected | ✅ |

## Build Verification

### go mod tidy -go=1.22
```
[paste output]
```

### go build ./...
```
[paste output]
```

### go vet ./...
```
[paste output]
```

### go test -race -count=1 ./...
```
[paste full output with test counts per package]
```

### Binary verification
```
[paste output of: file m31a]
```

### go.mod version check
```
[paste the "go" line from go.mod]
```

## Test Summary
- Total tests: [count]
- Passed: [count]
- Failed: [count]

## Remaining Audit Items
[List any audit findings that were NOT fixed and why they are deferred]

## Phase 3 Readiness
[GO / NO-GO verdict with brief justification]
```
