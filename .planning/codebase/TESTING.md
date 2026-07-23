# Testing Patterns

**Analysis Date:** 2026-07-23

## Test Framework

**Runner:** `go test` (standard library)
- Race detector: `go test -race ./...` (mandatory per `make test`)
- Coverage: `go test -cover -coverprofile=coverage.out ./...`
- HTML report: `make cover` → `coverage.html`
- Timeout: 30s default (`-timeout 30s` in Makefile)

**Assertion:** Standard library `testing.T` methods
- `t.Fatal/f`, `t.Error/f`, `t.Errorf`, `t.Log/f`
- `t.Parallel()` for parallelizable tests
- `t.Cleanup()` for teardown (replaces `defer`)

**No external assertion libraries** — use `if got != want { t.Errorf(...) }` pattern.

## Test Commands

```bash
make test           # Race-enabled tests with coverage (canonical)
make test-fast      # Tests without race detector (faster)
make test-verbose   # Verbose output with race
make test-specific TEST=TestFoo   # Run single test
make cover          # Generate HTML coverage report
make check          # fmt → tidy → vet → lint → test (full CI)
```

## Coverage Targets

| Package | Target |
|---------|--------|
| Overall | 75% |
| `pkg/taskrunner` | 90% |
| `pkg/bisect` | 90% |
| `pkg/rollback` | 90% |

## Test Organization

**Location:**
- Unit tests: co-located with source (`*_test.go` in same package)
- Integration tests: `tests/testutil/integration/`
- E2E tests: `tests/e2e/e2e_test.go` (package `m31a_test` — black box)
- Test utilities: `tests/testutil/`, `internal/testutil/`

**Naming:**
- Test functions: `Test<Type>_<Scenario>` or `Test<Function>_<Case>`
- Table-driven: `Test<Function>` with `t.Run` subtests
- Benchmarks: `Benchmark<Function>` in `*_test.go`

**Structure:**
```go
func TestFunction_Scenario(t *testing.T) {
    t.Parallel()  // when no shared state

    // Setup
    dir := t.TempDir()
    obj := NewObject(dir)

    // Act
    result, err := obj.Method(ctx, input)

    // Assert
    if err != nil { t.Fatal(err) }
    if result != expected { t.Errorf("got %v, want %v", result, expected) }
}
```

## Table-Driven Tests

**Pattern:** Slice of structs with `name`, input, expected output
```go
tests := []struct {
    name    string
    pattern string
    path    string
    want    bool
}{
    {"exact match", "Bash", "Bash", true},
    {"wildcard", "*", "Bash", true},
    {"no match", "Bash", "FileRead", false},
}

for _, tt := range tests {
    t.Run(tt.name, func(t *testing.T) {
        t.Parallel()
        got := matchToolName(tt.pattern, tt.tool)
        if got != tt.want {
            t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.tool, got, tt.want)
        }
    })
}
```

## Test Utilities

**Test Helpers (`tests/testutil/`):**
```go
// testutil/envtest.go
testutil.RequireAPIKey(t, "OPENROUTER_API_KEY")  // skips if unset
testutil.LoadTestDotEnv(t)  // loads .env.test once

// testutil/builders/dispatcher.go
builders.NewTestDispatcher(t)  // auto-stopped on cleanup
builders.NewTestDispatcherWithConfig(t, cfg)
```

**Mock Objects (`tests/testutil/mocks/`):**
```go
// mocks/provider.go
mocks.NewMockProvider("test")                    // basic
mocks.NewMockProviderWithResponse("test", "hi")  // with response

// mocks/tool.go
mocks.NewMockTool("name", types.RiskSafe)        // customizable ExecFunc
```

**Common Test Patterns:**
```go
// Temp dir for isolation
dir := t.TempDir()

// Context with timeout
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()

// t.Cleanup for resource release
dispatcher, err := tools.DefaultDispatcher(...)
t.Cleanup(func() { dispatcher.Stop() })
```

## Mocking Patterns

**Interface-Based Mocks:**
- Provider interface (`internal/integrations/provider.LLMProvider`) → `mocks.MockProvider`
- Tool interface (`internal/core/types.Tool`) → `mocks.MockTool`
- Keychain interface (`internal/integrations/keychain.Keychain`) → inline `mockKeychain` in test files

