# Testing Patterns

**Analysis Date:** 2026-07-11

## Test Framework

**Runner:** Standard `testing` package (no external test framework)

**Assertion Style:** Idiomatic `if got != want { t.Errorf(...) }` — no assert library.

**Test Files:** Co-located with source as `<name>_test.go` in same package.

**Parallel Execution:** Used extensively via `t.Parallel()` for unit tests.

```go
func TestRunner_SingleTask(t *testing.T) {
    t.Parallel()
    ...
}
```

## Test Commands

| Command | Description |
|---------|-------------|
| `make test` | Run all tests with race detector + coverage (`-race -cover`) |
| `make test-fast` | Run tests without race detector (`-cover`) |
| `make test-verbose` | Verbose output with race detector |
| `make test-specific TEST=TestFoo` | Run single test (`-v -race -run TestFoo ./...`) |
| `make bench` | Run benchmarks (`-bench=. -benchmem`) |
| `make cover` | Generate HTML coverage report (`go tool cover -html=coverage.out`) |

**Race Detector:** Enabled by default in `make test` (CI target). `make test-fast` skips it for speed.

## Coverage Targets

| Package | Target |
|---------|--------|
| Overall | 75% |
| `pkg/taskrunner` | 90% |
| `pkg/bisect` | 90% |
| `pkg/rollback` | 90% |

**Coverage Config:** No `go test -coverpkg` config found; targets enforced via CI/process.

**Coverage Files:**
- `coverage.out` — raw profile
- `coverage.html` — HTML report

## Test Organization

### Unit Tests (Package-Level)

Located in `*_test.go` alongside source. Patterns:

**Table-Driven Tests:**
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

**Test Helpers:** Private functions prefixed with `setup` or `new` in test files.
```go
func setupBisectRepo(t *testing.T) (string, *Bisect) {
    t.Helper()
    dir := t.TempDir()
    // ... git init, commits
    return dir, b
}
```

**Parallel Subtests:** `t.Run` with `t.Parallel()` for independent cases.

### E2E Tests (`e2e_test.go`)

**Location:** Module root (`e2e_test.go`, package `m31a_test`)

**Pattern:** Compiles binary via `go build`, executes as subprocess.

```go
func TestBinary_Prompt_NvidiaRealAPI(t *testing.T) {
    apiKey := os.Getenv("NVIDIA_API_KEY")
    if apiKey == "" {
        t.Skip("NVIDIA_API_KEY not set — skipping real API test")
    }
    bin := buildBinary(t)
    cmd := exec.Command(bin, "--prompt", "What is 2+2?", "--model", "meta/llama-3.1-8b-instruct")
    cmd.Env = append(cleanEnv(), "NVIDIA_API_KEY="+apiKey)
    out, err := cmd.CombinedOutput()
    // ...
}
```

