---
phase: 25
plan: 25-02
subsystem: provider
tags: [singleflight, health-check, sentinel-errors, sse, context, registry, credits]

# Dependency graph
requires:
  - phase: 25
    provides: Provider audit findings from comprehensive wiring analysis
provides:
  - Cache singleflight deduplication via cache.Refresh
  - ErrInvalidProvider sentinel for empty provider names
  - SSE context propagation from request to parser
  - ErrNoCredits parity between OpenRouter and Zen for HTTP 402
  - Sorted provider registry list
- Fixed maxRetryAfter comment accuracy
- Removed unused HealthCheckTicker parameters
- IsContextExceeded operator precedence regression test
affects: [provider, tui]

# Tech tracking
tech-stack:
  added: [golang.org/x/sync/singleflight]
  patterns: [singleflight-coalescing, sentinel-error-separation, context-threading]

key-files:
  created:
    - internal/provider/common_test.go
  modified:
    - internal/provider/cache.go (singleflight field already present, clients now use Refresh)
    - internal/provider/openrouter/client.go (cache.Refresh + HTTP 402 + SSE ctx)
    - internal/provider/zen/client.go (cache.Refresh + SSE ctx)
    - internal/provider/fallback.go (comment fix)
    - internal/provider/registry.go (ErrInvalidProvider)
    - internal/provider/sse_test.go (context cancel test)
    - internal/provider/resilience_test.go (ErrInvalidProvider)
    - internal/errors/errors.go (ErrInvalidProvider sentinel)
    - internal/tui/health.go (removed unused params)
    - internal/tui/app.go (updated caller)
    - internal/tui/app_update.go (updated caller)
    - internal/tui/health_test.go (updated callers)

key-decisions:
  - "W-02: Clients now use cache.Refresh(ctx, fetchFn) instead of bypassing singleflight via cache.Set()"
  - "W-04: New ErrInvalidProvider sentinel distinguishes empty name from unknown provider"
  - "W-08: Changed callers to NewSSEParserWithContext instead of adding ctx to NewSSEParser (preserves backward compat)"
  - "W-10: Already fixed — sort.Strings was present in registry.go List() from prior phase"
  - "W-06: Test cases adjusted to match actual IsContextExceeded signature (HTTP 400 guard, not rate-limit related)"

patterns-established:
  - "Cache refresh pattern: clients call cache.Refresh(ctx, fetchFn) for automatic singleflight deduplication"
  - "SSE context threading: always pass request context to NewSSEParserWithContext for proper cancellation"

requirements-completed: [WIRE-04]

# Metrics
duration: 10min
completed: 2026-06-06
---

# Phase 25 Plan 02: Provider Hardening Fixes Summary

**Singleflight cache deduplication, ErrInvalidProvider sentinel, SSE context propagation, HTTP 402 ErrNoCredits parity, and 4 additional provider-layer fixes**

## Performance

- **Duration:** 10 min
- **Started:** 2026-06-06T02:06:58Z
- **Completed:** 2026-06-06T02:17:14Z
- **Tasks:** 8 (7 code changes + 1 already-fixed verification)
- **Files modified:** 14

## Accomplishments
- Cache singleflight deduplication activated: both OpenRouter and Zen clients now use `cache.Refresh(ctx, fetchFn)` instead of bypassing with direct `cache.Set()`
- New `ErrInvalidProvider` sentinel distinguishes "empty provider name" from "provider not found"
- SSE context propagation: both provider clients pass request context to parser for proper cancellation
- HTTP 402 now returns `ErrNoCredits` on OpenRouter (parity with Zen)
- Unused `HealthCheckTicker` parameters removed, `maxRetryAfter` comment corrected
- 3 new regression tests added (IsContextExceeded precedence, SSE context cancel, HTTP 402 NoCredits)

## Task Commits

Each task was committed atomically:

1. **Task 1: W-02 Cache Singleflight Deduplication** - `e7ddaa3` (fix)
2. **Task 2: W-03 HealthCheckTicker Unused Parameters** - `69be57d` (fix)
3. **Task 2 fix: Update remaining test caller** - `8c61f25` (fix)
4. **Task 3: W-04 ErrInvalidProvider Sentinel** - `376b8fa` (fix)
5. **Task 4: W-05 maxRetryAfter Comment** - `0e14fc2` (fix)
6. **Task 5: W-06 IsContextExceeded Test** - `d493249` (test)
7. **Task 6: W-08 SSE Context Propagation** - `1f393f5` (fix)
8. **Task 8: W-11 HTTP 402 ErrNoCredits** - `74e28a6` (fix)

**Plan metadata:** pending (docs commit)

## Files Created/Modified
- `internal/provider/common_test.go` - New: IsContextExceeded operator precedence test (10 cases)
- `internal/provider/openrouter/client.go` - cache.Refresh, HTTP 402 ErrNoCredits, SSE ctx
- `internal/provider/zen/client.go` - cache.Refresh, SSE ctx
- `internal/provider/fallback.go` - Comment fix (60s → 120s)
- `internal/provider/registry.go` - ErrInvalidProvider for empty name
- `internal/provider/sse_test.go` - SSE context cancel test
- `internal/provider/resilience_test.go` - Updated for ErrInvalidProvider
- `internal/errors/errors.go` - ErrInvalidProvider sentinel + UserMessage
- `internal/tui/health.go` - Removed unused registry/activeProvider params
- `internal/tui/app.go` - Updated HealthCheckTicker caller
- `internal/tui/app_update.go` - Updated HealthCheckTicker caller
- `internal/tui/health_test.go` - Updated test callers

## Decisions Made
- W-02: Changed callers to use `cache.Refresh(ctx, fetchFn)` instead of adding singleflight to existing `Set()` path — cleaner separation of concerns
- W-04: New `ErrInvalidProvider` sentinel — more precise error than reusing `ErrProviderNotFound`
- W-08: Used existing `NewSSEParserWithContext` instead of modifying `NewSSEParser` signature — preserves backward compatibility
- W-10: Already fixed — `sort.Strings(names)` present in registry.go from prior phase
- W-06: Test cases adjusted to match actual `IsContextExceeded` signature (HTTP 400 guard, rate-limit cases removed)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] TestHealthCheckTicker_ZeroInterval missed parameter update**
- **Found during:** Task 2 verification (post-commit)
- **Issue:** One test caller was missed during the initial parameter removal
- **Fix:** Updated `TestHealthCheckTicker_ZeroInterval` to use new 2-param signature
- **Files modified:** internal/tui/health_test.go
- **Verification:** `go test -race ./internal/tui/...` passes
- **Committed in:** 8c61f25

---

**Total deviations:** 1 auto-fixed (1 missed test caller)
**Impact on plan:** Minimal — one additional test fix commit for completeness.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
All 8 provider warnings fixed. `go test -race ./internal/provider/... ./internal/tui/...` passes clean. Ready for next plan in Phase 25.

---
*Phase: 25*
*Completed: 2026-06-06*

## Self-Check: PASSED
