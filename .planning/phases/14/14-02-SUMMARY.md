---
phase: 14-tui-core-wiring-fixes
plan: 02
subsystem: tui
tags: [d-02, d-03, d-05, screen-wiring, workflow, model-dimensions, dead-end-fix]
dependency_graph:
  requires:
    - phase: 14-tui-core-wiring-fixes
      provides: "mockWorkflowEngine + workflowEngineInterface in tui package (14-01)"
  provides:
    - Non-zero width/height on every workflow screen model at construction
    - User-driven phase transitions via AppMsg{Screen: ScreenX} from sub-models
    - Stopped auto-advance from Plan/Execute/Verify to the next phase
    - Plan screen reachable in the auto-advance flow
  affects:
    - internal/tui/app.go (PhasePlan/Execute/Verify handlers; AppMsg routing)
    - internal/tui/plan.go, execute.go, verify.go, ship.go, diff.go (constructor signatures)
    - internal/tui/app_test.go (5 new screen-wiring tests)
tech-stack:
  added: []
  patterns:
    - "User-driven workflow: sub-models emit AppMsg{Screen: <next>} to drive the next phase via RunPhaseCmd"
    - "Eager SetMsgEmitter signal pattern: tests verify RunPhaseCmd wiring without executing the async cmd"
    - "Width/height stored in model struct at construction (no more 'Loading plan...' placeholder)"
key-files:
  created: []
  modified:
    - internal/tui/plan.go (NewPlanModel signature + width/height fields)
    - internal/tui/execute.go (NewExecuteModel signature + width/height fields)
    - internal/tui/verify.go (NewVerifyModel signature + width/height fields)
    - internal/tui/ship.go (NewShipModel signature + width/height fields)
    - internal/tui/diff.go (NewDiffModel signature + width/height fields)
    - internal/tui/app.go (PhasePlan/Execute/Verify stop auto-advance; AppMsg workflow routing; updated call sites)
    - internal/tui/app_test.go (5 new tests)
    - internal/tui/plan_test.go, execute_test.go, verify_test.go, ship_test.go, screens_test.go, commands_test.go (call-site updates for new signatures)
key-decisions:
  - "Kept NewPlanModel's existing float64 estCost + string estTime signature — only added width, height int at the end (the plan's pseudo-signature said int,float64 but the actual code had float64,string)"
  - "Added a new switch block at the top of the existing case AppMsg: handler (instead of a separate case) to dispatch ScreenExecute/Verify/Ship to RunPhaseCmd before the default screen-setting logic runs"
  - "Tests use the eagerly-called SetMsgEmitter as the signal that RunPhaseCmd wired the engine — same pattern as the 14-01 tests (avoids the racy goroutine timing in the runner)"
deviations:
  - "Minor: plan suggested a separate `case AppMsg` block; instead the workflow routing is added as a switch at the top of the existing `case AppMsg` handler. Functionally equivalent and avoids duplicating the ModelSelected/ScreenREPL/ScreenModelSelector handling."
  - "Minor: plan suggested `estimatedCost int` for NewPlanModel; actual current signature was `estCost float64`. Kept the existing types and only added width/height at the end."
  - "Minor: newTestAppForScreens helper added alongside the existing newTestAppForDiscuss helper. The new helper pre-installs a mock engine and sets width/height so test bodies stay short."
patterns-established:
  - "AppMsg from sub-models is now the universal channel for screen transitions that need to trigger workflow work. Sub-models emit AppMsg{Screen: <next-phase-sentinel>}; the TUI's AppMsg handler routes to RunPhaseCmd."
  - "Constructors that own a renderable area take width, height int as the final positional arguments. View() can render meaningfully on the first frame."
requirements-completed: [WIRE-D-02, WIRE-D-03, WIRE-D-05]
metrics:
  duration_seconds: 605
  completed_date: 2026-06-01
  tasks_completed: 3
  files_changed: 14
---

# Phase 14 Plan 02: Workflow Screen Wiring Summary

**Plan screen now reachable with non-zero dimensions; Execute/Verify/Ship stop flashing because phase transitions are user-driven via AppMsg**

## One-Line Summary

Five workflow screen models now take width/height at construction (no more "Loading plan..." placeholders), the Plan screen is finally set after a Plan phase result, and the Execute/Verify auto-advance that was replacing those screens in a single frame is gone — sub-models now emit `AppMsg{Screen: <next>}` to drive the next workflow phase.

## Performance

