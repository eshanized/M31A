# Testing Patterns

**Analysis Date:** 2026-08-04

## Test Framework

**Runner:**
- Go's built-in `testing` package
- No external test frameworks
- Config: `Makefile` targets

**Assertion Library:**
- Standard `testing.T` methods: `t.Error()`, `t.Fatal()`, `t.Fatalf()`
- Manual assertions with `if` statements
- No third-party assertion libraries

**Run Commands:**
```bash
make test           # Race-enabled tests with coverage
make test-fast      # Tests without race detector
make test-verbose   # Tests with verbose output
make test-specific TEST=TestFoo   # Run one test
make bench          # Run benchmarks
make cover          # Generate HTML coverage report
```

## Test File Organization

**Location:**
- Co-located with source files in the same package
- Separate `tests/` directory for E2E and integration tests
- Test utilities in `internal/testutil/` and `tests/testutil/mocks/`

**Naming:**
- Unit tests: `*_test.go` in same package
- E2E tests: `tests/e2e/e2e_test.go`
- Integration tests: `tests/testutil/integration/`
- Benchmarks: `*_benchmark_test.go` or `bench_test.go`

**Structure:**
```
internal/
  tools/
    dispatcher.go           # Source
    dispatcher_test.go      # Unit tests (same package)
    permissions_test.go     # Unit tests (same package)
    edit_benchmark_test.go  # Benchmarks
  engine/
    workflow/
      engine.go             # Source
      engine_test.go        # Unit tests
      engine_race_test.go   # Race condition tests
tests/
  e2e/
    e2e_test.go             # E2E tests (separate package)
  testutil/
    mocks/
      tool.go               # Mock implementations
      provider.go
      dispatcher.go
internal/testutil/
  ci/
    ci.go                   # CI environment detection
  testtimeout/
    testtimeout.go          # Test timeout helpers
```

## Test Structure

**Suite Organization:**
```go
package tools

import (
    "context"
    "testing"

    "github.com/eshanized/M31A/internal/core/config"
    "github.com/eshanized/M31A/internal/core/types"
    "github.com/eshanized/M31A/tests/testutil/mocks"
)

func TestDispatcher_RegisterAndExecute(t *testing.T) {
    t.Parallel()
    d := testDispatcher(t)
    d.Register(&mocks.MockTool{Name_: "test", RiskLevel_: types.RiskSafe})

    result, err := d.Execute(context.Background(), types.ToolCall{
        ID:    "call1",
        Name:  "test",
        Input: []byte(`{}`),
    })
    if err != nil {
        t.Fatal(err)
    }
    if result.Output != "<tool_output>\nok\n</tool_output>" {
        t.Errorf("expected output, got %q", result.Output)
    }
}
```

**Patterns:**
- Helper functions with `t.Helper()` for setup
- `t.Cleanup()` for automatic cleanup
- `t.TempDir()` for isolated test directories
- `t.Skip()` for conditionally skipped tests

## Mocking

**Framework:** Hand-written mocks (no mocking library)

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
- External API calls (providers)
- File system operations (when testing logic)
- Network requests
- Time-dependent operations

**What NOT to Mock:**
- Internal data structures
- Pure functions
- Concurrent primitives (test with real goroutines)

## Fixtures and Factories

**Test Data:**
```go
// Test dispatcher helper
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    d, _ := DefaultDispatcher("", "", "", nil, nil)
    t.Cleanup(func() { d.Stop() })
    return d
}

// Test dispatcher with config
func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
    t.Helper()
    d, _ := DefaultDispatcher("", "", "", cfg, nil)
    t.Cleanup(func() { d.Stop() })
    return d
}
```

**Location:**
- `tests/testutil/mocks/` for shared mocks
- `internal/testutil/` for test helpers
- Local `_test.go` files for package-specific helpers

## Coverage

