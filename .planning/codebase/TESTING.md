# Testing

**Analysis Date:** 2026-07-21

## Framework

- **Test Runner**: Go standard library `testing` package
- **Assertion Library**: `github.com/stretchr/testify` (assert, require, mock)
- **Race Detector**: Enabled for `make test` (`go test -race`)
- **Coverage**: Enabled for `make test` (`go test -cover -coverprofile=coverage.out`)

## Test Structure

**File Organization:**
- `*_test.go` files alongside source files (co-located)
- Table-driven tests are the dominant pattern
- Integration tests in `internal/testutil/integration/`
- E2E tests in `e2e_test.go` (package `m31a_test`)

**Naming Convention:**
- Test functions: `Test<Type>_<Scenario>` or `Test<Function>_<Scenario>`
- Subtests: `t.Run("description", func(t *testing.T) { ... })`
- Table-driven: `tests := []struct{name, input, want}{...}` with `t.Run(tt.name, ...)`

## Coverage Targets

From `AGENTS.md` and `Makefile`:
- **75%** overall coverage
- **90%** for critical packages:
  - `internal/taskrunner` (runner_test.go: 677 lines)
  - `internal/bisect` (bisect_test.go: 661 lines)
  - `internal/rollback` (rollback_test.go: 1198 lines)

**Coverage Commands:**
```bash
make test           # race-enabled with coverage
make test-fast      # no race detector
make test-specific TEST=TestFoo  # run single test
make cover          # HTML coverage report (coverage.html)
```

## Key Test Files

| File | Purpose | Lines |
|------|---------|-------|
| `e2e_test.go` | Binary E2E tests, real API tests (skipped without keys) | 191 |
| `internal/tools/dispatcher_test.go` | Dispatcher permissions, rate limiting, concurrency | 991 |
| `internal/taskrunner/runner_test.go` | Task scheduling, execution, dependencies | 677 |
| `internal/rollback/rollback_test.go` | Git rollback operations (soft/hard/safe reset) | 1198 |
| `internal/bisect/bisect_test.go` | Git bisect automation | 661 |
| `internal/workflow/engine_test.go` | Workflow engine phases | ~400 |
| `internal/provider/*/client_test.go` | Provider client unit tests | varies |

## Test Commands

```bash
# Full test suite with race detector and coverage
make test
# Equivalent to: go test -race -cover -coverprofile=coverage.out ./...

# Fast mode (no race detector)
make test-fast
# Equivalent to: go test -cover ./...

# Verbose output
make test-verbose
# Equivalent to: go test -v -race -cover ./...

# Single test
make test-specific TEST=TestDispatcher_RegisterAndExecute
# Equivalent to: go test -v -race -run TestDispatcher_RegisterAndExecute ./...

# Benchmarks
make bench
# Equivalent to: go test -bench=. -benchmem -run=^$ ./...

# Coverage HTML report
make cover
# Generates coverage.html from coverage.out
```

## Patterns

### Table-Driven Tests
**From `internal/tools/dispatcher_test.go`:**
```go
func TestMatchToolName(t *testing.T) {
    t.Parallel()
    tests := []struct {
        name    string
        pattern string
        tool    string
        want    bool
    }{
        {"exact match", "Bash", "Bash", true},
        {"wildcard", "*", "Bash", true},
        {"glob prefix", "B*", "Bash", true},
        {"no match", "Bash", "FileRead", false},
        {"doublestar pattern", "**", "Anything", true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := matchToolName(tt.pattern, tt.tool)
            if got != tt.want {
                t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.tool, got, tt.want)
            }
        })
    }
}
```

### Parallel Tests
- `t.Parallel()` used extensively for independent test cases
- Shared test setup in helper functions (e.g., `testDispatcher(t)`, `setupRollback(t)`)

### Test Helpers
**From `internal/tools/dispatcher_test.go`:**
```go
func testDispatcher(t *testing.T) *Dispatcher {
    t.Helper()
    return newDispatcher(nil) // no config = default permissions
}

func testDispatcherWithConfig(t *testing.T, cfg *config.PermissionsConfig) *Dispatcher {
    t.Helper()
    return newDispatcher(cfg)
}
```

**From `internal/rollback/rollback_test.go`:**
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

