# 01-02 Plan Summary — Session, Engine, and Dispatcher Races

## Plan Reference
- **Plan:** [01-02-PLAN.md](./01-02-PLAN.md)
- **Phase:** 01 — Audit Bug Fixes
- **Wave:** 2

## Bugs Fixed

| Bug | Description | Severity | Approach |
|-----|-------------|----------|----------|
| B04 | `LoadWorkflowState` reads session metadata without lock | Medium | Acquire `m.lock` before metadata read |
| B05 | `saveSessionAtomic` serializes entire session including messages | Medium | Marshal only `sessionMetadata` (excludes Messages) |
| B12 | `SaveCheckpointData` reads `planVersion` without lock | Medium | Read under `planMu.RLock()` before checkpoint construction |
| B13 | `GetCheckpointData` reads `checkpointData` without lock | Medium | Read under `planMu.RLock()` |
| B20 | `SaveCheckpoint` concurrent access to checkpoint state | Medium | `checkpointMu` mutex for serialization |
| B23 | Goroutine in repl.go mutates `resizePending` directly | High | `SetProgram` wiring, `resizeDebounceMsg` type, `program.Send()` for safe message passing |
| B24 | Dispatcher reads `d.collector` without lock | Medium | Protect with `d.mu.RLock()` snapshot |

## Commits

1. `4d7f9d2f` — B04/B05/B20: session persistence races
2. `546f3b64` — B12/B13/B23/B24: engine races, repl program wiring, dispatcher collector lock

## Key Design Decisions

- **planMu extended**: `planMu` now guards `checkpointData` and `refineFeedback` in addition to `planVersion`/`planMarkdown`. This avoids introducing a new mutex and keeps all plan-adjacent state under one lock.
- **SetRefinementFeedback restructured**: Full function body protected under `planMu.Lock()` with early unlock on duplicate feedback. Version read captured before unlock to avoid TOCTOU.
- **ReplModel program wiring**: `AppState.SetReplProgram()` public method added to bridge the unexported `replModel` field. Called after `tea.NewProgram()` in main.go.
- **plan.go reader protection**: `refineFeedback` and `planMarkdown` reads in `buildPlanContext` and completion path all snapshot under `planMu.RLock()` before use.

## Test Results

- All new race tests pass with `-race` flag: `TestSaveCheckpointData_Concurrent`, `TestGetCheckpointData_Concurrent`
- Pre-existing test failures (`TestRunPlan_WithRefinement`, `TestCheckDangerousCommand_Baseline`, etc.) confirmed not caused by these changes
- Build clean with `CGO_ENABLED=0`

## Verification

- [x] Build passes (`go build ./...`)
- [x] Race tests pass (`-race -run TestSaveCheckpointData_Concurrent|TestGetCheckpointData_Concurrent`)
- [x] Existing tests unaffected (pre-existing failures unchanged)
- [x] All 7 bugs in plan fixed and committed
