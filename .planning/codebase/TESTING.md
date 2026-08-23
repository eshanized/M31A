# Testing Patterns

**Analysis Date:** 2026-08-23

## Test Framework

**Runner:**
- Go standard library `testing` package
- Config: None (uses `go test` directly via Makefile)

**Run Commands:**
```bash
make test              # Run all tests with race detector and coverage (5m timeout)
make test-fast         # Run tests without race detector (faster)
make test-verbose      # Run tests with verbose output
make test-specific TEST=TestFoo   # Run specific test by name/regex
make cover             # Generate HTML coverage report (requires test first)
```

**Coverage Targets:**
- 75% overall
- 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Test File Organization

**Location:**
- Co-located with source: `<package>/<name>_test.go`
- Supplemental tests: `<name>_extra_test.go`, `<name>_fixes_test.go`, `<name>_coverage_boost_test.go`
- Benchmarks: `<name>_bench_test.go`

**Naming:**
- Test functions: `Test<Function>_<Scenario>` (e.g., `TestGrep_SimpleSearch`, `TestEdit_Execute_ExactMatch_FileIO`)
- Benchmark functions: `Benchmark<Function>`

**Directory Structure:**
```
internal/
  tools/
    grep_test.go
    grep_extra_test.go
    tools_test.go
    testutil_test.go        # Test helpers (testDispatcher)
  ui/tui/
    repl_model_test.go
    repl_state_extra_test.go
    app_channel_test.go
    handler_*_test.go
  core/types/
    types_test.go           # Rare — only for exported types

tests/
  testutil/
    mocks/
      provider.go           # MockProvider for LLM providers
      tool.go               # MockTool for tool interface
      dispatcher.go         # MockDispatcher
    integration/
      tool_integration_test.go  # Cross-package integration tests
    envtest.go              # Env var helpers, .env.test loading
  e2e/
    e2e_test.go             # Binary-level tests (compiles & runs m31a)
```

## Test Structure

**Suite Organization:**
- No setup/teardown suites (no `TestMain` except in e2e)
- Each test function independent
- `t.Parallel()` used extensively for parallel execution
- `t.TempDir()` for isolated filesystem

**Example from `internal/tools/grep_test.go`:**
```go
func TestGrep_SimpleSearch(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()
    os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"), 0644)

    g := search.NewGrep(dir)
    result, err := g.Execute(context.Background(), grepToolInput("pattern", "main"))
    if err != nil {
        t.Fatal(err)
    }
    if !strings.Contains(result.Output, "main.go") {
        t.Errorf("expected main.go in results, got: %s", result.Output)
    }
}
```

**Helper Functions:**
- Defined at bottom of test file or in `testutil_test.go`
- Use `t.Helper()` for correct line reporting
- Cleanup via `t.Cleanup()`

**Example from `internal/tools/testutil_test.go`:**
```go
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    d, _ := DefaultDispatcher("", "", "", nil, nil)
    t.Cleanup(func() { d.Stop() })
    return d
}
```

**Table-Driven Tests:**
```go
func TestParameterSchema_AllTools(t *testing.T) {
    t.Parallel()

    schemas := []struct {
        name   string
        schema string
    }{
        {"Bash", NewBash(".", 1800, nil, nil).ParameterSchema()},
        {"FileRead", fileops.NewFileRead(".").ParameterSchema()},
        // ...
    }

    for _, s := range schemas {
        t.Run(s.name, func(t *testing.T) {
            t.Parallel()
            if s.schema == "" {
                t.Errorf("%s: ParameterSchema() returned empty string", s.name)
                return
            }
            var raw json.RawMessage
            if err := json.Unmarshal([]byte(s.schema), &raw); err != nil {
                t.Errorf("%s: ParameterSchema() returned invalid JSON: %v", s.name, err)
            }
        })
    }
}
```

## Mocking

**Framework:**
- No mocking framework (no mockery, gomock, testify/mock)
- Hand-written mocks in `tests/testutil/mocks/`

**Mock Patterns:**

**Provider Mock (`tests/testutil/mocks/provider.go`):**
```go
type MockProvider struct {
    Name_          string
    APIKey_        string
    Response_      string
    Err_           error
    CallCount_     int
    MultiResponses []string
    HealthStatus_  types.HealthStatus
}

func (m *MockProvider) ChatCompletionStream(_ context.Context, _ provider.ChatRequest) (*types.StreamIterator, error) {
    m.CallCount_++
    // Returns StreamIterator with Next() and Close() funcs
    // Supports tool_call JSON and content responses
}
```

**Tool Mock (`tests/testutil/mocks/tool.go`):**
```go
type MockTool struct {
    Name_        string
    Description_ string
    RiskLevel_   types.RiskLevel
    ExecuteFn    func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
    Schema_      string
}
```

**Dispatcher Mock (`tests/testutil/mocks/dispatcher.go`):**
```go
type MockDispatcher struct {
    Tools_       map[string]types.Tool
    ExecuteFn    func(ctx context.Context, call types.ToolCall) (types.ToolResult, error)
    // Implements types.ToolDispatcher interface
}
```

**What to Mock:**
- External APIs (LLM providers, network calls)
- Time-dependent operations (use `context.WithTimeout`)
- File system for unit tests (use `t.TempDir()` with real FS preferred)

**What NOT to Mock:**
- File I/O — use `t.TempDir()` with real files (see integration tests)
- Internal logic — test real implementations
- Standard library — test behavior, not mock `os`, `io`, etc.

## Fixtures and Factories

