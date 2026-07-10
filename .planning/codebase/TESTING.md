# M31A — Testing Strategy

This document describes the complete testing strategy for the M31A project.
Sources: `TESTING.md`, `AGENTS.md`, `CONTRIBUTING.md`, `Makefile`, `e2e_test.go`,
`internal/testutil/envtest.go`, and test files across `pkg/` and `internal/`.

---

## Coverage Targets

| Scope | Target |
|---|---|
| Overall project | **75%** |
| `pkg/taskrunner` | **90%** |
| `pkg/bisect` | **90%** |
| `pkg/rollback` | **90%** |

The three 90% packages are the critical workflow-management packages. Coverage is measured via
`go test -race -cover -coverprofile=coverage.out ./...` (the `make test` target).

---

## Test Commands

```bash
make test                            # Race-enabled tests with coverage (canonical)
make test-fast                       # Tests without race detector (faster iteration)
make test-verbose                    # Verbose output with race + coverage
make test-specific TEST=TestFoo      # Run a single test by name across all packages
make cover                           # Generate HTML coverage report (runs make test first)
make bench                           # Run all benchmarks
make bench-verbose                   # Benchmarks with verbose output
```

### Underlying Go Commands

```bash
# make test
go test -race -cover -coverprofile=coverage.out ./...

# make test-fast
go test -cover ./...

# make test-specific TEST=TestMyTest
go test -v -race -run TestMyTest ./...

# make cover (after make test)
go tool cover -html=coverage.out -o coverage.html
```

---

## Race Detector Usage

The race detector (`-race`) is **required** for all CI and pre-commit test runs (`make test`).
It is only skipped in `make test-fast` for developer speed.

### Required Race Tests

All concurrent data structures and goroutine-managed state must have race-tested coverage:

- `WorkflowCache` — tested with 100 goroutines / 1000 iterations (`TestWorkflowCache_ConcurrentDynamicContext`)
- `DNSCache` — 50 goroutines / 1000 iterations (`TestDNSCache_HighContention`), plus eviction stress (`TestDNSCache_ConcurrentEviction`)

Use `t.Parallel()` in tests that are safe to run concurrently. Observed in:
- `internal/tools/bash_sandbox_test.go` — `t.Parallel()` at the start of each test
- `internal/provider/common_test.go` — `t.Parallel()` for all stateless unit tests

Avoid `t.Parallel()` for tests that modify shared state (e.g., environment variables, working directory, global caches).

---

## Test File Organization

### Co-location Rule

Test files live in the **same directory** as the source they test:

```
pkg/taskrunner/
    runner.go
    runner_test.go      # in-package (package taskrunner)
    doc_test.go         # example / doc tests

pkg/bisect/
    bisect.go
    bisect_test.go      # in-package (package bisect)
    extra_test.go
    doc_test.go

pkg/rollback/
    rollback.go
    rollback_test.go    # in-package (package rollback)
    doc_test.go
```

### Package Declaration Styles

| Style | Package decl | Use |
|---|---|---|
| White-box (in-package) | `package taskrunner` | Access unexported symbols |
| Black-box (external) | `package taskrunner_test` | API-only testing |
| E2E | `package m31a_test` | Binary-level testing |

Most test files in this project use **in-package** style to enable testing of unexported helpers
(e.g., `parseBisectLog` in `pkg/bisect/bisect_test.go`, `applyVarSubstitution` in `internal/config/loader_test.go`).

### Naming Conventions

- Test functions: `Test<TypeOrFunction>_<Scenario>` — e.g., `TestRunner_CircularDependency`, `TestBisect_ParseLog_EdgeCases`.
- Benchmark functions: `Benchmark<Feature>` — in `bench_test.go` files.
- Test helper functions: lowercase, with `t.Helper()` call at the top (e.g., `setupBisectRepo`, `buildBinary`, `createCommits`).

---

## E2E Tests (`e2e_test.go`)

### Package

```go
package m31a_test   // external black-box test package
```

Located at the **project root**, the E2E test file compiles and runs the full binary.

### How the Binary Is Compiled

The `buildBinary` helper function compiles the binary fresh for each test run:

