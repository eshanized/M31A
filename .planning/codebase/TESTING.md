# Testing Patterns

**Analysis Date:** 2026-06-14

## Test Framework

**Runner:**
- Go standard `testing` package
- No external test frameworks (no testify, gomega, etc.)
- Config: no external config file — uses Go conventions

**Assertion Library:**
- Standard `testing.T` methods: `t.Fatal()`, `t.Fatalf()`, `t.Error()`, `t.Errorf()`, `t.Log()`, `t.Logf()`
- Direct comparisons with `!=`, `strings.Contains()`, `!strings.Contains()`
- No assertion libraries — all assertions are explicit `if` checks

**Run Commands:**
```bash
go test -race -cover ./...           # All tests with race detector + coverage
go test -cover ./...                  # Fast mode (no race detector)
go test -v -race -cover ./...         # Verbose output
go test -v -race -run TestName ./...  # Run specific test
make test                             # Same as first (via Makefile)
make test-fast                        # No race detector
make test-specific TEST=TestFoo       # Run specific test
make cover                            # Generate HTML coverage report
make bench                            # Run benchmarks
```

## Test File Organization

**Location:** Co-located with source files in the same package (white-box testing).

**Naming patterns:**
- Primary tests: `<source>_test.go` — e.g., `bash_test.go`, `engine_test.go`, `session_test.go`
- Supplemental/edge-case tests: `extra_test.go` — exists in nearly every package
- Feature-specific split: `bash_security_test.go`, `bash_kill_test.go`, `webfetch_security_test.go`
- Concept-specific split: `permissions_test.go`, `permission_timeout_test.go`

**Structure:**
```
internal/
├── tools/
│   ├── bash.go                    # Source
│   ├── bash_test.go               # Primary tests
│   ├── bash_security_test.go      # Security-focused tests
│   ├── bash_kill_test.go          # Kill/signal tests
│   ├── extra_test.go              # Edge cases and helpers
│   ├── edit.go
│   ├── edit_test.go
│   ├── fileread.go
│   ├── fileread_test.go
│   └── ...
├── workflow/
│   ├── engine.go
│   ├── engine_test.go             # Core engine tests
│   ├── engine_extra_test.go       # Edge cases
│   ├── engine_parse_test.go       # Parse-specific tests
│   ├── integration_test.go        # Full workflow integration
│   ├── classify_test.go           # Phase-specific tests
│   ├── plan_test.go
│   ├── execute_test.go
│   └── ...
```

**Total: 115 test files, ~49,365 lines of test code, ~48,158 lines of source code** (test-to-source ratio ≈ 1.02:1)

## Test Structure

**Table-driven tests** (predominant pattern):
```go
func TestUserMessage(t *testing.T) {
    tests := []struct {
        name     string
        err      error
        expected string
    }{
        {"nil", nil, ""},
        {"ErrProviderUnreachable", ErrProviderUnreachable, "Provider unreachable..."},
        {"wrapped ErrInvalidKey", fmt.Errorf("auth failed: %w", ErrInvalidKey), "..."},
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

**Individual test functions** (for complex scenarios):
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

**Setup pattern:**
```go
func setupTestEngine(t *testing.T) (*Engine, func()) {
    t.Helper()
    dir := t.TempDir()
    // ... setup git, sessions, dispatcher ...
    return engine, cleanup
}
```

**Helper function pattern:**
```go
func testTheme() theme.Theme {
    return theme.Theme{
        Mode:    theme.ModeDark,
        Background: lipgloss.Color("#000000"),
        // ...
    }
}

func testAppState() *AppState {
    rm := NewReplModel(testTheme(), "v1")
    return &AppState{screen: ScreenREPL, replModel: &rm, ...}
}
```

## Mocking

**Framework:** Hand-rolled mocks (no mockgen or gomock)

**Mock provider pattern** (`internal/workflow/engine_test.go`):
```go
type mockProvider struct {
    response       string
    err            error
    callCount      int
    multiResponses []string
}