func createCommits(g *git.Git, n int) {
    for i := 0; i < n; i++ {
        f := filepath.Join(g.WorkDir(), fmt.Sprintf("f%d.txt", i))
        os.WriteFile(f, []byte(fmt.Sprintf("content %d", i)), 0644)
        g.Commit(fmt.Sprintf("commit %d", i))
    }
}
```

### Mocking
- `github.com/stretchr/testify/mock` for interface mocking
- `internal/testutil/mocks/` contains mock implementations
- **From `internal/tools/dispatcher_test.go`:**
```go
d.Register(&mocks.MockTool{Name_: "test", RiskLevel_: types.RiskSafe})
d.Register(&mocks.MockTool{Name_: "bash", RiskLevel_: types.RiskDangerous})
```

- Provider interfaces use custom test doubles (not testify mocks) for SSE streaming:
```go
// internal/provider/mock/streaming.go provides streaming test utilities
```

### E2E Tests
**From `e2e_test.go`:**
```go
func TestBinary_Version(t *testing.T) {
    bin := buildBinary(t)
    out := runBinary(t, bin, "--version")
    if !strings.Contains(out, "m31a") {
        t.Errorf("expected version output to contain 'm31a', got: %s", out)
    }
}

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
    // ... assertions
}

// Binary built fresh for each test run
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
```

**Real API Tests** (require env vars, skip gracefully):
- `OPENROUTER_API_KEY` → `TestBinary_Prompt_OpenRouterRealAPI`
- `ZEN_API_KEY` → `TestBinary_Prompt_ZenRealAPI`
- `NVIDIA_API_KEY` → `TestBinary_Prompt_NvidiaRealAPI`

### Integration Tests
**From `internal/testutil/integration/tool_integration_test.go`:**
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
    if err != nil {
        t.Fatalf("Execute failed: %v", err)
    }
    if result.Error != "" {
        t.Fatalf("Tool error: %s", result.Error)
    }

    got, _ := os.ReadFile(filePath)
    if !strings.Contains(string(got), "world") {
        t.Errorf("file content = %q, want contains 'world'", string(got))
    }
}
```

### Test Utilities
- `t.TempDir()` for isolated filesystem
- `context.Background()` or `context.WithTimeout` for timeouts
- `strings.Contains` / `strings.TrimSpace` for output assertions
- Custom `cleanEnv()` removes API keys for isolation

## CI / Linting

**From `Makefile` and `.golangci.yml`:**

```yaml
# .golangci.yml
version: "2"
run:
  timeout: 5m
linters:
  enable:
    - govet
    - staticcheck
    - errcheck
    - ineffassign
    - unused
  settings:
    govet:
      enable:
        - shadow
    errcheck:
      check-type-assertions: false
      check-blank: false
  exclusions:
    rules:
      - path: _test\.go
        linters:
          - errcheck
          - unused
```

**Makefile Targets:**
```bash
make lint           # golangci-lint (5m timeout)
make lint-fix       # with auto-fix
make vet            # go vet
make fmt            # gofmt + goimports
make tidy           # go mod tidy
make check          # fmt → tidy → vet → lint → test (full pipeline)
```

**Test files excluded from:**
- `errcheck` (error returns in tests often intentionally ignored)
- `unused` (test helpers may be unused in some runs)

## Workflow Engine Tests
**From `internal/workflow/engine_test.go` and related:**
- Phase-specific tests: `*_test.go` per phase (`discuss_test.go`, `plan_test.go`, `execute_test.go`, etc.)
- Race detection tests: `engine_race_test.go`
- State machine tests: `state_machine_test.go`
- Coverage gate tests: `coverage_gates_test.go`, `coverage_gates_extra_test.go`

## Provider Tests
**From `internal/provider/*/client_test.go`:**
```go
func TestOpenRouterClient_FetchModels(t *testing.T) {
    // Uses test server or mock HTTP
    client := New("test-key", Options{BaseURL: testServer.URL})
    models, err := client.FetchModels(context.Background())
    // assertions
}

func TestOpenRouterClient_ChatCompletionStream(t *testing.T) {
    // Tests streaming with mock SSE responses
}
```

## Coverage Verification

```bash
# Check coverage for specific packages
go test -cover ./internal/taskrunner/...
go test -cover ./internal/bisect/...
go test -cover ./internal/rollback/...

# View HTML report
make cover
# Opens coverage.html in browser
```

---

*Testing analysis: 2026-07-21*