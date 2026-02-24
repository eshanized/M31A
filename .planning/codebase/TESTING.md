# Testing

> Test framework, structure, patterns, and coverage approach.

## Framework

- **Standard `testing` package** — no external test framework
- Tests run with `-race` flag and `-cover` coverage
- Command: `go test -race -cover ./...`

## Test File Organization

Test files use `_test.go` suffix, co-located with source files:

| Package | Test Files | Coverage |
|---------|-----------|----------|
| `internal/tools/` | `bash_test.go`, `fileread_test.go`, `edit_test.go`, `grep_test.go`, etc. | Tool execution, security, edge cases |
| `internal/workflow/` | `engine_test.go`, `discuss_test.go`, `plan_test.go`, `verify_test.go`, etc. | Workflow phases, streaming, parsing |
| `internal/provider/` | `cache_test.go`, `sse_test.go`, `fallback_test.go`, etc. | Provider resilience, caching, streaming |
| `pkg/session/` | `session_test.go`, `checkpoint_test.go`, `planning_test.go` | Session persistence, resume |
| `pkg/taskrunner/` | `runner_test.go` | Dependency graph, topological sort |
| `pkg/ledger/` | `ledger_test.go` | Cross-session learning |
| `internal/tui/components/` | `toolcard_test.go`, `thinking_test.go`, etc. | Component rendering |

**Total: 69 test files**

## Test Patterns

### Parallel Tests

Most tests use `t.Parallel()` for concurrent execution:

```go
func TestBash_SimpleCommand(t *testing.T) {
    t.Parallel()
    b := NewBash(t.TempDir())
    // ...
}
```

### Table-Driven Tests

Used for multiple input variants:

```go
func TestBash_Timeout(t *testing.T) {
    tests := []struct {
        name    string
        command string
        wantErr bool
    }{
        {"immediate", "echo hello", false},
        {"timeout", "sleep 60", true},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) { ... })
    }
}
```

### Test Helpers

- `t.TempDir()` for isolated filesystem tests
- Context with timeout: `context.WithTimeout(context.Background(), 5*time.Second)`
- Mock providers for streaming tests

### Tool Testing Pattern

Tools implement the `types.Tool` interface and are tested via `Execute()`:

```go
result, err := tool.Execute(context.Background(), types.ToolInput{
    Name: "ToolName",
    Params: map[string]any{
        "param": "value",
    },
})
```

Each tool verifies: `Name()`, `Description()`, `RiskLevel()`, and execution behavior.

## Security Testing

Dedicated security test files:
- `bash_security_test.go` — command injection, SSRF protection
- `webfetch_security_test.go` — URL validation, IP blocking
- `permission_timeout_test.go` — permission gate timeout behavior

## Mocking Approach

- No mocking library — manual interface implementations
- Provider mocking via custom `LLMProvider` implementations in test files
- Channel-based permission mocking in dispatcher tests

## Coverage Strategy

- Unit tests per tool implementation
- Integration tests for workflow phases (`integration_test.go`)
- Edge case tests: truncation, timeout, cancellation, malformed input
- Race condition testing via `-race` flag

## Running Tests

```bash
go test -race -cover ./...           # all tests with coverage
go test -race -cover ./internal/tools/  # single package
go test -run TestBash ./internal/tools/  # specific test
```
