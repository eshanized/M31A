# Testing Patterns

**Analysis Date:** 2026-06-04

## Test Framework

**Runner:**
- Standard Go `testing` package — no external test frameworks
- Config: no config file; run via `go test` or Makefile targets
- Go version: 1.24 (per `go.mod`)

**Assertion Library:**
- Standard `testing.T` methods: `t.Fatal()`, `t.Fatalf()`, `t.Error()`, `t.Errorf()`
- `errors.Is()` for sentinel error assertions
- `strings.Contains()` for output validation
- No testify, no gomock, no external assertion libraries

**Run Commands:**
```bash
go test -race -cover ./...          # Run all tests with race detector + coverage
go test -v -race -run TestName ./... # Run specific test
make test                            # Same as go test -race -cover
make test-specific TEST=TestReplModel # Run specific test by name
make bench                           # Run benchmarks
```

## Test File Organization

**Location:** Co-located with source files (`*_test.go` in same package)

**Naming:**
- Test files: `<source>_test.go` — `bash_test.go`, `registry_test.go`, `loader_test.go`
- Security tests: `<source>_security_test.go` — `bash_security_test.go`, `webfetch_security_test.go`
- Package tests use `_test` suffix for black-box testing: `package provider_test` (rare)

**Structure:**
```
internal/
├── tools/
│   ├── bash.go              # Source
│   ├── bash_unix.go         # Platform-specific source
│   ├── bash_test.go         # Tests (same package)
│   ├── bash_security_test.go # Security-focused tests
│   ├── fileread.go
│   ├── fileread_test.go
│   ├── filewrite.go
│   ├── filewrite_test.go
│   ├── glob.go
│   ├── glob_test.go
│   ├── grep.go
│   ├── grep_test.go
│   └── dispatcher_test.go
├── provider/
│   ├── interface.go
│   ├── registry.go
│   ├── registry_test.go
│   ├── cache.go
│   ├── cache_test.go
│   ├── cache_refresh_test.go
│   ├── sse.go
│   ├── sse_test.go
│   ├── reasoning.go
│   ├── reasoning_test.go
│   ├── fallback.go
│   ├── resilience_test.go
│   └── openrouter/
│       ├── client.go
│       └── client_test.go
```

## Test Structure

**Suite Organization:**
```go
func TestFunctionName(t *testing.T) {
    t.Parallel()
    // Setup
    dir := t.TempDir()
    tool := NewTool(dir)

    // Execute
    result, err := tool.Execute(context.Background(), types.ToolInput{
        Name: "ToolName",
        Params: map[string]any{...},
    })

    // Assert
    if err != nil {
        t.Fatal(err)
    }
    if !strings.Contains(result.Output, "expected") {
        t.Errorf("expected 'expected' in output, got: %s", result.Output)
    }
}
```

**Table-Driven Tests:**
```go
func TestSanitizeProviderError(t *testing.T) {
    tests := []struct {
        name       string
        statusCode int
        body       string
        contains   string
        notContain string
    }{
        {"bad request", 400, "invalid param", "Bad request", ""},
        {"unauthorized", 401, "", "Invalid API key", ""},
        {"rate limited", 429, "", "Rate limited", ""},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            result := sanitizeProviderError(tt.statusCode, tt.body)
            if !strings.Contains(result, tt.contains) {
                t.Errorf("sanitizeProviderError(%d, %q) = %q, want it to contain %q",
                    tt.statusCode, tt.body, result, tt.contains)
            }
        })
    }
}
```

**Subtests:**
```go
func TestCacheRefresh(t *testing.T) {
    t.Run("IsRefreshing returns false initially", func(t *testing.T) {
        cache := NewModelCache(5 * time.Minute)
        if cache.IsRefreshing() {
            t.Error("expected IsRefreshing to be false initially")
        }
    })

    t.Run("IsRefreshing returns true during refresh", func(t *testing.T) {
        cache := NewModelCache(5 * time.Minute)
        cache.refreshing.Store(true)
        if !cache.IsRefreshing() {
            t.Error("expected IsRefreshing to be true during refresh")
        }
    })
}
```

## Mocking

**Framework:** Hand-written mocks — no external mocking library

**Patterns:**

