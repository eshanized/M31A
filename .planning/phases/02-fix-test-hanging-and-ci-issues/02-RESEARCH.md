# Phase 2 Research — Fix Test Hanging and CI Issues

**Phase:** 2 — Fix Test Hanging and CI Issues
**Date:** 2026-07-21
**Status:** Research complete

---

## Executive Summary

This research covers the technical approaches for investigating and resolving test hanging issues and CI pipeline problems in the M31A codebase. The phase focuses on deep investigation of root causes, CI pipeline fixes, and test reliability improvements.

---

## 1. Common Causes of Test Hanging in Go

### 1.1 Goroutine Leaks
- **Cause:** Goroutines started but never terminated (blocked on channels, mutexes, or I/O)
- **Detection:** `go test -race`, `pprof` (`go tool pprof`), runtime metrics
- **Prevention:** Always use `context.WithTimeout`, `sync.WaitGroup`, proper channel closing

### 1.2 Channel Deadlocks
- **Cause:** Sender/receiver mismatch, unbuffered channels with no receiver, circular dependencies
- **Detection:** Race detector, code review, timeout-based tests
- **Prevention:** Use buffered channels where appropriate, always have timeout on channel ops

### 1.3 Mutex/RWMutex Issues
- **Cause:** Lock not released, double-lock, lock ordering issues
- **Detection:** Race detector, `sync.Mutex` profiling
- **Prevention:** Use `defer mu.Unlock()`, avoid nested locks, use `sync.RWMutex` for read-heavy

### 1.4 External Resource Dependencies
- **Cause:** Tests waiting for network, database, filesystem, or API calls that hang
- **Detection:** Test timeouts, CI vs local differences
- **Prevention:** Mock external dependencies, use test containers, add timeouts

### 1.5 Test Framework Issues
- **Cause:** `t.Parallel()` misuse, shared state between tests, test cleanup failures
- **Detection:** Run tests in isolation, `-count=1`, `-p=1`
- **Prevention:** Proper test isolation, `t.Cleanup()`, avoid global state

---

## 2. Go Testing Best Practices

### 2.1 Timeout Configuration
```bash
# Package-level timeout
go test -timeout 30s ./...

# Per-test timeout (in test code)
func TestSomething(t *testing.T) {
    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    defer cancel()
    // ...
}
```

### 2.2 Race Detector Usage
```bash
# Always run with race detector in CI
go test -race -cover -coverprofile=coverage.out ./...

# For specific packages
go test -race ./internal/taskrunner/...
go test -race ./internal/bisect/...
go test -race ./internal/rollback/...
```

### 2.3 Test Isolation Patterns
```go
// Use t.TempDir() for filesystem isolation
func TestFileOps(t *testing.T) {
    dir := t.TempDir()
    // ...
}

// Use t.Cleanup() for resource cleanup
func TestWithCleanup(t *testing.T) {
    resource := acquireResource()
    t.Cleanup(func() { resource.Release() })
    // ...
}
```

### 2.4 Context Propagation
```go
// Always pass context through call chains
func DoWork(ctx context.Context) error {
    select {
    case <-ctx.Done():
        return ctx.Err()
    case result := <-workChan:
        return process(result)
    }
}
```

---

## 3. GitHub Actions CI Best Practices

### 3.1 Job Timeout Configuration
```yaml
jobs:
  test:
    timeout-minutes: 10  # Prevents stuck jobs
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: |
          go test -race -timeout 30s ./...
```

### 3.2 Environment Consistency
```yaml
# Use consistent Go version
- name: Set up Go
  uses: actions/setup-go@v5
  with:
    go-version: '1.25.0'  # Match go.mod
    
# Cache dependencies
- name: Cache Go modules
  uses: actions/cache@v4
  with:
    path: |
      ~/.cache/go-build
      ~/go/pkg/mod
    key: ${{ runner.os }}-go-${{ hashFiles('**/go.sum') }}
```

### 3.3 Matrix Testing for Coverage
```yaml
strategy:
  matrix:
    go-version: ['1.25.0']
    os: [ubuntu-latest, macos-latest, windows-latest]
```

### 3.4 Artifact Collection for Debugging
```yaml
- name: Upload test artifacts
  if: failure()
  uses: actions/upload-artifact@v4
  with:
    name: test-failure-logs
    path: |
      coverage.out
      test-output.log
```

