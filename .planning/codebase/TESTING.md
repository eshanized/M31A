# Testing Patterns

**Analysis Date:** 2026-07-04

## Test Framework

**Runner:**
- Go standard `testing` package
- No external test framework (no testify, gomock, etc.)

**Assertion Library:**
- Standard `testing.T` methods: `t.Fatal()`, `t.Fatalf()`, `t.Error()`, `t.Errorf()`
- Manual assertions with descriptive error messages

**Run Commands:**
```bash
make test           # Race-enabled tests with coverage
make test-fast      # Tests without race detector
make test-verbose   # Verbose output with race detector
make test-specific TEST=TestFoo  # Run single test
make bench          # Run benchmarks
make cover          # Generate HTML coverage report
```

## Test File Organization

**Location:**
- Co-located with source files (same package)
- Same directory as implementation

**Naming:**
- Standard tests: `*_test.go`
- Extra coverage tests: `*_extra_test.go`
- Doc/package compilation tests: `doc_test.go`
- Benchmark tests: `benchmarks_test.go` or in main test file

**Structure:**
```
pkg/bisect/
├── bisect.go           # Implementation
├── bisect_test.go      # Main tests
├── extra_test.go       # Additional coverage
├── doc.go              # Package documentation
└── doc_test.go         # Package compilation test
```

## Test Structure

**Suite Organization:**
```go
package workflow

import (
    "context"
    "testing"

    m31types "github.com/eshanized/M31A/internal/types"
)

func TestEngine_Initialization(t *testing.T) {
    engine, _ := setupTestEngine(t)

    if engine.sessionID == "" {
        t.Fatal("Expected non-empty session ID")
    }
}
```

**Patterns:**
- Table-driven tests with `[]struct` pattern
- Subtests with `t.Run(tt.name, func(t *testing.T) { ... })`
- Test helpers marked with `t.Helper()`
- Cleanup functions returned from setup helpers

## Mocking

**Framework:** Manual mocks (no mocking library)

**Patterns:**
```go
// Mock provider for testing
type mockProvider struct {
    response       string
    err            error
    callCount      int
    multiResponses []string
}

func (m *mockProvider) Name() string   { return "mock" }
func (m *mockProvider) APIKey() string { return "test-key" }
func (m *mockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
    return nil, nil
}
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
    m.callCount++
    content := m.response
    if len(m.multiResponses) > 0 {
        idx := m.callCount - 1
        if idx < len(m.multiResponses) {
            content = m.multiResponses[idx]
        }
    }
    // ... return mock stream
}
```

**What to Mock:**
- External API calls (LLM providers)
- File system operations (when testing logic, not I/O)
- Git operations (when testing algorithms, not git)

**What NOT to Mock:**
- Internal package functions (test the real thing)
- Simple data transformations
- Standard library functions

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
- Inline in test files (no separate fixture directory)
- Helper functions for common test data creation

## Coverage