**Real API Tests:** Require env vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`) — skipped if unset.

**Helpers:**
- `buildBinary(t *testing.T) string` — compiles to temp dir
- `cleanEnv() []string` — minimal env without API keys
- `runBinary(t, bin, args...)` — executes and returns stdout

### Doc Coverage Tests

Packages with `doc.go` have a trivial test to ensure package compiles and doc is present:
```go
// pkg/rollback/doc_test.go
func TestDoc(t *testing.T) {
    t.Parallel()
    // Verify the package compiles and doc.go is present
}
```

### Benchmarks

Located in `*_bench_test.go` or `*_test.go` with `Benchmark` prefix.
```go
func BenchmarkBisect_Run(b *testing.B) {
    // setup
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        b.Run(...)
    }
}
```
Run via `make bench` or `make bench-verbose`.

## Test Patterns by Package

### `pkg/taskrunner` (90% target)

- Topological sort correctness (linear, diamond, independent, circular)
- Execution: success, failure, skip propagation, retries, timeouts
- Callbacks: `OnTaskStart`, `OnTaskUpdate`
- Concurrency: semaphore bounded parallelism
- Edge cases: nil execute fn, terminal states, task not found

### `pkg/bisect` (90% target)

- Git repo setup via `t.TempDir()` + `exec.Command("git", ...)`
- Check function returning `bool` to drive bisect
- Log parsing edge cases (bracket formats, malformed lines)
- Error handling: invalid hashes, reset failures, empty repo
- Diff extraction and truncation

### `pkg/rollback` (90% target)

- Commit chain traversal (newest-first, limit, empty)
- Preview diffs, truncation at `BashOutputLimit`
- Soft/Hard/Safe reset behaviors (stash, no-op, dirty tree)
- Uncommitted changes detection (clean, dirty, staged, deleted)
- Callbacks on reset
- Error paths: invalid hash, destroyed repo

### `internal/errors`

- Sentinel error uniqueness
- Error wrapping (`%w`) preserves `errors.Is`/`errors.As`
- Structured errors: `ToolError`, `ProviderError`, `ConfigError`
- `UserMessage` mapping for all sentinels + pattern matching

### `internal/tools`

- Pure function unit tests (Levenshtein, diff, line replacement)
- Cascading replace strategies (exact → trimmed → normalized → anchor → fuzzy)
- Private function testing via same-package tests
- Security: `isPrivateIP` for SSRF protection

## Mocking & Test Doubles

**No Mock Framework** — uses manual fakes and interfaces.

**Patterns:**
1. **Interface + Fake Implementation** (e.g., `provider.LLMProvider` in tests)
2. **Function Parameters** — pass behavior as `func()` (e.g., `ExecuteFunc`, `checkFn`)
3. **Nil Logger** — accept `*slog.Logger` or `nil` (tested in `TestBisect_NilLogger`)

```go
// In bisect_test.go: checkFn drives the test
checkFn := func() bool {
    content, _ := os.ReadFile(filepath.Join(dir, "util.go"))
    return strings.Contains(string(content), "func helper")
}
result, err := b.Run(sessionStartHash, headHash, checkFn)
```

## Test Utilities

| Helper | Location | Purpose |
|--------|----------|---------|
| `t.TempDir()` | stdlib | Isolated temp directories |
| `t.Helper()` | stdlib | Mark helper for correct line numbers |
| `runGit(t, dir, args...)` | `pkg/bisect/bisect_test.go` | Git commands in tests |
| `writeFile(t, dir, name, content)` | `pkg/bisect/bisect_test.go` | Write test files |
| `cleanEnv()` | `e2e_test.go` | Minimal env without secrets |
| `buildBinary(t)` | `e2e_test.go` | Compile binary to temp path |

## Running Specific Test Patterns

```bash
# Single test
make test-specific TEST=TestRunner_ExecuteFunction

# Pattern match (all TaskRunner tests)
go test -v -race -run 'TestRunner' ./pkg/taskrunner/...

# Package only
go test -v -race ./pkg/bisect/...

# With coverage for specific package
go test -cover ./pkg/rollback/...
```

## CI Integration

**Full Check Pipeline:** `make check` runs:
```
fmt → tidy → vet → lint → test
```

**Release Validation:** `make validate-release` runs `scripts/validate-release.sh`

## Key Files for Test Reference

| File | Pattern Demonstrated |
|------|---------------------|
| `e2e_test.go` | Binary E2E, real API tests, temp dir builds |
| `internal/errors/errors_test.go` | Table-driven, error wrapping, sentinels |
| `pkg/taskrunner/runner_test.go` | Concurrency, callbacks, state transitions |
| `pkg/bisect/bisect_test.go` | Git repo setup, check functions, log parsing |
| `pkg/rollback/rollback_test.go` | Git operations, stash, reset variants |
| `internal/tools/edit_test.go` | Pure function tests, string algorithms |
| `internal/workflow/coverage_gates_test.go` | Plan validation gate tests |

---

*Testing analysis: 2026-07-11*