---

## 4. CI Environment Differences

### 4.1 Common Differences
| Factor | Local | CI | Mitigation |
|--------|-------|-----|------------|
| Go version | May differ | Fixed | Use `setup-go` with exact version |
| Dependencies | Cached | Fresh | Cache `go/pkg/mod` |
| File permissions | User-specific | Runner-specific | Use `t.TempDir()` |
| Network | Full access | May be restricted | Mock external calls |
| Resources | Abundant | Limited (CPU/RAM) | Add timeouts, reduce parallelism |
| Environment vars | User-defined | Minimal | Set required vars explicitly |

### 4.2 Detection Pattern
```go
// Detect CI environment
func isCI() bool {
    return os.Getenv("CI") == "true" || 
           os.Getenv("GITHUB_ACTIONS") == "true" ||
           os.Getenv("GITLAB_CI") == "true"
}

// Skip tests that require local resources
func TestRequiresLocalResource(t *testing.T) {
    if isCI() {
        t.Skip("Skipping in CI - requires local resource")
    }
    // ...
}
```

---

## 5. Codebase-Specific Patterns (from TESTING.md, CONVENTIONS.md, STRUCTURE.md)

### 5.1 Existing Test Infrastructure
- **Test Runner:** Go standard library `testing` package
- **Assertion Library:** `github.com/stretchr/testify` (assert, require, mock)
- **Race Detector:** Enabled for `make test` (`go test -race`)
- **Coverage:** Enabled for `make test` (`go test -cover -coverprofile=coverage.out`)

### 5.2 Coverage Targets (from AGENTS.md)
- **75%** overall coverage
- **90%** for critical packages:
  - `internal/taskrunner` (runner_test.go: 677 lines)
  - `internal/bisect` (bisect_test.go: 661 lines)
  - `internal/rollback` (rollback_test.go: 1198 lines)

### 5.3 Key Test Files
| File | Purpose | Lines |
|------|---------|-------|
| `e2e_test.go` | Binary E2E tests, real API tests | 191 |
| `internal/tools/dispatcher_test.go` | Dispatcher permissions, rate limiting, concurrency | 991 |
| `internal/taskrunner/runner_test.go` | Task scheduling, execution, dependencies | 677 |
| `internal/rollback/rollback_test.go` | Git rollback operations | 1198 |
| `internal/bisect/bisect_test.go` | Git bisect automation | 661 |
| `internal/workflow/engine_test.go` | Workflow engine phases | ~400 |

### 5.4 Test Patterns Used
- **Table-driven tests** (dominant pattern)
- **Parallel tests** with `t.Parallel()`
- **Test helpers** (`testDispatcher`, `setupRollback`)
- **Mocking** with `testify/mock` and custom test doubles
- **Integration tests** in `internal/testutil/integration/`
- **E2E tests** with binary execution and temp dirs

### 5.5 Linting Configuration (.golangci.yml)
```yaml
linters:
  enable:
    - govet
    - staticcheck
    - errcheck
    - ineffassign
    - unused
  settings:
    govet:
      enable:
        - shadow
    errcheck:
      check-type-assertions: false
      check-blank: false
  exclusions:
    rules:
      - path: _test\.go
        linters:
          - errcheck
          - unused
```

### 5.6 Makefile Targets
```bash
make test           # race + coverage
make test-fast      # no race
make test-verbose   # verbose + race + coverage
make test-specific TEST=TestFoo  # single test
make cover          # HTML coverage report
```

---

## 6. Investigation Strategy (Based on CONTEXT.md Decisions)

### 6.1 Phase 1: Identify Hanging Tests
1. Run `make test` with `-v` flag to see which test hangs
2. Add `-timeout 30s` to test commands
3. Run with `-race` flag to detect race conditions
4. Focus on packages with concurrent operations:
   - `internal/taskrunner` (parallel execution, semaphores)
   - `internal/tools/dispatcher` (concurrency sem, rate limiting)
   - `internal/workflow/engine` (state machine, pause/resume channels)
   - `internal/tools/subagent` (Git worktrees, parallel execution)
   - `internal/rollback` (Git operations)
   - `internal/bisect` (Git bisect automation)

