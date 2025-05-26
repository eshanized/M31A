# Testing Patterns

**Analysis Date:** 2026-06-02

## Test Framework

**Framework:** Standard library `testing` only. **No `testify`**, no `gofakeit`, no third-party assertion library.
- Assertion API is `t.Errorf`, `t.Fatalf`, `t.Logf` directly.
- Benchmark files: none present in the repository (verified with `grep -r "^func Benchmark"`).

**Run commands** (from `Makefile`):
```bash
go test -race -cover -coverprofile=coverage.out ./...   # full suite, with race detector + coverage
go test -race -cover ./...                              # CI shorthand (CONTRIBUTING.md)
```

**Required flags:**
- `-race` — race detector is **mandatory**. PRs are rejected if race conditions are introduced (CONTRIBUTING.md).
- `-cover` — coverage is always measured.

## Test File Organization

**Location:** Co-located with source. Every production `.go` file has a matching `*_test.go` in the same directory.
- `internal/tools/bash.go` → `internal/tools/bash_test.go`
- `internal/provider/zen/client.go` → `internal/provider/zen/client_test.go`
- `internal/tui/components/toolcard.go` → `internal/tui/components/toolcard_test.go`

**Package:** Same package as the source file (white-box testing), not `_test` package suffix. Examples: `package tools`, `package workflow`, `package provider`.

**Naming:**
- File: `<source>_test.go`
- Functions: `TestX_Y` (Type_Method or Type_Scenario) — see `internal/tools/bash_test.go`:
  - `TestBash_SimpleCommand`, `TestBash_Name`, `TestBash_Description`, `TestBash_RiskLevel`
  - `TestBash_WithWorkingDirectory`, `TestBash_Stderr`, `TestBash_Timeout`
  - `TestBash_ContextCancellation`, `TestBash_NonZeroExit`, `TestBash_OutputTruncated`
  - `TestBash_BinaryOutput`, `TestBash_CommandNotFound`, `TestBash_MissingCommandParam`
  - `TestBash_CommandNotString`, `TestBash_CustomTimeout`, `TestBash_InvalidTimeout`
- Internal helpers (unexported): `TestLimitWriter_UnderLimit`, `TestIsBinary_Empty` (`bash_test.go:222-328`).
- Constructor coverage via dedicated tests: `TestBash_Name`, `TestBash_Description`, `TestBash_RiskLevel`.

## Test Structure

**Setup helpers** (created in-test, not in a base suite):
```go
func TestBash_SimpleCommand(t *testing.T) {
    t.Parallel()
    b := NewBash(t.TempDir())          // temp dir
    result, err := b.Execute(context.Background(), types.ToolInput{...})
    // assertions
}
```

`t.TempDir()` is used universally for filesystem isolation; the directory is cleaned up automatically.

`t.Helper()` is used to mark helper functions (`pkg/ledger/ledger_test.go:43`).

**Suite-level helpers** in test packages (e.g. `pkg/autodream/autodream_test.go:17-91`):
- `makeMessages(n int) []types.Message`
- `makeMessagesWithContent(content []string) []types.Message`
- `makeSystemMessages(n int, sysIdx []int) []types.Message`
- `makeToolCallMessages(n int, toolIdx []int) []types.Message`
- `sampleContent(i int) string`
- `format(s string, args ...interface{}) string` (avoids `fmt.Sprintf` import in test tables)

**Test organization convention:**
- Each test file starts with a comment block dividing test categories when non-trivial (e.g. `internal/tools/dispatcher_test.go:364-366`: `// Permission ruleset matching tests`; `pkg/autodream/autodream_test.go:12-94`: `// Test helpers`, `// CanConsolidate tests`, `// Consolidate tests`, `// Pause / Resume tests`, etc.).
- Section comment headers look like:
  ```go
  // ---------------------------------------------------------------------------
  // Doublestar integration verification
  // ---------------------------------------------------------------------------
  ```

## Parallelism

`t.Parallel()` is invoked at the **top of every test function** that doesn't share state with siblings. ~106 occurrences across the repository.

When to skip `t.Parallel()`:
- Tests that mutate shared state across the test binary (e.g. long-running bash tests with explicit timing assertions in `bash_test.go:53-68, 87-108, 155-170`).
- Tests using channels for handshakes with goroutines inside the test body.

**Helper goroutines** for permission flow tests (`internal/tools/dispatcher_test.go:82-109, 111-131, 133-167`):
```go
errCh := make(chan error, 1)
go func() {
    _, err := d.Execute(context.Background(), types.ToolCall{ID: "call1", Name: "bash", Input: []byte(`{}`)})
    errCh <- err
}()

req := <-d.RequestCh()
if req.ToolName != "bash" { t.Errorf(...) }
d.ApprovePermission(true, false)

if err := <-errCh; err != nil { t.Errorf(...) }
```
Always pair: spawn goroutine, wait for request on channel, respond, drain result from channel.

## Mocking

**Mock strategies used** (in order of preference):

