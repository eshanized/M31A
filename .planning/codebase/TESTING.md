# Testing Patterns

**Analysis Date:** 2026-07-13

## Test Framework

**Runner:**
- Standard Go `testing` package
- No external test frameworks (no testify, no gomock)
- Config: No separate config file; tests run via `go test`

**Assertion Library:**
- Manual assertions using `t.Fatalf`, `t.Errorf`, `t.Fatal`, `t.Error`
- `errors.Is()` for error sentinel matching
- `strings.Contains()` for string matching

**Run Commands:**
```bash
make test              # Race-enabled tests with coverage
make test-fast         # Tests without race detector
make test-specific TEST=TestFoo  # Run one test
make cover             # Generate HTML coverage report
make bench             # Run benchmarks
```

## Test File Organization

**Location:**
- Co-located with source files in the same package
- Test files use `_test.go` suffix
- Extra test files use `_extra_test.go` suffix for additional coverage
- Integration tests use `_integration_test.go` suffix

**Naming:**
- `*_test.go` for unit and integration tests
- `e2e_test.go` at root for end-to-end binary tests
- `doc_test.go` for package documentation tests

**Structure:**
```
pkg/bisect/
├── bisect.go           # Source
├── bisect_test.go      # Unit tests
├── doc.go              # Package documentation
├── doc_test.go         # Documentation test
├── exec.go             # Git exec helpers
└── extra_test.go       # Additional coverage tests
```

## Test Structure

**Suite Organization:**
```go
package bisect

import (
    "errors"
    "log/slog"
    "testing"

    m31errors "github.com/eshanized/M31A/internal/errors"
)

func TestBisect_Successful(t *testing.T) {
    dir, b := setupBisectRepo(t)
    // ... test body
}

func TestBisect_ParseLog(t *testing.T) {
    tests := []struct {
        name string
        log  string
        want string
    }{
        {"standard format", "...", "abc123"},
        {"empty log", "", ""},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := parseBisectLog(tt.log)
            if got != tt.want {
                t.Errorf("parseBisectLog: expected %q, got %q", tt.want, got)
            }
        })
    }
}
```

**Patterns:**

1. **Setup helpers using `t.Helper()` and `t.TempDir()`:**
```go
func setupBisectRepo(t *testing.T) (string, *Bisect) {
    t.Helper()
    dir := t.TempDir()
    // ... init repo, create commits
    b := New(dir, slog.Default())
    return dir, b
}
```

2. **Table-driven tests for parameterized cases:**
```go
tests := []struct {
    name     string
    pattern  string
    toolName string
    want     bool
}{
    {"exact match", "Bash", "Bash", true},
    {"wildcard matches all", "*", "Anything", true},
}

for _, tt := range tests {
    t.Run(tt.name, func(t *testing.T) {
        t.Parallel()
        got := matchToolName(tt.pattern, tt.toolName)
        if got != tt.want {
            t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.toolName, got, tt.want)
        }
    })
}
```

3. **`t.Parallel()` for concurrent test execution:**
```go
func TestCheckPermission_AgentDefaultAllow(t *testing.T) {
    t.Parallel()
    // ... test body
}
```

4. **Error assertion with `errors.Is()`:**
```go
_, err := r.Schedule()
if err == nil {
    t.Fatal("Expected error for circular dependency")
}
if err != m31errors.ErrCircularDependency {
    t.Errorf("Expected ErrCircularDependency, got %v", err)
}
```

5. **Defer for cleanup:**
```go
func TestCheckPermission_AgentDefaultAllow(t *testing.T) {
    t.Parallel()
    d := NewDispatcher(&config.PermissionsConfig{...})
    if err := d.SelectAgent("autonomous"); err != nil {
        t.Fatalf("SelectAgent failed: %v", err)
    }
    defer d.SelectAgent("default")
    // ... test body
}
```

## Mocking

**Framework:** None — manual test doubles only.

**Patterns:**

1. **Mock tool struct for dispatcher tests:**
```go
type mockTool struct {
    name      string
    riskLevel types.RiskLevel
}

func (m *mockTool) Name() string             { return m.name }
func (m *mockTool) Description() string      { return "mock tool" }
func (m *mockTool) RiskLevel() types.RiskLevel { return m.riskLevel }
func (m *mockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    return types.ToolResult{Output: "ok"}, nil
}
```

2. **GitRunner interface for test injection:**
```go
type GitRunner interface {
    Run(args ...string) (string, error)
}

// In tests, implement with exec.Command or mock:
type mockGit struct {
    output string
    err    error
}

func (m *mockGit) Run(args ...string) (string, error) {
    return m.output, m.err
}
```

3. **HTTP response mocking for provider tests:**
```go
func bodyReader(s string) *http.Response {
    return &http.Response{
        Body: io.NopCloser(strings.NewReader(s)),
    }
}

func TestSSEParser_SingleDataLine(t *testing.T) {
    resp := bodyReader("data: {\"key\":\"val\"}\n\n")
    p := NewSSEParser(resp)
    defer p.Close()
    // ... test parser
}
```

