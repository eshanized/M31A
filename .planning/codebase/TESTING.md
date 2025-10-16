# Testing Patterns

**Analysis Date:** 2026-06-07

## Test Framework

**Runner:**
- Go's built-in `testing` package
- No external test frameworks (no testify, no gomock)
- Config: No separate config file — uses `go test` flags

**Assertion Library:**
- Standard library only: `t.Fatal()`, `t.Fatalf()`, `t.Error()`, `t.Errorf()`, `t.Log()`, `t.Logf()`
- No assertion helpers from external packages
- Manual comparison with clear error messages

**Run Commands:**
```bash
go test -race -cover ./...           # Run all tests with race detector and coverage
go test -cover ./...                 # Run tests without race detector (faster)
go test -v -race -cover ./...        # Run tests with verbose output
go test -v -race -run TestBash ./... # Run specific test
go test -bench=. -benchmem ./...     # Run benchmarks
make test                            # Via Makefile
make test-fast                       # Without race detector
make test-verbose                    # With verbose output
make test-specific TEST=TestBash    # Run specific test
```

## Test File Organization

**Location:**
- Co-located with source files in the same package
- Test files follow Go convention: `<source>_test.go`
- No separate `test/` directory

**Naming:**
- Test files: `bash_test.go`, `cache_test.go`, `app_test.go`
- Table-driven tests: `TestUserMessage`, `TestDispatcher_PermissionDenied_Typed`
- Helper functions: `setupTestEngine()`, `newTestManager()`, `toolInput()`

**Structure:**
```
internal/
├── tools/
│   ├── bash.go                    # Source
│   ├── bash_test.go               # Tests (co-located)
│   ├── bash_security_test.go      # Security-focused tests
│   └── bash_kill_test.go          # Kill/signal tests
├── provider/
│   ├── openrouter/
│   │   ├── client.go
│   │   └── client_test.go
│   └── cache_test.go
pkg/
├── session/
│   ├── manager.go
│   ├── manager_test.go
│   └── planning_test.go
```

## Test Structure

**Suite Organization:**
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

**Patterns:**
- Table-driven tests for multiple scenarios: `tests := []struct{ name string; ... }`
- Subtests: `t.Run("Background", func(t *testing.T) { ... })`
- Parallel execution: `t.Parallel()` at test start
- Helper functions with `t.Helper()`: `func newTestManager(t *testing.T) (*Manager, string)`
- Temp directories: `t.TempDir()` (auto-cleanup)

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
    
    // ... setup dispatcher, tools, etc.
    
    cleanup := func() {}
    return engine, cleanup
}
```

## Mocking

**Framework:** None — manual mock implementations

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
func (m *mockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
    return nil, nil
}
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
    content := m.response
    done := false
    next := func() (*types.StreamChunk, error) {
        if done {
            return nil, io.EOF
        }
        done = true
        return &types.StreamChunk{Delta: content}, nil
    }
    return &types.StreamIterator{Next: next, Close: func() error { return nil }}, nil
}
```

**What to Mock:**
- LLM provider responses (`mockProvider`)
- Keychain operations (`mockKeychain`)
- HTTP servers (using `httptest.NewServer`)
- File system operations (using `t.TempDir()`)

**What NOT to Mock:**
- Go standard library functions
- Simple data structures
- Configuration loading (use temp files instead)

**HTTP Test Server Pattern:**
```go
ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
    if r.URL.Path == "/auth/key" {
        w.WriteHeader(http.StatusOK)
        w.Write([]byte(`{"status":"ok"}`))
    }
}))
defer ts.Close()

c, _ := New("test-key", Options{})
c.baseURL = ts.URL
```

## Fixtures and Factories

**Test Data:**
```go
// Helper function for creating tool inputs
func toolInput(key, value string) types.ToolInput {
    return types.ToolInput{
        Name:   "test",
        Params: map[string]any{key: value},
    }
}

// Session manager test helper
func newTestManager(t *testing.T) (*Manager, string) {
    t.Helper()
    dir, err := os.MkdirTemp("", "m31a-session-test-*")
    if err != nil {
        t.Fatalf("Failed to create temp dir: %v", err)
    }
    return NewManager(dir, ManagerOpts{}), dir
}

// App test helper with API key
func newTestAppWithKey(t *testing.T) *AppState {
    t.Helper()
    tmpDir := t.TempDir()
    configPath := filepath.Join(tmpDir, "config.toml")
    cfg := `[provider]
default = "openrouter"
[provider.openrouter]
api_key = "sk-or-v1-test-key"
`
    if err := os.WriteFile(configPath, []byte(cfg), 0644); err != nil {
        t.Fatal(err)
    }
    app, err := NewApp("test", nil, configPath)
    if err != nil {
        t.Fatal(err)
    }
    return app
}
```