```go
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

Each test calling `buildBinary` incurs a full compile, so E2E tests are expected to be slow.

### Environment Isolation

The `cleanEnv` helper creates a minimal environment for each binary run, stripping all API keys:

```go
func cleanEnv() []string {
    return []string{
        "PATH=" + os.Getenv("PATH"),
        "HOME=" + os.TempDir(),
        "M31A_CONFIG=" + filepath.Join(os.TempDir(), "m31a-test-config.toml"),
    }
}
```

Each binary invocation uses `cmd.Dir = t.TempDir()` to isolate file system state.

### E2E Test Inventory

| Test | What It Checks |
|---|---|
| `TestBinary_Version` | `--version` flag prints `m31a` and `linux/amd64` |
| `TestBinary_Help` | `--help` prints "Terminal AI Coding Agent" and `-prompt` flag |
| `TestBinary_Prompt_NoProvider` | Headless mode fails gracefully with "no provider" message |
| `TestBinary_Prompt_NvidiaRealAPI` | Real NVIDIA API call (skips if `NVIDIA_API_KEY` unset) |
| `TestBinary_Prompt_ZenRealAPI` | Real Zen API call (skips if `ZEN_API_KEY` unset) |
| `TestBinary_Prompt_OpenRouterRealAPI` | Real OpenRouter API call (skips if `OPENROUTER_API_KEY` unset) |
| `TestBinary_Prompt_Timeout` | Binary exits within 30 seconds for a large prompt |

---

## Real API Tests and Skip Conditions

Real API tests use `t.Skip()` when the required environment variable is not set:

```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }
    // ...
}
```

### Guarded Environment Variables

| Variable | Provider |
|---|---|
| `OPENROUTER_API_KEY` | OpenRouter API |
| `ZEN_API_KEY` | Zen API |
| `NVIDIA_API_KEY` | NVIDIA NIM API |

### `internal/testutil` Helpers

The `internal/testutil` package (`internal/testutil/envtest.go`) provides reusable guard utilities:

```go
// Skip if env var unset
key := testutil.RequireAPIKey(t, "OPENROUTER_API_KEY")

// Skip if none of the vars are set
key := testutil.RequireAnyAPIKey(t, "OPENROUTER_API_KEY", "ZEN_API_KEY")