**1. In-package mock implementations** for interfaces, written inline in the test file:
```go
// internal/tools/dispatcher_test.go:15-29
type mockTool struct {
    name      string
    riskLevel types.RiskLevel
    execFunc  func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}
func (m *mockTool) Name() string { return m.name }
func (m *mockTool) Description() string { return "mock tool for testing" }
func (m *mockTool) RiskLevel() types.RiskLevel { return m.riskLevel }
func (m *mockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    if m.execFunc != nil { return m.execFunc(ctx, input) }
    return types.ToolResult{Output: "ok"}, nil
}
```

**2. In-memory fakes** for storage-style interfaces (`pkg/keychain/keychain_test.go:8-35`):
```go
type mockKeychain struct { store map[string]string }
func (m *mockKeychain) Get(service string) (string, error)   { ... }
func (m *mockKeychain) Set(service, value string) error      { ... }
func (m *mockKeychain) Delete(service string) error          { ... }
```

**3. `httptest.NewServer` for HTTP mocks** (full server, not `httptest.NewRecorder`):
```go
// internal/provider/openrouter/client_test.go:39-57
ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
    if r.URL.Path == "/auth/key" {
        w.WriteHeader(http.StatusOK)
        w.Write([]byte(`{"status":"ok"}`))
    }
}))
defer ts.Close()

c, _ := New("test-key", Options{})
c.baseURL = ts.URL         // inject the test server URL directly
status := c.HealthCheck(context.Background())
```

The test rewrites `c.baseURL` to the test server's URL after construction — this is the standard injection pattern for clients whose `Options.BaseURL` is set at construction.

**4. Streaming SSE test bodies** (in-memory readers, no server):
```go
// internal/provider/sse_test.go:10-14
func bodyReader(s string) *http.Response {
    return &http.Response{Body: io.NopCloser(strings.NewReader(s))}
}
```
This is a one-off helper used to feed the SSE parser with synthetic stream content.

**5. Real OS test files via `t.TempDir()` and `os.WriteFile`** for tools that touch the filesystem (`internal/tools/fileread_test.go`, `internal/tools/filewrite_test.go`, `internal/tools/grep_test.go`, `internal/tools/glob_test.go`, `pkg/ledger/ledger_test.go`).

**What to mock** (from observations):
- HTTP clients: use `httptest.NewServer`.
- Permission flows: use a real `Dispatcher` with a `mockTool` registered, drive it via channels.
- Keychain: in-memory `mockKeychain`.
- Tool dependencies: never mock the tool under test itself; provide a `mockTool` to the dispatcher.

**What NOT to mock:**
- Time — use `time.Now()` directly; for cache staleness tests, manually backdate fields:
  ```go
  // internal/provider/cache_test.go:28
  c.fetched = time.Now().Add(-10 * time.Minute)
  ```
- The tool implementation itself — always exercise the real `*Bash`, `*FileRead`, etc.

## Fixtures and Factories

**Test data factories** are defined at the top of the test file and prefixed with the type (`newTest*`):
- `newTestEntry` in `pkg/ledger/ledger_test.go:14-25`
- `newTestSession` in `pkg/ledger/ledger_test.go:28-39`
- `newTask` in `pkg/taskrunner/runner_test.go:12-19`
- `setupLedger` in `pkg/ledger/ledger_test.go:42-48` (returns `(*Ledger, string)`)
- `makeMessages`, `makeMessagesWithContent`, `makeSystemMessages`, `makeToolCallMessages` in `pkg/autodream/autodream_test.go:17-86`

**Parametrized tool input helper:**
```go
// internal/tools/grep_test.go: uses toolInput("pattern", "main") shorthand
```
Although the helper itself is not always defined as exported, the pattern is consistent: build a `types.ToolInput` with one named param.

**Test fixtures on disk** (no shared `testdata/` directory is heavily used; tests inline content):
- `pkg/ledger/ledger_test.go:76-83` writes a pre-formatted `LEDGER.md` content string directly via `os.WriteFile`.

## Table-Driven Tests

The repository uses both table tests and individual test functions. Table tests are preferred when the matrix is small and uniform; otherwise individual `TestX_Y` functions are written.

