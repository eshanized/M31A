# Testing Patterns

**Analysis Date:** 2026-06-16

## Test Framework

**Runner:**
- Go standard `testing` package (no testify, no gomock)
- Config: none (uses `go test` directly)
- Version: Go 1.24

**Assertion Library:**
- Manual assertions with `t.Errorf`, `t.Fatalf`, `t.Fatal`, `t.Error`
- No third-party assertion libraries

**Run Commands:**
```bash
make test              # Run all tests with race detector and coverage
make test-fast         # Run tests without race detector (faster)
make test-verbose      # Run tests with verbose output
make test-specific TEST=TestFoo  # Run specific test by name
make bench             # Run benchmarks
make cover             # Generate HTML coverage report
```

## Test File Organization

**Location:**
- Co-located with source files in the same package
- Test files: `{source}_test.go`
- Supplementary tests: `{source}_extra_test.go`
- Doc coverage tests: `doc_test.go` (empty, verifies compilation)
- Helper files: `test_helpers_test.go`

**Naming:**
- Test functions: `TestFunctionName` or `TestFunctionName_Variant`
- Table tests: `TestFunctionName` with subtests
- Benchmarks: `BenchmarkFunctionName`

**Structure:**
```
internal/
├── config/
│   ├── loader.go           # Source
│   ├── loader_test.go      # Primary tests
│   ├── extra_test.go       # Supplementary tests
│   └── types_test.go       # Type tests
├── git/
│   ├── git.go
│   ├── git_test.go
│   └── git_extra_test.go
└── ...
```

## Test Structure

**Suite Organization:**
```go
func TestFunctionName(t *testing.T) {
    // Setup
    dir := t.TempDir()
    
    // Test logic
    result, err := FunctionUnderTest(input)
    
    // Assertions
    if err != nil {
        t.Fatalf("FunctionUnderTest failed: %v", err)
    }
    if result != expected {
        t.Errorf("expected %q, got %q", expected, result)
    }
}
```

**Table-Driven Tests:**
```go
func TestParsePlan_TasksFromJSON(t *testing.T) {
    t.Parallel()
    tests := []struct {
        name     string
        input    string
        expected int
    }{
        {"single task", "json...", 1},
        {"multiple tasks", "json...", 3},
    }
    
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := FunctionUnderTest(tt.input)
            if got != tt.expected {
                t.Errorf("expected %d, got %d", tt.expected, got)
            }
        })
    }
}
```

**Setup/Teardown Patterns:**
```go
// Setup helper
func setupRepo(t *testing.T) (*Git, string) {
    t.Helper()
    dir := t.TempDir()
    g := New(dir)
    if err := g.Init(); err != nil {
        t.Fatalf("Init failed: %v", err)
    }
    return g, dir
}

// No explicit teardown; t.TempDir() auto-cleans
```

## Mocking

**Framework:** None — manual mock implementations

**Patterns:**
```go
// Mock keychain for testing
type mockKeychain struct {
    store map[string]string
}

func newMockKeychain() *mockKeychain {
    return &mockKeychain{store: make(map[string]string)}
}

func (m *mockKeychain) Get(service string) (string, error) {
    if v, ok := m.store[service]; ok {
        return v, nil
    }
    return "", errors.New("not found")
}

func (m *mockKeychain) Set(service, value string) error {
    m.store[service] = value
    return nil
}

func (m *mockKeychain) Delete(service string) error {
    delete(m.store, service)
    return nil
}
```

**What to Mock:**
- External services (keychain, API calls)
- File system operations (use `t.TempDir()`)
- Git operations (use real git with temp repos)

**What NOT to Mock:**
- Internal functions (test them directly)
- Standard library functions
- Simple data structures

## Fixtures and Factories

**Test Data:**
```go
// Helper to create test tasks
func newTask(id int, desc string, deps []int) types.Task {
    return types.Task{
        ID:           id,
        Description:  desc,
        Action:       "Create",
        Dependencies: deps,
    }
}

// Helper to create test files
func writeFile(t *testing.T, dir, name, content string) {
    t.Helper()
    if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
        t.Fatalf("writeFile %s failed: %v", name, err)
    }
}

// Helper to run git commands
func runGit(t *testing.T, dir string, args ...string) {
    t.Helper()
    cmd := exec.Command("git", args...)
    cmd.Dir = dir
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("git %s failed: %v\n%s", args[0], err, string(out))
    }
}
```

