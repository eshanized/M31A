# Fix Verification Report

**Date:** 2026-07-22
**Plan:** 02-03

## Summary

All critical hanging tests identified in hanging-test-analysis.md (Plan 02-01) have been fixed. Full test suite runs with `-race -timeout 30s` and previously hanging tests now pass.

## Fixes Applied

### 1. TestDispatcher_PermissionAllowed_NoError (internal/tools/toolinput_test.go)

**Root cause:** The test tried to re-register a `Bash` tool that was already registered by `DefaultDispatcher`. The registration failed silently, and the original Bash tool with empty `workDir` was used. When executed, the Bash tool's `applyLandlock` restricted the test process's filesystem access, causing the test to hang.

**Fix:** Unregistered the original Bash tool and replaced with a mock tool that returns output without subprocess execution. The mock has the same name and risk level to test the permission flow correctly.

### 2. TestDispatcher_SyncTodoFromTasks_NilTodoWrite (internal/tools/coverage_boost_test.go)

**Root cause:** The DefaultDispatcher creates TodoWrite with an empty sessionID. When `SyncTodoFromTasks` calls `writeTodoFile`, the sessionID regex validation (`^[a-zA-Z0-9_-]+$`) fails on the empty string.

**Fix:** Set a valid sessionID ("test-session") on the TodoWrite tool before calling SyncTodoFromTasks.

### 3. TestBisect_Successful (internal/engine/bisect/bisect_test.go)

**Root cause:** Two issues:
1. The `parseBisectLog` function only matched `"# first bad commit:"` but git bisect outputs `"# first 'bad' commit:"` (with quotes around 'bad'). The bisect loop never detected completion.
2. No iteration limit — the loop ran indefinitely checking the same commit.

**Fix:**
1. Updated `parseBisectLog` to match both quoted and unquoted variants.
2. Updated loop condition to check for both `"first bad commit"` and `"first 'bad' commit"`.
3. Added `maxIterations = 50` limit to prevent infinite loops.
4. Added timeout guard in the test (25s) as a safety net.

### 4. E2E Tests — CI Detection (tests/e2e/e2e_test.go)

**Added:** `ci.SkipIfCI` calls to `TestBinary_Prompt_NvidiaRealAPI`, `TestBinary_Prompt_ZenRealAPI`, and `TestBinary_Prompt_OpenRouterRealAPI` to skip external API tests in CI environments per D-08, D-15.

## Test Results

### Previously Hanging Tests (All Pass)
- `TestDispatcher_PermissionAllowed_NoError` — PASS (0.00s)
- `TestDispatcher_SyncTodoFromTasks_NilTodoWrite` — PASS (0.00s)
- `TestBisect_Successful` — PASS (0.05s)

### Full Test Suite Results (pre-existing failures only)
- `internal/tools` — 7 `TestCheckDangerousCommand_*` failures (pre-existing, security pattern matching)
- `internal/engine/workflow` — timeout (pre-existing)
- `tests/e2e` — binary build path mismatch (pre-existing, `tests/e2e/cmd/m31a` doesn't exist)

### Packages Verified Clean
- `internal/engine/bisect` — all pass
- `internal/testutil/ci` — all pass
- `internal/testutil/testtimeout` — all pass
- `internal/engine/rollback` — all pass
- `internal/engine/taskrunner` — all pass
