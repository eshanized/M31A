---
phase: 07-signature-features
plan: 03
subsystem: arbitrage
tags: model-cost, complexity-scoring, token-estimation, recommendation

# Dependency graph
requires:
  - phase: 01-provider-layer
    provides: types.ModelInfo, types.Pricing, types.Task
provides:
  - pkg/arbitrage/ engine for complexity scoring, cost comparison, model recommendation
affects: model-selector, plan-screen, /optimize command

# Tech tracking
tech-stack:
  added:
    - "pkg/arbitrage/ — pure computation package, no external deps beyond stdlib + internal/types"
  patterns:
    - "Keyword-based complexity classification with file/dependency boosting"
    - "Deterministic token estimation using midpoint ranges with per-file adjustment"
    - "CompareModels skips zero-pricing models, sorts ascending by TotalCost"
    - "Recommendation engine with context-window filtering and threshold-aware selection"

key-files:
  created:
    - pkg/arbitrage/arbitrage.go — Scorer, CostEstimate, ArbitrageRecommendation, all exports
    - pkg/arbitrage/arbitrage_test.go — 15 test functions covering all exported functions
  modified: []

key-decisions:
  - "Keyword-based complexity classification (not token-threshold-based) with simple/moderate/complex keyword lists from D-02 CONTEXT.md"
  - "Midpoint deterministic token estimation (not random range picks) for reproducible results"
  - "Complex tasks require >64K context window per D-02"
  - "Threshold applied only when cheapest model lacks capability (complex tasks with small context)"
  - "Per-file adjustment of 500 tokens to both input and output"

patterns-established:
  - "New pkg/ packages import internal/types and use stdlib only — no new go.mod dependencies"
  - "Tests use helper functions (newTestTask, newTestModel) for DRY fixture construction"
  - "Sentinel errors (internal/errors/) can be imported when needed for shared error patterns"

requirements-completed: [AC-22]

# Metrics
duration: 8min
completed: 2026-05-28
---

# Phase 7: Signature Features — Plan 03 Summary

**Cost-aware model arbitrage engine — complexity scoring, token estimation, model cost comparison, and recommendation with context-window-aware threshold selection**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-28T06:02:52Z
- **Completed:** 2026-05-28T06:10:53Z
- **Tasks:** 2 (1 implementation, 1 tests)
- **Files modified:** 2

## Accomplishments

- ComplexityLevel type with Simple/Moderate/Complex classification via keyword matching
- Scorer with Score() method using keyword analysis + file/dependency count boosting
- EstimateTokens() with per-file adjustment — Simple: 2000/1000, Moderate: 5500/2750, Complex: 14000/7000 midpoint values
- CompareModels() ascending sort by TotalCost, gracefully skipping zero-pricing models
- Recommend() with context-window filtering: complex tasks (>64K) get capability-aware selection within threshold
- ShouldArbitrage() threshold comparison with zero-cost and equal-cost guards
- 15 test functions covering scoring, estimation, comparison, recommendation, and edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement Scorer, complexity analysis, and token estimation** - `6760ca6` (feat)
2. **Task 2: Write tests for all arbitrage functions** - `883bfb6` (test)

**Plan metadata:** Pending (following SUMMARY commit)

## Files Created/Modified

- `pkg/arbitrage/arbitrage.go` - All types (ComplexityLevel, CostEstimate, ArbitrageRecommendation, Scorer) and functions (NewScorer, Score, EstimateTokens, CompareModels, Recommend, ShouldArbitrage) with internal helpers (boostLevel, classifyText)
- `pkg/arbitrage/arbitrage_test.go` - 15 test functions with helper fixtures (newTestTask, newTestModel)

## Decisions Made

- **Keyword-based scoring**: Following 07-CONTEXT.md D-02, complexity is classified by keyword matching (complex keywords checked first, then moderate, then simple, defaulting to Simple) rather than token-threshold-based scoring. The Scorer's Simple/ComplexThreshold fields exist for future alternative scoring but are not used by Score().
- **Deterministic midpoint estimation**: EstimateTokens returns the range midpoint (Simple→2000/1000, Moderate→5500/2750, Complex→14000/7000) rather than random values within ranges, ensuring reproducible results across calls.
- **Threshold applied to capability gaps only**: For simple/moderate tasks, the cheapest model is always recommended. For complex tasks, if the cheapest has insufficient context (≤64K), we search for the cheapest capable model within price threshold of the cheapest.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all tests pass, `go build` and `go vet` clean on first run.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `pkg/arbitrage/` is ready for consumption by the model selector UI (P7.1), plan screen (`/optimize`), and cost display
- Package exports 6 public functions with clean signatures — integration requires no structural changes
- Ready for Plan 04 (AutoDream), Plan 05 (Ledger), Plan 06 (Gap Fixes) in Wave 2

---

*Phase: 07-signature-features*
*Completed: 2026-05-28*
