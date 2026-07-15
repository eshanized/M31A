# Testing Patterns

**Analysis Date:** 2026-07-16

## Test Framework

**Runner:**
- **Standard library `testing`** — All tests use `go test`
- **Config:** No separate config file; flags passed via `make` targets

**Run Commands:**
```bash
make test           # Race-enabled tests with coverage (go test -race -cover -coverprofile=coverage.out ./...)
make test-fast      # Tests without race detector (go test -cover ./...)
make test-verbose   # Verbose output with race (go test -v -race -cover ./...)
make test-specific TEST=TestFoo  # Single test (go test -v -race -run TestFoo ./...)
make bench          # Benchmarks (go test -bench=. -benchmem -run=^$ ./...)
make bench-verbose  # Verbose benchmarks
make cover          # HTML coverage report (go tool cover -html=coverage.out -o coverage.html)
```

**Assertion Library:** Standard library only — no external assertion library. Uses `if err != nil { t.Fatalf(...) }` and direct comparisons.

---

## Test File Organization

**Location:** Co-located with source files in same package (`*_test.go`).

**Naming:**
- Unit tests: `<file>_test.go` (e.g., `engine_test.go`, `runner_test.go`)
- Extra edge-case tests: `<file>_extra_test.go` (e.g., `compaction_extra_test.go`, `coverage_gates_extra_test.go`)
- Benchmarks: `benchmarks_test.go` or `<file>_bench_test.go`
- Integration/E2E: `e2e_test.go` (package `m31a_test`), `integration_test.go` (in `internal/workflow/`)

**Structure:**
```
pkg/
  taskrunner/
    runner_test.go        # 677 lines — comprehensive DAG scheduler tests
    doc_test.go           # Package-level example tests
  bisect/
    bisect_test.go        # 661 lines — git bisect automation tests
    extra_test.go         # Edge cases
    doc_test.go
  rollback/
    rollback_test.go      # 1198 lines — git rollback tests
    doc_test.go
  narrative/
    engine_test.go        # Core narrative engine tests
    grouper_test.go       # Event grouping tests
    classifier_test.go    # Event classification tests
  ...
internal/
  workflow/
    engine_test.go        # 1065 lines — workflow engine tests
    engine_race_test.go   # Race detector tests
    integration_test.go   # Full workflow integration tests
    state_machine_test.go # Phase transition tests
    verify_test.go        # Verification phase tests
    coverage_gates_test.go # Plan quality gate tests
    ... (30+ test files)
  tools/
    dispatcher_test.go    # 1006 lines — tool dispatcher tests
    extra_test.go         # 1800+ lines — tool edge cases
  session/
    session_test.go       # Session persistence tests
    manager_test.go       # Session manager tests
    ...
```

---

## Test Structure

### Suite Organization
- **Table-driven tests** preferred for parameterized cases
- **`t.Run()`** for sub-tests with descriptive names
- **`t.Parallel()`** used where safe (no shared state)
- **Helper functions** prefixed with `setup` or `test` (e.g., `setupTestEngine`, `testDispatcher`)

### Standard Test Pattern
```go
func TestEngine_Initialization(t *testing.T) {
    engine, cleanup := setupTestEngine(t)
    defer cleanup()

    if engine.sessionID == "" {
        t.Fatal("Expected non-empty session ID")
    }
    if engine.workDir == "" {
        t.Fatal("Expected non-empty workDir")
    }
}

func TestDispatcher_RegisterAndExecute(t *testing.T) {
    t.Parallel()
    d := testDispatcher(t)
    d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

    result, err := d.Execute(context.Background(), types.ToolCall{
        ID:    "call1",
        Name:  "test",
        Input: []byte(`{}`),
    })
    if err != nil {
        t.Fatal(err)
    }
    if result.Output != "<tool_output>\nok\n</tool_output>" {
        t.Errorf("expected '<tool_output>\nok\n</tool_output>', got %q", result.Output)
    }
}
```

### Helper Pattern
```go
func setupTestEngine(t *testing.T) (*Engine, func()) {
    t.Helper()
    dir := t.TempDir()

    g := git.New(dir)
    g.Init()
    g.ConfigUser("Test", "test@test.com")

    sessionBaseDir := filepath.Join(dir, "sessions")
    os.MkdirAll(sessionBaseDir, 0755)
    mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

    s, err := mgr.NewSession("test-model", "test-provider")
    if err != nil {
        t.Fatalf("NewSession failed: %v", err)
    }

    dispatcher := tools.NewDispatcher(nil)
    dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
    // ... register more tools
    dispatcher.SetPermission("Bash", true) // pre-approve for tests

    est := tokens.NewEstimator("test-model")

    engine, _ := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
        &mockProvider{}, "test-model", dispatcher, est, mgr, nil)
    engine.git = g

    return engine, func() {}
}
```

