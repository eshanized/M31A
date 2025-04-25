---
focus: quality
last_mapped: 2026-05-30
---

# M31A — Testing Practices

## Test Framework

- **stdlib `testing` package only** — no testify, no Ginkgo, no external test frameworks
- **Race detector**: `go test -race ./...` is the standard test command
- **Coverage**: `go test -race -cover -coverprofile=coverage.out ./...`

## Test Organization

- Test files co-located with source files: `*_test.go` in the same package
- 56 test files across the codebase (44% of 128 total Go files)
- Table-driven tests with anonymous structs are the dominant pattern
- Subtests via `t.Run()` for grouped test cases

## Mocking Strategy

- **HTTP providers**: `httptest.NewServer` with scripted responses for OpenRouter and Zen tests (`internal/provider/openrouter/client_test.go`, `internal/provider/zen/client_test.go`)
- **Filesystem**: `t.TempDir()` for all tools (FileRead, FileWrite, Glob, Grep) and session tests
- **Git**: scripted git operations against temp directories with `git init`
- **No mock frameworks** — pure function composition and interface testing

## Coverage Areas

| Package | Test File | What's Tested |
|---------|-----------|---------------|
| `internal/config/` | `loader_test.go` | Config load, save, API key resolution |
| `internal/git/` | `git_test.go` | Init, commit, log, diff, bisect, reset |
| `internal/log/` | `log_test.go` | Logger init, rotation, formatting |
| `internal/provider/` | `cache_test.go` | TTL expiry, stale fallback |
| | `registry_test.go` | Register, set active, get, list |
| | `sse_test.go` | SSE line parsing |
| | `reasoning_test.go` | Reasoning detection, SSE chunk parsing |
| | `openrouter/client_test.go` | Model fetch, streaming, error handling |
| | `zen/client_test.go` | Model fetch, streaming, error handling |
| `internal/tokens/` | `estimator_test.go` | Token estimation, EMA calibration, warning banner |
| `internal/tools/` | `bash_test.go` | Command execution, timeout, binary output detection |
| | `fileread_test.go` | File reading, binary detection, size limit, path safety |
| | `filewrite_test.go` | Atomic write, backup, directory creation, path safety |
| | `glob_test.go` | Pattern matching, result limits, recursive patterns |
| | `grep_test.go` | Regex search, ripgrep detection, fallback |
| | `dispatcher_test.go` | Tool registration, execution, permission gate |
| `internal/tui/` | `app_test.go` | App state, screen routing |
| | `repl_test.go` | REPL model, streaming |
| | `commands_test.go` | All 16 slash commands |
| | `screens_test.go` | Screen model validations |
| | `modelselector_test.go` | Model search, filtering |
| | `settings_test.go` | Settings editing, tabs |
| | `plan_test.go`, `execute_test.go`, `verify_test.go`, `ship_test.go` | Workflow screen models |
| | `firstrun_test.go`, `resume_test.go` | Setup wizards |
| | `header_test.go`, `statusbar_test.go` | Header/status bar rendering |
| | `streaming_test.go`, `health_test.go`, `cache_test.go` | Tickers, streaming |
| | `theme/theme_test.go` | Theme cycle, default |
| | `components/message_test.go`, `thinking_test.go`, `permission_test.go`, `toolcard_test.go` | Component rendering |
| `internal/workflow/` | `engine_test.go` | Phase routing, task parsing, validation |
| | `initialize_test.go`, `discuss_test.go`, `plan_test.go`, `execute_test.go`, `verify_test.go`, `ship_test.go` | Individual phases |
| | `integration_test.go` | Full workflow integration |
| `pkg/arbitrage/` | `arbitrage_test.go` | Complexity scoring, cost comparison, recommendation |
| `pkg/autodream/` | `autodream_test.go` | Consolidation, protection rules, stats |
| `pkg/bisect/` | `bisect_test.go` | Bisect execution, result parsing |
| `pkg/keychain/` | `keychain_test.go` | Keychain operations |
| `pkg/ledger/` | `ledger_test.go` | Append, query, stats, truncate |
| `pkg/rollback/` | `rollback_test.go` | Chain, soft/hard reset |
| `pkg/session/` | `session_test.go` | Session CRUD |
| | `manager_test.go` | File management |
| | `checkpoint_test.go` | Checkpoint save/load |
| | `planning_test.go` | Markdown file parsing |
| `pkg/taskrunner/` | `runner_test.go` | Topological sort, execution groups, status tracking |

## Key Test Patterns

### Table-Driven Tests

```go
func TestParseCommand(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        want    string
        args    []string
        handled bool
    }{
        {name: "simple", input: "/help", want: "help", args: nil, handled: true},
        // ...
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) { /* ... */ })
    }
}
```

### Mock HTTP Server

```go
server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
    json.NewEncoder(w).Encode(mockResponse)
}))
```

### Temp Directory Cleanup

```go
t.TempDir() // automatically removed after test
```

## Current Test Status

- All tests pass with `go test -race -cover ./...`
- Phase 7 tests pass (latest verified)
- Integration test (`internal/workflow/integration_test.go`) tests full phase pipeline

## Linting & Static Analysis

- `golangci-lint` with 6 linters: govet, staticcheck, errcheck, ineffassign, unused, gosimple
- `go vet ./...` runs in CI
- 5-minute lint timeout in `.golangci.yml`
- Race detector enabled in all test runs