**Location:**
- Inline in test files
- `test_helpers_test.go` for shared helpers within a package

## Coverage

**Requirements:**
- Overall target: 75% (currently ~74.7%)
- Critical packages: 90% — `pkg/taskrunner` (89.9%), `pkg/bisect` (91.3%), `pkg/rollback` (89.1%)

**View Coverage:**
```bash
make cover              # Generates coverage.html
go tool cover -html=coverage.out -o coverage.html
```

**Coverage Gaps:**
- `internal/tui`: 38.6% (dangerously low for primary UI)
- `internal/tui/commands`: 10.6%
- `internal/tui/streaming`: 29.5%
- `pkg/keychain`: 20.3%
- `cmd/m31a`: 0.0%

## Test Types

**Unit Tests:**
- Scope: Individual functions/methods
- Approach: Pure functions tested directly, methods on struct instances
- Examples: `TestParsePlan_TasksFromJSON`, `TestAtomicWrite_BasicWriteAndReadback`

**Integration Tests:**
- Scope: Real git repos, temp dirs, HTTP test servers
- Approach: Full workflow with real I/O
- Examples: `TestBisect_Successful`, `TestChain`, `TestRollback`

**Security Tests:**
- Scope: SSRF protection, timeout enforcement, path traversal
- Approach: Malicious inputs, boundary conditions
- Examples: Tests in `internal/tools/` for WebFetch SSRF

**E2E Tests:**
- Framework: Not used
- TUI tested with mock models

## Common Patterns

**Async Testing:**
```go
func TestWatchConfig_DetectsChange(t *testing.T) {
    dir := t.TempDir()
    path := filepath.Join(dir, "config.toml")
    
    ch := make(chan ConfigReloadMsg, 1)
    ctx, cancel := context.WithCancel(context.Background())
    defer cancel()
    
    go WatchConfig(ctx, path, ch)
    
    time.Sleep(100 * time.Millisecond)
    os.WriteFile(path, []byte("[ui]\ntheme = \"light\"\n"), 0644)
    
    select {
    case msg := <-ch:
        if msg.Config.UI.Theme != "light" {
            t.Errorf("expected 'light', got %q", msg.Config.UI.Theme)
        }
    case <-time.After(10 * time.Second):
        t.Fatal("timed out waiting for config reload")
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

**Parallel Testing:**
```go
func TestConstants(t *testing.T) {
    t.Parallel()
    if ModelCacheTTL != 5*time.Minute {
        t.Errorf("expected 5min, got %v", ModelCacheTTL)
    }
}
```

**Subtests:**
```go
func TestHasUncommittedChanges(t *testing.T) {
    t.Run("clean", func(t *testing.T) {
        _, g := setupRollback(t)
        createCommits(g, 1)
        r := New(g)
        dirty, err := r.HasUncommittedChanges()
        if err != nil {
            t.Fatalf("HasUncommittedChanges failed: %v", err)
        }
        if dirty {
            t.Error("Expected HasUncommittedChanges=false on clean repo")
        }
    })
    
    t.Run("dirty", func(t *testing.T) {
        // ...
    })
}
```

## Key Testing Utilities

**Temp Directories:**
- `t.TempDir()` — auto-cleaned, preferred for all test isolation
- `os.MkdirTemp()` — manual cleanup required, used when `t.TempDir()` insufficient

**Environment Variables:**
- `t.Setenv()` — auto-restored after test
- Manual `os.Setenv`/`os.Unsetenv` for complex scenarios

**Git Repos:**
- Real git repos created in temp dirs
- Helper functions: `setupRepo(t)`, `setupBisectRepo(t)`, `setupRollback(t)`

**HTTP Test Servers:**
- `httptest.NewServer()` for API mocking
- Used in provider tests

---

*Testing analysis: 2026-06-16*
