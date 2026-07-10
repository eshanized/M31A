# Testing Patterns

**Analysis Date:** 2026-07-10

## Test Framework

**Runner:**
- Go's built-in `testing` package
- Config: No external test config file; all configuration via Makefile targets and `go test` flags

**Assertion Library:**
- Standard library only (`testing.T` methods: `Error`, `Errorf`, `Fatal`, `Fatalf`, `Skipf`)
- No external assertion libraries (no testify, no gomega)

**Run Commands:**
```bash
make test              # Run all tests with race detector + coverage
make test-fast         # Run tests without race detector (faster)
make test-verbose      # Run tests with verbose output
make test-specific TEST=TestFoo   # Run specific test by name
make bench             # Run all benchmarks
make bench-verbose     # Run benchmarks with verbose output
make cover             # Generate HTML coverage report (after test)
go test -race ./...    # Direct race-enabled test run
go test -bench=. -benchmem -run=^$ ./...  # Direct benchmark run
```

## Test File Organization

**Location:** Test files are co-located with source files in the same package (standard Go convention).

**Naming:**
- `<name>_test.go` — primary test file (e.g., `rollback_test.go`, `runner_test.go`)
- `<name>_extra_test.go` — edge-case and coverage-boost tests (e.g., `manager_extra_test.go`, `compaction_extra_test.go`)
- `<name>_bench_test.go` — benchmark tests (e.g., `edit_benchmark_test.go`, `codecomplexity_benchmark_test.go`)
- `doc_test.go` — empty test to cover `doc.go` file presence (e.g., `pkg/bisect/doc_test.go`)
- `e2e_test.go` — root-level end-to-end tests that compile and run the binary

**Structure:**
```
internal/
  errors/
    errors.go           # source
    errors_test.go      # tests (same package: `package errors`)
  config/
    loader.go
    loader_test.go
    config_extra_test.go  # additional edge cases
pkg/
  rollback/
    rollback.go
    rollback_test.go
    doc.go
    doc_test.go          # covers doc.go
```

## Test Structure

**Package declaration:** Tests use the same package name as the source (whitebox testing):
```go
package rollback  // NOT package rollback_test

import (
    "testing"
    "github.com/eshanized/M31A/internal/git"
)
```

**Exception:** E2E tests use `_test` suffix package:
```go
package m31a_test  // blackbox test at root level
```

**Suite organization (table-driven tests):**
```go
func TestUserMessage(t *testing.T) {
    tests := []struct {
        name     string
        err      error
        expected string
    }{
        {"nil", nil, ""},
        {"ErrProviderUnreachable", ErrProviderUnreachable, "Provider unreachable..."},
        {"wrapped ErrInvalidKey", fmt.Errorf("auth failed: %w", ErrInvalidKey), "Invalid API key..."},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := UserMessage(tt.err)
            if got != tt.expected {
                t.Errorf("UserMessage(%v) = %q, want %q", tt.err, got, tt.expected)
            }
        })
    }
}
```

**Subtests with `t.Run`:**
```go
func TestHasUncommittedChanges(t *testing.T) {
    t.Run("clean", func(t *testing.T) {
        _, g := setupRollback(t)
        createCommits(g, 1)
        r := New(g)
        dirty, err := r.HasUncommittedChanges()
        // ...
    })
    t.Run("dirty", func(t *testing.T) {
        // ...
    })
    t.Run("modified", func(t *testing.T) {
        // ...
    })
}
```

**Setup pattern:**
```go
func setupRollback(t *testing.T) (*Rollback, *git.Git) {
    t.Helper()
    dir := t.TempDir()
    g := git.New(dir)
    if err := g.Init(); err != nil {
        t.Fatalf("Init failed: %v", err)
    }
    if err := g.ConfigUser("Test", "test@test.com"); err != nil {
        t.Fatalf("ConfigUser failed: %v", err)
    }
    return New(g), g
}
```

**Helper pattern:**
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

## Mocking

**Framework:** No external mocking library. Mocks are hand-written structs implementing interfaces.

**Mock pattern (keychain):**
```go
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

**Mock pattern (GitRunner interface for bisect):**
```go
// In pkg/bisect/bisect.go:
type GitRunner interface {
    Run(args ...string) (string, error)
}