**What to Mock:**
- External dependencies (git, HTTP, file system)
- Provider API calls
- User input channels

**What NOT to Mock:**
- Internal package functions (test the real thing)
- Standard library functions
- Simple data transformations

## Fixtures and Factories

**Test Data:**
```go
func newTask(id int, desc string, deps []int) types.Task {
    return types.Task{
        ID:           id,
        Description:  desc,
        Action:       "Create",
        Dependencies: deps,
    }
}
```

**Location:**
- Test helper functions defined at top of test file
- Shared test data inline in test functions
- No separate fixtures directory

## Coverage

**Requirements:**
- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover             # Generates coverage.html
go tool cover -html=coverage.out -o coverage.html
```

**Coverage Profile:**
```bash
go test -coverprofile=coverage.out ./...
go tool cover -func=coverage.out
```

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Pattern: Table-driven with edge cases
- Examples: `TestBisect_ParseLog`, `TestMatchToolName_GlobEdgeCases`

**Integration Tests:**
- Scope: Multiple components working together
- Pattern: Real git repos, real file system
- Examples: `TestBisect_Successful`, `TestSoftReset_WithUncommitted`
- Suffix: `_integration_test.go`

**E2E Tests:**
- Framework: Custom binary compilation and execution
- Location: Root `e2e_test.go`
- Pattern: Compiles binary, runs with args, checks output
- Real API tests require env vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
- Skip when env vars unset:
```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }
    // ... test body
}
```

**Benchmarks:**
```bash
make bench             # Run benchmarks
make bench-verbose     # Run with verbose output
```

**Race Tests:**
```bash
go test -race ./...    # Required for concurrent structures
```

High-contention tests:
- `TestWorkflowCache_ConcurrentDynamicContext` — 100 goroutines, 1000 iterations
- `TestDNSCache_HighContention` — 50 goroutines, 1000 iterations
- `TestCheckPermission_ConcurrentAccess` — 10 goroutines

## Common Patterns

**Async Testing:**
```go
func TestBinary_Prompt_Timeout(t *testing.T) {
    bin := buildBinary(t)
    cmd := exec.Command(bin, "--prompt", "Write a 10000 word essay...")
    done := make(chan error, 1)
    go func() {
        _, err := cmd.CombinedOutput()
        done <- err
    }()

    select {
    case <-done:
        // Expected — either timeout error or auth error
    case <-time.After(30 * time.Second):
        cmd.Process.Kill()
        t.Fatal("binary did not exit within 30 seconds")
    }
}
```

**Error Testing:**
```go
func TestBisect_ResetOnError(t *testing.T) {
    dir, b := setupBisectRepo(t)
    _, err := b.Run("invalid1", "invalid2", func() bool { return true })
    if err == nil {
        t.Fatal("Expected error for invalid hashes")
    }
    // Verify repo is still clean
    out, _ := exec.Command("git", "-C", dir, "status").CombinedOutput()
    if len(out) == 0 {
        t.Fatal("Expected status output")
    }
}
```

**State Verification:**
```go
func TestRunner_AllDone(t *testing.T) {
    tasks := []types.Task{
        newTask(1, "task 1", nil),
        newTask(2, "task 2", nil),
    }
    r := New(tasks)

    if r.AllDone() {
        t.Error("Expected AllDone to be false initially")
    }

    groups, _ := r.Schedule()
    r.ExecuteGroup(context.Background(), groups[0], func(ctx context.Context, task types.Task) TaskResult {
        return TaskResult{Success: true}
    })

    if !r.AllDone() {
        t.Error("Expected AllDone to be true after all tasks executed")
    }
}
```

**Cleanup Verification:**
```go
func TestBisect_ResetFailure_WrapsErrBisectResetFailed(t *testing.T) {
    // ... setup
    _, err := b.Run("0000000000000000000000000000000000000000", "1111111111111111111111111111111111111111", func() bool {
        return true
    })
    // The error may or may not wrap ErrBisectResetFailed depending on
    // whether the reset itself fails. The key thing is the Run function
    // doesn't panic and handles the error path.
    if err != nil {
        _ = err
    }
}
```

## Test Naming Conventions

**Pattern:** `Test<Type>_<Scenario>` or `Test<Type>_<Scenario>_<SubScenario>`

**Examples:**
```
TestBisect_Successful
TestBisect_AlwaysPasses
TestBisect_ResetOnError
TestBisect_ParseLog_EdgeCases
TestBisect_ParseBisectLog_Unit
TestSoftReset_WithUncommitted
TestSoftReset_WithUncommitted_MessageStashed
TestCheckPermission_AgentDefaultAllow
TestCheckPermission_RuleOverridesAgentDefault
TestBatchApproval_ConcurrentAccess
```

**Subtests:** Use `t.Run()` for named subtests within table-driven tests:
```go
for _, tt := range tests {
    t.Run(tt.name, func(t *testing.T) {
        t.Parallel()
        // ... test body
    })
}
```

---

*Testing analysis: 2026-07-13*
