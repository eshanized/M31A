---
phase: 04-technical-debt
plan: 04
subsystem: config, tools, tokens, provider
tags: [reflection, edit, tokenization, capabilities, confidence]

# Dependency graph
requires:
  - phase: 04-technical-debt
    provides: "Previous refactoring tasks (04-01 to 04-03)"
provides:
  - "Type-safe config merge without reflection"
  - "Edit tool with 5 strategies and confidence scoring"
  - "Multi-provider token estimation for 8 provider families"
  - "Runtime capability detection with health checks"
affects: [config, tools, tokens, provider]

# Tech tracking
tech-stack:
  added: []
  patterns: [type-safe-merge, confidence-scoring, provider-detection, capability-cache]

key-files:
  created: [internal/config/merge.go, internal/config/merge_test.go]
  modified: [internal/config/loader.go, internal/tools/edit.go, internal/tools/edit_test.go, internal/tools/edit_benchmark_test.go, internal/tools/extra_test.go, internal/tools/edit_integration_test.go, internal/tokens/estimator.go, internal/tokens/estimator_test.go, internal/provider/capabilities.go, internal/provider/capabilities_test.go]

key-decisions:
  - "Type-safe merge via helper struct preserves TOML-defined bool semantics"
  - "Go methods cannot have type parameters; generic functions are standalone"
  - "5 edit strategies instead of 7: merged line-trimmed + whitespace-normalized into trimmed"
  - "8 provider families: OpenAI, Anthropic, Google, Meta, Mistral, Qwen, DeepSeek, Cohere"
  - "Capability detection uses cache with hardcoded fallback table"

patterns-established:
  - "Type-safe merge: Use helper struct with defined map for TOML bool semantics"
  - "Confidence scoring: Each strategy returns float64 confidence (0.7-1.0)"
  - "Provider detection: DetectProviderFamily routes to provider-specific heuristics"

requirements-completed: [TECH-03, TECH-04, TECH-05, TECH-06]

# Metrics
duration: 45min
completed: 2026-07-02
---

# Phase 04 Plan 04: Config Merge Refactor & Edit Tool Simplification Summary

**Type-safe config merge without reflection, edit tool with 5 strategies and confidence scoring, multi-provider token estimation for 8 provider families, and runtime capability detection with caching**

## Performance

- **Duration:** 45 min
- **Started:** 2026-07-02T07:30:00Z
- **Completed:** 2026-07-02T08:15:00Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishes

- Config merge now uses type-safe helper struct with zero reflection
- Edit tool reduced from 7 to 5 strategies with confidence scoring (0.7-1.0)
- Token estimator supports 8 provider families with provider-specific heuristics
- Capability detection with cache and hardcoded fallback table for known models

## Task Commits

Each task was committed atomically:

1. **Task 1: Remove Reflection-Based Config Merge** - `26ef125` (refactor)
2. **Task 2: Simplify Edit Tool and Add Confidence Scoring** - `b7ce6e4` (refactor)
3. **Task 3: Replace Token Estimator and Add Runtime Capability Detection** - `fec5888f` (feat)

## Files Created/Modified

- `internal/config/merge.go` - Type-safe config merge with MergeConfig function
- `internal/config/merge_test.go` - Tests for type-safe merge
- `internal/config/loader.go` - Removed reflect import, delegates to MergeConfig
- `internal/tools/edit.go` - 5 strategies with confidence scoring
- `internal/tools/edit_test.go` - Updated tests for new strategy names
- `internal/tools/edit_benchmark_test.go` - Updated benchmarks for 5 strategies
- `internal/tools/extra_test.go` - Updated strategy names in tests
- `internal/tools/edit_integration_test.go` - Updated LineSkip test
- `internal/tokens/estimator.go` - Multi-provider token estimation
- `internal/tokens/estimator_test.go` - Tests for provider detection
- `internal/provider/capabilities.go` - ModelCapabilities and DetectCapabilities
- `internal/provider/capabilities_test.go` - Tests for capability detection

## Decisions Made

- Type-safe merge via helper struct preserves TOML-defined bool semantics
- Go methods cannot have type parameters; generic functions are standalone
- 5 edit strategies instead of 7: merged line-trimmed + whitespace-normalized into trimmed
- 8 provider families: OpenAI, Anthropic, Google, Meta, Mistral, Qwen, DeepSeek, Cohere
- Capability detection uses cache with hardcoded fallback table

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test strategy names after strategy consolidation**
- **Found during:** Task 2 (Edit tool simplification)
- **Issue:** Tests referenced old strategy names (line-trimmed, whitespace-normalized, fuzzy-anchor) after strategies were merged/renamed
- **Fix:** Updated test assertions to use new strategy names (trimmed, normalized, anchor)
- **Files modified:** internal/tools/extra_test.go, internal/tools/edit_integration_test.go
- **Verification:** All tests pass with new strategy names
- **Committed in:** b7ce6e4 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed sync.Map caching to store pointers**
- **Found during:** Task 3 (Capability detection)
- **Issue:** sync.Map returns copies of values, not pointers; caching test failed
- **Fix:** Store *ModelCapabilities pointers in cache instead of values
- **Files modified:** internal/provider/capabilities.go
- **Verification:** Caching test now passes
- **Committed in:** fec5888f (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered

None - plan executed as written with minor auto-fixes.

## Next Phase Readiness

- All 4 tasks completed and committed
- Config merge, edit tool, token estimator, and capability detection all improved
- Tests pass, `make check` green
- Ready for next phase or verification

---
*Phase: 04-technical-debt*
*Completed: 2026-07-02*
