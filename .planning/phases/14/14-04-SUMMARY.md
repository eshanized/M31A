---
phase: 14-tui-core-wiring-fixes
plan: 04
subsystem: workflow-persistence
tags: [d-06, persistence, workflow-state, session-json, resume, atomic-write]
dependency_graph:
  requires:
    - phase: 14-tui-core-wiring-fixes
      provides: "PhaseResultMsg handler, NewApp, and SlashCommandMsg infrastructure (14-01/02/03)"
  provides:
    - Workflow state persistence to session.json (D-06 fix)
    - Startup resume detection with toast
    - /workflow resume slash command
    - Atomic save helper (saveSessionAtomic)
  affects:
    - pkg/session/session.go (Session struct fields, SetWorkflowState/WorkflowState)
    - pkg/session/manager.go (UpdateWorkflowState/LoadWorkflowState/saveSessionAtomic)
    - internal/tui/app.go (persistWorkflowState, checkResumedWorkflowState, sessionID field, PhaseResultMsg wiring, SlashCommandMsg routing)
    - internal/tui/commands.go (CommandResult extension, /workflow resume subcommand)
    - pkg/session/session_test.go (new file, 9 tests)
    - internal/tui/app_test.go (15 new tests + 3 helpers)
tech-stack:
  added: []
  patterns:
    - "Atomic write pattern: temp file + rename for session.json updates (existing atomicWrite helper)"
    - "Defensive nil/empty checks on persistWorkflowState (no panic if sessionManager is nil or sessionID is empty)"
    - "Resume detection: pre-populate AppState from session.json on startup, show toast for user awareness"
    - "Workflow resume is MANUAL (slash command) not AUTO — avoids surprising the user with mid-workflow jumps"
key-files:
  created:
    - pkg/session/session_test.go (337 lines, 9 tests)
  modified:
    - pkg/session/session.go (+24 lines)
    - pkg/session/manager.go (+46 lines)
    - internal/tui/app.go (+80 lines)
    - internal/tui/commands.go (+89 lines, -13)
    - internal/tui/app_test.go (+792 lines)
key-decisions:
  - "Toast via toastText/toastType/toastExpires fields (not a toasts slice or ToastMsg queue) — matches existing convention in cycleRecentModel and renderToast"
  - "Used Type: 'info' string instead of the non-existent ToastInfo constant"
  - "Added sessionID field to AppState, set by initWorkflowEngine — the plan used m.sessionID throughout"
  - "LoadWorkflowState returns zero values (not error) for ErrSessionCorrupted (missing session) — matches plan's contract"
  - "saveSessionAtomic only writes session.json, NOT messages.json — used for targeted updates; SaveSession remains the full-save path"
  - "/workflow resume is intercepted by the command registry (handleWorkflow) rather than the /workflow <goal> prefix match in app.go"
  - "The /workflow <goal> prefix match in app.go is updated to skip when the goal is 'resume' so it falls through to the registry"
  - "PhaseShip case resets both persisted state (via UpdateWorkflowState with empty values) AND in-memory workflowGoal/currentPhase"
decisions:
  - "Toast uses Type: 'info' string — no ToastInfo constant exists, and ToastMsg has Type string not Level"
  - "loadWorkflowState returns zero values (not error) on ErrSessionCorrupted — matches the plan's 'session doesn't exist' handling"
  - "SaveSessionAtomic is a partial save (session.json only) — use SaveSession for full saves including messages.json"
  - "Added sessionID field to AppState rather than computing from workflowEngine.SessionID() at every call site"
  - "PhaseShip reset logic intentionally runs after persistWorkflowState so the current (Ship) phase IS persisted first, then cleared"
  - "/workflow resume intercept happens in handleWorkflow (commands.go) — the /workflow <goal> prefix match in app.go was updated to fall through for 'resume'"
metrics:
  duration_seconds: 600
  completed_date: 2026-06-02
  tasks_completed: 5
  files_changed: 6
---

# Phase 14 Plan 04: Workflow State Persistence Summary

**Workflow state (goal, phase, discuss questions) is now persisted to `session.json` on every phase transition. Closing the app mid-workflow preserves progress; reopening shows a resume toast, and `/workflow resume` restarts the workflow at the saved phase.**

## One-Line Summary