- **Duration:** 10 min (605 s)
- **Started:** 2026-06-01T22:41:48Z
- **Completed:** 2026-06-01T22:51:53Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- All five workflow screen model constructors (`NewPlanModel`, `NewExecuteModel`, `NewVerifyModel`, `NewShipModel`, `NewDiffModel`) now accept `width, height int` and store them in the struct. No more "Loading plan..." placeholder on first render.
- `PhaseResultMsg` for `PhasePlan` now sets `m.screen = ScreenPlan` and returns `(m, nil)` — the auto-advance to `PhaseExecute` is removed. The user must press `a`/`A` (the `PlanModel.Update` already returns `AppMsg{Screen: ScreenExecute}` on accept — that path is now wired through a new top-of-AppMsg-case dispatch).
- `PhaseResultMsg` for `PhaseExecute` and `PhaseVerify` similarly return `(m, nil)` — no more immediate screen flash. The corresponding sub-models already emit `AppMsg{Screen: ScreenVerify|ScreenShip}` on completion; the new AppMsg workflow routing wires those into `RunPhaseCmd`.
- Five new tests in `app_test.go` cover the full screen-wiring flow.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add width/height parameters to all model constructors** - `1094d86` (feat)
2. **Task 2: Update app.go call sites and fix PhasePlan/Execute/Verify cases** - `767948d` (feat)
3. **Task 3: Add tests for screen wiring and dimensions** - `32080c5` (test)

## Files Created/Modified

- `internal/tui/plan.go` — `NewPlanModel` now takes `width, height int`; stores them in struct
- `internal/tui/execute.go` — `NewExecuteModel` now takes `width, height int`; stores them in struct
- `internal/tui/verify.go` — `NewVerifyModel` now takes `width, height int`; stores them in struct
- `internal/tui/ship.go` — `NewShipModel` now takes `width, height int`; stores them in struct
- `internal/tui/diff.go` — `NewDiffModel` now takes `width, height int`; stores them in struct (DiffModel is a value type — no pointer change)
- `internal/tui/app.go` — `PhasePlan` sets `m.screen = ScreenPlan` and stops auto-advance; `PhaseExecute`/`PhaseVerify` also stop auto-advance; new workflow routing added at the top of `case AppMsg:`; all call sites updated to pass `m.width, m.height`
- `internal/tui/app_test.go` — 5 new tests + a `newTestAppForScreens` helper
- `internal/tui/plan_test.go`, `execute_test.go`, `verify_test.go`, `ship_test.go`, `commands_test.go`, `screens_test.go` — call-site updates for the new constructor signatures

## Decisions Made

- **Kept the existing PlanModel signature types.** The plan's pseudo-code suggested `NewPlanModel(..., estimatedCost int, costDisplay string, ...)`, but the actual current signature was `(..., estCost float64, estTime string, ...)`. Kept the existing types and only added `width, height int` at the end. This avoided changing the type of `estCost` and breaking every existing test assertion like `m.estCost != 0.05`.
- **Added the AppMsg workflow routing inside the existing `case AppMsg:`** as a `switch msg.Screen` block at the top, rather than as a separate case. The plan suggested a separate case block, but that would have duplicated the `ModelSelected` / `ScreenREPL` / `ScreenModelSelector` handling. The in-place switch is functionally equivalent and matches the plan's intent ("dispatch before the existing screen-specific block").
- **Tests verify RunPhaseCmd wiring via the eagerly-called `SetMsgEmitter`** — same pattern used by the 14-01 Discuss tests. `RunPhaseCmd` calls `eng.SetMsgEmitter(...)` synchronously before returning, so the mock's `emitterSetCount` is a deterministic signal that the engine was wired. This avoids the racy timing of actually executing the returned `tea.Cmd` (which schedules the runner goroutine asynchronously).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `NewPlanModel` call sites in test files had varying number of args**
- **Found during:** Task 1
- **Issue:** The constructor signature change from 6 to 8 params required updating 100+ call sites across 6 test files. One site (`plan_test.go:182`) had a unique nil-tasks pattern that wasn't caught by `replaceAll`.
- **Fix:** Used `replaceAll` for the common patterns, then targeted the unique line individually.
- **Files modified:** all of `internal/tui/*_test.go`
- **Verification:** `go build` and `go test -race` both pass
- **Committed in:** `1094d86` (Task 1 commit)

**2. [Rule 2 - Test signal] Tests for `TestApp_PlanAccept_RunsExecutePhase` and `TestApp_ExecuteModel_AllDone_TransitionsToVerify` initially checked `runPhaseCalls`**
- **Found during:** Task 3
- **Issue:** `RunPhaseCmd` returns a `tea.Cmd` that schedules a goroutine. By the time `Update()` returns, the goroutine may not have called `eng.RunPhase` yet, so `runPhaseCalls` was empty and the tests failed.
- **Fix:** Switched to checking `emitterSetCount` (which is incremented synchronously by `SetMsgEmitter` inside `RunPhaseCmd`) — same pattern as the 14-01 Discuss tests. Added a comment in each test explaining the signal.
- **Files modified:** `internal/tui/app_test.go`
- **Verification:** All 5 new tests pass
- **Committed in:** `32080c5` (Task 3 commit)

### Plan-level Adjustments

**1. `NewPlanModel` signature types not changed as suggested.** The plan's pseudo-code showed `estimatedCost int, costDisplay string` but the actual current signature is `estCost float64, estTime string`. Kept the existing types — only added `width, height int` at the end. This was the right call because changing the float64 to int would have broken tests that compare against `0.05` and `0.12`.

