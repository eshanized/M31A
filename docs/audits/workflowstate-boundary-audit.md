# WorkflowState Boundary Audit

**Date:** 2026-07-11  
**Auditor:** opencode (GSD workflowstate-boundary-audit task)

## Summary

**2 UNSAFE sites found** where the TUI reads workflow engine state outside of `Update()` message handling:

1. **`internal/tui/app_view.go:1146`** — `renderDecisionsContent()` calls `m.workflowEngine.SnapshotDecisions()` from `View()`
2. **`internal/tui/app_update_phase.go:201-202`** — `handlePhaseResult()` calls `m.workflowEngine.PlanContent()` and `m.workflowEngine.PlanVersion()` — *technically safe (called from Update()), but reads state that is mutated in the workflow goroutine without synchronization*

**No test coverage** exists for concurrent phase transition + LLM message streaming (race detector gap).

---

## Classification Table

| file:line | field / method | access type | classification | notes |
|-----------|----------------|-------------|----------------|-------|
| `internal/tui/app_view.go:1146` | `SnapshotDecisions()` | **View()** — direct engine call during render | **UNSAFE** | Called from `renderDecisionsContent()` which is invoked by `View()`. The decision logger uses mutexes internally (`flushMu`, `ringMu`), so no data race, but violates Bubble Tea contract: all state must cross thread boundary via `tea.Msg` into `Update()`. View() runs on Bubble Tea main thread; workflow engine runs in separate goroutine. Stale data possible if decisions are being written concurrently. |
| `internal/tui/app_update_phase.go:116` | `DiscussState()` | **Update()** — message handler | **SAFE** | Called from `handlePhaseResultMsg()` which is dispatched via `Update()`. Returns a value copy (`DiscussState` struct), so no shared mutable reference escapes. |
| `internal/tui/app_update_phase.go:201` | `PlanContent()` | **Update()** — message handler | **SAFE (with caveat)** | Called from `handlePhaseResultMsg()` via `Update()`. Returns `state.planMarkdown` (string) by value. **However**: `planMarkdown` is written in workflow goroutine (`runPlan()`, `runDiscuss()`) without a mutex — only `transitionMu` guards phase transitions, not individual fields. Reading here races with writes in `runPlan()`/`runDiscuss()`. |
| `internal/tui/app_update_phase.go:202` | `PlanVersion()` | **Update()** — message handler | **SAFE (with caveat)** | Same as above — `planVersion` (int) written in workflow goroutine without mutex. |
| `internal/tui/app_update_phase.go:368` | `SetRefinementFeedback()` | **Update()** — message handler | **SAFE** | Write-only via engine method; called from `Update()` with user feedback. |
| `internal/tui/app_update_phase.go:105` | `Transition()` | **Update()** — message handler | **SAFE** | Uses `transitionMu` mutex internally. |
| `internal/tui/app_update_phase.go:143` | `SkipDiscuss()` | **Update()** — message handler | **SAFE** | Calls `FinalizeDiscuss()` which uses `transitionMu`. |
| `internal/tui/app.go:764` | `SetModel()` | **Update()** — called from `checkAutoArbitrage()` | **SAFE** | Uses `modelIDMu` mutex. |
| `internal/tui/app_session.go:281` | `SubmitDiscussAnswer()` | **Update()** — message handler | **SAFE** | Called from `handleDiscussAnswerMsg()`. |
| `internal/tui/app_session.go:292` | `FinalizeDiscuss()` | **Update()** — message handler | **SAFE** | Uses `transitionMu`. |
| `internal/tui/app_session.go:306` | `Transition()` | **Update()** — message handler | **SAFE** | Uses `transitionMu`. |
| `internal/tui/app_nav.go:162` | `Transition()` | **Update()** — message handler | **SAFE** | Called from `handleDiscussCompleteMsg()` via `Update()`. |
| `internal/tui/app_nav.go:432` | `Transition()` | **Update()** — message handler | **SAFE** | Called from `handlePlanRefineMsg()` via `Update()`. |
| `internal/tui/app_input.go:197` | `SetWorkflowMode()` | **Update()** — message handler | **SAFE** | Uses `workflowModeMu`. |
| `internal/tui/app_input.go:340` | `Transition()` | **Update()** — message handler | **SAFE** | Uses `transitionMu`. |
| `internal/tui/app_handlers_misc.go:120-126` | `SetPhaseModel()` | **Update()** — message handler | **SAFE** | Uses `perPhaseModelsMu`. |
| `internal/tui/app.go:196` | type assertion to `*workflow.Engine` | **Init()** — startup | **SAFE** | One-time setup during `initWorkflowEngine()`. |
| `internal/tui/app.go:1146` | `SnapshotDecisions()` | **View()** — direct engine call during render | **UNSAFE** | Same as line 1146 in app_view.go (same function). |

---

## Detailed Findings

