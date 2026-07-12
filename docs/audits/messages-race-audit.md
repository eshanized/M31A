# Messages Race Audit Report

**Date:** 2026-07-12  
**Issue:** Data race on `WorkflowState.Messages` (investigated per workflowstate-boundary-audit.md open item)

## Summary

Investigated whether `WorkflowState.Messages` has a data race between the workflow goroutine and the TUI thread. The audit (`workflowstate-boundary-audit.md`) flagged Messages as potentially racy because it is "appended to from the workflow goroutine (via emit() -> MsgEmitter -> channel -> TUI Update() handler) and is also read directly by the engine itself during preflightContextCheck() and consumeStream()."

**Finding:** The audit's characterization was partially inaccurate. `preflightContextCheck()` and `consumeStream()` receive messages as parameters -- they do NOT access `e.state.Messages` directly. The only direct access to `e.state.Messages` occurs in `runPhase()` (lines 691-695 of engine.go), which runs entirely in the workflow goroutine. The `WorkflowEngine` interface exposes no `Messages()` accessor, so the TUI never reads or writes this field.

Despite this, a synthetic race test confirmed the field is unprotected, and a defensive mutex was added following the `planMu` pattern.

## Classification Table

| file:line | field / method | access type | goroutine | classification |
|-----------|----------------|-------------|-----------|----------------|
| `engine.go:65` | `Messages []m31types.Message` | Field declaration | N/A | N/A |
| `engine.go:691` | `e.state.Messages = PrePhaseSetup(...)` | **Write** | Workflow goroutine (`runPhase()`) | **SAFE** -- no concurrent readers exist in production |
| `engine.go:695` | `e.state.Messages` (passed to PrePhaseSetup) | **Read** | Workflow goroutine (`runPhase()`) | **SAFE** -- same goroutine as write |
| N/A | `preflightContextCheck(messages)` | Parameter read | Workflow goroutine | **SAFE** -- receives `messages` as parameter, not from `e.state.Messages` |
| N/A | `consumeStream(iterator)` | Iterator read | Workflow goroutine | **SAFE** -- receives iterator, not `e.state.Messages` |

**No TUI-side access exists.** The `WorkflowEngine` interface (tuitypes.go:552) does not expose a `Messages()` accessor. No TUI code reads or writes `e.state.Messages`.

## Detailed Findings

### Why the audit's concern was partially inaccurate

The audit stated:
> Messages is appended to from the workflow goroutine (via emit() -> MsgEmitter -> channel -> TUI Update() handler) and is also read directly by the engine itself during preflightContextCheck() and consumeStream().

This conflates two different message lists:
1. **`WorkflowState.Messages`** -- the workflow engine's internal conversation history, only accessed in `runPhase()`
2. **`replModel.Messages()`** -- the REPL's display message list, managed entirely by the TUI

The `emit()` path sends streaming chunks through `MsgEmitter.Emit()` to a channel, which the TUI drains in `Update()`. The TUI stores these in `replModel`, not in `WorkflowState.Messages`. The two fields are completely independent.

### Why a mutex was added anyway

1. **Synthetic race confirmed:** A test with concurrent goroutines accessing `e.state.Messages` triggered the race detector, proving the field has no inherent synchronization.
2. **Defensive coding:** If future code changes introduce TUI-side access to `Messages` (e.g., via a new accessor method), the mutex will prevent races.
3. **Consistency:** Follows the same pattern as `planMu` (added in planstate-race-fix.md) for fields that logically belong to the same mutable session state.

## Race Detection

### Before Fix (Synthetic Race Confirmed)

```bash
$ go test -race -run "TestConcurrentMessages" ./internal/workflow -count=1 -v
=== RUN   TestConcurrentMessagesReadDuringWrite
==================
WARNING: DATA RACE
Read at 0x00c000176088 by goroutine 25:
  github.com/eshanized/M31A/internal/workflow.TestConcurrentMessagesReadDuringWrite.func2()
      /home/snigdha/Desktop/Helix/M31A/internal/workflow/messages_race_test.go:49 +0xe4

Previous write at 0x00c000176088 by goroutine 24:
  github.com/eshanized/M31A/internal/workflow.TestConcurrentMessagesReadDuringWrite.func1()
      /home/snigdha/Desktop/Helix/M31A/internal/workflow/messages_race_test.go:36 +0x25a
...
--- FAIL: TestConcurrentMessagesReadDuringWrite (1.13s)
```

