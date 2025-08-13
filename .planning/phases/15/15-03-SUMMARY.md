---
phase: 15-comprehensive-audit-fixes
plan: 03
subsystem: workflow-engine
tags: [parse-tool-calls, oom-prevention, json-parsing, sse-usage, cost-display]

# Dependency graph
requires:
  - phase: 06-workflow-engine
    provides: parseToolCalls, extractJSONObject, streaming message infrastructure
provides:
  - "ErrToolInputTooLarge sentinel for oversized LLM responses"
  - "MaxLLMResponseBytes constant (1 MB input cap)"
  - "Capped parseToolCalls (1 MB input + 16 tool limit)"
  - "Comment-stripping extractJSONObject preserving string literals"
  - "SSE usage propagation to StreamDoneMsg for cost display"
affects: [workflow-engine, provider-layer, tui-streaming]

# Tech tracking
tech-stack:
  added: []
  patterns: [bounded-input-validation, sse-usage-propagation, comment-stripping-state-machine]

key-files:
  created: []
  modified:
    - internal/errors/errors.go
    - internal/types/constants.go
    - internal/types/types.go
    - internal/workflow/engine.go
    - internal/workflow/engine_test.go
    - internal/workflow/execute.go
    - internal/provider/reasoning.go
    - internal/provider/reasoning_test.go
    - internal/tui/streaming.go

key-decisions:
  - "Used bounded JSON scan (64 KB per-object) instead of regex to avoid ReDoS class issues"
  - "Comment stripper uses inString state machine to preserve string literals"
  - "StreamChunk.Usage field added to propagate SSE usage without changing StreamIterator interface"
  - "Tool count cap at 16 with log warning for model regression signal"

patterns-established:
  - "Bounded input validation: Always cap input size and output count for untrusted LLM responses"
  - "SSE usage bridge: Parse usage from final SSE chunk, carry through StreamDoneMsg for cost display"

requirements-completed: [C-4, H-5, M-22]

# Metrics
duration: 15min
completed: 2026-06-02
---

# Phase 15 Plan 03: LLM Input Safety & Output Parsing Hardening Summary

**parseToolCalls OOM guard, comment-stripping JSON parser, and SSE usage propagation for accurate cost display**

## Performance

- **Duration:** 15 min
- **Started:** 2026-06-02T22:07:00+05:30
- **Completed:** 2026-06-02T22:22:00+05:30
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments
- Prevented OOM from oversized LLM responses via 1 MB input cap and 16 tool count limit (C-4)
- Enabled JSON comment stripping in extractJSONObject while preserving string literals (H-5)
- Fixed $0.00 cost display by propagating SSE usage to StreamDoneMsg (M-22)
- All 3 audit findings resolved with regression tests

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ErrToolInputTooLarge sentinel; cap parseToolCalls size + tool count** - `6cd3f02` (fix)
2. **Task 2: Strip JSON comments in extractJSONObject and populate lastUsage from SSE** - `01434e5` (fix)
3. **Task 3: Add regression tests for C-4, H-5, and M-22** - `6cd3f02` / `01434e5` (tests included in task commits)

**Plan metadata:** pending (docs: complete plan)

## Files Created/Modified
- `internal/errors/errors.go` - Added ErrToolInputTooLarge sentinel
- `internal/types/constants.go` - Added MaxLLMResponseBytes (1 MB) constant
- `internal/types/types.go` - Added Usage field to StreamChunk
- `internal/workflow/engine.go` - Capped parseToolCalls (1 MB input, 16 tools, bounded JSON scan); added comment stripping in extractJSONObject
- `internal/workflow/engine_test.go` - Updated tests for new (calls, error) return signature
- `internal/workflow/execute.go` - Updated caller for new parseToolCalls signature
- `internal/provider/reasoning.go` - ParseSSEChunk extracts usage from final SSE chunk
- `internal/provider/reasoning_test.go` - Fixed unused import (deviation from original plan scope)
- `internal/tui/streaming.go` - StreamDoneMsg.Usage populated from SSE usage instead of empty struct

## Decisions Made
- Bounded JSON scan (64 KB per-object) chosen over regex to avoid ReDoS class issues
- Comment stripper uses inString state machine to avoid stripping inside JSON string literals
- StreamChunk.Usage field added to propagate SSE usage without changing the StreamIterator interface
- Tool count cap at 16 with log warning serves as model regression signal

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed unused import in reasoning_test.go**
- **Found during:** Verification after Task 2
- **Issue:** `github.com/eshanized/M31A/internal/types` was imported but not used in `reasoning_test.go`, causing `go build` and `go vet` failures
- **Fix:** Removed the unused import
- **Files modified:** internal/provider/reasoning_test.go
- **Verification:** `go test -count=1 -race ./internal/provider/...` passes
- **Committed in:** (uncommitted — part of verification step)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minimal — unused import cleanup. No scope creep.

## Issues Encountered
- Unused import in `reasoning_test.go` caused build failure after Task 2 commit — fixed during verification

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 3 audit findings (C-4, H-5, M-22) resolved with regression tests
- Tests pass with `-race` detector on all affected packages
- Ready for subsequent 15-xx plans to build on hardened foundation

---
*Phase: 15-comprehensive-audit-fixes*
*Completed: 2026-06-02*