**Test Data:**
- Created inline in tests using `t.TempDir()` and `os.WriteFile()`
- No external fixture files
- Helper: `grepToolInput(key, value string) types.ToolInput` in `grep_test.go:520`

**Test Environment:**
- `tests/testutil/envtest.go`:
  - `RequireAnyAPIKey(t, envVars...)` — skips unless env var set
  - `LoadTestDotEnv(t)` — loads `.env.test` from project root (once via `sync.Once`)

**CI Detection (`internal/testutil/ci/ci.go`):**
```go
func SkipIfCI(t *testing.T, reason string) {
    t.Helper()
    if IsCI() {
        t.Skip(reason)
    }
}
```

## Coverage

**Requirements:**
- 75% overall (enforced in CI)
- 90% for critical packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover    # Generates coverage.html from coverage.out
```

**Coverage Boost Files:**
- `<name>_coverage_boost_test.go` — tests specifically to increase coverage
- Found in `internal/ui/tui/theme/`, `internal/ui/tui/streaming/`, `internal/ui/tui/layout/`, `internal/ui/tui/commands/`

## Test Types

**Unit Tests:**
- Scope: Single function/method/package
- Location: Co-located `_test.go` files
- Approach: Test public API, use `t.Parallel()`, real implementations where fast
- Examples: `TestGrep_*`, `TestReplView_*`, `TestParameterSchema_AllTools`

**Integration Tests:**
- Scope: Cross-package, real I/O
- Location: `tests/testutil/integration/`
- Approach: Full pipeline through dispatcher → tool → file system
- Example: `TestEdit_Execute_ExactMatch_FileIO` (real file read/write/backup)

**E2E Tests:**
- Scope: Full binary execution
- Location: `tests/e2e/e2e_test.go`
- Framework: Compiles binary via `go build`, runs via `exec.Command`
- Real API tests: `TestBinary_Prompt_*RealAPI` — require env vars, skip gracefully
- Headless mode tests: `--prompt`, `--goal`, `--version`, `--help`

**Benchmarks:**
- Location: `<name>_bench_test.go` (e.g., `theme/bench_alloc_test.go`, `app_channel_bench_test.go`)
- Run: `make bench` or `make bench-verbose`
- Comparison: `make bench-save` then `make bench-compare` (uses `benchstat`)

## Common Patterns

**Async Testing:**
```go
func TestEdit_Execute_ContextCancellation(t *testing.T) {
    t.Parallel()
    dir := t.TempDir()
    // ... setup ...
    ctx, cancel := context.WithCancel(context.Background())
    cancel() // cancel immediately
    _, err := edit.Execute(ctx, types.ToolInput{...})
    if err == nil {
        t.Fatal("expected error for cancelled context, got nil")
    }
}
```

**Error Testing:**
```go
func TestGrep_InvalidRegex(t *testing.T) {
    t.Parallel()
    g := search.NewGrep(t.TempDir())
    _, err := g.Execute(context.Background(), grepToolInput("pattern", "[invalid"))
    if err == nil {
        t.Error("expected error for invalid regex")
    }
    // Check error message content
    if !strings.Contains(err.Error(), "invalid regex") {
        t.Errorf("expected 'invalid regex' error, got: %v", err)
    }
}
```

**Testing Tool Results (not Go errors):**
```go
func TestEdit_Execute_NoMatchReturnsError(t *testing.T) {
    t.Parallel()
    // ...
    result, err := edit.Execute(ctx, types.ToolInput{...})
    if err != nil {
        t.Fatalf("Execute returned Go error: %v", err)
    }
    // Error should be in ToolResult.Output, not a Go error
    if result.Error == "" && result.Output == "" {
        t.Error("expected error in ToolResult.Output")
    }
}
```

**Real API Tests (E2E):**
```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    ci.SkipIfCI(t, "requires NVIDIA_API_KEY and network access not available in CI")
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }
    bin := buildBinary(t)
    cmd := exec.Command(bin, "--prompt", "What is 2+2?", "--model", "meta/llama-3.1-8b-instruct")
    cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
    // ...
}
```

**Clean Environment for Tests:**
```go
func cleanEnv() []string {
    return []string{
        "PATH=" + os.Getenv("PATH"),
        "HOME=" + os.TempDir(),
        "USERPROFILE=" + os.TempDir(),
        "M31A_CONFIG=" + filepath.Join(os.TempDir(), "m31a-test-config.toml"),
    }
}
```

## Special Test Infrastructure

**TUI Test Harness (`internal/ui/tui/tui_harness_test.go`):**
- Headless Bubble Tea program for testing TUI components
- `NewTestProgram(t, model)` — runs model in test context

**Emitter Stress Test (`internal/ui/tui/emitter_stress_test.go`):**
- Tests channel saturation and drop handling
- Verifies `globalDropCounter` and bounded retry logic

**Binary Build Helper (E2E):**
```go
func buildBinary(t *testing.T) string {
    t.Helper()
    binName := "m31a"
    if runtime.GOOS == "windows" {
        binName += ".exe"
    }
    bin := filepath.Join(t.TempDir(), binName)
    // Walk up to find go.mod
    dir := mustGetwd(t)
    for {
        if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
            break
        }
        dir = filepath.Dir(dir)
    }
    cmd := exec.Command("go", "build", "-o", bin, "./cmd/m31a")
    cmd.Dir = dir
    out, err := cmd.CombinedOutput()
    if err != nil {
        t.Fatalf("build failed: %v\n%s", err, string(out))
    }
    return bin
}
```

---

*Testing analysis: 2026-08-23*