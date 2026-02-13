# Testing Patterns

**Analysis Date:** 2026-06-11

## Test Framework

**Runner:** Go standard `testing` package (`go test`)
**Assertion:** Standard `testing.T` methods (`t.Fatal`, `t.Fatalf`, `t.Error`, `t.Errorf`)
**Mocking:** Hand-written mocks (no mock generation framework)

**Run Commands:**
```bash
go test -race -cover ./...           # Run all tests with race detector and coverage
go test -v -race -cover ./...        # Verbose mode
go test -cover ./...                 # Fast mode (no race detector)
go test -run TestSpecificName ./...  # Run specific test
go test -bench=. -benchmem ./...     # Run benchmarks
make test                            # Via Makefile
make test-specific TEST=TestName     # Specific test via Makefile
```

## Test File Organization

**Location:** Co-located with source files (standard Go convention)

**Naming:** `*_test.go` suffix in the same package directory

**Structure:**
```
internal/tools/
├── bash.go
├── bash_test.go
├── fileread.go
├── fileread_test.go
├── dispatcher.go
├── dispatcher_test.go
└── permissions_test.go

pkg/session/
├── manager.go
├── manager_test.go
├── session.go
├── session_test.go
└── checkpoint_test.go
```

**Test packages:**
- Internal tests: `package tools` (white-box, access unexported symbols)
- External tests: `package session` (black-box, same package but testing exported API)
- Most tests use same package as source

## Test Structure

**Suite organization:** Flat test functions (no test suites or frameworks)

**Pattern:**
```go
func TestFunctionName_Scenario(t *testing.T) {
    t.Parallel()
    // Arrange
    dir := t.TempDir()
    // ...
    
    // Act
    result, err := someFunction(input)
    
    // Assert
    if err != nil {
        t.Fatal(err)
    }
    if result != expected {
        t.Errorf("expected %q, got %q", expected, result)
    }
}
```

**Naming convention:**
- `TestTypeName_MethodName_Scenario`: `TestBash_SimpleCommand`, `TestDispatcher_UnknownTool`
- `TestFunctionName_Scenario`: `TestCanConsolidate_Empty`, `TestMaskAPIKeys`

**Helper functions:**
- Use `t.Helper()` in test helper functions
- Prefix with descriptive name: `setupTestEngine`, `setupBisectRepo`, `setupRollback`
- Return cleanup function when needed: `func setupTestEngine(t *testing.T) (*Engine, func())`

## Mocking

**Framework:** Hand-written struct implementations

**Mock provider pattern:**
```go
type mockProvider struct {
    response       string
    err            error
    callCount      int
    multiResponses []string
}

func (m *mockProvider) Name() string   { return "mock" }
func (m *mockProvider) APIKey() string { return "test-key" }
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
    m.callCount++
    content := m.response
    if len(m.multiResponses) > 0 {
        idx := m.callCount - 1
        if idx < len(m.multiResponses) {
            content = m.multiResponses[idx]
        }
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
    return &m31types.StreamIterator{Next: next, Close: close}, nil
}
```

**Mock tool pattern:**
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

**HTTP test servers:**
```go
ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
    if r.URL.Path == "/models" {
        resp := map[string][]map[string]any{
            "data": {
                {"id": "gpt-4o", "name": "GPT-4o"},
            },
        }
        json.NewEncoder(w).Encode(resp)
    }
}))
defer ts.Close()

c, _ := New("test-key", Options{})
c.baseURL = ts.URL
```

**What to mock:**
- LLM providers (for workflow and tool tests)
- HTTP endpoints (for API client tests)
- File system (use `t.TempDir()` for isolation)

**What NOT to mock:**
- Git operations (use real git in temp directory)
- Standard library functions
- Configuration loading (pass config structs directly)

## Test Helpers

**Test data creation:**
```go
// makeMessages creates n alternating user/assistant messages with sample content.
func makeMessages(n int) []types.Message {
    msgs := make([]types.Message, n)
    for i := 0; i < n; i++ {
        role := "user"
        if i%2 == 1 {
            role = "assistant"
        }
        msgs[i] = types.Message{
            Role:      role,
            Content:   sampleContent(i),
            CreatedAt: time.Now().Add(-time.Duration(n-i) * time.Minute),
        }
    }
    return msgs
}
```

**Git test helpers:**
```go
func runGit(t *testing.T, dir string, args ...string) {
    t.Helper()
    cmd := exec.Command("git", args...)
    cmd.Dir = dir
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("git %s failed: %v\n%s", args[0], err, string(out))
    }
}

func writeFile(t *testing.T, dir, name, content string) {
    t.Helper()
    if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
        t.Fatalf("writeFile %s failed: %v", name, err)
    }
}
```

**Setup functions:**
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
    
    // ... more setup
    
    cleanup := func() {}
    return engine, cleanup
}
```

## Coverage

**Targets:**
- Overall: 75%
- Critical packages (`pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`): 90%

**View Coverage:**
```bash
make cover                    # Generate HTML report
go tool cover -html=coverage.out -o coverage.html
```

**CI coverage:**
```yaml
- name: Run tests
  run: go test -race -coverprofile=coverage.out -covermode=atomic ./...
- name: Upload coverage
  uses: actions/upload-artifact@v4
  with:
    name: coverage
    path: coverage.out