### Table-Driven Tests
```go
func TestRunner_NewWithExistingStatus(t *testing.T) {
    tests := []struct {
        name          string
        initialStatus types.TaskStatus
        expected      types.TaskStatus
    }{
        {"empty status defaults to pending", "", types.StatusPending},
        {"preserves running", types.StatusRunning, types.StatusRunning},
        {"preserves done", types.StatusDone, types.StatusDone},
        {"preserves failed", types.StatusFailed, types.StatusFailed},
        {"preserves skipped", types.StatusSkipped, types.StatusSkipped},
        {"preserves unrecoverable", types.StatusUnrecoverable, types.StatusUnrecoverable},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            task := types.Task{ID: 1, Description: "test", Action: "Create", Status: tt.initialStatus}
            r := New([]types.Task{task})
            if r.Status(1) != tt.expected {
                t.Errorf("Expected %s, got %s", tt.expected, r.Status(1))
            }
        })
    }
}
```

### Race Tests
```go
func TestConcurrentSetModelAndProviderAndModel(t *testing.T) {
    engine, _ := setupTestEngine(t)

    var wg sync.WaitGroup
    const goroutines = 10
    const iterations = 100

    // Concurrent writers: SetModel
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                p := &mockProviderWithModel{model: &types.ModelInfo{ID: "model-writer"}}
                engine.SetModel("model-writer", p)
            }
        }(i)
    }

    // Concurrent readers: providerAndModel
    for i := 0; i < goroutines; i++ {
        wg.Add(1)
        go func(id int) {
            defer wg.Done()
            for j := 0; j < iterations; j++ {
                p, m := engine.providerAndModel()
                if p != nil && m == "" {
                    t.Errorf("goroutine %d iter %d: provider set but modelID empty", id, j)
                }
            }
        }(i)
    }

    wg.Wait()
}
```

---

## Mocking

### Framework
- **No external mocking library** — hand-written mocks (structs with function fields)
- **Interface-based** — mocks implement the same interfaces as production code

### Mock Patterns

**Provider Mock** (`internal/workflow/engine_test.go`):
```go
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
    if content == "" { content = "OK" }
    done := false
    next := func() (*m31types.StreamChunk, error) {
        if done { return nil, io.EOF }
        done = true
        return &m31types.StreamChunk{Delta: content}, nil
    }
    close := func() error { return nil }
    return &m31types.StreamIterator{Next: next, Close: close}, m.err
}
func (m *mockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (m *mockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus { return m31types.HealthStatus{Status: "live"} }
func (m *mockProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (m *mockProvider) CachedModels() []m31types.ModelInfo { return nil }
```

**Tool Mock** (`internal/tools/dispatcher_test.go`):
```go
type mockTool struct {
    name      string
    riskLevel types.RiskLevel
    execFunc  func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

func (m *mockTool) Name() string               { return m.name }
func (m *mockTool) Description() string        { return "mock tool for testing" }
func (m *mockTool) RiskLevel() types.RiskLevel { return m.riskLevel }
func (m *mockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    if m.execFunc != nil {
        return m.execFunc(ctx, input)
    }
    return types.ToolResult{Output: "ok"}, nil
}
```

**Dispatcher Test Helper:**
```go
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    d := NewDispatcher(&config.PermissionsConfig{TimeoutSeconds: 1})
    // Override permission channel buffer for test speed
    return d
}
```

### What to Mock
- External APIs (LLM providers, HTTP calls)
- File system (use `t.TempDir()` for real FS)
- Time (inject `now` function for deterministic tests)
- Git operations (use real git in temp dir via `t.TempDir()`)

### What NOT to Mock
- Internal logic (test real implementation)
- Standard library (time, os, filepath — use `t.TempDir()`, inject time func)
- Data structures (test real types)

---

## Fixtures and Factories

### Test Data Builders
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

### Inline Test Data
- JSON strings for plan parsing tests
- Map literals for tool input params
- Temp directories for file operations (`t.TempDir()`)

### Git Repo Fixture
```go
func setupBisectRepo(t *testing.T) (string, *Bisect) {
    t.Helper()
    dir := t.TempDir()

    runGit(t, dir, "init")
    runGit(t, dir, "config", "user.name", "Test")
    runGit(t, dir, "config", "user.email", "test@test.com")

    writeFile(t, dir, "main.go", "package main\nfunc main() {}\n")
    runGit(t, dir, "add", "-A")
    runGit(t, dir, "commit", "-m", "initial")

    // ... more commits
    b := New(dir, slog.Default())
    return dir, b
}
```

---

## Coverage