```go
// Mock provider (internal/provider/registry_test.go)
type mockProvider struct {
    name         string
    healthStatus string
}

func (m *mockProvider) Name() string { return m.name }
func (m *mockProvider) APIKey() string { return "mock-key" }
func (m *mockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) { return nil, nil }
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error) { return nil, nil }
func (m *mockProvider) EstimateCost(modelID string, usage types.Usage) float64 { return 0 }
func (m *mockProvider) HealthCheck(ctx context.Context) types.HealthStatus {
    return types.HealthStatus{Status: m.healthStatus}
}
func (m *mockProvider) GetModel(id string) (*types.ModelInfo, error) { return nil, nil }
```

```go
// Mock tool with configurable exec function (internal/tools/dispatcher_test.go)
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

```go
// Mock keychain (internal/config/loader_test.go)
type mockKeychain struct {
    store map[string]string
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

```go
// Mock ReadCloser for SSE testing (internal/provider/resilience_test.go)
type mockReadCloser struct {
    data    []byte
    offset  int
    closeFn func()
    closed  bool
}

func (m *mockReadCloser) Read(p []byte) (int, error) {
    if m.offset >= len(m.data) {
        return 0, io.EOF
    }
    n := copy(p, m.data[m.offset:])
    m.offset += n
    return n, nil
}

func (m *mockReadCloser) Close() error {
    if !m.closed {
        m.closed = true
        if m.closeFn != nil {
            m.closeFn()
        }
    }
    return nil
}
```

**What to Mock:**
- LLM providers (network calls)
- OS keychain (platform-specific)
- File system (use `t.TempDir()` instead)
- HTTP endpoints (use `httptest.NewServer`)

**What NOT to Mock:**
- Internal data structures
- Standard library functions
- Git operations (use real git in temp dirs)

## Fixtures and Factories

**Test Data:**
```go
// Helper function to create ToolInput (internal/tools/glob_test.go)
func toolInput(key, value string) types.ToolInput {
    return types.ToolInput{
        Name:   "test",
        Params: map[string]any{key: value},
    }
}

// Helper to set up a real git repo (internal/git/git_test.go)
func setupRepo(t *testing.T) (*Git, string) {
    t.Helper()
    dir := t.TempDir()
    g := New(dir)
    if err := g.Init(); err != nil {
        t.Fatalf("Init failed: %v", err)
    }
    if err := g.ConfigUser("Test User", "test@test.com"); err != nil {
        t.Fatalf("ConfigUser failed: %v", err)
    }
    return g, dir
}
```

**Location:** Inline in test files — no separate fixture directories.

## Coverage

**Requirements:** Race detector enabled (`-race`), coverage profile generated

**View Coverage:**
```bash
make cover         # Generate HTML report (coverage.html)
go tool cover -html=coverage.out -o coverage.html
```

**CI Coverage:** Uploaded as GitHub Actions artifact (`coverage.out`)

## Test Types

**Unit Tests:**
- Scope: Individual functions/methods in isolation
- Pattern: Construct → Execute → Assert
- Examples: `TestBash_SimpleCommand`, `TestFileRead_SimpleRead`, `TestGlob_SimplePattern`

**Integration Tests:**
- Scope: Multiple components working together
- Pattern: Real HTTP servers (`httptest.NewServer`), real git repos (`t.TempDir()`)
- Examples: `TestChatCompletionStream_*` (SSE parsing + provider client), `TestGit_*` (full git operations)

**E2E Tests:**
- Framework: Not used at this scale — integration tests cover the critical paths
- Full workflow tests are handled at the application level

## Common Patterns

**Async Testing:**
```go
func TestBash_ContextCancellation(t *testing.T) {
    ctx, cancel := context.WithCancel(context.Background())
    b := NewBash(t.TempDir())

    errCh := make(chan error, 1)
    go func() {
        _, err := b.Execute(ctx, types.ToolInput{
            Name:   "Bash",
            Params: map[string]any{"command": "sleep 30"},
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

**HTTP Server Mocking:**
```go
func TestHealthCheck_Live(t *testing.T) {
    ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        if r.URL.Path == "/auth/key" {
            w.WriteHeader(http.StatusOK)
            w.Write([]byte(`{"status":"ok"}`))
        }
    }))
    defer ts.Close()

    c, _ := New("test-key", Options{})
    c.baseURL = ts.URL  // Override base URL for testing

    status := c.HealthCheck(context.Background())
    if status.Status != "live" {
        t.Fatalf("expected status %q, got %q", "live", status.Status)
    }
}
```

**Permission Gating Tests:**
```go
func TestDispatcher_DangerousToolPermissionGranted(t *testing.T) {
    d := NewDispatcher(nil)
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

    req := <-d.RequestCh()  // Read from permission channel
    if req.ToolName != "bash" {
        t.Errorf("expected request for 'bash', got %q", req.ToolName)
    }
    d.ApprovePermission(true, false)  // Approve permission

    if err := <-errCh; err != nil {
        t.Errorf("expected nil error after approval, got: %v", err)
    }
}
```

**Error Assertion:**
```go
// Sentinel error comparison
if err != m31errors.ErrInvalidKey {
    t.Fatalf("expected ErrInvalidKey, got %v", err)
}