**Table-test example with anonymous struct** (`internal/tools/dispatcher_test.go:368-393`):
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
        // ...
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := matchToolName(tt.pattern, tt.tool)
            if got != tt.want { t.Errorf(...) }
        })
    }
}
```

**Named struct slice for table tests** (`pkg/taskrunner/runner_test.go:446-472`):
```go
tests := []struct {
    name          string
    initialStatus types.TaskStatus
    expected      types.TaskStatus
}{...}
```

**Subtests via `t.Run(name, fn)`** — used for all table tests and for grouping within a single test function (`internal/tui/components/toolcard_test.go:178-219` for `TestSanitizeOutput`).

## Error Testing

**Sentinel error comparison:** `errors.Is` (or direct equality for tests that import the same package).
```go
// internal/tools/dispatcher_test.go:128
if err := <-errCh; err != m31errors.ErrPermissionDenied {
    t.Errorf("expected ErrPermissionDenied, got: %v", err)
}
```

**Error message substring matching:** when a wrapped error is expected, check the message:
```go
// internal/tools/bash_test.go:216
if !strings.Contains(err.Error(), "missing parameter: command") {
    t.Errorf("expected missing parameter error, got: %v", err)
}
```

**Panic testing:** use `defer recover()` and assert the panic value:
```go
// internal/tools/dispatcher_test.go:351-362
func TestDispatcher_RegisterDuplicate(t *testing.T) {
    t.Parallel()
    d := NewDispatcher(nil)
    d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})
    defer func() {
        if r := recover(); r == nil {
            t.Error("expected panic for duplicate registration")
        }
    }()
    d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})
}
```

## Coverage

**Targets** (CONTRIBUTING.md):
- **75% overall** across the module.
- **90% for critical packages:** `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`.

**Current coverage files** (root): `coverage.out`, `coverage.html`, `coverage_phase6.out`, `cover.out` — measured during phase work.

**View coverage:**
```bash
go test -race -cover -coverprofile=coverage.out ./...
go tool cover -html=coverage.out -o coverage.html
```

## Test Types

**Unit tests** (primary mode):
- All packages have unit tests in the same file directory.
- Test one method or behavior per function (with table subtests for variations).

**Integration tests:**
- `internal/workflow/integration_test.go` — full six-phase workflow.
- `internal/workflow/engine_test.go` — engine orchestration.

**Subprocess / shell tests:**
- `internal/tools/bash_test.go` actually spawns subprocesses (`echo`, `sleep`, `exit`, `printf`).
- `TestBash_ContextCancellation` (`bash_test.go:110-136`) launches a 30s sleep in a goroutine, cancels context, asserts the goroutine returns within 5s.

**Compile-time interface checks** in production code (e.g. `internal/provider/zen/client.go:23`):
```go
var _ provider.LLMProvider = (*Client)(nil)
```
Tests rely on the same interfaces but don't have a `var _` line.

## Common Patterns

**Async testing with timeout:**
```go
// internal/tools/dispatcher_test.go:341-348
select {
case err := <-errCh:
    if err == nil { t.Error("expected context cancellation error") }
case <-time.After(2 * time.Second):
    t.Fatal("expected context cancellation to be handled")
}
```

**Temp file in subdirectory:**
```go
os.MkdirAll(filepath.Join(dir, "src", "pkg"), 0755)
os.WriteFile(filepath.Join(dir, "src", "pkg", "main.go"), []byte("..."), 0644)
```
Used in `grep_test.go:138-139`, `glob_test.go` to simulate project layout.

**Symlink safety tests** (`internal/tools/fileread_test.go:119-142`):
```go
if err := os.Symlink(outsideFile, symlinkPath); err != nil {
    t.Skip("symlinks not supported on this system")
}
```
Skip gracefully when the test environment cannot create symlinks.

**Time-based cache testing** (`internal/provider/cache_test.go:107-141`):
- Use very short TTLs (`10 * time.Millisecond`) plus `time.Sleep(20 * time.Millisecond)` to test expiry.
- Or directly mutate the `fetched` field to simulate the passage of time.

**Results() copy test** (`pkg/taskrunner/runner_test.go:596-616`):
- Get the map, mutate the copy, assert internal state is unchanged.
- Verifies defensive copying in accessor methods.

**Immutability test for returned slices** (`pkg/autodream/autodream_test.go:394-416`):
```go
got := c.Messages()
got[0].Content = "MUTATED"
got = append(got, types.Message{...})
internal := c.Messages()
if internal[0].Content == "MUTATED" { t.Error("mutating returned slice should not affect internal state") }
```

**JSON object comparison** via `json.Marshal`/`json.Equal` is **not** used; tests use field-by-field comparison with `t.Errorf` per field.

**Atomic counter test patterns:** rarely needed; the codebase uses `atomic.AddInt64` in `internal/workflow/engine.go:647` (`nextCallID`) but does not have a dedicated test for that helper.

## Where to Add New Tests

| New code goes in | Tests live in |
|---|---|
| `internal/tools/mytool.go` | `internal/tools/mytool_test.go` (co-located) |
| `internal/provider/<provider>/client.go` | `internal/provider/<provider>/client_test.go` |
| `internal/workflow/<phase>.go` | `internal/workflow/<phase>_test.go` |
| `internal/tui/<screen>.go` | `internal/tui/<screen>_test.go` |
| `internal/tui/components/<comp>.go` | `internal/tui/components/<comp>_test.go` |
| `pkg/<pkg>/<file>.go` | `pkg/<pkg>/<file>_test.go` |
| `cmd/m31a/main.go` | No tests (entry point only; covered by integration). |

**Minimum test coverage for a new tool:**
- `TestX_Name`, `TestX_Description`, `TestX_RiskLevel` (constructor coverage).
- At least one happy-path test exercising real filesystem/process.
- At least one parameter-validation test (missing param, wrong type).
- At least one error test (binary file, timeout, cancellation, permission denied, file-not-found).

**Test naming within a new package:** follow `TestX_Y` style with a stable prefix matching the type under test.

---

*Testing analysis: 2026-06-02*