```

## Test Types

**Unit tests:**
- Scope: Single function or method
- Pattern: Direct function call with assertions
- Example: `TestBash_SimpleCommand`, `TestRunner_EmptyList`

**Integration tests:**
- Scope: Multiple components working together
- Pattern: Full workflow with mocked external dependencies
- Example: `TestFullWorkflow` in `internal/workflow/integration_test.go`
- Uses: Real git, temp directories, mocked LLM providers

**E2E tests:**
- Not present in current codebase (reserved for future)

**Security tests:**
- SSRF protection tests: `TestWebFetch_SSRFBlocksPrivateIP`
- Bash injection tests: `internal/tools/bash_security_test.go`
- Path traversal tests: `internal/tools/filewrite_test.go`

## Common Test Patterns

**Parallel execution:**
```go
func TestSomething(t *testing.T) {
    t.Parallel()
    // ...
}
```

**Table-driven tests:**
```go
func TestMaskAPIKeys(t *testing.T) {
    tests := []struct {
        name     string
        input    string
        expected string
    }{
        {name: "standard sk- key", input: "sk-abc123def456ghi7", expected: "sk-a****ghi7"},
        {name: "empty string", input: "", expected: ""},
        // ...
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := maskAPIKeys(tt.input)
            if got != tt.expected {
                t.Errorf("maskAPIKeys(%q) = %q, want %q", tt.input, got, tt.expected)
            }
        })
    }
}
```

**Temp directory isolation:**
```go
func TestFileWrite_SimpleWrite(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()
    backupDir := t.TempDir()
    fw := NewFileWrite(dir, backupDir)
    // ...
}
```

**Error assertion:**
```go
if err == nil {
    t.Fatal("expected error for private IP, got nil")
}
if !strings.Contains(err.Error(), "private") && !strings.Contains(err.Error(), "SSRF") {
    t.Errorf("expected SSRF/private IP error, got: %v", err)
}
```

**Sentinel error checking:**
```go
if err != m31errors.ErrInvalidKey {
    t.Fatalf("expected ErrInvalidKey, got %v", err)
}
```

**Skipping tests:**
```go
if e.tokenizer == nil {
    t.Skip("tiktoken-go not available for gpt-4o")
}
```

**Environment variable mocking:**
```go
func TestNewLogger_JSONFormat(t *testing.T) {
    tmpDir := t.TempDir()
    t.Setenv("HOME", tmpDir)
    t.Setenv("M31A_LOG_FORMAT", "json")
    // ...
}
```

## CI/CD Test Execution

**GitHub Actions workflow:** `.github/workflows/ci.yml`

**Jobs:**
1. **lint** — `gofmt` check + `golangci-lint` + GoReleaser config validation
2. **test** — `go test -race -coverprofile=coverage.out -covermode=atomic ./...`
3. **security** — `govulncheck ./...`
4. **build** — Cross-platform build matrix (linux/darwin/windows × amd64/arm64)
5. **release** — GoReleaser on tag push

**Test triggers:**
- Push to `master`
- Pull request to `master`
- Manual workflow dispatch

**Build matrix:**
```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    arch: [amd64, arm64]
    go: ["1.24"]
    exclude:
      - os: windows-latest
        arch: arm64
```

## Test File Locations

**Core packages:**
- `internal/tools/*_test.go` — Tool implementations
- `internal/provider/*_test.go` — Provider clients and SSE parsing
- `internal/workflow/*_test.go` — Workflow engine and phases
- `internal/tui/components/*_test.go` — TUI component tests
- `internal/tui/theme/*_test.go` — Theme system tests
- `internal/errors/errors_test.go` — Error message mapping
- `internal/config/loader_test.go` — Config loading
- `internal/tokens/*_test.go` — Token estimation
- `internal/git/git_test.go` — Git operations
- `internal/log/log_test.go` — Logger initialization

**Public packages:**
- `pkg/session/*_test.go` — Session lifecycle
- `pkg/taskrunner/runner_test.go` — Task scheduling
- `pkg/bisect/bisect_test.go` — Git bisect
- `pkg/rollback/rollback_test.go` — Rollback chain
- `pkg/autodream/autodream_test.go` — Context consolidation
- `pkg/ledger/ledger_test.go` — Cross-session learning
- `pkg/arbitrage/arbitrage_test.go` — Model cost optimization
- `pkg/keychain/keychain_test.go` — OS keychain

## Benchmark Tests

**Location:** Same test files or dedicated `_test.go` files

**Run:**
```bash
make bench           # Run all benchmarks
make bench-verbose   # Verbose output
go test -bench=. -benchmem -run=^$ ./...
```

**Pattern:** Standard Go benchmark functions (not observed in current codebase but supported by Makefile)

## Test Quality Rules

1. **Always use `t.Parallel()`** for independent tests
2. **Always use `t.TempDir()`** for file system tests (automatic cleanup)
3. **Always use `t.Helper()`** in helper functions
4. **Use `t.Fatal`/`t.Fatalf`** for setup failures (stop test immediately)
5. **Use `t.Error`/`t.Errorf`** for assertion failures (continue test)
6. **One assertion concept per test function**
7. **Test both success and error paths**
8. **Use descriptive test names** that explain the scenario
9. **Mock external dependencies** (HTTP, LLM providers)
10. **Use real implementations** for internal components (git, file system)

---

*Testing analysis: 2026-06-11*