### 6.2 Phase 2: Fix Individual Tests
1. Add `context.WithTimeout` to tests using goroutines/channels
2. Add `defer` cleanup for resources (goroutines, files, connections)
3. Use `t.Cleanup()` for test-scoped cleanup
4. Add proper channel closing patterns
5. Ensure `sync.WaitGroup` usage is correct

### 6.3 Phase 3: CI Pipeline Fixes
1. Update Makefile: add `-timeout 30s` to all test targets
2. Update GitHub Actions: add `timeout-minutes: 10` to test jobs
3. Add CI detection to tests that use external resources
4. Add artifact collection for failed tests

### 6.4 Phase 4: Flaky Test Identification
1. Run tests multiple times: `go test -count=10 ./...`
2. Use `-p=1` to run sequentially and isolate ordering issues
3. Check for tests that depend on global state or shared resources
4. Add proper isolation for integration tests

---

## 7. Specific Codebase Areas to Investigate

### 7.1 TaskRunner (internal/taskrunner/)
- Uses semaphores for concurrency control
- Topological sorting for task dependencies
- Self-healing retries with exponential backoff
- **Risk:** Semaphore leaks, goroutine leaks in retry logic

### 7.2 Tool Dispatcher (internal/tools/dispatcher/)
- Concurrency semaphore (`concurrencySem`)
- Rate limiter (token bucket `rateTokens`)
- Permission checking with user prompts
- **Risk:** Semaphore not released on error, rate limiter blocking

### 7.3 Workflow Engine (internal/workflow/engine.go)
- State machine with phase transitions
- Pause/resume channels (`pauseCh`, `resumeCh`)
- Multiple mutexes (`planMu`, `messagesMu`, `transitionMu`, `pauseMu`)
- **Risk:** Deadlocks in mutex ordering, channel blocking

### 7.4 Subagent Manager (internal/tools/subagent/manager.go)
- Git worktree management
- Parallel subagent execution
- **Risk:** Worktree cleanup failures, process leaks

### 7.5 E2E Tests (e2e_test.go)
- Binary building per test
- Real API calls (skipped without keys)
- Temp directory isolation
- **Risk:** Binary build hanging, temp dir cleanup

---

## 8. Recommended Tools and Commands

### 8.1 Investigation Commands
```bash
# Run with verbose output and timeout
go test -v -timeout 30s ./...

# Run with race detector
go test -race -timeout 30s ./...

# Run specific package multiple times
go test -count=10 -timeout 30s ./internal/taskrunner/...

# Run tests in isolation (no parallel)
go test -p=1 -timeout 30s ./...

# Profile goroutines
go test -timeout 30s -blockprofile=block.prof ./...
go tool pprof block.prof

# Check for goroutine leaks
go test -timeout 30s ./... 2>&1 | grep -E "(PASS|FAIL|timeout)"
```

### 8.2 Debugging Commands
```bash
# Get stack traces of all goroutines
kill -SIGQUIT <pid>  # Prints to stderr

# Use pprof for runtime analysis
go test -timeout 30s -cpuprofile=cpu.prof ./...
go tool pprof cpu.prof
```

---

## 9. Implementation Priority

Based on the CONTEXT.md decisions and codebase analysis:

### High Priority (Immediate)
1. Add `-timeout 30s` to Makefile test targets
2. Add `timeout-minutes: 10` to GitHub Actions test jobs
3. Run full test suite with `-v -timeout 30s -race` to identify hanging tests
4. Add CI detection to tests using external resources

### Medium Priority (After Identification)
5. Fix individual hanging tests with proper timeouts and cleanup
6. Add `context.WithTimeout` to concurrent test operations
7. Ensure proper `defer` cleanup for goroutines and resources

### Low Priority (Follow-up)
8. Add test artifacts collection in CI for debugging
9. Consider adding `-count=3` for flaky test detection
10. Document test patterns for future contributors

---

## 10. References

- [Go Testing Package](https://pkg.go.dev/testing)
- [Go Race Detector](https://go.dev/doc/articles/race_detector)
- [GitHub Actions Timeout](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions#jobsjob_idtimeout-minutes)
- [Effective Go Testing](https://go.dev/doc/tutorial/add-a-test)
- Project: `.planning/codebase/TESTING.md`
- Project: `.planning/codebase/CONVENTIONS.md`
- Project: `.planning/codebase/STRUCTURE.md`
- Project: `AGENTS.md`
- Project: `Makefile`
- Project: `.golangci.yml`