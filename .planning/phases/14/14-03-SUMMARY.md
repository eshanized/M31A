---
phase: 14-tui-core-wiring-fixes
plan: 03
subsystem: tui
tags: [d-04, msgchan, drainer, race-fix, workflow, tui-engine-wiring]

# Dependency graph
requires:
  - phase: 14-tui-core-wiring-fixes
    plan: 01
    provides: "mockWorkflowEngine + workflowEngineInterface in tui package (14-01)"
provides:
  - "Per-phase msgDone channel that synchronizes runner and drainer lifecycle"
  - "phaseGen counter on AppState that lets drainers detect phase changes"
  - "safeClose helper for defensive double-close protection"
  - "workflowMsgDrainer signature (app, gen, done) with msgCh/done/100ms-poll select"
affects:
  - internal/tui/app.go (AppState, RunPhaseCmd, workflowMsgDrainer, 3 caller updates)
  - internal/tui/app_test.go (4 new tests)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-phase done channel: runner uses defer close(doneCh); drainer checks done + gen"
    - "phaseGen counter incremented before channel swap; drainer captures gen at spawn"
    - "Defensive close pattern: select { case <-ch: default: close(ch) }"
    - "100ms time.After poll to ensure forward progress when no messages pending"

key-files:
  created: []
  modified:
    - internal/tui/app.go (+95 / -22)
    - internal/tui/app_test.go (+121)

key-decisions:
  - "Runner uses bare 'defer close(doneCh)' (local var) — safe because the runner runs once per phase and doneCh is freshly created; the OLD app.msgDone is closed via safeClose in RunPhaseCmd before the new one is created"
  - "Drainer re-schedules itself via 'return workflowMsgDrainer(app, gen, done)()' on 100ms timeout to keep the drain loop alive without blocking"
  - "Drainer gen check happens at BOTH entry and inside the timeout branch to ensure phase supersession is caught as soon as possible"
  - "Update()'s 3 drainer re-arm call sites (PlanReadyMsg, TaskStartMsg, TaskUpdateMsg) pass m.phaseGen and m.msgDone at the time of the call — gen mismatch returns nil, providing natural backstop"

deviations:
  - "TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase captures oldDoneCh BEFORE the second RunPhaseCmd (vs. checking m.msgDone after) because m.msgDone is the NEW channel after the second call and would NOT be closed. The plan's literal code would fail. This is a correctness fix, not a behavior change."

requirements-completed: [WIRE-D-04]

# Metrics
duration: ~6 min
completed: 2026-06-01
---

# Phase 14 Plan 03: msgChan Drainer Synchronization Summary

**Per-phase msgDone channel + phaseGen counter replace the racy `app.msgChan = nil` pattern; the runner closes its done channel via `defer close(doneCh)` and the drainer selects on `msgCh`, `done`, and a 100ms poll, returning nil if `app.phaseGen` changes.**

## Performance

- **Duration:** ~6 min (started 23:03:26Z, completed 23:09:30Z)
- **Started:** 2026-06-01T23:03:26Z
- **Completed:** 2026-06-01T23:09:30Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `phaseGen int` and `msgDone chan struct{}` fields to `AppState` (Task 1).
- Added `currentPhaseGen()` snapshot method on `*AppState` (Task 1).
- Refactored `RunPhaseCmd` to: cancel previous context, increment `phaseGen`, close old `msgDone` via `safeClose`, create fresh `msgCh`/`doneCh`, capture `currentGen`, and run the runner with `defer close(doneCh)` (Task 2).
- Refactored `workflowMsgDrainer` to the new signature `(app, gen, done)`; selects on `msgCh`, `done`, and 100ms `time.After`; returns nil if `app.phaseGen != gen` (Task 2).
- Added `safeClose(ch chan struct{}) bool` helper for defensive double-close + nil-channel protection (Task 2).
- Updated 3 caller sites in `Update()` (`PlanReadyMsg`, `TaskStartMsg`, `TaskUpdateMsg` handlers) to pass `m.phaseGen` and `m.msgDone` to the new drainer signature (Task 2).
- Added 4 new tests in `app_test.go` covering all synchronization paths (Task 3).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add phaseGen and msgDone fields to AppState** - `20e9826` (feat)
2. **Task 2: Refactor RunPhaseCmd with per-phase done channel** - `1591168` (feat)
3. **Task 3: Add drainer synchronization tests** - `b350afc` (test)

## Files Created/Modified

- `internal/tui/app.go` — `AppState` struct gets `msgDone` + `phaseGen`; new `currentPhaseGen()` method; `RunPhaseCmd` refactored; `workflowMsgDrainer` signature changed; new `safeClose` helper; 3 caller updates in `Update()`.
- `internal/tui/app_test.go` — 4 new tests: `TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase`, `TestApp_RunPhaseCmd_MessageDrainSynchronized`, `TestApp_RunPhaseCmd_DoneClosesOnRunnerCompletion`, `TestApp_SafeClose_HandlesDoubleClose`.

## Decisions Made