`Session` struct gains `WorkflowGoal`/`DiscussQuestions` fields; `Manager` gains `UpdateWorkflowState`/`LoadWorkflowState`; `app.go` calls `persistWorkflowState()` after every phase transition and shows a resume toast on startup; `/workflow resume` slash command restarts the workflow at the persisted phase.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Extend Session struct with workflow state fields | `d1971f3` | pkg/session/session.go |
| 2 | Add UpdateWorkflowState/LoadWorkflowState on Manager | `e95f599` | pkg/session/manager.go |
| 3 | Wire persistence into PhaseResultMsg handler and NewApp | `5e4906e` | internal/tui/app.go |
| 4 | Add /workflow resume slash command | `940a123` | internal/tui/commands.go, internal/tui/app.go |
| 5 | Add tests for workflow state persistence | `c1f1f84` | pkg/session/session_test.go (new), internal/tui/app_test.go |

## Key Files Changed

- **`pkg/session/session.go`** — `Session` struct gets `WorkflowGoal string` and `DiscussQuestions []string` (both `omitempty` for clean JSON when unset). New `SetWorkflowState(goal, phase, questions)` and `WorkflowState() (goal, phase, questions)` methods. `NewSession` initializes `DiscussQuestions` to an empty slice (not nil).
- **`pkg/session/manager.go`** — Three new methods on `*Manager`:
  - `UpdateWorkflowState(id, goal, phase, questions) error` — loads the session, mutates the workflow state, writes `session.json` atomically.
  - `LoadWorkflowState(id) (goal, phase, questions, err)` — returns the persisted state; missing sessions (ErrSessionCorrupted) return zero values with no error.
  - `saveSessionAtomic(session) error` — partial atomic save (only `session.json`, not `messages.json`).
  - Imports the `errors` package for `errors.Is(err, m31errors.ErrSessionCorrupted)` checks.
- **`internal/tui/app.go`** — 
  - `AppState` gains a `sessionID string` field, set by `initWorkflowEngine` after creating the new session.
  - New `persistWorkflowState()` method — calls `sessionManager.UpdateWorkflowState` with the current goal/phase/questions; logs but does not return errors.
  - New `checkResumedWorkflowState()` method — called from `NewApp` right after `initWorkflowEngine`. If the loaded phase is not `PhaseIdle` and not `PhaseShip`, pre-populates `workflowGoal`/`currentPhase`/`discussQuestions` and emits an info toast for 10 seconds.
  - `PhaseResultMsg` handler now calls `persistWorkflowState()` after every successful `m.currentPhase` update.
  - `PhaseShip` case additionally calls `UpdateWorkflowState` with empty values (post-Ship reset) and clears in-memory `workflowGoal`/`currentPhase`.
  - `SlashCommandMsg` handler: the `/workflow <goal>` prefix match skips when goal is `resume` (lets the command registry handle it). After `cmdRegistry.Execute`, the result is checked for `WorkflowResume: true` and routed to `RunPhaseCmd`.
- **`internal/tui/commands.go`** —
  - `CommandResult` extended with `WorkflowResume bool`, `ResumePhase types.WorkflowPhase`, `ResumeGoal string`, `ResumeQuestions []string`.
  - `handleWorkflow` checks for a `resume` subcommand first. If matched, loads the persisted state and returns a `CommandResult` with `WorkflowResume: true` and the loaded values.
- **`pkg/session/session_test.go`** (new file) — 9 tests covering the typed accessors and the manager-level persistence path.
- **`internal/tui/app_test.go`** — 15 new tests + 3 new helpers (`newTestAppSessionManager`, `newTestAppWithSession`, `newTestAppBareWithSession`).

## Acceptance Criteria

- [x] `Session` struct has `WorkflowGoal`, `DiscussQuestions` fields
- [x] `Manager.UpdateWorkflowState` and `Manager.LoadWorkflowState` methods exist
- [x] `app.go` calls `persistWorkflowState()` after every phase transition
- [x] `app.go` `NewApp` calls `LoadWorkflowState` and shows resume toast
- [x] `/workflow resume` command exists and triggers `RunPhaseCmd` for the saved phase
- [x] After Ship phase, persisted state resets to idle
- [x] All new tests pass; all existing tests still pass
- [x] `go build`, `go vet`, `go test -race` all pass
- [x] Closing the app mid-workflow preserves progress

## Test Results

