# Plan 15-06: Workflow Engine Correctness — SUMMARY

**Phase:** 15-comprehensive-audit-fixes  
**Plan:** 06  
**Wave:** 2  
**Status:** COMPLETE  
**Commit:** `a2b4fa1`

---

## Findings Addressed

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| H-1 | High | `parseToolCalls` returns `nil, nil` on malformed JSON | FIXED — returns `ErrToolExecution` |
| H-2 | High | `extractJSONObject` doesn't strip comments | ALREADY FIXED (15-05) |
| H-3 | High | `TaskRunner.Schedule()` cycle error is string, not typed | ALREADY FIXED (existing `ErrCircularDependency`) |
| H-4 | High | Plan validator doesn't check unique IDs / dep existence | ALREADY FIXED (existing `validateTasks`) |
| H-6 | High | Phase transition guard is missing | FIXED — `validPhaseTransitions` map in `Transition()` |
| H-15 | High | No checkpoint before each task in Execute | FIXED — pre-task `SaveCheckpoint` in execute loop |
| H-16 | High | Ledger append is non-atomic (O_APPEND) | FIXED — `Append` now calls `rewriteFile` (temp+rename) |
| H-17 | High | `Session.Load` doesn't validate required fields | FIXED — validates ID, StartedAt, WorkflowPhase |
| H-18 | High | Ledger overwrites duplicate session entries silently | FIXED — returns `ErrTaskFailed` on duplicate |
| M-3 | Medium | `parseToolCalls` doesn't handle single object `{...}` | FIXED — `parseSingleToolCall` handles both forms |
| M-4 | Medium | Plan validator doesn't check self-referential deps | ALREADY FIXED (existing `validateTasks`) |
| M-5 | Medium | Dead `toolCallCount` variable | ALREADY USED (not dead) |

## Changes Made

### Files Modified

| File | Changes |
|------|---------|
| `internal/workflow/engine.go` | Added `validPhaseTransitions` map, phase guard in `Transition()`, `parseToolCalls` returns `ErrToolExecution` on malformed JSON, `parseSingleToolCall` returns errors + supports single object form |
| `internal/workflow/execute.go` | Added pre-task `SaveCheckpoint` call in execute loop |
| `internal/workflow/engine_test.go` | Added `APIKey()` to mock provider |
| `internal/tui/app_update.go` | Added nil guard for `dispatcher` in `QuestionResponseMsg` handler |
| `pkg/ledger/ledger.go` | `Append` now uses `rewriteFile` (atomic) instead of `O_APPEND`; returns `ErrTaskFailed` on duplicate session ID |
| `pkg/ledger/ledger_test.go` | Updated `TestAppend_Dedup` to expect typed error |
| `pkg/session/manager.go` | `LoadSession` validates ID non-empty, StartedAt non-zero, WorkflowPhase known |

### Files Created

| File | Purpose |
|------|---------|
| `internal/workflow/workflow_test.go` | 15 regression tests for all 15-06 findings |

## Test Results

- `go build ./...` — PASS
- `go vet ./...` — PASS
- `go test -count=1 -race ./...` — ALL PASS

## Key Decisions

- **Phase transition guard uses a map** rather than a switch for clarity and extensibility.
- **Pre-task checkpoint reuses existing `SaveCheckpoint`** with `TaskCount` field for task identification.
- **Ledger atomicity uses `rewriteFile`** (full in-memory rewrite + temp+rename) instead of O_APPEND.
- **`parseSingleToolCall` now returns `(ToolCall, error)`** — callers in `parseToolCalls` handle errors.
- **Dispatcher nil guard** added for `QuestionResponseMsg` to prevent panic during early init.
