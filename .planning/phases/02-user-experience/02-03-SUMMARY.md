---
phase: 02-user-experience
plan: 03
subsystem: ui
tags: [bubbletea, collapsible, time-estimates, execute-highlight]

# Dependency graph
requires:
  - phase: 02-user-experience
    provides: "FooterInfo enrichment pattern and workflow phase integration"
provides:
  - "Collapsible wave sections in plan view with summary headers"
  - "Rough time estimates per task based on complexity heuristic"
  - "Current task highlighting during execution with brand color"
affects: [02-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [collapsible-sections, time-estimate-heuristic, current-task-highlight]

key-files:
  created:
    - internal/ui/tui/plan_model_test.go
  modified:
    - internal/ui/tui/plan_model.go
    - internal/ui/tui/execute_model.go

key-decisions:
  - "Time estimates use complexity heuristic: deps>2 or long description = ~8min, has deps = ~5min, simple = ~2min"
  - "Collapsed sections show summary: '▸ Wave N — Title (X tasks, Y min total)'"
  - "Current task highlighted with brand color bold and ▸ prefix"

patterns-established:
  - "PlanModel collapse pattern: collapsed map + toggleCollapse + renderTasks guard"
  - "Time estimate heuristic: dependency count + description length → duration"
  - "ExecuteModel current task highlight: brand color + bold + ▸ prefix"

requirements-completed: [UX-02, UX-03, UX-04]

coverage:
  - id: D1
    description: "Collapsible wave sections with summary headers showing task count and total time"
    requirement: UX-02
    verification:
      - kind: unit
        ref: "internal/ui/tui/plan_model_test.go#TestPlanCollapse"
        status: pass
      - kind: unit
        ref: "internal/ui/tui/plan_model_test.go#TestPlanCollapseAll"
        status: pass
    human_judgment: false
  - id: D2
    description: "Time estimates rendered next to each task in expanded view"
    requirement: UX-03
    verification:
      - kind: unit
        ref: "internal/ui/tui/plan_model_test.go#TestPlanTimeEstimates"
        status: pass
      - kind: unit
        ref: "internal/ui/tui/plan_model_test.go#TestComputeTaskDuration"
        status: pass
    human_judgment: false
  - id: D3
    description: "Current executing task visually highlighted with brand color in execute view"
    requirement: UX-04
    verification:
      - kind: manual
        ref: "execute view shows ▸ prefix and brand color for current task"
        status: pass
    human_judgment: true
    rationale: "Visual styling requires manual verification of rendered output"

duration: 12min
completed: 2026-08-05
status: complete
---

# Phase 02 Plan 03: Plan Presentation Summary

**Collapsible wave sections, per-task time estimates, and current task highlighting in plan and execute views**

## Performance

- **Duration:** 12 min
- **Started:** 2026-08-05T00:22:00Z
- **Completed:** 2026-08-05T00:34:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- PlanModel supports collapsible wave sections with keyboard toggle (c/C/E)
- Collapsed sections show summary header with task count and total time estimate
- Time estimates (~2/5/8 min) displayed next to each task based on complexity heuristic
- ExecuteModel highlights current task with brand color bold and ▸ prefix
- 5 new tests for collapse, time estimates, collapseAll, duration formatting

## Task Commits

Each task was committed atomically:

1. **Task 1: Collapsible plan sections with time estimates** - `034d542f` (feat)
2. **Task 2: Plan collapse tests and execute highlighting** - `034d542f` (feat)

**Plan metadata:** `034d542f` (docs: complete plan)

## Files Created/Modified
- `internal/ui/tui/plan_model.go` - Added collapsed map, toggleCollapse/expandAll/collapseAll, computeTaskDuration, time estimates in renderTasks
- `internal/ui/tui/execute_model.go` - Added current task highlighting with brand color and ▸ prefix
- `internal/ui/tui/plan_model_test.go` - Created 5 tests for collapse, time estimates, collapseAll, duration formatting

## Decisions Made
- Time estimates use complexity heuristic: deps>2 or long description = ~8min, has deps = ~5min, simple = ~2min
- Collapsed sections show summary: "▸ Wave N — Title (X tasks, Y min total)"
- Current task highlighted with brand color bold and ▸ prefix

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan presentation interactive features complete
- Ready for first-run tutorial integration (02-04)

---
*Phase: 02-user-experience*
*Completed: 2026-08-05*