**Mock Features:**
- Configurable responses (`Response_`, `MultiResponses`, `Err_`)
- Call counting (`CallCount_`)
- Health status injection
- Tool call chunk simulation (streaming)

**Example — Provider Mock:**
```go
mp := mocks.NewMockProviderWithResponse("test", "response")
mp.MultiResponses = []string{"chunk1", "chunk2"}
mp.HealthStatus_ = types.HealthStatus{Status: "degraded"}
```

**Example — Tool Mock:**
```go
mock := &mocks.MockTool{
    Name_:      "test",
    RiskLevel_: types.RiskSafe,
    ExecFunc: func(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
        return types.ToolResult{Output: "custom"}, nil
    },
}
```

**Dispatcher Test Helper (`internal/tools/dispatcher_test.go`):**
```go
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    d, err := DefaultDispatcher("", "", "", nil, nil)
    if err != nil { t.Fatal(err) }
    t.Cleanup(func() { d.Stop() })
    return d
}

func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
    t.Helper()
    d, err := DefaultDispatcher("", "", "", cfg, nil)
    if err != nil { t.Fatal(err) }
    t.Cleanup(func() { d.Stop() })
    return d
}
```

## Test Patterns by Category

### Unit Tests (Pure Logic)

**File:** `internal/core/errors/errors_test.go`
```go
func TestUserMessage(t *testing.T) {
    tests := []struct {
        name     string
        err      error
        expected string
    }{
        {"nil", nil, ""},
        {"ErrRateLimited", ErrRateLimited, "Rate limited — retry in a moment"},
        {"wrapped ErrInvalidKey", fmt.Errorf("auth failed: %w", ErrInvalidKey), "Invalid API key — run /settings to update"},
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

**Error Type Tests:**
```go
func TestToolError(t *testing.T) {
    inner := errors.New("file not found")
    toolErr := &ToolError{Tool: "bash", Op: "execute", Err: inner}

    // Error()
    want := "tool bash: execute: file not found"
    if got := toolErr.Error(); got != want { t.Errorf(...) }

    // Unwrap()
    if !errors.Is(toolErr, inner) { t.Error("should unwrap") }

    // errors.As
    var target *ToolError
    if !errors.As(toolErr, &target) { t.Error("errors.As should find *ToolError") }
}
```

### Integration Tests (Real I/O)

**File:** `tests/testutil/integration/tool_integration_test.go`
```go
func TestEdit_Execute_ExactMatch_FileIO(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()
    backupDir := filepath.Join(dir, "backups")
    os.MkdirAll(backupDir, 0755)

    content := "package main\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"
    filePath := filepath.Join(dir, "main.go")
    os.WriteFile(filePath, []byte(content), 0644)

    edit := tools.NewEdit(dir, backupDir)
    result, err := edit.Execute(context.Background(), types.ToolInput{
        Params: map[string]any{
            "path":       "main.go",
            "old_string": "fmt.Println(\"hello\")",
            "new_string": "fmt.Println(\"world\")",
        },
    })
    if err != nil { t.Fatalf("Execute failed: %v", err) }
    if result.Error != "" { t.Fatalf("Tool error: %s", result.Error) }

    got, _ := os.ReadFile(filePath)
    if !strings.Contains(string(got), "world") {
        t.Errorf("file content = %q, want contains 'world'", string(got))
    }
}
```

### Race Detector Tests (Concurrency)

**Required for:** All concurrent data structures, goroutine lifecycle, shared mutable state
```go
func TestWorkflowCache_ConcurrentDynamicContext(t *testing.T) {
    // 100 goroutines, 1000 iterations each
    // Run with: go test -race ./...
}
```

**High-Contention Tests (from TESTING.md):**
- `TestWorkflowCache_ConcurrentDynamicContext` — 100 goroutines × 1000 iters
- `TestDNSCache_HighContention` — 50 goroutines × 1000 iters
- `TestDNSCache_ConcurrentEviction` — low eviction threshold stress

### TUI Tests (Bubble Tea)

**Harness:** `internal/ui/tui/tui_harness_test.go`
```go
func testAppState() *AppState {
    // Minimal AppState for testing — no external deps
    return &AppState{
        themeManager: theme.NewManager(theme.ModeDark),
        screen:       ScreenREPL,
        // ... nil-safe defaults
    }
}

