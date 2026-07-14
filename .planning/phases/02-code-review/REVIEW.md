---
phase: 02-code-review
reviewed: 2026-07-14T05:40:00Z
depth: deep
files_reviewed: 7
files_reviewed_list:
  - internal/tools/dispatcher_test.go
  - internal/tools/dispatcher.go
  - internal/tools/extra_test.go
  - internal/tools/git.go
  - internal/tui/emitter_stress_test.go
  - internal/tui/phase_transition_model.go
  - internal/provider/mock/streaming.go
findings:
  critical: 1
  warning: 4
  info: 3
  total: 8
status: issues_found
---

# Phase 2: Code Review Report

**Reviewed:** 2026-07-14T05:40:00Z
**Depth:** deep
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Review focused on test coverage gaps, copylocks bug, error handling patterns, and correctness issues across the tools and TUI subsystems. All test runs passed (disk quota failures in race-detector run are environment-specific, not code defects). One confirmed `go vet` copylocks violation, zero test files for `git.go` and `phase_transition_model.go`, and a logic bug in `extractCommitMessage` that silently truncates multi-word commit messages.

## Critical Issues

### CR-01: `extractCommitMessage` truncates multi-word commit messages

**File:** `internal/tools/git.go:527-554`
**Issue:** `strings.Fields(args)` splits on all whitespace, so a commit invocation like `Git{operation:"commit", args:"-m \"fix login bug\""}` produces fields `["-m", "\"fix", "login", "bug\""]`. The function returns only `"fix` (the second field), silently dropping the rest of the message. Every commit with a multi-word message will have its message truncated to a single word.
**Fix:**
```go
// Replace strings.Fields with a proper quoted-string parser, or restructure to
// accept the message as a separate parameter instead of parsing it from args.
// For example, use a simple state-machine parser:
func extractCommitMessage(args string) string {
    // ... find -m flag index ...
    // Then take everything after -m as the message, stripping surrounding quotes
    // rather than taking only the next field.
}
```

## Warnings

### WR-01: `newStreamingPlanProvider` passes `sync.Mutex` by value (go vet copylocks)

**File:** `internal/tui/emitter_stress_test.go:61`
**Issue:** `func newStreamingPlanProvider(cfg mock.StreamingMockProvider)` accepts the struct by value. `mock.StreamingMockProvider` contains a `sync.Mutex` field (`mu` at `internal/provider/mock/streaming.go:34`). Copying the struct copies the mutex, which is undefined behavior per Go's sync package contract and flagged by `go vet`. All callers at lines 259, 325, 396 pass a struct literal, but the copy still occurs on function entry. If the concurrency limit is ever tested from a concurrent goroutine, this would produce data races or deadlocks.
**Fix:**
```go
func newStreamingPlanProvider(cfg *mock.StreamingMockProvider) *streamingPlanProvider {
    return &streamingPlanProvider{
        streaming: cfg,
    }
}
// Update all call sites to pass &mock.StreamingMockProvider{...}
```

### WR-02: `git.go` has zero test coverage

**File:** `internal/tools/git.go` (554 lines)
**Issue:** The `Git` tool is registered in the default dispatcher (verified in `TestDispatcher_DefaultDispatcher` at line 207) but has no dedicated test file (`git_test.go` does not exist). The tool exposes 8 operations (`add`, `commit`, `diff`, `log`, `branch`, `checkout`, `stash`, `status`), each with non-trivial output parsing logic. The `gitStatus` parser at lines 460-479 classifies files by two-character status codes -- an off-by-one in the switch conditions could misclassify staged vs. modified files. The `extractCommitMessage` bug (CR-01) above went undetected precisely because of this coverage gap.
**Fix:** Create `internal/tools/git_test.go` covering at minimum: each operation with a real temp git repo, `extractCommitMessage` with multi-word messages, `gitStatus` parsing for all status code combinations, error paths (git not installed, repo not initialized).

