# Testing Patterns

**Analysis Date:** 2026-08-03

## Test Framework

**Runner:**
- Go standard `testing` package
- Config: No external config file (uses `go test` flags)

**Assertion Library:**
- Standard `testing` package assertions
- Manual error checking with `t.Error()`, `t.Errorf()`, `t.Fatal()`, `t.Fatalf()`
- No third-party assertion libraries

**Run Commands:**
```bash
make test           # Run all tests with race detector and coverage
make test-fast      # Run tests without race detector
make test-specific TEST=TestFoo   # Run specific test
make cover          # Generate HTML coverage report
make bench          # Run benchmarks
```

## Test File Organization

**Location:**
- Co-located with source files in the same package
- Test helpers in separate files: `test_helpers_test.go`, `testutil_test.go`
- Shared mocks in `tests/testutil/mocks/`

**Naming:**
- Test files: `*_test.go`
- Benchmark files: `*_benchmark_test.go`
- Test helpers: `*_helpers_test.go`

**Structure:**
```
internal/
├── tools/
│   ├── dispatcher.go
│   ├── dispatcher_test.go
│   ├── permissions.go
│   ├── permissions_test.go
│   ├── testutil_test.go
│   └── fileops/
│       ├── edit.go
│       ├── edit_test.go
│       └── test_helpers.go
├── ui/tui/
│   ├── app.go
│   ├── app_test.go
│   └── test_helpers_test.go
tests/
├── e2e/
│   └── e2e_test.go
└── testutil/
    └── mocks/
        ├── tool.go
        └── dispatcher.go
```

## Test Structure

**Suite Organization:**
```go
func TestFunctionName(t *testing.T) {
    t.Parallel() // Use when safe

    // Setup
    dir := t.TempDir()
    tool := NewTool(dir)

    // Execute
    result, err := tool.Execute(context.Background(), types.ToolInput{
        Params: map[string]any{
            "key": "value",
        },
    })

    // Assert
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
    if result.Output == "" {
        t.Error("expected non-empty output")
    }
}
```

**Patterns:**
- Use `t.Parallel()` for independent tests
- Use `t.Helper()` for test helper functions
- Use `t.TempDir()` for temporary directories
- Use `t.Cleanup()` for cleanup functions
- Use table-driven tests for parameterized cases

## Mocking

**Framework:**
- Manual mocks (no third-party mocking library)

**Patterns:**
```go
// MockTool implements types.Tool for testing.
type MockTool struct {
    Name_        string
    Description_ string
    RiskLevel_   types.RiskLevel
    ExecFunc     func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

func (m *MockTool) Name() string               { return m.Name_ }
func (m *MockTool) Description() string        { return m.Description_ }
func (m *MockTool) RiskLevel() types.RiskLevel { return m.RiskLevel_ }

func (m *MockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    if m.ExecFunc != nil {
        return m.ExecFunc(ctx, input)
    }
    return types.ToolResult{Output: "ok"}, nil
}
```

**What to Mock:**
- External API calls
- File system operations (use `t.TempDir()`)
- Network operations
- Time-dependent operations

**What NOT to Mock:**
- Internal functions (test actual behavior)
- Simple data structures
- Standard library functions

## Fixtures and Factories

**Test Data:**
```go
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    d, _ := DefaultDispatcher("", "", "", nil, nil)
    t.Cleanup(func() { d.Stop() })
    return d
}

func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
    t.Helper()
    d, _ := DefaultDispatcher("", "", "", cfg, nil)
    t.Cleanup(func() { d.Stop() })
    return d
}
```

**Location:**
- Test helpers in `*_helpers_test.go` files
- Shared mocks in `tests/testutil/mocks/`
- Test utilities in `internal/testutil/`

## Coverage

