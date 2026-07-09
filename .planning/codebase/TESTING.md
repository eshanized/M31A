# Testing — M31A

> Mapped: 2026-07-09

## Test Framework

- **Standard Go `testing` package** — no third-party test frameworks (no testify, no gomega)
- Table-driven tests used where applicable

## Running Tests

```bash
make test           # go test -race -cover ./...
make test-fast      # go test -cover ./... (no race detector)
make test-verbose   # go test -v -race -cover ./...
make test-specific  # go test -v -race -run <TEST> ./...
make bench          # go test -bench=. -benchmem -run=^$ ./...
make cover          # test + HTML coverage report
```

## Coverage Targets

| Scope | Target |
|-------|--------|
| Overall | 75% |
| `pkg/taskrunner/` | 90% |
| `pkg/bisect/` | 90% |
| `pkg/rollback/` | 90% |

## Test Structure

Tests are adjacent to implementation files:
```
internal/tools/permissions.go        → internal/tools/permissions_test.go
internal/tools/permissions.go        → internal/tools/permission_timeout_test.go (edge cases)
```

Special files:
- `e2e_test.go` (root) — End-to-end binary tests (builds binary, runs with args)
- `*_extra_test.go` — Additional test files for complex packages
- `*_integration_test.go` — Integration tests
- `*_benchmark_test.go` — Benchmark tests

## Test Categories

### Unit Tests

Standard Go unit tests covering individual functions and types. Most packages have >80% unit test coverage.

### Integration Tests

- `internal/wiring/` — End-to-end integration regression tests
- `internal/workflow/integration_test.go` — Workflow integration
- `internal/config/integration_test.go` — Config integration

### E2E Tests (`e2e_test.go`)

Tests that compile the binary and run it:
- `TestBinary_Version` — Verify version output
- `TestBinary_Help` — Verify help output
- `TestBinary_Prompt_NoProvider` — Graceful failure without API keys
- `TestBinary_Prompt_Timeout` — Timeout handling (30s timeout)

### Real API Tests (conditional)

Require environment variables — skipped when unset:
- `TestBinary_Prompt_NvidiaRealAPI` — Requires `NVIDIA_API_KEY`
- `TestBinary_Prompt_ZenRealAPI` — Requires `ZEN_API_KEY`
- `TestBinary_Prompt_OpenRouterRealAPI` — Requires `OPENROUTER_API_KEY`

### Benchmark Tests

- `internal/workflow/website_build_test.go` — Website build benchmarks
- `internal/tools/codecomplexity_benchmark_test.go` — Complexity analysis
- `internal/tools/edit_benchmark_test.go` — Edit performance
- `internal/tui/app_channel_bench_test.go` — Channel performance

### Regression Tests

- `internal/tui/app_regression_test.go` — TUI regression tests
- `internal/tui/resume_fixes_test.go` — Session resume fixes
- `internal/tui/sidebar_fixes_test.go` — Sidebar fixes
- `internal/tui/m1_regression_test.go` — M1 chip regression

## Test Patterns Used

### Table-Driven Tests

```go
func TestPermission(t *testing.T) {
    tests := []struct {
        name string
        input string
        want bool
    }{
        {"allow known command", "ls -la", true},
        {"deny dangerous command", "rm -rf /", false},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            // test logic
        })
    }
}
```

### Test Helpers

- `buildBinary(t)` — Compiles the M31A binary for e2e tests
- `cleanEnv()` — Minimal environment without API keys
- Test helpers in `internal/testutil/`

### Mocking

- No external mocking library
- Interface-based design allows test implementations (e.g., mock providers, mock tools)
- Test files create local mock implementations

## CI Test Pipeline

From `.github/workflows/ci.yml`:

1. **lint** — gofmt check + golangci-lint + goreleaser config check (10min timeout)
2. **test** — `go test -race -coverprofile=coverage.out -covermode=atomic ./...` (10min timeout)
3. **security** — govulncheck (10min timeout)
4. **build** — Matrix build: 3 OS x 2 arch (-windows/arm64) with go vet (10min timeout)
5. **release** — goreleaser (draft, tagged releases only)

## Test Exclusions

- Test files excluded from `errcheck` and `unused` linters (configured in `.golangci.yml`)
- Real API tests skip when env vars are unset
- Coverage generation excludes test files themselves