```
$ go test -count=1 -race ./pkg/session/... -run "TestSession_SetWorkflowState"
ok  	github.com/eshanized/M31A/pkg/session	1.010s

$ go test -count=1 -race ./pkg/session/... -run "TestManager_UpdateWorkflowState"
ok  	github.com/eshanized/M31A/pkg/session	1.013s

$ go test -count=1 -race ./pkg/session/... -run "TestManager_LoadWorkflowState"
ok  	github.com/eshanized/M31A/pkg/session	1.011s

$ go test -count=1 -race ./internal/tui/... -run "TestApp_PhaseResultMsg_PersistsWorkflowState"
ok  	github.com/eshanized/M31A/internal/tui	1.036s

$ go test -count=1 -race ./internal/tui/... -run "TestSlashCommand_WorkflowResume"
ok  	github.com/eshanized/M31A/internal/tui	1.025s

$ go test -count=1 -race -timeout 180s ./...
ok  	github.com/eshanized/M31A/internal/config	1.036s
ok  	github.com/eshanized/M31A/internal/git	1.211s
ok  	github.com/eshanized/M31A/internal/log	1.017s
ok  	github.com/eshanized/M31A/internal/provider	1.041s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	3.538s
ok  	github.com/eshanized/M31A/internal/provider/zen	9.061s
ok  	github.com/eshanized/M31A/internal/tokens	6.573s
ok  	github.com/eshanized/M31A/internal/tools	2.190s
ok  	github.com/eshanized/M31A/internal/tui	2.048s
ok  	github.com/eshanized/M31A/internal/tui/components	1.211s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.015s
ok  	github.com/eshanized/M31A/internal/workflow	1.735s
ok  	github.com/eshanized/M31A/pkg/arbitrage	1.013s
ok  	github.com/eshanized/M31A/pkg/autodream	1.010s
ok  	github.com/eshanized/M31A/pkg/bisect	1.426s
ok  	github.com/eshanized/M31A/pkg/keychain	1.008s
ok  	github.com/eshanized/M31A/pkg/ledger	1.023s
ok  	github.com/eshanized/M31A/pkg/rollback	2.181s
ok  	github.com/eshanized/M31A/pkg/session	1.050s
ok  	github.com/eshanized/M31A/pkg/taskrunner	1.013s

$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # OK
$ go vet ./...                                      # clean
```

## Deviations from Plan

### Plan-level Adjustments

**1. Toast uses `Type: "info"` (not `Level: ToastInfo`)**

- **Plan said:** `m.toasts = append(m.toasts, ToastMsg{Text: ..., Level: ToastInfo})` and `ToastInfo` constant
- **Actual codebase:** ToastMsg has `Type string` field, not `Level`. `ToastInfo` constant does not exist. There is no `toasts` slice — the AppState has `toastText`/`toastType`/`toastExpires` fields directly. `renderToast` switches on string values: `"success"`, `"warning"`, `"error"`, default.
- **Implementation:** Set `m.toastText = ...`, `m.toastType = "info"`, `m.toastExpires = time.Now().Add(10 * time.Second)` directly. Matches the convention in `cycleRecentModel` and the existing `ToastMsg` handler in `case ToastMsg:`.
- **Why:** Plan's pseudo-code assumed types that don't exist in the actual codebase. The implementation follows the existing convention.

**2. Added `sessionID` field to `AppState` (not used `m.workflowEngine.SessionID()` everywhere)**

- **Plan said:** `m.sessionID` in NewApp, PhaseResultMsg, persistWorkflowState
- **Actual codebase:** `AppState` had no `sessionID` field; `m.workflowEngine.SessionID()` is used in places (and returns empty if engine init failed)
- **Implementation:** Added `sessionID string` to `AppState`, set by `initWorkflowEngine` after `m.sessionManager.NewSession(...)`. All new code uses `m.sessionID` directly.
- **Why:** Plan's code uses `m.sessionID` throughout, so the field had to exist. Computing from the engine on every call would also work but adds a method indirection.

**3. `LoadWorkflowState` returns zero values (not nil error) for missing sessions**

- **Plan said:** "Returns zero values (empty goal, PhaseIdle, empty questions) if the session doesn't exist or the workflow state is unset"
- **Actual code:** `LoadSession` returns `m31errors.ErrSessionCorrupted` for missing files (also for unmarshal errors). The plan's `LoadWorkflowState` checks `errors.Is(err, ErrSessionCorrupted)` and returns zero values with `nil` error. This is the correct implementation.
- **No change needed** — the plan's code is what was implemented. Documented here for clarity.

**4. `saveSessionAtomic` only writes `session.json`, not `messages.json`**

- **Plan said:** "saveSessionAtomic writes the session to disk atomically. Used by UpdateWorkflowState; also a building block for future session-modification operations."
- **Implementation:** `saveSessionAtomic` only writes `session.json` (no `messages.json`). The existing `SaveSession` method remains the full-save path.
- **Why:** The plan is ambiguous — "writes the session to disk" could mean either. Splitting the partial save (for `UpdateWorkflowState`) from the full save (for general use) is the safer pattern; if `UpdateWorkflowState` accidentally wrote `messages.json`, it could clobber in-flight message updates from the REPL.

**5. `/workflow resume` falls through to the command registry**