- **Runner uses bare `defer close(doneCh)`, not `safeClose`.** The runner runs once per phase and `doneCh` is a fresh local channel created in `RunPhaseCmd`. The OLD `app.msgDone` is closed via `safeClose` BEFORE the new one is created, so there's no double-close risk on the new doneCh. The `safeClose` guard is for the OLD `app.msgDone` only.
- **Drainer re-schedules via `return workflowMsgDrainer(app, gen, done)()` on poll timeout.** This keeps the drain loop alive without blocking the Bubble Tea event loop. The gen check inside the timeout branch ensures the re-scheduled drainer also returns nil if the phase has been superseded.
- **`Update()` callers pass `m.phaseGen` and `m.msgDone` at call time.** This is safe because: (1) gen mismatch makes the drainer return nil, providing a natural backstop if a new phase started between the message receipt and the drainer re-arm, and (2) `m.msgDone` is the channel that the runner of the CURRENT phase will close when it completes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed test that checked the new msgDone instead of the old one**

- **Found during:** Task 3 (writing `TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase`)
- **Issue:** The plan's literal test code did:
  ```go
  RunPhaseCmd(m, types.PhaseDiscuss, "goal")  // creates new msgDone, closes old
  select {
  case <-m.msgDone:  // WRONG: this is the NEW channel, not closed
  default: t.Error(...)
  }
  ```
  After the second `RunPhaseCmd`, `m.msgDone` is the NEW channel (not the closed one). The test as written would always fail.
- **Fix:** Capture `oldDoneCh := m.msgDone` between the two `RunPhaseCmd` calls and check `oldDoneCh` is closed. Added comment explaining the deviation.
- **Files modified:** `internal/tui/app_test.go`
- **Verification:** `go test -race ./internal/tui/... -run TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase` passes
- **Committed in:** `b350afc` (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 test correctness bug)
**Impact on plan:** The deviation is a test correctness fix — the test's intent (verify old drainer is signaled) is preserved, but the implementation correctly identifies the OLD channel rather than checking the new one.

## Issues Encountered

None.

## Test Results

```
$ go test -count=1 -race -v -timeout 30s ./internal/tui/... -run "TestApp_RunPhaseCmd|TestApp_SafeClose"
=== RUN   TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase
--- PASS: TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase (0.00s)
=== RUN   TestApp_RunPhaseCmd_MessageDrainSynchronized
--- PASS: TestApp_RunPhaseCmd_MessageDrainSynchronized (0.00s)
=== RUN   TestApp_RunPhaseCmd_DoneClosesOnRunnerCompletion
--- PASS: TestApp_RunPhaseCmd_DoneClosesOnRunnerCompletion (0.00s)
=== RUN   TestApp_SafeClose_HandlesDoubleClose
--- PASS: TestApp_SafeClose_HandlesDoubleClose (0.00s)
PASS
ok  	github.com/eshanized/M31A/internal/tui	1.059s
```

```
$ go test -count=1 -race -timeout 180s ./...
ok  	github.com/eshanized/M31A/internal/config	1.031s
ok  	github.com/eshanized/M31A/internal/git	1.203s
ok  	github.com/eshanized/M31A/internal/log	1.018s
ok  	github.com/eshanized/M31A/internal/provider	1.044s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	3.537s
ok  	github.com/eshanized/M31A/internal/provider/zen	9.059s
ok  	github.com/eshanized/M31A/internal/tokens	6.407s
ok  	github.com/eshanized/M31A/internal/tools	2.196s
ok  	github.com/eshanized/M31A/internal/tui	1.858s
ok  	github.com/eshanized/M31A/internal/tui/components	1.153s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.010s
ok  	github.com/eshanized/M31A/internal/workflow	1.661s
ok  	github.com/eshanized/M31A/pkg/arbitrage	1.010s
ok  	github.com/eshanized/M31A/pkg/autodream	1.010s
ok  	github.com/eshanized/M31A/pkg/bisect	1.436s
ok  	github.com/eshanized/M31A/pkg/keychain	1.012s
ok  	github.com/eshanized/M31A/pkg/ledger	1.032s
ok  	github.com/eshanized/M31A/pkg/rollback	2.177s
ok  	github.com/eshanized/M31A/pkg/session	1.049s
ok  	github.com/eshanized/M31A/pkg/taskrunner	1.009s
```

```
$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # OK
$ go vet ./...                                      # clean
```

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: race-fix | internal/tui/app.go | D-04 race fix: per-phase `msgDone` channel and `phaseGen` counter ensure the old drainer cannot process new-phase messages. Verified by `TestApp_RunPhaseCmd_OldDrainerStopsOnNewPhase`. |

## Next Phase Readiness

- D-04 (`msgChan` race) is resolved. The workflow message bus is now safely synchronized across phase boundaries.
- No new exports or breaking API changes beyond `workflowMsgDrainer` (internal function).
- The next plan (14-04: Workflow State Persistence) can build on this — the workflow loop is now race-free.

---
*Phase: 14-tui-core-wiring-fixes*
*Completed: 2026-06-01*

## Self-Check: PASSED

- [x] `.planning/phases/14/14-03-SUMMARY.md` exists
- [x] `internal/tui/app.go` modified (Task 1 + Task 2)
- [x] `internal/tui/app_test.go` modified (Task 3)
- [x] Commit `20e9826` (Task 1) exists in git log
- [x] Commit `1591168` (Task 2) exists in git log
- [x] Commit `b350afc` (Task 3) exists in git log
- [x] `go build ./internal/tui/...` passes
- [x] `go vet ./...` passes
- [x] `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` passes
- [x] `go test -count=1 -race ./internal/tui/... -run "TestApp_RunPhaseCmd"` passes
- [x] `go test -count=1 -race ./internal/tui/... -run "TestApp_SafeClose"` passes
- [x] `go test -count=1 -race ./...` (full regression) passes
- [x] All 4 new tests pass
