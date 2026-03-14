# Testing Patterns

**Analysis Date:** 2026-06-12

## Test Framework

**Runner:**
- Go standard library `testing` package
- No external test frameworks (no testify, no gomock)

**Assertion Library:**
- Standard `testing.T` methods: `t.Fatal()`, `t.Fatalf()`, `t.Error()`, `t.Errorf()`, `t.Log()`, `t.Logf()`
- Manual assertions with conditional checks

**Run Commands:**
```bash
go test -race -cover ./...              # Run all tests with race detector and coverage
go test -cover ./...                    # Fast mode without race detector
go test -v -race -cover ./...           # Verbose output
go test -v -race -run TestSpecific ./... # Run specific test
go test -bench=. -benchmem -run=^$ ./... # Run benchmarks
```

**Coverage Targets:**
- 75% overall
- 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Test File Organization

**Location:**
- Co-located with source files (standard Go convention)
- Test files in same package as implementation

**Naming:**
- Test files: `*_test.go`
- Example: `engine_test.go`, `bash_test.go`, `session_test.go`

**Structure:**
```
internal/
├── workflow/
│   ├── engine.go
│   ├── engine_test.go
│   ├── plan.go
│   ├── plan_test.go
│   └── ...
pkg/
├── session/
│   ├── session.go
│   ├── session_test.go
│   └── ...
```

## Test Structure

**Suite Organization:**
- Table-driven tests for multiple scenarios
- Individual test functions for specific behaviors
- Helper functions prefixed with `setup*` or `make*`

**Pattern: Table-Driven Tests**
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
        {
            name:      "fallback question",
            content:   "What framework should we use?\nHow about testing?",
            wantLen:   2,
            wantFirst: "What framework should we use?",
        },
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            questions := parseQuestions(tt.content)
            if len(questions) != tt.wantLen {
                t.Errorf("Expected %d questions, got %d: %v", tt.wantLen, len(questions), questions)
            }
            if len(questions) > 0 && questions[0] != tt.wantFirst {
                t.Errorf("Expected first question %q, got %q", tt.wantFirst, questions[0])
            }
        })
    }
}
```

**Pattern: Individual Tests**
```go
func TestBash_SimpleCommand(t *testing.T) {
    t.Parallel()
    b := NewBash(t.TempDir())
    result, err := b.Execute(context.Background(), types.ToolInput{
        Name: "Bash",
        Params: map[string]any{
            "command": `echo "hello world"`,
        },
    })
    if err != nil {
        t.Fatal(err)
    }
    if !strings.Contains(result.Output, "hello world") {
        t.Errorf("expected 'hello world' in output, got: %s", result.Output)
    }
}
```

**Setup Pattern:**
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
    mgr := session.NewManager(sessionBaseDir, session.ManagerOpts{})

    s, err := mgr.NewSession("test-model", "test-provider")
    if err != nil {
        t.Fatalf("NewSession failed: %v", err)
    }

    // ... setup dispatcher, estimator, etc.

    engine, err := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
        &mockProvider{}, "test-model", dispatcher, est, mgr, nil)
    engine.git = g

    cleanup := func() {}
    return engine, cleanup
}
```

**Teardown Pattern:**
- Use `defer os.RemoveAll(dir)` for temp directories
- Use `t.TempDir()` for automatic cleanup
- Cleanup functions returned from setup helpers

**Assertion Pattern:**
```go
// Fatal for critical checks
if err != nil {
    t.Fatalf("Init failed: %v", err)
}

// Error for non-critical checks
if !g.IsRepo() {
    t.Fatal("Expected IsRepo to return true after Init")
}

// Log for expected conditions
if err == nil {
    t.Log("expected timeout error, got: %v", err)
}
```

## Mocking

**Framework:**
- Manual mock implementations (no mock generation tools)

**Patterns:**
```go
// Mock provider for testing
type mockProvider struct {
    response       string
    err            error
    callCount      int
    multiResponses []string // if set, returns responses[callCount] per call
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
    if content == "" {
        content = "OK"
    }
    done := false
    next := func() (*m31types.StreamChunk, error) {
        if done {
            return nil, io.EOF
        }
        done = true
        return &m31types.StreamChunk{Delta: content}, nil
    }
    close := func() error { return nil }
    return &m31types.StreamIterator{Next: next, Close: close}, m.err
}
func (m *mockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (m *mockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
    return m31types.HealthStatus{Status: "live"}
}
func (m *mockProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (m *mockProvider) CachedModels() []m31types.ModelInfo              { return nil }
```

**What to Mock:**
- External dependencies (LLM providers, file system, network)
- Complex state (git repositories, session managers)
- Time-dependent operations

**What NOT to Mock:**
- Core business logic
- Data structures
- Simple utility functions

## Fixtures and Factories