func (m *mockProvider) Name() string            { return "mock" }
func (m *mockProvider) APIKey() string           { return "test-key" }
func (m *mockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
    return nil, nil
}
func (m *mockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
    // Return canned response or iterate multiResponses
}
func (m *mockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (m *mockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
    return m31types.HealthStatus{Status: "live"}
}
func (m *mockProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (m *mockProvider) CachedModels() []m31types.ModelInfo { return nil }
```

**Multi-turn mock** (`internal/workflow/integration_test.go`):
```go
type multiTurnMockProvider struct {
    callCount int
    responses []string
}
// Returns responses[callCount] per call, increments callCount
```

**Mock keychain** (`internal/config/loader_test.go`):
```go
type mockKeychain struct {
    store map[string]string
}

func (m *mockKeychain) Get(service string) (string, error) {
    if v, ok := m.store[service]; ok { return v, nil }
    return "", errors.New("not found")
}
func (m *mockKeychain) Set(service, value string) error { ... }
func (m *mockKeychain) Delete(service string) error { ... }
```

**Pre-approval pattern** (skip permission prompts in tests):
```go
dispatcher.SetPermission("Bash", true)
dispatcher.SetPermission("FileRead", true)
dispatcher.SetPermission("FileWrite", true)
```

**What to Mock:**
- LLM providers (always mocked — never call real APIs in tests)
- File system operations use `t.TempDir()` (no mocking needed)
- Keychain for config tests
- Git operations use real git in `t.TempDir()` (integration-style)

**What NOT to Mock:**
- File system — use `t.TempDir()` instead
- Git — use real git init/commit in temp directory
- Bubble Tea message handling — test `Update()` and `View()` directly

## Fixtures and Factories

**Test data:**
- Created inline in test functions — no shared fixture files
- Temporary directories via `t.TempDir()` (auto-cleaned)
- Temporary environment variables via `t.Setenv()` (auto-restored)

**Factory pattern for complex objects:**
```go
func testAppState() *AppState {
    rm := NewReplModel(testTheme(), "v1")
    tm := theme.NewManager(theme.ModeDark)
    return &AppState{
        screen:       ScreenREPL,
        replModel:    &rm,
        themeManager: tm,
        toasts:       []Toast{},
        toastTimers:  make(map[int]*time.Timer),
        width:        80,
        height:       24,
    }
}
```

**Test data in maps/slices:**
```go
tasks := []types.Task{
    {ID: 1, Description: "task 1", Status: types.StatusPending},
    {ID: 2, Description: "task 2", Status: types.StatusPending, Dependencies: []int{1}},
}
```

**Location:** Test data is always inline within test files — no separate fixture directories.

## Coverage

**Targets (from `CONTRIBUTING.md`):**
- 75% overall
- 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**View Coverage:**
```bash
make cover                    # Generates coverage.html
go tool cover -html=coverage.out -o coverage.html
```

**Coverage output:** `coverage.out` (text), `coverage.html` (visual)
- `.gitignore` includes `coverage.out` and `*.out`

## Test Types

**Unit Tests:**
- Scope: Individual functions and methods
- Pattern: Table-driven or individual test functions
- Examples: `TestUserMessage`, `TestBash_SimpleCommand`, `TestCascadingReplace_ExactMatchTakesPriority`
- Use `t.Parallel()` for independent tests
- Use `t.TempDir()` for filesystem isolation

**Integration Tests:**
- Scope: Full workflow execution end-to-end
- File: `internal/workflow/integration_test.go`
- Pattern: `TestFullWorkflow` — creates real git repo, session, dispatcher, mock provider; runs Initialize→Discuss→Plan→Execute→Verify→Ship
- Uses multi-turn mock provider to simulate LLM responses

**Security Tests:**
- Dedicated files: `bash_security_test.go`, `webfetch_security_test.go`
- Scope: SSRF protection, path traversal, command injection
- Examples: `TestIsPrivateIP_*`, tests for path outside workDir

**Edge Case Tests (extra_test.go pattern):**
- Supplemental coverage for boundary conditions
- Exist in 15 packages
- Cover nil inputs, empty strings, overflow, race conditions
- Often test unexported functions directly (white-box)

## Common Patterns

**Async Testing:**
```go
func TestBash_ContextCancellation(t *testing.T) {
    ctx, cancel := context.WithCancel(context.Background())
    b := NewBash(t.TempDir())
    errCh := make(chan error, 1)
    go func() {
        _, err := b.Execute(ctx, types.ToolInput{...})
        errCh <- err
    }()
    time.Sleep(100 * time.Millisecond)
    cancel()
    select {
    case err := <-errCh:
        // assert on err
    case <-time.After(5 * time.Second):
        t.Fatal("command not cancelled in time")
    }
}
```

**Error Testing:**
```go
func TestBash_MissingCommandParam(t *testing.T) {
    t.Parallel()
    b := NewBash(t.TempDir())
    _, err := b.Execute(context.Background(), types.ToolInput{
        Name:   "Bash",
        Params: map[string]any{},
    })
    if err == nil {
        t.Fatal("expected error for missing command param")
    }
    if !strings.Contains(err.Error(), "missing parameter: command") {
        t.Errorf("expected missing parameter error, got: %v", err)
    }
}
```

**Parallel Tests:**
- Use `t.Parallel()` on tests that don't share mutable state
- Most tool tests are parallel-safe (use `t.TempDir()`)
- Workflow tests generally NOT parallel (shared mock state)

**Cleanup:**
- `t.TempDir()` auto-cleanup (preferred over manual `os.RemoveAll`)
- `defer os.RemoveAll(dir)` used in some older tests (`pkg/session/session_test.go`)
- `defer cancel()` for context cancellation

## Test File Counts by Package

| Package | Test Files | Notable Coverage |
|---------|-----------|-----------------|
| `internal/workflow/` | 18 | Full workflow, per-phase, integration, streaming, parsing |
| `internal/tools/` | 17 | All tools, security, permissions, edge cases |
| `internal/tui/` | 18 | App state, views, models, components, theme, layout |
| `internal/tui/components/` | 6 | Message, toolcard, thinking, sparkline, permission, starfield |
| `internal/tui/theme/` | 3 | Theme, registry |
| `internal/tui/layout/` | 3 | Page, responsive, layout |
| `internal/provider/` | 9 | Registry, cache, fallback, resilience, capabilities |
| `internal/provider/openrouter/` | 2 | Client, extra |
| `internal/config/` | 2 | Loader, extra |
| `pkg/session/` | 6 | Session, manager, planning, checkpoint |
| `pkg/bisect/` | 2 | Bisect, extra |
| `pkg/taskrunner/` | 1 | Runner |
| `pkg/arbitrage/` | 1 | Arbitrage |
| `pkg/autodream/` | 2 | Autodream, extra |
| `pkg/ledger/` | 2 | Ledger, extra |
| `pkg/keychain/` | 2 | Keychain, extra |
| `pkg/rollback/` | 1 | Rollback |
| `internal/errors/` | 1 | UserMessage |
| `internal/fileutil/` | 2 | Atomic write, extra |
| `internal/log/` | 2 | Logger, extra |
| `internal/tokens/` | 2 | Estimator, context warning |
| `internal/types/` | 2 | Types, extra |
| `internal/git/` | 2 | Git, extra |

## Untested Packages

| Package | Reason | Risk |
|---------|--------|------|
| `cmd/m31a/` | Entry point with OS interaction (flag parsing, signal handling) | Low — integration coverage via workflow tests |
| `cmd/firstrunpreview/` | Preview utility, not core | Low |
| `internal/tui/agent_loop*` | Complex Bubble Tea wiring — partially tested via `agent_loop_extra_test.go` | Medium — covered by extra tests |
| `docs/` | Markdown documentation, no code | N/A |

## Anti-Patterns Observed

**Tolerance in some tests:** Some tests use `t.Log()` instead of `t.Error()` for expected failures, making them non-failing:
```go
// In bash_test.go
if result.Error == "" && err == nil {
    t.Fatal("expected error or non-zero exit for non-existent command")
}
```
This is intentional for platform-sensitive tests (some commands may not exist).

**No `t.Cleanup()` usage:** Most tests use `defer` or rely on `t.TempDir()` auto-cleanup. The `t.Cleanup()` pattern is rarely used.

---

*Testing analysis: 2026-06-14*
