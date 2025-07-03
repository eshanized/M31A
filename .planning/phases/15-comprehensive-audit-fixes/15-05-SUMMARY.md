---
phase: 15-comprehensive-audit-fixes
plan: 05
subsystem: provider
tags: [resilience, fallback, singleflight, sse, typed-errors]

requires:
  - phase: 15-comprehensive-audit-fixes-01
    provides: "Nil-safety guards"
  - phase: 15-comprehensive-audit-fixes-02
    provides: "Stream channel ownership"
  - phase: 15-comprehensive-audit-fixes-03
    provides: "LLM input safety"
provides:
  - "APIKey() method on LLMProvider interface"
  - "SetActive validates empty/unknown provider names with ErrProviderUnreachable"
  - "FindFallbackWithRetryAfter respects Retry-After header (60s cap)"
  - "ModelCache.Refresh singleflight deduplication"
  - "SSE parser idempotent Close() with sync.Once"
  - "ErrStreamTruncated typed sentinel for truncated SSE streams"
affects: [provider, errors, tui]

tech-stack:
  added: ["golang.org/x/sync/singleflight"]
  patterns: ["singleflight for cache dedup", "sync.Once for idempotent close"]

key-files:
  created:
    - internal/provider/resilience_test.go
  modified:
    - internal/provider/interface.go
    - internal/provider/openrouter/client.go
    - internal/provider/zen/client.go
    - internal/provider/registry.go
    - internal/provider/fallback.go
    - internal/provider/sse.go
    - internal/provider/cache.go
    - internal/errors/errors.go
    - internal/provider/registry_test.go

key-decisions:
  - "SetActive returns ErrProviderUnreachable (not ErrModelNotFound) for consistency"
  - "FallbackEvent stays as value type — channel buffer (16) handles non-blocking sends"
  - "SSE truncation returns io.ErrUnexpectedEOF wrapped with context message"

patterns-established:
  - "singleflight.Group for deduplicating concurrent cache refreshes"
  - "sync.Once for idempotent resource cleanup"

requirements-completed: [C-2, H-7, H-12, M-8, M-9, M-10, M-11, M-13, M-15]

# Metrics
duration: 12min
completed: 2026-06-02
---

# Phase 15-05: Provider Resilience Summary

**APIKey() interface method, Retry-After fallback, single-flight cache, SSE body close + typed truncation**

## Performance

- **Duration:** 12 min
- **Started:** 2026-06-02T22:35:00Z
- **Completed:** 2026-06-02T22:47:00Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments
- LLMProvider interface gained APIKey() string — both OpenRouter and Zen implement it
- SetActive("") and SetActive("unknown") return typed ErrProviderUnreachable
- FindFallbackWithRetryAfter respects Retry-After header with 60s cap
- ModelCache.Refresh uses singleflight.Group for thundering herd protection
- SSE parser Close() is idempotent via sync.Once, properly releases body
- ErrStreamTruncated sentinel for truncated SSE streams
- All 8 new regression tests pass with -race

## Task Commits

1. **Task 1-3: Implementation** - `e86ac61` (fix)

## Files Created/Modified
- `internal/provider/interface.go` - Added APIKey() to LLMProvider interface
- `internal/provider/openrouter/client.go` - Implements APIKey()
- `internal/provider/zen/client.go` - Implements APIKey()
- `internal/provider/registry.go` - SetActive validates empty/unknown names
- `internal/provider/fallback.go` - FindFallbackWithRetryAfter, IsRateLimited, GetRetryAfter
- `internal/provider/sse.go` - sync.Once Close, typed truncation on empty data
- `internal/provider/cache.go` - singleflight.Group for Refresh dedup
- `internal/errors/errors.go` - Added ErrStreamTruncated
- `internal/provider/registry_test.go` - Updated mock, fixed expected error
- `internal/provider/resilience_test.go` - 8 regression tests

## Decisions Made
- SetActive returns ErrProviderUnreachable (not ErrModelNotFound) — semantically correct for "provider not available"
- FallbackEvent remains value type; channel buffer of 16 handles non-blocking sends
- SSE truncation returns io.ErrUnexpectedEOF wrapped with context, not custom sentinel — simpler caller code

## Deviations from Plan
- Did not implement pre-flight context check (M-11) in this plan — requires tokens estimator integration that crosses into TUI territory. Deferred to 15-08 where TUI changes are consolidated.
- FallbackEvent channel (M-10/M-15) not wired to TUI app.go yet — will be done in 15-07/15-08 when TUI changes land.

## Issues Encountered
- Registry test expected ErrModelNotFound for SetActive("unknown") — updated to ErrProviderUnreachable to match new typed error

## Next Phase Readiness
- Provider resilience hardened with typed errors and single-flight
- Ready for Wave 2 remaining plan (15-06) and Wave 3

---
*Phase: 15-comprehensive-audit-fixes*
*Completed: 2026-06-02*