// Key simulation
func updateWithKeyMsg(m *AppState, key string) (*AppState, tea.Cmd) {
    msg := testKeyMsg(key)
    model, batch := m.Update(msg)
    return model.(*AppState), batch
}
```

**Patterns:**
- Direct state manipulation for unit tests (bypass Update)
- `t.Parallel()` for screen routing, rendering tests
- Full Update→View cycles for integration
- `defer recover()` for panic safety in `View()` tests

### E2E Tests (Binary Execution)

**File:** `tests/e2e/e2e_test.go` (package `m31a_test`)
```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    ci.SkipIfCI(t, "requires NVIDIA_API_KEY...")
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" { t.Skip("NVIDIA_API_KEY not set") }

    bin := buildBinary(t)
    cmd := exec.Command(bin, "--prompt", "What is 2+2?", "--model", "meta/llama-3.1-8b-instruct")
    cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
    cmd.Dir = t.TempDir()

    out, err := cmd.CombinedOutput()
    if err != nil { t.Fatalf("failed: %v\noutput: %s", err, string(out)) }

    resp := strings.TrimSpace(string(out))
    if resp == "" { t.Fatal("expected non-empty response") }
    t.Logf("NVIDIA API response: %q", resp)
}
```

**Environment:**
- Clean env via `cleanEnv()` — no API keys, temp HOME
- Binary built fresh per test run (`go build -o ... ./cmd/m31a`)
- Real API tests skipped in CI (`ci.SkipIfCI`)

### Security Tests

**Command Injection** (`internal/tools/bash_security_test.go`):
```go
func TestBash_ObfuscationDetection(t *testing.T) {
    tests := []struct{
        name, cmd string
        wantBlock bool
    }{
        {"double space", "echo  hello", true},
        {"tab", "echo\thello", true},
        {"mixed case", "EcHo hello", true},
        {"var expansion", "echo $HOME", true},
        {"cmd sub", "echo $(id)", true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            // verify detection
        })
    }
}
```

**SSRF Protection** (`internal/tools/webfetch_security_test.go`):
```go
func TestWebFetch_SSRF_PrivateIPBlocked(t *testing.T) {
    // 127.0.0.1, 10.x, 172.16-31.x, 192.168.x, 169.254.169.254
}
```

**Type Safety** (`internal/tools/permissions_test.go`):
```go
func TestPermissions_InvalidType(t *testing.T) {
    // comma-ok assertion guards on interface values
    // graceful handling of invalid types in concurrent maps
}
```

## Test Lifecycle

**Setup (per test):**
- `t.TempDir()` for isolated filesystem
- `t.Cleanup()` for resource release (dispatcher.Stop, context cancel)
- `t.Setenv()` for environment overrides (auto-restored)

**Teardown:**
- Automatic via `t.Cleanup` (LIFO order)
- No global state pollution — each test independent

**Parallelism:**
- `t.Parallel()` by default for stateless tests
- Omit for tests with shared mutable state or sequential requirements

## Special Test Files

| File | Purpose |
|------|---------|
| `internal/tools/extra_test.go` | Large test file (127KB) — edge cases, stress tests |
| `internal/tools/dispatcher_test.go` | 991 lines — comprehensive dispatcher coverage |
| `internal/engine/workflow/engine_test.go` | 1028 lines — workflow engine, prompts, parsing |
| `internal/core/config/loader_test.go` | 981 lines — config loading, validation, merging |
| `internal/ui/tui/tui_harness_test.go` | TUI test infrastructure |

## CI Integration

**GitHub Actions (implied by Makefile):**
- `make check` runs full pipeline
- Race detector enabled
- Coverage uploaded
- Linter: `golangci-lint` with 5m timeout

**Skipped in CI:**
- Real API tests (`ci.SkipIfCI`)
- Tests requiring network/auth

---

*Testing analysis: 2026-07-23*