### WR-03: `PhaseTransitionModel` has zero test coverage

**File:** `internal/tui/phase_transition_model.go` (224 lines)
**Issue:** No test file exists. The model implements `Update()` with keyboard navigation (up/down/enter/esc/y/n/b), `View()` rendering, and `phaseTransitionSummary()` with a switch on all workflow phases. The cursor bounds check (`cursor < 2` at line 53) allows cursor values 0, 1, 2 which matches the 3 options -- correct but untested. The `phaseTransitionSummary` function at line 203 has a `default` case that returns empty string for `PhaseInitialize` and `PhaseShip`, which may silently produce blank UI in production. `transition_extra_test.go` only tests helper functions (padFrameLines, renderSlide, etc.), not the model itself.
**Fix:** Create `internal/tui/phase_transition_model_test.go` covering: keyboard navigation (up, down, enter on each option, esc, y, n, b), cursor bounds, View() rendering, phaseTransitionSummary for all phase values.

### WR-04: Multiple `Dispatcher` methods lack unit test coverage

**File:** `internal/tools/dispatcher.go` (lines 193, 209, 369, 377, 385, 392)
**Issue:** The following exported/public methods have no dedicated tests: `SetCollector` (line 193), `Unregister` (line 209), `SetTodoWriteCallback` (line 369), `SyncTodoFromTasks` (line 377), `SetOutputStore` (line 385), `workDir` (line 392). While some are covered incidentally through integration paths, `Unregister` has no test verifying that a previously registered tool is actually removed and returns `false` from `GetTool`. `SyncTodoFromTasks` has error-path logic that is entirely untested.
**Fix:** Add tests for `Unregister` (register -> unregister -> verify GetTool returns false), `SetOutputStore` (verify output bounding), `SyncTodoFromTasks` with a configured TodoWrite.

## Info

### IN-01: `TestCheckPermission_ToolFilter` has duplicate assertions

**File:** `internal/tools/dispatcher_test.go:618-630`
**Issue:** Lines 624-629 duplicate the exact same assertions as lines 618-623 (`if pctx == nil` and `if pctx.Source != "risk_level"`). This is dead test code that adds noise without additional coverage.
**Fix:** Remove lines 624-630.

### IN-02: `git.go` accepts unvalidated path arguments

**File:** `internal/tools/git.go:82-89`
**Issue:** The `paths` parameter from `input.Params["paths"]` is passed directly to `exec.CommandContext` args without any path sanitization (no `filepath.Clean`, no `..` rejection). While the tool is rated `RiskDangerous` and requires permission, unlike `Bash` and `FileWrite`, there is no explicit `isPathSafe` check. The LLM could craft paths like `../../etc/passwd` for `git add` or `git diff`. This is mitigated by the permission system (RiskDangerous requires approval) but is inconsistent with other tools that perform explicit path checks.
**Fix:** Consider adding path validation consistent with `FileWrite`/`FileRead`, or document that `git` operations are intentionally unrestricted within the repo context.

### IN-03: `Dispatcher.SetCollector` acquires two locks sequentially

**File:** `internal/tools/dispatcher.go:193-205`
**Issue:** `SetCollector` acquires `d.mu.Lock()` (line 194), releases it (line 196), then acquires `d.mu.RLock()` (line 198). Between unlock and RLock, another goroutine could call `SetCollector` concurrently, causing a TOCTOU issue where the collector is set to a different value. In practice this is a startup-only call, so the risk is negligible, but the pattern is inconsistent with the single-lock-with-defer pattern used elsewhere.
**Fix:** Use a single `d.mu.Lock()` with `defer d.mu.Unlock()` covering both the assignment and the propagation loop, using `d.mu.RUnlock()` + `d.mu.RLock()` inside the loop if needed. Alternatively, document the initialization-only constraint.

---

_Reviewed: 2026-07-14T05:40:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