**2. AppMsg workflow routing added inside the existing `case AppMsg:` instead of as a new case.** The plan said "Place this case before the existing screen-specific `case Screen*:` dispatch block". The existing `case AppMsg:` already had the screen-setting fallback, so the cleanest implementation is a `switch msg.Screen` at the top of that case (functionally equivalent to "a new case that matches first").

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 test signal), 2 plan-level adjustments
**Impact on plan:** All auto-fixes necessary for build correctness and test determinism. The plan-level adjustments preserved the intent (user-driven phase transitions; non-zero dimensions at construction) while staying compatible with existing code.

## Issues Encountered

None — the plan was clear, the existing code was well-organized, and the 14-01 `mockWorkflowEngine` made the test wiring straightforward.

## Test Results

```
$ go test -count=1 -race -timeout 60s ./internal/tui/... -run "TestApp_PhaseResultMsg_Plan_ReachesScreenPlan|TestApp_PlanAccept_RunsExecutePhase|TestApp_NewPlanModel_NonZeroSize|TestApp_PhaseResultMsg_Execute_StaysOnScreen|TestApp_ExecuteModel_AllDone_TransitionsToVerify"
ok  	github.com/eshanized/M31A/internal/tui	1.106s
ok  	github.com/eshanized/M31A/internal/tui/components	1.039s [no tests to run]
ok  	github.com/eshanized/M31A/internal/tui/theme	1.013s [no tests to run]
```

```
$ go test -count=1 -race -timeout 180s ./...
ok  	github.com/eshanized/M31A/internal/config	1.027s
ok  	github.com/eshanized/M31A/internal/git	1.191s
ok  	github.com/eshanized/M31A/internal/log	1.016s
ok  	github.com/eshanized/M31A/internal/provider	1.040s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	3.536s
ok  	github.com/eshanized/M31A/internal/provider/zen	9.057s
ok  	github.com/eshanized/M31A/internal/tokens	6.581s
ok  	github.com/eshanized/M31A/internal/tools	2.188s
ok  	github.com/eshanized/M31A/internal/tui	1.857s
ok  	github.com/eshanized/M31A/internal/tui/components	1.127s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.014s
ok  	github.com/eshanized/M31A/internal/workflow	1.644s
ok  	github.com/eshanized/M31A/pkg/arbitrage	1.012s
ok  	github.com/eshanized/M31A/pkg/autodream	1.011s
ok  	github.com/eshanized/M31A/pkg/bisect	1.430s
ok  	github.com/eshanized/M31A/pkg/keychain	1.008s
ok  	github.com/eshanized/M31A/pkg/ledger	1.024s
ok  	github.com/eshanized/M31A/pkg/rollback	2.192s
ok  	github.com/eshanized/M31A/pkg/session	1.050s
ok  	github.com/eshanized/M31A/pkg/taskrunner	1.010s
```

```
$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # OK
$ go vet ./internal/tui/...                         # clean
```

## Audit Verification

```
$ grep -n "RunPhaseCmd(m, types.PhaseExecute\|RunPhaseCmd(m, types.PhaseVerify\|RunPhaseCmd(m, types.PhaseShip" internal/tui/app.go
938:			return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)   # AppMsg{Screen: ScreenExecute} → Execute
944:			return m, RunPhaseCmd(m, types.PhaseVerify, m.workflowGoal)    # AppMsg{Screen: ScreenVerify} → Verify
950:			return m, RunPhaseCmd(m, types.PhaseShip, m.workflowGoal)      # AppMsg{Screen: ScreenShip} → Ship
1241:		return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)      # Plan phase: no tasks → skip to Execute
```

Only the workflow-routing AppMsg handler and the "no tasks" skip-ahead path in PhasePlan remain. The previous auto-advance calls from PhaseExecute→Verify, PhaseVerify→Ship, and PhasePlan→Execute are all gone.

## Auth Gates

None — no API keys, no external service interactions.

## Known Stubs

None — all paths wire to real implementations or the mock.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: trust-boundary | internal/tui/app.go | New `switch msg.Screen` block in `case AppMsg:` matches `ScreenExecute/Verify/Ship` and calls `RunPhaseCmd` if a workflow engine is set. The handler checks `m.workflowEngine == nil` first, so a missing engine is handled gracefully (returns nil cmd, no panic). |

## Next Phase Readiness

- The Plan, Execute, and Verify screens are now reachable and stay visible until the user drives the next transition.
- The "no tasks" fallback in `PhasePlan` (line 1241) still calls `RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)` — this is intentional: when the plan phase returns no tasks, the workflow skips directly to Execute (a valid edge case for trivial goals).
- The next plan (14-04: Workflow State Persistence) can build on this — the user-driven phase transitions are now stable and won't be lost in the next auto-advance.
- No blockers.

---

*Phase: 14-tui-core-wiring-fixes*
*Completed: 2026-06-01*