// Wrapped sentinel error (works with errors.Is)
wrappedErr := fmt.Errorf("auth failed: %w", m31errors.ErrInvalidKey)
if !errors.Is(wrappedErr, m31errors.ErrInvalidKey) {
    t.Errorf("expected wrapped error to match sentinel")
}

// Error message content
if !strings.Contains(err.Error(), "outside working directory") {
    t.Errorf("expected 'outside working directory' error, got: %v", err)
}
```

**Parallel Testing:**
```go
func TestTool_Something(t *testing.T) {
    t.Parallel()  // Mark test as safe for parallel execution
    // ... test body
}
```

**Temp Directory Isolation:**
```go
func TestFileWrite_SimpleWrite(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()      // Auto-cleaned after test
    backupDir := t.TempDir()
    fw := NewFileWrite(dir, backupDir)
    // ... test body using isolated directories
}
```

## Test Naming Conventions

**Pattern:** `Test<Type>_<Method>_<Scenario>`

**Examples:**
- `TestBash_SimpleCommand` — basic functionality
- `TestBash_Timeout` — edge case
- `TestBash_ContextCancellation` — concurrency scenario
- `TestFileWrite_PathOutsideWorkDir` — security boundary
- `TestFileWrite_BinaryContent` — error condition
- `TestChatCompletionStream_WithThinking` — feature-specific
- `TestRegistry_SetActive_Unknown` — error path
- `TestModelCache_Get_StaleFallback` — resilience scenario

## Test Helpers

**Common Helpers:**
```go
// t.Helper() for setup functions
func setupRepo(t *testing.T) (*Git, string) {
    t.Helper()
    // ... setup code
}

// ToolInput factory
func toolInput(key, value string) types.ToolInput {
    return types.ToolInput{
        Name:   "test",
        Params: map[string]any{key: value},
    }
}
```

## CI/CD Testing

**GitHub Actions:**
```yaml
# .github/workflows/ci.yml
test:
  runs-on: ubuntu-latest
  steps:
    - name: Run tests
      run: go test -race -coverprofile=coverage.out -covermode=atomic ./...
    - name: Upload coverage
      uses: actions/upload-artifact@v4

build:
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]
      arch: [amd64, arm64]
  steps:
    - name: Build
      run: CGO_ENABLED=0 GOARCH=${{ matrix.arch }} go build -o m31a ./cmd/m31a
    - name: Vet
      run: GOARCH=${{ matrix.arch }} go vet ./cmd/m31a
```

**Makefile Targets:**
- `make test` — `go test -race -cover -coverprofile=coverage.out ./...`
- `make test-fast` — `go test -cover ./...` (no race detector)
- `make test-verbose` — `go test -v -race -cover ./...`
- `make test-specific TEST=TestName` — Run specific test
- `make bench` — `go test -bench=. -benchmem -run=^$ ./...`
- `make cover` — Generate HTML coverage report
- `make lint` — `golangci-lint run ./... --timeout=5m`
- `make check` — `fmt tidy vet test` (full CI pipeline)

## Key Testing Statistics

- **Test files:** 80
- **Total Go files:** 194
- **Test-to-source ratio:** ~41%
- **Framework:** Standard `testing` only
- **Mocking:** Hand-written mocks
- **CI:** GitHub Actions with race detector + coverage

---

*Testing analysis: 2026-06-04*