- **Plan said:** "In `app.go SlashCommandMsg` handler (around line 605), after the existing switch on command name, handle `WorkflowResume: true`"
- **Implementation:** The `/workflow <goal>` prefix match in `app.go` was modified to check for `goal == "resume"` and skip the interception, falling through to `cmdRegistry.Execute`. The `WorkflowResume: true` check happens in the registry result handler.
- **Why:** The command registry's `handleWorkflow` is the natural home for subcommand dispatch. Intercepting `/workflow resume` in app.go would duplicate that logic.

**6. The `session_test.go` file did not exist — created new**

- **Plan said:** "Add tests in `pkg/session/session_test.go`"
- **Implementation:** Created the new file. Existing tests in `manager_test.go` use the `Session` type and `Manager` type — the new file follows the same pattern.
- **Why:** Plan's `files_modified` listed `session_test.go`; the file didn't exist (only `manager_test.go`, `checkpoint_test.go`, `planning_test.go` did).

### Auto-fixed Issues

**1. [Rule 1 - Bug] `LoadWorkflowState` initial implementation returned 3 values from a 4-value function**

- **Found during:** Task 2 build check
- **Issue:** The first cut of `LoadWorkflowState` called `return session.WorkflowState()` which returns 3 values, but the function signature has 4 named return values.
- **Fix:** Captured the 3 return values into local variables and returned them with `nil` error: `g, p, q := session.WorkflowState(); return g, p, q, nil`.
- **Committed in:** `e95f599`

**2. [Rule 1 - Bug] Test expected non-empty `currentPhase` in idle case**

- **Found during:** Task 5 (`TestApp_NewApp_NoResumeToastForIdleState`)
- **Issue:** The test helper `newTestAppBareWithSession` did not initialize `currentPhase`, leaving it as the zero value (empty string). The test then expected `currentPhase == types.PhaseIdle` to remain true.
- **Fix:** Added `currentPhase: types.PhaseIdle` to the helper struct literal.
- **Committed in:** `c1f1f84`

## Auth Gates

None — no API keys, no external service interactions.

## Known Stubs

None — all paths wire to real implementations. The mock workflow engine from 14-01 is reused (with the same `emitterSetCount` eager-signal pattern).

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: trust-boundary | pkg/session/manager.go | New `UpdateWorkflowState` uses `LoadSession` + `saveSessionAtomic` — atomic write prevents torn session.json on crash. Read fails gracefully with `ErrSessionCorrupted` (zero values returned). |
| threat_flag: trust-boundary | internal/tui/app.go | New `persistWorkflowState()` logs but never returns errors — the workflow continues even if persistence fails (defensive DoS mitigation per the threat model T-14-04-04). |
| threat_flag: trust-boundary | internal/tui/commands.go | `/workflow resume` reads the loaded phase from the engine's persisted state, not user input — no spoofing vector (matches T-14-04-03). |

## Next Phase Readiness

- D-06 is resolved. The workflow loop is now resumable across app restarts.
- The persistence layer (session.json) is the foundation for future cross-session features (e.g., cross-session learning ledger already exists in `pkg/ledger/`).
- No new exports or breaking API changes beyond the new methods on `Session` and `Manager`.
- The next plan (14-05: Discuss Streaming + Low Severity) can build on this — the workflow loop is now both stable and persistent.

---

*Phase: 14-tui-core-wiring-fixes*
*Plan: 04*
*Completed: 2026-06-02*

## Self-Check: PASSED

- [x] `.planning/phases/14/14-04-SUMMARY.md` exists
- [x] `pkg/session/session.go` modified (Task 1)
- [x] `pkg/session/manager.go` modified (Task 2)
- [x] `internal/tui/app.go` modified (Tasks 3, 4)
- [x] `internal/tui/commands.go` modified (Task 4)
- [x] `pkg/session/session_test.go` created (Task 5)
- [x] `internal/tui/app_test.go` modified (Task 5)
- [x] Commit `d1971f3` (Task 1) exists in git log
- [x] Commit `e95f599` (Task 2) exists in git log
- [x] Commit `5e4906e` (Task 3) exists in git log
- [x] Commit `940a123` (Task 4) exists in git log
- [x] Commit `c1f1f84` (Task 5) exists in git log
- [x] `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` passes
- [x] `go vet ./...` passes
- [x] `go test -count=1 -race ./pkg/session/... -run "TestSession_SetWorkflowState"` passes
- [x] `go test -count=1 -race ./pkg/session/... -run "TestManager_UpdateWorkflowState"` passes
- [x] `go test -count=1 -race ./internal/tui/... -run "TestApp_PhaseResultMsg_PersistsWorkflowState"` passes
- [x] `go test -count=1 -race ./...` (full regression) passes