**Requirements:**
- **75%** overall coverage target
- **90%** for critical packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover          # Generate HTML report
go tool cover -html=coverage.out  # View in browser
```

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Approach: Table-driven tests with parallel execution
- Location: Co-located with source files

**Integration Tests:**
- Scope: Multiple components working together
- Approach: Real dependencies with test fixtures
- Location: `tests/testutil/integration/`

**E2E Tests:**
- Scope: Full application workflow
- Approach: Compile and run binary
- Location: `tests/e2e/e2e_test.go`
- Requires: API keys for real API tests (skip when unset)

**Race Tests:**
- Scope: Concurrent data structures and goroutine lifecycle
- Approach: `go test -race ./...`
- Location: Dedicated `*_race_test.go` files
- Examples: `engine_race_test.go`

## Common Patterns

**Async Testing:**
```go
func TestDispatcher_DangerousToolPermissionGranted(t *testing.T) {
    d := testDispatcher(t)
    d.Register(&mocks.MockTool{Name_: "bash", RiskLevel_: types.RiskDangerous})

    errCh := make(chan error, 1)
    go func() {
        _, err := d.Execute(context.Background(), types.ToolCall{
            ID:    "call1",
            Name:  "bash",
            Input: []byte(`{}`),
        })
        errCh <- err
    }()

    req := <-d.RequestCh()
    d.ApprovePermission(req.ID, true, false)

    if err := <-errCh; err != nil {
        t.Errorf("expected nil error after approval, got: %v", err)
    }
}
```

**Error Testing:**
```go
func TestDispatcher_DangerousToolPermissionDenied(t *testing.T) {
    d := testDispatcher(t)
    d.Register(&mocks.MockTool{Name_: "bash", RiskLevel_: types.RiskDangerous})

    _, err := d.Execute(context.Background(), types.ToolCall{
        ID:    "call1",
        Name:  "bash",
        Input: []byte(`{}`),
    })
    if err != m31errors.ErrPermissionDenied {
        t.Errorf("expected ErrPermissionDenied, got: %v", err)
    }
}
```

**Context Timeout Testing:**
```go
func TestDispatcher_DangerousToolContextCancelled(t *testing.T) {
    d := testDispatcher(t)
    d.Register(&mocks.MockTool{Name_: "bash", RiskLevel_: types.RiskDangerous})

    ctx, cancel := context.WithCancel(context.Background())

    errCh := make(chan error, 1)
    go func() {
        _, err := d.Execute(ctx, types.ToolCall{
            ID:    "call1",
            Name:  "bash",
            Input: []byte(`{}`),
        })
        errCh <- err
    }()

    <-d.RequestCh()
    cancel()

    select {
    case err := <-errCh:
        if err == nil {
            t.Error("expected context cancellation error")
        }
    case <-time.After(2 * time.Second):
        t.Fatal("expected context cancellation to be handled")
    }
}
```

**Race Condition Testing:**
```go
func TestConcurrentSetModelAndProviderAndModel(t *testing.T) {
    engine, _ := setupTestEngine(t)

    var wg sync.WaitGroup
    const goroutines = 10
    const iterations = 100

    // Concurrent writers
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                engine.SetModel("model-writer", nil)
            }
        }(i)
    }

    // Concurrent readers
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                _, _ = engine.providerAndModel()
            }
        }(i)
    }

    wg.Wait()
}
```

## E2E Test Helpers

```go
// buildBinary compiles the M31A binary and returns the path.
func buildBinary(t *testing.T) string {
    t.Helper()
    bin := filepath.Join(t.TempDir(), "m31a")
    cmd := exec.Command("go", "build", "-o", bin, "./cmd/m31a")
    cmd.Dir = projectRoot(t)
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("build failed: %v\n%s", err, string(out))
    }
    return bin
}

// runBinary executes the binary with args and returns stdout.
func runBinary(t *testing.T, bin string, args ...string) string {
    t.Helper()
    cmd := exec.Command(bin, args...)
    cmd.Env = cleanEnv()
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("run failed: %v\noutput: %s", err, string(out))
    }
    return string(out)
}
```

## Benchmark Tests

```go
func BenchmarkCascadingReplace(b *testing.B) {
    sizes := []struct {
        name    string
        lines   int
        lineLen int
    }{
        {"tiny_10x50", 10, 50},
        {"small_100x80", 100, 80},
        {"medium_1000x100", 1000, 100},
        {"large_5000x120", 5000, 120},
    }

    for _, size := range sizes {
        b.Run(size.name, func(b *testing.B) {
            content := generateContent(size.lines, size.lineLen)
            b.ReportAllocs()
            for i := 0; i < b.N; i++ {
                _, _, _, _ = fileops.CascadingReplace(content, "old", "new", false, 0.8)
            }
        })
    }
}
```

## CI Integration

**CI Detection:**
```go
// IsCI returns true if running in CI environment
func IsCI() bool {
    return os.Getenv("CI") == "true" ||
        os.Getenv("GITHUB_ACTIONS") == "true" ||
        os.Getenv("GITLAB_CI") == "true" ||
        os.Getenv("CI_NAME") != ""
}

// SkipIfCI skips test in CI environments
func SkipIfCI(t *testing.T, reason string) {
    t.Helper()
    if IsCI() {
        t.Skip(reason)
    }
}
```

## Test Timeout Helpers

```go
// WithTimeout returns a context with timeout and auto-cleanup
func WithTimeout(t *testing.T, duration time.Duration) (context.Context, context.CancelFunc) {
    t.Helper()
    ctx, cancel := context.WithTimeout(context.Background(), duration)
    t.Cleanup(cancel)
    return ctx, cancel
}
```

---

*Testing analysis: 2026-08-04*
