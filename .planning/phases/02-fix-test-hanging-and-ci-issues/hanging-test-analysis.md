# Hanging Test Analysis - Phase 02

Generated from test run with `go test -v -timeout 30s -race ./...` on 2026-07-23.

## Hanging Tests

| Package | Test Name | Symptom | Suspected Root Cause |
|---------|-----------|---------|---------------------|
| internal/engine/bisect | TestBisect_Successful | Infinite loop checking same commit repeatedly | Bisect algorithm loops on same commit without progress; likely incorrect commit traversal or termination condition |

## Flaky Tests

| Package | Test Name | Failure Mode | Notes |
|---------|-----------|--------------|-------|
| internal/tools | TestDispatcher_SyncTodoFromTasks_NilTodoWrite | Panic: invalid session ID | Test expects nil todo write but gets invalid session |
| internal/tools | TestDispatcher_PermissionAllowed_NoError | Timeout: timed out waiting for Execute to return | Goroutine leak or channel deadlock in permission flow |
| internal/tools | TestNewDispatcherFactory_WithManager | Panic: nil pointer dereference in agent.go:277 | Factory returns nil dispatcher in some error path |

## Tests with External Resource Dependencies (Need CI Detection)

| Package | Test Name | External Dependency | Reason for CI Skip |
|---------|-----------|---------------------|-------------------|
| tests/e2e | TestBinary_Prompt_NvidiaRealAPI | NVIDIA_API_KEY | Requires NVIDIA API access |
| tests/e2e | TestBinary_Prompt_ZenRealAPI | ZEN_API_KEY | Requires Zen API access |
| tests/e2e | TestBinary_Prompt_OpenRouterRealAPI | OPENROUTER_API_KEY | Requires OpenRouter API access |

## Tests Failing Due to Missing Build Artifacts

| Package | Test Name | Issue |
|---------|-----------|-------|
| tests/e2e | TestBinary_Version | Test binary not built - expects cmd/m31a in tests/e2e/ |
| tests/e2e | TestBinary_Help | Test binary not built |
| tests/e2e | TestBinary_Prompt_NoProvider | Test binary not built |
| tests/e2e | TestBinary_Prompt_Timeout | Test binary not built |

## Race Conditions Detected

None found in passing tests. Hanging tests prevented race detection in bisect and dispatcher packages.

## Recommended Fixes (for Plan 02-03)

### 1. Fix Bisect Hanging Test (HIGH PRIORITY - D-09)
**File:** `internal/engine/bisect/bisect_test.go`
- Add `context.WithTimeout` with 30s deadline around bisect execution
- Fix root cause: ensure bisect loop makes progress (check commit traversal logic)
- Add `defer cancel()` cleanup per D-11

### 2. Fix Dispatcher Flaky Tests (HIGH PRIORITY - D-10, D-11)
**Files:** `internal/tools/dispatcher_test.go`, `internal/tools/extra_test.go`
- TestDispatcher_PermissionAllowed_NoError: Add timeout context to Execute call
- TestDispatcher_SyncTodoFromTasks_NilTodoWrite: Fix test setup to provide valid session
- TestNewDispatcherFactory_WithManager: Handle nil return from factory

### 3. Add CI Detection to E2E Tests (D-08, D-15)
**File:** `tests/e2e/e2e_test.go`
- Import `github.com/eshanized/M31A/internal/testutil/ci`
- Add `ci.SkipIfCI(t, "requires API keys")` to real API tests
- Build test binary in CI before running e2e tests

### 4. Fix E2E Build Path (D-10)
**File:** `tests/e2e/e2e_test.go` or Makefile
- Add `make build` as test prerequisite or build in TestMain
- Ensure binary path is correct for test execution

## Summary by Decision References

| Decision | Addressed In |
|----------|--------------|
| D-01: Run full suite with -v -timeout 30s -race | Done - test run completed |
| D-02: Add -timeout 30s to Makefile test targets | Done - Makefile updated |
| D-03: Identify hanging tests | Done - bisect test hangs |
| D-04: Identify race conditions | None in passing tests |
| D-05: Identify flaky tests | Done - 3 dispatcher tests flaky |
| D-06: Environment audit | Go version pinned in CI |
| D-07: External resource dependencies | Identified - 3 e2e tests need CI skip |
| D-08: Implement isCI() helper | Planned for Plan 02-03 |
| D-09: Fix hanging tests with timeouts | Planned for Plan 02-03 |
| D-10: Fix flaky tests | Planned for Plan 02-03 |
| D-11: Add defer cleanup | Planned for Plan 02-03 |
| D-12: CI job timeouts | Done - 10min timeout in CI |
| D-13: Test-level -timeout 30s | Done - Makefile and CI updated |
| D-14: CI artifact upload | Done - coverage upload on failure |
| D-15: CI detection for external deps | Planned for Plan 02-03 |

## Action Items for Plan 02-03

1. **Create `internal/testutil/ci/ci.go`** with `IsCI()` and `SkipIfCI()` helpers
2. **Fix `internal/engine/bisect`** - add timeout context and fix bisect loop
3. **Fix `internal/tools/dispatcher_test.go`** - resolve 3 flaky tests
4. **Update `tests/e2e/e2e_test.go`** - add CI detection and build binary
5. **Run full test suite** with `-race -timeout 30s` to verify all fixes