// In tests, any struct implementing Run(args ...string) (string, error) works as a mock.
```

**Mock tools for dispatcher tests:** `mockTool` struct defined in `internal/tools/dispatcher_test.go`.

**What to mock:**
- External services (API providers, keychain, git operations)
- File system operations (via temp directories — prefer `t.TempDir()` over mocking)
- Time-dependent code (rare; usually tested with real time)

**What NOT to mock:**
- Internal data structures (test directly)
- Standard library functions
- Go built-in types

## Fixtures and Factories

**Test data creation:**
```go
func createCommits(g *git.Git, n int) {
    for i := 0; i < n; i++ {
        f := filepath.Join(g.WorkDir(), fmt.Sprintf("f%d.txt", i))
        if err := os.WriteFile(f, []byte(fmt.Sprintf("content %d", i)), 0644); err != nil {
            panic(fmt.Sprintf("WriteFile failed: %v", err))
        }
        if err := g.Commit(fmt.Sprintf("commit %d", i)); err != nil {
            panic(fmt.Sprintf("Commit %d failed: %v", i, err))
        }
    }
}
```

**Location:** Fixtures and factories are defined as helper functions in the test files themselves, not in separate fixture directories.

**Temp directories:** Always use `t.TempDir()` for test isolation. Never use shared directories.

**Test environment:**
```go
// internal/testutil/envtest.go provides:
testutil.RequireAPIKey(t, "OPENROUTER_API_KEY")  // Skip if env var not set
testutil.RequireAnyAPIKey(t, "KEY1", "KEY2")     // Skip if none set
testutil.LoadTestDotEnv(t)                        // Load .env.test file
```

## Coverage

**Targets:**
- **75%** overall
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**Run coverage:**
```bash
make test              # Generates coverage.out
make cover             # Generates coverage.html from coverage.out
go tool cover -html=coverage.out -o coverage.html  # Direct
```

**Coverage strategy:**
- Core packages (`taskrunner`, `bisect`, `rollback`) require 90% coverage
- Use `_extra_test.go` files to add edge-case tests for coverage gaps
- Test files excluded from `errcheck` and `unused` lint checks (`.golangci.yml`)

## Test Types

**Unit Tests:**
- Package-level tests in the same package (whitebox)
- Test individual functions and methods
- Use `t.TempDir()` for filesystem isolation
- Use hand-written mocks for external dependencies

**Integration Tests:**
- Config loading with real TOML files (`internal/config/loader_test.go`)
- Session persistence with real filesystem (`pkg/session/session_test.go`)
- Git operations with real temp repositories (`internal/git/git_test.go`)

**E2E Tests:**
- `e2e_test.go` at root level compiles and runs the `m31a` binary
- Uses `exec.Command` to invoke the binary
- Tests `--version`, `--help`, `--prompt` flags
- Real API tests (`TestBinary_Prompt_*RealAPI`) skip when env vars not set:
```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }
    // ...
}
```

**Benchmark Tests:**
- `Benchmark*` functions in `_test.go` or `_bench_test.go` files
- Use `b.ResetTimer()`, `b.StopTimer()`, `b.StartTimer()` for accurate measurement
- Use `b.RunParallel()` for concurrent benchmarks
- Use `b.ReportAllocs()` for memory allocation tracking
- Example locations: `pkg/narrative/benchmarks_test.go`, `internal/codeintel/bench_test.go`

## Common Patterns

**Parallel execution:**
```go
func TestExtractReviewNotes_EmptySection(t *testing.T) {
    t.Parallel()
    notes := extractReviewNotes("")
    if notes != nil {
        t.Errorf("expected nil for empty section, got %v", notes)
    }
}
```
Use `t.Parallel()` when the test has no shared mutable state. Avoid for tests that modify package-level variables or share resources.

**Error testing:**
```go
func TestRunner_CircularDependency(t *testing.T) {
    tasks := []types.Task{
        newTask(1, "a", []int{2}),
        newTask(2, "b", []int{1}),
    }
    r := New(tasks)

    _, err := r.Schedule()
    if err == nil {
        t.Fatal("Expected error for circular dependency")
    }
    if err != m31errors.ErrCircularDependency {
        t.Errorf("Expected ErrCircularDependency, got %v", err)
    }
}
```

**Callback testing:**
```go
func TestSoftReset_Callback_Invoked(t *testing.T) {
    callbackHash := ""
    callbackCalled := false
    onReset := func(newHead string) error {
        callbackCalled = true
        callbackHash = newHead
        return nil
    }

    result, err := r.SoftReset(firstHash, onReset)
    // ...
    if !callbackCalled {
        t.Error("Expected onReset callback to be called")
    }
}
```

**Sentinel error uniqueness:**
```go
func TestSentinelsAreUnique(t *testing.T) {
    sentinels := []error{
        ErrProviderUnreachable, ErrRateLimited, ErrInvalidKey, // ...
    }
    seen := make(map[error]bool)
    for i, s := range sentinels {
        if seen[s] {
            t.Errorf("duplicate sentinel: %v", s)
        }
        seen[s] = true
    }
}
```

**Doc coverage tests:**
```go
// pkg/bisect/doc_test.go
func TestDoc(t *testing.T) {
    t.Parallel()
    // Verify the package compiles and doc.go is present
}
```

**E2E helper functions:**
```go
func buildBinary(t *testing.T) string {
    t.Helper()
    bin := filepath.Join(t.TempDir(), "m31a")
    cmd := exec.Command("go", "build", "-o", bin, "./cmd/m31a")
    cmd.Dir = filepath.Join(mustGetwd(t), ".")
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("build failed: %v\n%s", err, string(out))
    }
    return bin
}

func cleanEnv() []string {
    return []string{
        "PATH=" + os.Getenv("PATH"),
        "HOME=" + os.TempDir(),
        "M31A_CONFIG=" + filepath.Join(os.TempDir(), "m31a-test-config.toml"),
    }
}
```

## Test Count

- **262** test files across the codebase
- **~100+** uses of `t.Parallel()` for concurrent test execution
- Tests run with race detector by default (`make test`)

---

*Testing analysis: 2026-07-10*
