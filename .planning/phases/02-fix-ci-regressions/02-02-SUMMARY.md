# Phase 2, Plan 02 - Summary

**Phase:** 02-fix-ci-regressions  
**Plan:** 02-02  
**Wave:** 2  
**Status:** ✅ COMPLETE

## Objective
Fix Root Cause C (workflow engine RunPhase transition enforcement, 19 tests) and the execute.go:571 lint error. Add test-only RunPhaseDirect bypass per D-01/D-02, fix the heal loop ineffectual assignment per D-08, and fix the nil pointer dereference in TestHandlePhase_InvalidPhaseName.

## Changes Made

### Commit 1: `fix(workflow): add RunPhaseDirect for test bypass`
**File:** `internal/engine/workflow/engine.go`

- Added `RunPhaseDirect` method (lines 916-971) after `RunPhase`
- Skips `stateMachine.Transition()` validation — allows tests to run any phase directly from Initialize
- Clearly documented as "INTENDED FOR TEST USE ONLY"
- Production `RunPhase` still enforces transitions via `stateMachine.Transition()`

**Updated 18 tests to use RunPhaseDirect:**
- `engine_extra_test.go`: 3 tests (TestRunPlan_WithRefinement, TestRunVerify_SkipsNonDoneTasks, TestEngineTransitionMultiplePhases)
- `plan_test.go`: 5 tests (TestEngine_RunPlan_Success, TestEngine_RunPlan_ParsesJSONFromMarkdown, TestEngine_Plan_SavesTasks, TestEngine_RunPlan_RetryWithErrorFeedback, TestEngine_RunPlan_ManualFallback)
- `ship_test.go`: 6 tests (TestEngine_RunShip, TestEngine_RunShip_NoTasks, TestEngine_RunShip_NoGit, TestEngine_RunShip_WithFailedTasks, TestEngine_RunShip_WithSkippedTasks, TestShip_SaveStateBeforeArchive)
- `verify_test.go`: 4 tests (TestEngine_RunVerify_NoTasks, TestEngine_RunVerify_WithDoneTasks, TestEngine_RunVerify_MissingFile, TestEngine_RunVerify_SkipsPendingTasks)

**Fix TestHandlePhase_InvalidPhaseName nil pointer:**
- Fixed in `internal/ui/tui/commands/commands_all_test.go`
- Changed zero-value `session.Manager{}` to properly initialized manager using `session.NewManager()`

### Commit 2: `fix(workflow): consume proactiveCompactCheck result in heal loop`
**File:** `internal/engine/workflow/execute.go`

- Hoisted `messages` variable declaration outside the heal loop (line 244)
- Changed loop assignment from `:=` to `=` to preserve compaction result across iterations
- Added `e.state.Messages = messages` write-back at end of iteration
- Fixes `golangci-lint` ineffectual assignment error at execute.go:571

## Test Results

All 19 workflow transition tests pass with `-race` flag:

### Workflow Transition Tests (18 tests)
- ✅ `TestEngine_RunPlan_Success`
- ✅ `TestEngine_RunPlan_ParsesJSONFromMarkdown`
- ✅ `TestEngine_Plan_SavesTasks`
- ✅ `TestEngine_RunPlan_RetryWithErrorFeedback`
- ✅ `TestEngine_RunPlan_ManualFallback`
- ✅ `TestEngine_RunShip`
- ✅ `TestEngine_RunShip_NoTasks`
- ✅ `TestEngine_RunShip_NoGit`
- ✅ `TestEngine_RunShip_WithFailedTasks`
- ✅ `TestEngine_RunShip_WithSkippedTasks`
- ✅ `TestShip_SaveStateBeforeArchive`
- ✅ `TestEngine_RunVerify_NoTasks`
- ✅ `TestEngine_RunVerify_WithDoneTasks`
- ✅ `TestEngine_RunVerify_MissingFile`
- ✅ `TestEngine_RunVerify_SkipsPendingTasks`
- ✅ `TestRunPlan_WithRefinement`
- ✅ `TestRunVerify_SkipsNonDoneTasks`
- ✅ `TestEngineTransitionMultiplePhases`

### Nil Pointer Test
- ✅ `TestHandlePhase_InvalidPhaseName` — no panic

### Lint Fix
- ✅ `golangci-lint run ./internal/engine/workflow/` — ineffectual assignment fixed (only pre-existing issues remain)

### Proactive Compaction Test
- ✅ `TestProactiveCompactCheck_ResultApplied` — passes with `-race`

### Full Package Tests
- ✅ `go test -race ./internal/engine/workflow/` — all pass
- ✅ `go test -race ./internal/core/config/ ./internal/engine/session/ ./internal/engine/workflow/ ./internal/testutil/ci/ ./internal/tools/` — all pass

## Verification Checklist

- [x] `go build ./...` — clean compilation
- [x] `go vet ./...` — no issues
- [x] `golangci-lint run` — clean (only pre-existing issues remain)
- [x] `go test -race ./internal/engine/workflow/` — all pass (19 tests)
- [x] 2 atomic commits matching D-13 convention
- [x] No adjacent cleanups or refactors (per D-08)
- [x] All 19 workflow engine tests pass (18 transition + 1 nil pointer)
- [x] `golangci-lint run` clean (execute.go:571 fixed)

## Next Steps
Proceed to Wave 3: Plan 02-03 (Root Cause D: TestIsCI race + Root Cause F: AskUserQuestion timeout)