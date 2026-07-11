# PlanState Race Fix Report

**Date:** 2026-07-11  
**Issue:** Data race on `WorkflowState.planMarkdown` and `WorkflowState.planVersion`

## Summary

Fixed a confirmed data race in the workflow engine where `planMarkdown` (string) and `planVersion` (int) were written by the workflow goroutine but read by the TUI thread via `Engine.PlanContent()` and `Engine.PlanVersion()` without synchronization.

## Race Detection

### Before Fix (Race Confirmed)

```bash
$ go test -race -run TestConcurrentPlanReadDuringWrite ./internal/workflow -count=5
=== RUN   TestConcurrentPlanReadDuringWrite
==================
WARNING: DATA RACE
Write at 0x00c0000de008 by goroutine 24:
  github.com/eshanized/M31A/internal/workflow.TestConcurrentPlanReadDuringWrite.func1()
      /home/snigdha/Desktop/Helix/M31A/internal/workflow/plan_race_test.go:95 +0x19b

Previous read at 0x00c0000de008 by goroutine 25:
  github.com/eshanized/M31A/internal/workflow.(*Engine).PlanContent()
      /home/snigdha/Desktop/Helix/M31A/internal/workflow/engine.go:1110 +0xbe
  github.com/eshanized/M31A/internal/workflow.TestConcurrentPlanReadDuringWrite.func2()
      /home/snigdha/Desktop/Helix/M31A/internal/workflow/plan_race_test.go:105 +0xa2
...
--- FAIL: TestConcurrentPlanReadDuringWrite (0.55s)
```

The race was detected on both fields:
- `planMarkdown` (string header: len/ptr) - torn reads possible
- `planVersion` (int) - data race on concurrent read/write

### After Fix (Race Resolved)

```bash
$ go test -race -run TestConcurrentPlanReadDuringWrite ./internal/workflow -count=5
=== RUN   TestConcurrentPlanReadDuringWrite
--- PASS: TestConcurrentPlanReadDuringWrite (0.21s)
=== RUN   TestConcurrentPlanReadDuringWrite
--- PASS: TestConcurrentPlanReadDuringWrite (0.20s)
=== RUN   TestConcurrentPlanReadDuringWrite
--- PASS: TestConcurrentPlanReadDuringWrite (0.16s)
=== RUN   TestConcurrentPlanReadDuringWrite
--- PASS: TestConcurrentPlanReadDuringWrite (0.22s)
=== RUN   TestConcurrentPlanReadDuringWrite
--- PASS: TestConcurrentPlanReadDuringWrite (0.21s)
PASS
ok  	github.com/eshanized/M31A/internal/workflow	2.055s
```

## Changes Made

### 1. Added `planMu sync.RWMutex` to `WorkflowState` (engine.go:47)

```go
type WorkflowState struct {
	transitionMu sync.Mutex
	planMu       sync.RWMutex  // NEW: guards planMarkdown and planVersion
	planMarkdown string
	planVersion  int
	// ...
}
```

### 2. Protected Read Methods (engine.go:1116-1126)

```go
func (e *Engine) PlanContent() string {
	e.state.planMu.RLock()
	defer e.state.planMu.RUnlock()
	return e.state.planMarkdown
}

func (e *Engine) PlanVersion() int {
	e.state.planMu.RLock()
	defer e.state.planMu.RUnlock()
	return e.state.planVersion
}
```

### 3. Protected Write Sites

| Location | Method | Protection |
|----------|--------|------------|
| plan.go:160-165 | `runPlan()` | `planMu.Lock()/Unlock()` around `planMarkdown` + `planVersion` |
| engine.go:358-360 | `LoadCheckpointData()` | `planMu.Lock()/Unlock()` around `planVersion` |
| engine.go:1099-1101 | `SetRefinementFeedback()` | `planMu.Lock()/Unlock()` around `planVersion++` |

### 4. Added Regression Test (plan_race_test.go)

```go
func TestConcurrentPlanReadDuringWrite(t *testing.T) {
	// Writer: uses mutex-protected write (simulating fixed workflow)
	// Reader: calls PlanContent()/PlanVersion() (mutex-protected read)
	// Verifies no race under -race
}
```

## Full Race Test Suite

```bash
$ go test -race ./internal/workflow ./internal/tui -count=1
ok  	github.com/eshanized/M31A/internal/workflow	8.567s
ok  	github.com/eshanized/M31A/internal/tui	4.518s
```

All tests pass with race detector enabled.

## Files Changed

1. **internal/workflow/engine.go** - Added `planMu`, protected reads/writes
2. **internal/workflow/plan.go** - Protected writes in `runPlan()`
3. **internal/workflow/plan_race_test.go** - New regression test

## Notes

- This fix is scoped to `planMarkdown` and `planVersion` only
- `WorkflowState.Messages` and `decisionLog` snapshot race (from workflowstate-boundary-audit.md) are **not** addressed here
- The fix follows the existing pattern using `transitionMu` for phase transitions
- No API changes - only internal synchronization added