// Load .env.test from project root (safe to call multiple times)
testutil.LoadTestDotEnv(t)
```

`LoadTestDotEnv` walks up the directory tree to find `go.mod`, then loads `.env.test` into the process environment. Existing environment variables are **never overridden**. The loading is protected by a `sync.Once` so it is idempotent.

---

## Unit Test Patterns

### Table-Driven Tests

The **dominant pattern** across the codebase. Every function with multiple input/output scenarios
uses `[]struct` tables:

```go
tests := []struct {
    name     string
    err      error
    expected string
}{
    {"nil", nil, ""},
    {"ErrInvalidKey", ErrInvalidKey, "Invalid API key — run /settings to update"},
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
```

Fields always include `name string` as the first field for `t.Run` subtest naming.

### Mock Patterns

Mocks are hand-written structs implementing the relevant interface — no mock-generation framework is used.

Example from `internal/config/loader_test.go`:

```go
type mockKeychain struct {
    store map[string]string
}

func newMockKeychain() *mockKeychain {
    return &mockKeychain{store: make(map[string]string)}
}

func (m *mockKeychain) Get(service string) (string, error) { ... }
func (m *mockKeychain) Set(service, value string) error    { ... }
func (m *mockKeychain) Delete(service string) error        { ... }
```

The mock implements the `keychain.Keychain` interface exactly, enabling `*mockKeychain` to be substituted anywhere the interface is accepted.

Mock tools in `internal/tools/` are defined in `dispatcher_test.go` as a `mockTool` struct.

### Git Repo Fixtures

Tests in `pkg/bisect` and `pkg/rollback` create real temporary git repositories using helper functions:

```go
func setupBisectRepo(t *testing.T) (string, *Bisect) {
    t.Helper()
    dir := t.TempDir()
    runGit(t, dir, "init")
    runGit(t, dir, "config", "user.name", "Test")
    runGit(t, dir, "config", "user.email", "test@test.com")
    // Create commits...
    return dir, New(dir, slog.Default())
}
```

`t.TempDir()` is always used (never `os.TempDir()` directly) so the directory is automatically cleaned up after the test.

### Sentinel Error Assertions

Tests use `errors.Is` to check for sentinel errors:

```go
if err != m31errors.ErrCircularDependency {
    t.Errorf("Expected ErrCircularDependency, got %v", err)
}
```

And `errors.As` for structured error type assertions:

```go
var target *ToolError
if !errors.As(toolErr, &target) {
    t.Error("errors.As should find *ToolError")
}
```

### Output/State Boundary Tests

Tests verify that public methods return defensive copies, not internal state:

```go
// TestRunner_ResultsReturnsCopy
results1 := r.Results()
results1[1] = TaskResult{Output: "modified"}
results2 := r.Results()
if results2[1].Output == "modified" {
    t.Error("Results() should return a copy")
}
```

---

## Test Helpers and Utilities

### `internal/testutil/envtest.go`

| Function | Purpose |
|---|---|
| `RequireAPIKey(t, envVar)` | Skip test if env var is empty |
| `RequireAnyAPIKey(t, vars...)` | Skip test if none of the env vars are set |
| `LoadTestDotEnv(t)` | Load `.env.test` from project root (idempotent) |

### `t.Helper()` Usage

All helper functions call `t.Helper()` as their first statement so that test failure lines point to the calling test, not to the helper:

```go
func setupRollback(t *testing.T) (*Rollback, *git.Git) {
    t.Helper()
    // ...
}
```

### Environment Variable Isolation

Tests that modify environment variables use `t.Setenv(key, value)` (not `os.Setenv`). `t.Setenv` automatically restores the original value after the test:

```go
t.Setenv("M31A_THEME", "light")  // automatically restored after test
```

### In-Package String Utilities

`pkg/bisect/bisect_test.go` defines minimal string helper functions (`stringsSplit`, `stringsTrimSpace`, `stringsContains`, `index`) rather than importing the `strings` package. This is intentional to minimize test-file imports and keep tests self-contained.

---

## Security Testing Specifics

### Command Injection (`internal/tools/bash_test.go`)

- `TestBash_ObfuscationDetection` — tests double spaces, tabs, mixed case bypass attempts.
- Tests for variable expansion: `$VAR`, `${VAR}`, `$(cmd)`.
- Tests for newlines and special character sequences.

### SSRF Protection (`internal/tools/webfetch_test.go`)

- `TestWebFetch_SSRF*` — tests blocking of loopback (127.x.x.x), RFC1918 (10.x, 192.168.x, 172.16-31.x), and link-local addresses.
- Tests metadata endpoint blocking (`169.254.169.254`).
- Tests DNS pinning via shared DNS cache.

### Type Safety (`internal/tools/permissions_test.go`)

- `TestPermissions_InvalidType` — tests comma-ok type assertion guards on interface values.
- Tests graceful handling of invalid types in concurrent maps.

### Output Limits

Tests verify that tool output is capped at `types.BashOutputLimit` and that a truncation marker is appended:

```go
// TestBashTool_OutputCap
if len(result.Output) > types.BashOutputLimit+100 {
    t.Fatalf("output too large")
}
if !result.Truncated {
    t.Fatal("expected Truncated=true")
}
if !strings.Contains(result.Output, "truncated") {
    t.Fatal("expected truncation marker")
}
```

---

## Coverage Report Generation

```bash
# Step 1: run tests with coverage profile
make test
# → produces coverage.out

# Step 2: open HTML report in browser
make cover
# → produces coverage.html, calls: go tool cover -html=coverage.out -o coverage.html
```

The `coverage.out` file (profile format) and `coverage.html` (report) are listed in `make clean` as artifacts to remove:

```makefile
clean:
    rm -f coverage.out coverage.html
```

To check per-package coverage on the command line:

```bash
go tool cover -func=coverage.out | grep -E "(pkg/taskrunner|pkg/bisect|pkg/rollback|total)"
```

---

## Integration Tests

Integration tests are in files named `integration_test.go` within their package directories:

- `internal/config/integration_test.go`
- `internal/provider/nvidia/integration_test.go`
- `internal/provider/openrouter/integration_test.go`
- `internal/provider/zen/integration_test.go`

These tests use `testutil.RequireAPIKey` or `testutil.LoadTestDotEnv` to guard against missing credentials and are skipped in standard CI runs without API keys.