### UNSAFE #1: `renderDecisionsContent()` calls `SnapshotDecisions()` from `View()`

**Location:** `internal/tui/app_view.go:1146` (inside `renderDecisionsContent`)

```go
decisions := decision.RedactSlice(m.workflowEngine.SnapshotDecisions())
```

- **Thread context:** `View()` runs on Bubble Tea's main render thread.
- **Writer:** `decision.Logger.drain()` goroutine (started in `NewLogger()`) writes to `flushed` slice and ring buffer under `flushMu` + `ringMu`.
- **Reader:** `Snapshot()` acquires both mutexes — no data race per se.
- **Problem:** Violates Elm/Bubble Tea architecture. State must flow: workflow goroutine → `tea.Msg` (via channel emitter) → `Update()` → AppState fields → `View()` reads AppState. Here, `View()` reaches back into the engine directly.
- **Risk:** Stale decisions displayed if `View()` renders while `drain()` is mid-append. Also, if `decisionLog` is `nil` (early startup), returns `nil` — but that's handled.

### UNSAFE #2: `PlanContent()` / `PlanVersion()` read `planMarkdown` / `planVersion` without synchronization

**Location:** `internal/tui/app_update_phase.go:201-202` (inside `handlePhaseResultMsg`)

```go
m.planModel.SetPlanContent(m.workflowEngine.PlanContent())
m.planModel.SetPlanVersion(m.workflowEngine.PlanVersion())
```

- **Thread context:** `Update()` handler (Bubble Tea main thread).
- **Writer:** Workflow goroutine in `runPlan()` (and `runDiscuss()` via `FinalizeDiscuss()`) writes:
  - `e.state.planMarkdown = ...` (string)
  - `e.state.planVersion++` (int)
- **Synchronization:** Only `transitionMu` guards phase transitions. **No mutex protects `planMarkdown` or `planVersion`.**
- **Risk:** Data race under `-race`. Torn reads of string header (len/ptr) or int. Stale plan content/version displayed to user.
- **Note:** `DiscussState()` returns a **value copy** of the struct, so it's safe. But `PlanContent()` returns `string` (by value, but string header can race) and `PlanVersion()` returns `int` (can race on 32-bit, though Go 1.25+ is 64-bit only).

---

## Unclear / Requires Tracing

| file:line | field / method | why unclear |
|-----------|----------------|-------------|
| `internal/tui/app_update_commands.go:446` | type assertion to `*workflow.Engine` | Only used to call `SetModel()` which is mutex-protected. Safe if assertion succeeds. |
| `internal/tui/app.go:764` | `SetModel()` from `checkAutoArbitrage()` | Called from `checkAutoArbitrage()` which is invoked from `RunPhaseCmd()` → phase goroutine. **Wait** — `checkAutoArbitrage()` is called from `RunPhaseCmd()` which returns a `tea.Cmd` that runs in a goroutine! Let me re-check... |

**Correction on `checkAutoArbitrage`:**
- `RunPhaseCmd()` (app.go:315) creates `phaseCmd` that runs `engine.RunPhase()` in a goroutine.
- `checkAutoArbitrage()` is called **before** returning the `tea.Batch(phaseCmd, m.drainEmitterCmd())` — so it runs on the Bubble Tea main thread (inside `Update()` via the `GoalSubmittedMsg` or similar handler).
- `SetModel()` uses `modelIDMu` — safe.

---

## Race Detector Coverage Gap

**No test exists** that exercises:
- A workflow phase transition (`Transition()`) happening concurrently with
- LLM streaming messages being appended to `WorkflowState.Messages` (via `emit()` → channel → `drainEmitterCmd()` → `Update()`)

The `Messages` slice in `WorkflowState` is written from the workflow goroutine (via `emit()` → `MsgEmitter` → channel → TUI `Update()` handler) and also read by the engine during `preflightContextCheck()` and `consumeStream()`. No mutex guards `Messages` — only the channel provides ordering.

**Recommended test:** Start a workflow, trigger a phase transition, and simultaneously stream a large LLM response that appends messages. Run under `go test -race ./internal/workflow ./internal/tui`.

---

## Recommendations (Not Fixes — Per Audit Scope)

1. **Move `SnapshotDecisions()` data into a `tea.Msg`** — Have the workflow engine emit `DecisionsSnapshotMsg` periodically or on demand; TUI stores in `AppState.decisions` and `View()` reads that.

2. **Protect `planMarkdown` and `planVersion` with a mutex** — Add `planMu sync.RWMutex` to `WorkflowState`; guard reads in `PlanContent()`/`PlanVersion()` and writes in `runPlan()`/`SetRefinementFeedback()`.

3. **Add race test** — `TestConcurrentPhaseTransitionAndStreaming` in `engine_messages_test.go` or `integration_test.go`.

4. **Consider making `WorkflowState.Messages` access serialized** — Either via mutex or by ensuring all access goes through the emitter channel (current design intent).