**Requirements:**
- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover          # Generate HTML coverage report
go tool cover -html=coverage.out -o coverage.html
```

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Approach: Test inputs/outputs, edge cases, error conditions
- Example: `TestDispatcher_RegisterAndExecute`, `TestBash_WorkdirValidation`

**Integration Tests:**
- Scope: Multiple components working together
- Approach: Test real workflows, permission flows, session management
- Example: `TestDispatcher_DangerousToolPermissionGranted`, `TestPermissionTimeout`

**E2E Tests:**
- Framework: Custom binary compilation and execution
- Scope: Full application workflow
- Example: `TestBinary_Version`, `TestBinary_Prompt_NoProvider`
- Location: `tests/e2e/e2e_test.go`

**Benchmarks:**
- Scope: Performance-critical code
- Approach: Measure execution time and allocations
- Example: `BenchmarkCascadingReplace`, `BenchmarkCascadingReplace_Strategies`
- Location: `*_benchmark_test.go`

## Common Patterns

**Async Testing:**
```go
func TestAsyncOperation(t *testing.T) {
    t.Parallel()
    ch := make(chan error, 1)
    go func() {
        _, err := operation()
        ch <- err
    }()

    select {
    case err := <-ch:
        if err != nil {
            t.Fatalf("unexpected error: %v", err)
        }
    case <-time.After(3 * time.Second):
        t.Fatal("timed out waiting for operation")
    }
}
```

**Error Testing:**
```go
func TestErrorCase(t *testing.T) {
    t.Parallel()
    _, err := operation()
    if err == nil {
        t.Fatal("expected error")
    }
    if !errors.Is(err, ExpectedError) {
        t.Errorf("expected ErrExpected, got: %v", err)
    }
}
```

**Table-Driven Tests:**
```go
func TestTableDriven(t *testing.T) {
    tests := []struct {
        name     string
        input    string
        expected string
    }{
        {"case1", "input1", "output1"},
        {"case2", "input2", "output2"},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel()
            result, err := function(tt.input)
            if err != nil {
                t.Fatalf("unexpected error: %v", err)
            }
            if result != tt.expected {
                t.Errorf("got %q, want %q", result, tt.expected)
            }
        })
    }
}
```

**Concurrent Testing:**
```go
func TestConcurrentAccess(t *testing.T) {
    t.Parallel()
    var wg sync.WaitGroup
    numGoroutines := 50

    for i := 0; i < numGoroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            // Test concurrent access
        }(i)
    }

    wg.Wait()
}
```

## Security Testing

**Command Injection:**
- Test obfuscation bypass attempts (double spaces, tabs, mixed case)
- Test variable expansion detection (`$VAR`, `${VAR}`, `$(cmd)`)
- Test newline and special character handling
- See `TestCheckDangerousCommand_Baseline` in `internal/tools/bash_security_test.go`

**SSRF Protection:**
- Test private IP blocking (loopback, RFC1918, link-local)
- Test metadata endpoint blocking (169.254.169.254)
- Test DNS pinning via shared cache
- See `TestWebFetch_Blocks_PrivateIPv4` in `internal/tools/webfetch_security_test.go`

**Type Safety:**
- Test comma-ok type assertion guards on interface values
- Test graceful handling of invalid types in concurrent maps
- See `TestPermissions_InvalidType` in `internal/tools/permissions_test.go`

## Race Testing

**Required Race Tests:**
- All concurrent data structures (WorkflowCache, DNSCache)
- All goroutine lifecycle management
- All shared mutable state access

**Running Race Tests:**
```bash
make test           # Includes -race flag
go test -race ./... # Explicit race detection
```

**High-Contention Tests:**
- `TestWorkflowCache_ConcurrentDynamicContext` - 100 goroutines, 1000 iterations
- `TestDNSCache_HighContention` - 50 goroutines, 1000 iterations
- `TestDNSCache_ConcurrentEviction` - Low eviction threshold stress test

## Test Utilities

**CI Helpers:**
```go
// SkipIfCI skips the test if running in a CI environment
func SkipIfCI(t *testing.T, reason string) {
    t.Helper()
    if IsCI() {
        t.Skip(reason)
    }
}
```

**Environment Helpers:**
```go
// RequireAnyAPIKey skips the test unless at least one API key is set
func RequireAnyAPIKey(t *testing.T, envVars ...string) string {
    t.Helper()
    for _, v := range envVars {
        if val := os.Getenv(v); val != "" {
            return val
        }
    }
    t.Skipf("skipping: none of %v set in environment", envVars)
    return ""
}
```

**Test Data Helpers:**
```go
// contains checks if a string contains a substring
func contains(s, substr string) bool {
    return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && (s[:len(substr)] == substr || contains(s[1:], substr)))
}
```

---

*Testing analysis: 2026-08-03*