**Requirements:**
- **75%** overall coverage
- **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover          # Generates coverage.html
go tool cover -func=coverage.out  # Terminal summary
```

**Coverage Gates:** `internal/workflow/coverage_gates_test.go` enforces plan quality checks (granularity, security, gap analysis).

---

## Test Types

### Unit Tests
- **Scope:** Single package, isolated logic
- **Examples:** `runner_test.go`, `grouper_test.go`, `state_machine_test.go`
- **Speed:** Fast (< 100ms each)
- **Parallel:** Safe (`t.Parallel()`)

### Integration Tests
- **Scope:** Multiple packages working together
- **Examples:** `integration_test.go` (full workflow), `engine_test.go` (engine + session + tools)
- **Setup:** Real git repo, real temp dirs, mock providers
- **Speed:** Medium (1-5s each)

### E2E Tests
- **File:** `e2e_test.go` (package `m31a_test`)
- **Scope:** Compiles and runs the actual binary
- **Real API Tests:** `TestBinary_Prompt_NvidiaRealAPI`, `TestBinary_Prompt_ZenRealAPI`, `TestBinary_Prompt_OpenRouterRealAPI`
  - Require env vars: `NVIDIA_API_KEY`, `ZEN_API_KEY`, `OPENROUTER_API_KEY`
  - **Skip gracefully** when unset (`t.Skip()`)
- **Speed:** Slow (10-30s each)

### Race Tests
- **File:** `*_race_test.go` (e.g., `engine_race_test.go`, `messages_race_test.go`)
- **Scope:** Concurrent access patterns
- **Run:** `go test -race ./...` (part of `make test`)
- **Patterns:** 10-100 goroutines × 100 iterations

### Benchmark Tests
- **File:** `benchmarks_test.go` (e.g., `pkg/narrative/benchmarks_test.go`, `internal/codeintel/bench_test.go`)
- **Run:** `make bench`

---

## Common Patterns

### Async Tool Execution Test
```go
func TestDispatcher_DangerousToolPermissionGranted(t *testing.T) {
    d := testDispatcher(t)
    d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

    errCh := make(chan error, 1)
    go func() {
        _, err := d.Execute(context.Background(), types.ToolCall{
            ID:    "call1",
            Name:  "bash",
            Input: []byte(`{"name": "bash", "params": {"command": "echo hello"}}`),
        })
        errCh <- err
    }()

    req := <-d.RequestCh()
    if req.ToolName != "bash" { t.Errorf(...) }
    d.ApprovePermission(req.ID, true, false)

    if err := <-errCh; err != nil {
        t.Errorf("expected nil error after approval, got: %v", err)
    }
}
```

### State Machine Test
```go
func TestStateMachine_ValidTransition(t *testing.T) {
    sm := NewStateMachine()

    if err := sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize); err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
    if sm.CurrentPhase() != m31types.PhaseInitialize {
        t.Errorf("expected PhaseInitialize, got %s", sm.CurrentPhase())
    }
}
```

### Error Assertion Pattern
```go
_, err := r.Schedule()
if err == nil {
    t.Fatal("Expected error for circular dependency")
}
if err != m31errors.ErrCircularDependency {
    t.Errorf("Expected ErrCircularDependency, got %v", err)
}
```

### Snapshot/Golden File Pattern
Not used — tests assert on structured output directly.

---

## Special Test Categories

### Security Tests
- **Command Injection:** `internal/tools/bash_test.go` — `TestBash_ObfuscationDetection` (double spaces, tabs, mixed case, variable expansion, newlines)
- **SSRF Protection:** `internal/tools/webfetch_test.go` — `TestWebFetch_SSRF*` (private IPs, metadata endpoints, DNS pinning)
- **Type Safety:** `internal/tools/permissions_test.go` — `TestPermissions_InvalidType` (comma-ok guards on interface values)

### Concurrency Tests
- `TestWorkflowCache_ConcurrentDynamicContext` — 100 goroutines, 1000 iterations
- `TestDNSCache_HighContention` — 50 goroutines, 1000 iterations
- `TestDNSCache_ConcurrentEviction` — low eviction threshold stress test

### Regression Tests
- `*_fixes_test.go` — tests for specific bug fixes (e.g., `internal/workflow/fixes_test.go`)
- Named after issue/bug ID when applicable

---

## Test Maintenance

**Adding Tests:**
1. Create `*_test.go` alongside source file
2. Use `t.TempDir()` for isolation
3. Use `t.Parallel()` for independent tests
4. Follow table-driven pattern for multiple cases
5. Add race test if concurrent access possible

**Running Tests:**
```bash
make check          # Full pipeline: fmt → tidy → vet → lint → test
make test           # Race + coverage
make test-fast      # Quick feedback loop
```

**CI:** Runs `make check` on every push/PR.

---

*Testing analysis: 2026-07-16*