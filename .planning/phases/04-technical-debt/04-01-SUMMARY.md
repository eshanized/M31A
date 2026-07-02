---
phase: 04-technical-debt
plan: 01
subsystem: workflow-engine
tags: [refactor, god-object, prompt-builder, cost-tracker, context-builder]

# Dependency graph
requires: []
provides:
  - "PromptBuilder with named prompt access and system prompt composition"
  - "CostTracker with atomic cost tracking and budget management"
  - "ContextBuilder with prompt composition and dynamic context reconciliation"
affects: [04-02, 04-03, 04-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [component-extraction, delegate-pattern, atomic-uint64-bit-packing]

key-files:
  created:
    - internal/workflow/prompt_builder.go
    - internal/workflow/prompt_builder_test.go
    - internal/workflow/cost_tracker.go
    - internal/workflow/cost_tracker_test.go
    - internal/workflow/context_builder.go
    - internal/workflow/context_builder_test.go
  modified:
    - internal/workflow/engine.go
    - internal/workflow/discuss.go
    - internal/workflow/discuss_check.go
    - internal/workflow/execute.go
    - internal/workflow/plan.go
    - internal/workflow/plan_check.go
    - internal/workflow/plan_chunk.go
    - internal/workflow/research.go
    - internal/workflow/ship.go
    - internal/workflow/website_build_test.go
    - internal/workflow/engine_extra_test.go

key-decisions:
  - "PromptBuilder.Prompt() panics on invalid names (compile-time constants only)"
  - "ContextBuilder uses callback for modelForPhase to avoid circular dependency"
  - "CostTracker preserves existing atomic uint64 bit-packing pattern for lock-free access"

patterns-established:
  - "Component extraction: Engine delegates to PromptBuilder, CostTracker, ContextBuilder"
  - "Named prompt access: e.promptBuilder.Prompt(\"name\") replaces e.prompts.Field"

requirements-completed: [TECH-01]

# Metrics
duration: 10min
completed: 2026-07-02
---

# Phase 4 Plan 01: Extract Components from Engine Summary

**Extracted PromptBuilder, CostTracker, and ContextBuilder from Engine's 1604-line God Object into focused components with dedicated tests**

## Performance

- **Duration:** 10 min
- **Started:** 2026-07-02T01:34:01Z
- **Completed:** 2026-07-02T01:44:41Z
- **Tasks:** 3
- **Files modified:** 13

## Accomplishments
- PromptBuilder extracted with named prompt access (GetPrompt, Prompt, BuildSystemPrompt)
- CostTracker extracted with atomic cost tracking and budget management
- ContextBuilder extracted with prompt composition and dynamic context reconciliation
- Engine reduced by 116 lines (1604 to 1488), 3 new focused components created

## Task Commits

Each task was committed atomically:

1. **Task 1: Extract PromptBuilder from Engine** - `77d13a4` (refactor)
2. **Task 2: Extract CostTracker from Engine** - `e3a6ea7` (refactor)
3. **Task 3: Extract ContextBuilder from Engine** - `5901ec2` (refactor)

## Files Created/Modified
- `internal/workflow/prompt_builder.go` - PromptRegistry, LoadPrompts, PromptBuilder wrapper
- `internal/workflow/prompt_builder_test.go` - 11 tests for PromptBuilder
- `internal/workflow/cost_tracker.go` - Atomic cost tracking with budget management
- `internal/workflow/cost_tracker_test.go` - 12 tests including concurrent access
- `internal/workflow/context_builder.go` - Prompt composition and dynamic context
- `internal/workflow/context_builder_test.go` - 10 tests for ContextBuilder
- `internal/workflow/engine.go` - Delegates to extracted components, removed 116 lines
- `internal/workflow/discuss.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/discuss_check.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/execute.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/plan.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/plan_check.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/plan_chunk.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/research.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/ship.go` - Updated e.prompts.X to e.promptBuilder.Prompt()
- `internal/workflow/website_build_test.go` - Updated test references to promptBuilder
- `internal/workflow/engine_extra_test.go` - Updated cost tracking tests to use CostTracker

## Decisions Made
- PromptBuilder.Prompt() panics on invalid names since prompt names are compile-time constants; this avoids error handling boilerplate in 20+ call sites while keeping GetPrompt() for cases requiring error returns
- ContextBuilder uses a callback function for modelForPhase to avoid circular dependency between Engine and ContextBuilder
- CostTracker preserves the existing atomic uint64 bit-packing pattern (math.Float64bits/Float64frombits) for lock-free concurrent access

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Engine decomposed into 4 focused components (Engine, PromptBuilder, CostTracker, ContextBuilder)
- Ready for further extraction of remaining Engine responsibilities in subsequent plans
- All existing tests pass without modification

---
*Phase: 04-technical-debt*
*Completed: 2026-07-02*
