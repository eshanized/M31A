---
phase: 14-tui-core-wiring-fixes
plan: 06
subsystem: tui
tags: [d-11, deferred, architectural-smell,AppState-refactor]
status: deferred
deferred_to: V1.1
reason: "All functional fixes (D-01 through D-10) are complete. The AppState refactor is a structural/architectural change with high regression risk. The plan explicitly marks it as optional/stretch."
---

# Phase 14 Plan 06: AppState God-Object Refactor — DEFERRED

## Summary

Plan 14-06 was **deferred to V1.1**. The plan called for decomposing the `AppState`
god object (1694 lines, 35+ fields, 950-line `Update()` method) into three
coordinator types:

- `WorkflowCoordinator` — workflow state, phase handling, msgChan sync
- `ScreenRouter` — screen state, navigation, discuss Q&A state
- `StreamCoordinator` — streaming state, chunk handling

## Rationale for Deferral

1. **All functional fixes complete.** Plans 14-01 through 14-05 resolve all
   10 wiring issues (D-01 through D-10). The workflow loop now runs
   end-to-end without deadlocks, screen flashes, message drops, or resource leaks.

2. **High regression risk.** Refactoring a 1694-line file with 35+ fields and
   a 950-line `Update()` method is inherently risky. The plan itself states:
   "the project may decide to defer it if the refactor proves too risky for
   the v1.0 release window."

3. **No functional impact.** D-11 is an architectural smell (maintainability
   concern), not a functional bug. The code works correctly — it's just hard
   to maintain.

4. **Clean foundation for V1.1.** With all wiring fixes in place, the V1.1
   features (ghost mode, PiP, subagents) will provide the natural pressure
   to decompose AppState when they're implemented.

## What Was Planned

The plan called for:
- 3 new files: `workflow_coordinator.go`, `screen_router.go`, `stream_coordinator.go`
- Each with constructor, `Handle(msg)` method, and unit tests
- `AppState` embedding the 3 coordinators
- `Update()` becoming a thin router (~50 lines)
- Behavior-preserving refactor (all existing tests pass without modification)

## When to Revisit

Revisit Plan 14-06 when:
- V1.1 feature development begins (ghost mode, PiP, subagents)
- The cost of not refactoring exceeds the risk of doing it
- A dedicated refactor sprint is scheduled with full test coverage

## Pre-conditions Met

The pre-conditions from the plan are all satisfied:
- [x] Plan 14-01 complete (Discuss Q&A state exists)
- [x] Plan 14-02 complete (Screen transitions stable)
- [x] Plan 14-03 complete (msgChan sync working)
- [x] Plan 14-04 complete (Persistence fields exist)
- [x] Plan 14-05 complete (StreamChunkMsg exists)

The state to relocate is well-defined and the plan is ready to execute
when the timing is right.

---

*Phase: 14-tui-core-wiring-fixes*
*Plan: 06*
*Status: DEFERRED to V1.1*
*Date: 2026-06-02*