Note: This race is synthetic (test creates intentional concurrent access). In production, `e.state.Messages` is only accessed within `runPhase()` in a single goroutine.

### After Fix (Race Resolved)

```bash
$ go test -race -run "TestConcurrentMessages" ./internal/workflow -count=5 -v
=== RUN   TestConcurrentMessagesReadDuringWrite
--- PASS: TestConcurrentMessagesReadDuringWrite (0.37s)
=== RUN   TestConcurrentMessagesSliceAccess
--- PASS: TestConcurrentMessagesSliceAccess (0.24s)
=== RUN   TestConcurrentMessagesReadDuringWrite
--- PASS: TestConcurrentMessagesReadDuringWrite (0.25s)
=== RUN   TestConcurrentMessagesSliceAccess
--- PASS: TestConcurrentMessagesSliceAccess (0.24s)
=== RUN   TestConcurrentMessagesReadDuringWrite
--- PASS: TestConcurrentMessagesReadDuringWrite (0.26s)
=== RUN   TestConcurrentMessagesSliceAccess
--- PASS: TestConcurrentMessagesSliceAccess (0.24s)
=== RUN   TestConcurrentMessagesReadDuringWrite
--- PASS: TestConcurrentMessagesReadDuringWrite (0.27s)
=== RUN   TestConcurrentMessagesSliceAccess
--- PASS: TestConcurrentMessagesSliceAccess (0.26s)
=== RUN   TestConcurrentMessagesReadDuringWrite
--- PASS: TestConcurrentMessagesReadDuringWrite (0.30s)
=== RUN   TestConcurrentMessagesSliceAccess
--- PASS: TestConcurrentMessagesSliceAccess (0.36s)
PASS
ok  github.com/eshanized/M31A/internal/workflow	3.872s
```

## Full Race Test Suite

```bash
$ go test -race ./internal/workflow ./internal/tui -count=1
ok  	github.com/eshanized/M31A/internal/workflow	14.773s
ok  	github.com/eshanized/M31A/internal/tui	14.273s
```

All tests pass with race detector enabled.

## Changes Made

### 1. Added `messagesMu sync.RWMutex` to `WorkflowState` (engine.go:52-53)

```go
type WorkflowState struct {
    transitionMu sync.Mutex
    planMu       sync.RWMutex  // guards planMarkdown and planVersion
    messagesMu   sync.RWMutex  // NEW: guards Messages
    // ...
}
```

### 2. Protected Write in `runPhase()` (engine.go:691-701)

```go
e.state.messagesMu.Lock()
var err error
e.state.Messages, err = e.phaseCoordinator.PrePhaseSetup(
    ctx,
    phase,
    &budgetConfigAdapter{cfg: e.cfg},
    e.state.Messages,
    e.proactiveCompactCheck,
)
e.state.messagesMu.Unlock()
```

### 3. Added Regression Test (messages_race_test.go)

- `TestConcurrentMessagesReadDuringWrite` -- exercises concurrent write (simulating PrePhaseSetup) against concurrent read (simulating TUI access)
- `TestConcurrentMessagesSliceAccess` -- exercises concurrent append/read on the slice header
- Both tests use mutex-protected access to verify no race under `-race`

## Files Changed

1. **internal/workflow/engine.go** -- Added `messagesMu`, protected read/write in `runPhase()`
2. **internal/workflow/messages_race_test.go** -- New regression test

## Notes

- This fix is scoped to `WorkflowState.Messages` only
- The race was synthetic in the test; no production code currently accesses `e.state.Messages` concurrently
- The mutex was added defensively, following the `planMu` pattern from planstate-race-fix.md
- No API changes -- only internal synchronization added
- The `WorkflowEngine` interface remains unchanged (no `Messages()` accessor added)