**Location:**
- Test helpers defined in `_test.go` files (package-private)
- No shared fixture files across packages
- Each package maintains its own test utilities

## Coverage

**Requirements:**
- No explicit coverage threshold enforced in CI
- CI runs `go test -race -coverprofile=coverage.out -covermode=atomic ./...`
- Coverage report uploaded as artifact

**View Coverage:**
```bash
go test -coverprofile=coverage.out ./...
go tool cover -html=coverage.out -o coverage.html
make cover                          # Via Makefile
```

**Current State:**
- 91 test files across 152 source files (~31,726 test LOC)
- Coverage varies by package:
  - `internal/tools/`: High coverage (bash, filewrite, grep, glob)
  - `internal/provider/`: High coverage (SSE, cache, registry, resilience)
  - `internal/tui/`: Moderate coverage (app, commands, streaming)
  - `pkg/session/`: High coverage (manager, planning, checkpoint)

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Pattern: Isolated tests with temp directories
- Example: `TestBash_SimpleCommand`, `TestModelCache_Get_StaleFallback`
- Parallel: Most unit tests run in parallel

**Integration Tests:**
- Scope: Multiple components working together
- Pattern: Full workflow execution with mocked providers
- Example: `setupTestEngine()` creates complete environment
- Files: `workflow/engine_test.go`, `workflow/integration_test.go`
- Always run sequentially (not parallel)

**E2E Tests:**
- Framework: Not used — no external E2E test framework
- Pattern: Manual integration tests against real providers
- Coverage: Tested via `make dev` with real API keys

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
            t.Log("context cancellation returned no error")
        }
    case <-time.After(5 * time.Second):
        t.Fatal("command not cancelled in time")
    }
}
```

**Error Testing:**
```go
func TestDispatcher_PermissionDenied_Typed(t *testing.T) {
    d := NewDispatcher(nil)
    bash := NewBash(t.TempDir())
    d.Register(bash)

    call := types.ToolCall{
        ID:   "test-1",
        Name: "Bash",
    }
    callBytes, _ := json.Marshal(map[string]any{
        "name": "Bash",
        "params": map[string]any{
            "command": "echo test",
        },
    })
    call.Input = callBytes

    ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
    defer cancel()

    type result struct {
        res types.ToolResult
        err error
    }
    ch := make(chan result, 1)
    go func() {
        r, e := d.Execute(ctx, call)
        ch <- result{r, e}
    }()

    // Deny permission
    req := <-d.RequestCh()
    d.ApprovePermission(req.ID, false, false)

    select {
    case r := <-ch:
        if r.err == nil && r.res.Error == "" {
            t.Fatal("expected error for permission denial")
        }
        if !errors.Is(r.err, m31errors.ErrPermissionDenied) {
            t.Fatalf("expected ErrPermissionDenied, got: %v", r.err)
        }
    case <-ctx.Done():
        t.Fatal("timed out waiting for Execute to return")
    }
}
```

**Table-Driven Tests:**
```go
func TestUserMessage(t *testing.T) {
    tests := []struct {
        name     string
        err      error
        expected string
    }{
        {"nil", nil, ""},
        {"ErrProviderUnreachable", ErrProviderUnreachable, "Provider unreachable — check your internet connection"},
        {"wrapped ErrInvalidKey", fmt.Errorf("auth failed: %w", ErrInvalidKey), "Invalid API key — run /settings to update"},
        {"unknown", errors.New("something completely unexpected"), "An unexpected error occurred"},
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

**Interface Compliance Check:**
```go
func TestProvider_OpenRouter_APIKey(t *testing.T) {
    // Compile-time check — if APIKey() is removed from the interface,
    // this test file won't compile.
    var _ LLMProvider = (*mockProvider)(nil)
}
```

## Test Conventions

**Naming:**
- `Test<Type>_<Method>` for simple tests: `TestBash_SimpleCommand`
- `Test<Type>_<Method>_<Scenario>` for complex tests: `TestModelCache_Get_StaleFallback`
- Helper functions: `new<Type>()` or `setup<Type>()` prefix

**Assertions:**
- Use `t.Fatal()` for critical failures (test cannot continue)
- Use `t.Error()` for non-critical failures (test can continue)
- Use `t.Logf()` for debug information
- Never use `assert` package — manual comparisons only

**Cleanup:**
- Use `t.TempDir()` for automatic cleanup
- Use `defer os.RemoveAll(dir)` for manual cleanup
- Use `defer ts.Close()` for HTTP test servers
- Use `defer cancel()` for context cancellation

**CI Integration:**
- All tests run with race detector: `-race`
- Coverage uploaded as artifact
- Tests run on: ubuntu-latest, macos-latest, windows-latest
- Go version: 1.22 (consistent across all platforms)

---

*Testing analysis: 2026-06-07*