**Requirements:**
- **75%** overall
- **90%** for critical packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover          # Generate HTML coverage report
go tool cover -html=coverage.out  # View in browser
```

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Pattern: Table-driven tests with edge cases
- Example: `TestRunner_DiamondDependency`, `TestBisect_Successful`

**Integration Tests:**
- Scope: Multiple components working together
- Pattern: Setup real dependencies (git repos, temp dirs)
- Example: `TestFullWorkflow` in `internal/workflow/integration_test.go`

**E2E Tests:**
- Framework: Compile and run binary
- Location: `e2e_test.go` at project root
- Pattern: Build binary, execute commands, verify output
- Example: `TestBinary_Version`, `TestBinary_Prompt_NvidiaRealAPI`

**Benchmarks:**
- Pattern: `BenchmarkXxx(b *testing.B)` with `b.ResetTimer()`
- Parallel benchmarks: `b.RunParallel()` for concurrent testing
- Example: `BenchmarkClassifierClassify`, `BenchmarkFullPipeline`

## Common Patterns

**Table-Driven Tests:**
```go
func TestEngine_ParseQuestions(t *testing.T) {
    tests := []struct {
        name      string
        content   string
        wantLen   int
        wantFirst string
    }{
        {
            name:      "numbered format",
            content:   "1. What framework?\n2. What language?",
            wantLen:   2,
            wantFirst: "What framework?",
        },
        // ... more cases
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            questions := parseQuestions(tt.content)
            if len(questions) != tt.wantLen {
                t.Errorf("Expected %d questions, got %d", tt.wantLen, len(questions))
            }
        })
    }
}
```

**Setup Helpers:**
```go
func setupTestEngine(t *testing.T) (*Engine, func()) {
    t.Helper()
    dir := t.TempDir()

    // Init git repo
    g := git.New(dir)
    g.Init()
    g.ConfigUser("Test", "test@test.com")

    // Create session
    sessionBaseDir := filepath.Join(dir, "sessions")
    os.MkdirAll(sessionBaseDir, 0755)
    mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

    s, err := mgr.NewSession("test-model", "test-provider")
    if err != nil {
        t.Fatalf("NewSession failed: %v", err)
    }

    // ... setup engine
    cleanup := func() {}
    return engine, cleanup
}
```

**Temp Directory Tests:**
```go
func TestRollback_Chain(t *testing.T) {
    _, g := setupRollback(t)
    createCommits(g, 5)

    r := New(g)
    entries, err := r.Chain(0)
    if err != nil {
        t.Fatalf("Chain failed: %v", err)
    }
    if len(entries) != 5 {
        t.Fatalf("Expected 5 entries, got %d", len(entries))
    }
}
```

**Concurrent Tests:**
```go
func TestStateMachine_ConcurrentTransitions(t *testing.T) {
    sm := NewStateMachine()
    sm.SetPhase(m31types.PhaseInitialize)

    var wg sync.WaitGroup
    errs := make(chan error, 100)

    for i := 0; i < 100; i++ {
        wg.Add(1)
        go func() {
            defer wg.Done()
            err := sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss)
            if err != nil {
                errs <- err
            }
        }()
    }

    wg.Wait()
    close(errs)

    successCount := 0
    for err := range errs {
        if err == nil {
            successCount++
        }
    }
    if successCount != 1 {
        t.Errorf("Expected exactly 1 successful transition, got %d", successCount)
    }
}
```

**E2E Test Pattern:**
```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }

    bin := buildBinary(t)
    cmd := exec.Command(bin, "--prompt", "What is 2+2?", "--model", "meta/llama-3.1-8b-instruct")
    cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
    cmd.Dir = t.TempDir()

    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("headless mode failed: %v\noutput: %s", err, string(out))
    }

    resp := strings.TrimSpace(string(out))
    if resp == "" {
        t.Fatal("expected non-empty response from NVIDIA API")
    }
    t.Logf("NVIDIA API response: %q", resp)
}
```

## Test Utilities

**Location:** `internal/testutil/envtest.go`

**Functions:**
- `RequireAPIKey(t, envVar)` — skip test if env var not set
- `RequireAnyAPIKey(t, envVars...)` — skip if none of the env vars set
- `LoadTestDotEnv(t)` — load `.env.test` into environment (once)

**Usage:**
```go
func TestLiveChat(t *testing.T) {
    key := testutil.RequireAPIKey(t, "OPENROUTER_API_KEY")
    // ... use key
}
```

## Build Tags

**Platform-Specific Tests:**
```go
//go:build linux

package keychain

import (
    "testing"
    // ... linux-specific imports
)

func TestValidateService_Valid(t *testing.T) {
    // ... linux-specific test
}
```

## Doc Tests

**Pattern:** Minimal tests that verify package compilation
```go
package session

import (
    "testing"
)

func TestDoc(t *testing.T) {
    t.Parallel()
    // Verify the package compiles and doc.go is present
    // This test exists to cover the doc.go file
}
```

## Key Conventions

- **No external test frameworks** — use standard `testing` package only
- **Table-driven tests preferred** — for multiple input/output cases
- **Setup helpers return cleanup** — `func()` for test isolation
- **t.Helper() in setup functions** — improves error reporting
- **t.TempDir() for file tests** — automatic cleanup
- **Race detector enabled** — `make test` uses `-race` flag
- **Skip real API tests** — use `t.Skip()` when env vars not set
- **Benchmarks with b.ResetTimer()** — exclude setup from timing

---

*Testing analysis: 2026-07-04*
