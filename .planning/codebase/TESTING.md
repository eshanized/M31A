# TESTING.md — M31A Testing Practices

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Framework

- **Go standard `testing` package** — no third-party test framework
- **Race detector**: Enabled via `-race` flag in `make test`
- **Coverage**: `-cover` flag, output to `coverage.out`
- **Benchmarks**: `testing.B` for performance tests

## Test Structure

### Unit Tests
- Located alongside source files (`*_test.go`)
- Same package (white-box testing)
- Pattern: `TestFunctionName` or `TestType_Method`
- Examples:
  - `internal/tools/edit_test.go`
  - `internal/engine/workflow/engine_test.go`
  - `internal/integrations/provider/capabilities_test.go`

### Additional Test Variants
| Suffix | Purpose |
|--------|---------|
| `*_extra_test.go` | Additional coverage for edge cases |
| `*_race_test.go` | Race condition stress tests |
| `*_benchmark_test.go` | Performance benchmarks |
| `*_integration_test.go` | Cross-package integration tests |

### Integration Tests
- Location: `internal/tests/`
- `internal/tests/tools/` — tool integration tests
- `internal/tests/tui/` — TUI integration tests

### E2E Tests
- Location: `tests/e2e/`
- Compiles the binary and runs it as a subprocess
- Validates CLI flags, output, exit codes

## Test Helpers

### `internal/testutil/`
- `ci/` — CI-specific utilities (environment detection, skip logic)
- `testtimeout/` — Timeout helpers for long-running tests

### Common Patterns
```go
func TestSomething(t *testing.T) {
    if testing.Short() {
        t.Skip("skipping in short mode")
    }
    // ...
}
```

## Mocking & Fixtures

### Provider Mocks
- `internal/integrations/provider/common_test.go` — mock provider implementations
- `internal/integrations/provider/interface_test.go` — interface verification

### Tool Test Utilities
- `internal/tools/testutil_test.go` — shared test helpers
- `internal/tools/defaults_test.go` — dispatcher setup helpers

### Filesystem Fixtures
- Tests create temp directories via `t.TempDir()`
- Clean up automatically via `testing.T` cleanup

## Running Tests

```bash
make test              # Full test suite with race detector + coverage
make test-fast         # Tests without race detector (faster)
make test-verbose      # Verbose output
make test-specific TEST=TestFoo  # Single test
make bench             # Benchmarks
make cover             # HTML coverage report
```

## Coverage

### Targets
- **75%** overall project coverage
- **90%** for critical packages:
  - `internal/engine/workflow/` — workflow orchestration
  - `internal/tools/` — tool execution
  - `internal/integrations/provider/` — LLM providers

### Coverage Reports
```bash
make test              # Generates coverage.out
make cover             # Generates coverage.html
go tool cover -html=coverage.out  # View in browser
```

## Test Organization by Package

| Package | Test Files | Focus |
|---------|------------|-------|
| `internal/tools/` | ~20 test files | Tool execution, permissions, safety |
| `internal/engine/workflow/` | ~30 test files | Phase orchestration, parsing, verification |
| `internal/integrations/provider/` | ~10 test files | Provider interface, caching, fallback |
| `internal/ui/tui/` | ~40 test files | TUI models, handlers, navigation |
| `internal/engine/session/` | ~10 test files | Session persistence, checkpoints |
| `cmd/m31a/` | 2 test files | CLI flags, usage output |

## CI Integration

- Tests run in GitHub Actions
- Race detector enabled in CI
- Coverage uploaded to Codecov
- Lint checks via golangci-lint