**Test Data:**
```go
// Create test session
s := NewSession("test-id", "gpt-4o", "openrouter")

// Create test task
task := types.Task{
    ID:                 1,
    Description:        "Create a REST API",
    Action:             "create",
    Dependencies:       []int{},
    Files:              []string{"api.go"},
    AcceptanceCriteria: []string{"Works correctly"},
    Status:             types.StatusPending,
}

// Create test message
msg := types.Message{
    Role:    "user",
    Content: "Hello, world!",
    CreatedAt: time.Now(),
}
```

**Location:**
- Inline in test files
- Setup helper functions: `setupTestEngine()`, `setupBisectRepo()`, `setupRepo()`

## Coverage

**Requirements:**
- 75% overall
- 90% for critical packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
go test -coverprofile=coverage.out ./...
go tool cover -html=coverage.out -o coverage.html
```

**Coverage Report:**
- Generated in project root: `coverage.out`
- HTML report: `coverage.html`

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Approach: Table-driven tests, mock external dependencies
- Example: `TestEngine_ParseQuestions`, `TestBash_SimpleCommand`

**Integration Tests:**
- Scope: Multiple components working together
- Approach: Real git repos, temp directories, mock LLM providers
- Example: `TestEngine_Transition`, `TestManager_UpdateWorkflowState_PersistsAndLoads`

**E2E Tests:**
- Framework: Not used (TUI testing is manual)

## Common Patterns

**Async Testing:**
```go
func TestBash_ContextCancellation(t *testing.T) {
    ctx, cancel := context.WithCancel(context.Background())
    b := NewBash(t.TempDir())

    errCh := make(chan error, 1)
    go func() {
        _, err := b.Execute(ctx, types.ToolInput{
            Name: "Bash",
            Params: map[string]any{
                "command": "sleep 30",
            },
        })
        errCh <- err
    }()

    time.Sleep(100 * time.Millisecond)
    cancel()

    select {
    case err := <-errCh:
        if err == nil {
            t.Log("context cancellation returned no error (expected error or cancellation)")
        }
    case <-time.After(5 * time.Second):
        t.Fatal("command not cancelled in time")
    }
}
```

**Error Testing:**
```go
func TestEngine_ErrorHandling(t *testing.T) {
    engine, _ := setupTestEngine(t)

    // Unknown phase should error
    _, err := engine.RunPhase(context.Background(), m31types.WorkflowPhase("unknown"), "goal")
    if err == nil {
        t.Fatal("Expected error for unknown phase")
    }
}
```

**Parallel Testing:**
```go
func TestBash_SimpleCommand(t *testing.T) {
    t.Parallel()
    // ... test implementation
}
```

**Table-Driven Tests:**
```go
func TestStripCodeBlocks(t *testing.T) {
    tests := []struct {
        name  string
        input string
        want  string
    }{
        {"json tag", "```json\n[{\"id\":1}]\n```", "[{\"id\":1}]\n"},
        {"go tag", "```go\nfunc main() {}\n```", "func main() {}\n"},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := stripCodeBlocks(tt.input)
            if got != tt.want {
                t.Errorf("Expected %q, got %q", tt.want, got)
            }
        })
    }
}
```

## Test Helper Functions

**Common Helpers:**
```go
// Helper to run git commands in tests
func runGit(t *testing.T, dir string, args ...string) {
    t.Helper()
    cmd := exec.Command("git", args...)
    cmd.Dir = dir
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("git %s failed: %v\n%s", args[0], err, string(out))
    }
}

// Helper to write files in tests
func writeFile(t *testing.T, dir, name, content string) {
    t.Helper()
    if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
        t.Fatalf("writeFile %s failed: %v", name, err)
    }
}

// Helper to get commit hash
func commitHash(t *testing.T, dir string) string {
    t.Helper()
    out, err := exec.Command("git", "rev-parse", "HEAD").CombinedOutput()
    if err != nil {
        t.Fatalf("rev-parse failed: %v", err)
    }
    return string(out[:len(out)-1]) // trim newline
}
```

**Helper Guidelines:**
- Always call `t.Helper()` at the start
- Use `t.Fatalf()` for failures (not `t.Error()`)
- Prefix with descriptive name: `setup*`, `run*`, `write*`, `get*`

## Test File Locations

**Key Test Files:**
- `internal/workflow/engine_test.go` — Engine initialization, transitions, context building
- `internal/tools/bash_test.go` — Bash tool execution, timeout, cancellation
- `pkg/session/session_test.go` — Session workflow state persistence
- `pkg/bisect/bisect_test.go` — Git bisect operations
- `internal/errors/errors_test.go` — Error message generation

**Test Coverage by Package:**
- `internal/workflow/` — 15+ test files covering all phases
- `internal/tools/` — 12+ test files for each tool
- `pkg/session/` — 5 test files for session lifecycle
- `pkg/bisect/` — Comprehensive bisect testing
- `internal/provider/` — 8+ test files for provider layer

---

*Testing analysis: 2026-06-12*
