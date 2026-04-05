# TESTING.md — Testing Patterns & Practices

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## Test Framework

- **Standard library:** `testing` package (Go built-in)
- **Assertions:** Standard `t.*` methods (`t.Errorf`, `t.Fatalf`, `t.Logf`) — no external assertion library
- **Test runner:** `go test` via `make test` (with race detector) or `make test-fast` (without)

## Test Distribution

**Total test files:** ~120 across the codebase

| Package | Test Files | Coverage |
|---|---|---|
| `internal/config/` | 2 (+1 extra) | Config loading, project context |
| `internal/errors/` | 1 | Error sentinel checks |
| `internal/fileutil/` | 2 | Atomic file operations |
| `internal/git/` | 2 | Git commands |
| `internal/log/` | 2 | Logger initialization |
| `internal/provider/` | 11 | Providers, caching, fallback, SSE, capabilities |
| `internal/provider/openrouter/` | 2 | OpenRouter API client |
| `internal/provider/zen/` | 1 | Zen API client |
| `internal/tokens/` | 2 | Token estimation, context warnings |
| `internal/tools/` | 18 | All tool implementations, dispatcher, subagent, permissions |
| `internal/tui/` | 25 | TUI models, views, components, theme, layout |
| `internal/types/` | 2 | Shared types |
| `internal/workflow/` | 18 | Workflow engine, plan parsing, phases |
| `pkg/arbitrage/` | 1 | Cost arbitrage |
| `pkg/autodream/` | 2 | Autonomous task chaining |
| `pkg/bisect/` | 2 | Git bisect |
| `pkg/keychain/` | 2 | Keychain integration |
| `pkg/ledger/` | 2 | Decision ledger |
| `pkg/rollback/` | 1 | Git rollback |
| `pkg/session/` | 7 | Session management, checkpoints, planning |
| `pkg/taskrunner/` | 1 | Task runner |

## Test Patterns

### Standard Unit Tests
Tests use standard Go table-driven tests extensively:
```go
func TestSomething(t *testing.T) {
    tests := []struct {
        name string
        input string
        want  string
    }{
        {name: "valid input", input: "hello", want: "HELLO"},
        {name: "empty input", input: "", want: ""},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            got := process(tt.input)
            if got != tt.want {
                t.Errorf("process() = %q, want %q", got, tt.want)
            }
        })
    }
}
```

### Extra Test Files
Many packages have `extra_test.go` files that contain integration-style tests or tests requiring external dependencies:
- `internal/provider/extra_test.go`
- `internal/tools/extra_test.go`
- `internal/tui/extra_test.go`
- `internal/tui/components/extra_test.go`
- `internal/tui/config_model_extra_test.go`
- `internal/tui/settings_extra_test.go`
- `internal/tui/settings_model_extra_test.go`
- `internal/tui/plan_extra_test.go`
- `pkg/session/manager_extra_test.go`
- `pkg/session/planning_extra_test.go`

These are separated from main test files to distinguish pure unit tests from tests that may need more setup or run slower.

### Mocking Strategy
- Interfaces used extensively for testability (e.g., `LLMProvider`, `PermissionGate`, `MsgEmitter`)
- No external mocking library — tests use hand-written mock implementations
- Example provider mock pattern:
```go
type mockProvider struct {
    name string
    err  error
}
func (m *mockProvider) Name() string { return m.name }
```

### Concurrency Testing
- Race detector enabled in `make test` (`-race` flag)
- Tests use `sync.WaitGroup` for goroutine coordination
- Channel-based tests with timeouts to prevent deadlocks

## Coverage

- **Coverage command:** `make cover` (generates HTML report)
- **Coverage targets:** All packages via `./...`
- **Coverage output files:** `cover.out`, `coverage_final.out`, `cov2.out` in project root
- **No specific coverage threshold enforced** — measured but not gated

## Test Configuration (`Makefile`)

```makefile
test:
    @go test -race -cover -coverprofile=$(COVER_OUT) ./...

test-fast:
    @go test -cover ./...

test-verbose:
    @go test -v -race -cover ./...

test-specific:
    @go test -v -race -run $(TEST) ./...

bench:
    @go test -bench=. -benchmem -run=^$$ ./...
```

## Notable Test Files

### Provider Tests
| File | Tests |
|---|---|
| `internal/provider/cache_test.go` | Model caching with TTL, stale behavior |
| `internal/provider/capabilities_test.go` | Model capabilities detection |
| `internal/provider/fallback_test.go` | Auto-fallback between providers |
| `internal/provider/reasoning_test.go` | Reasoning/thinking support |
| `internal/provider/resilience_test.go` | Provider resilience under failures |
| `internal/provider/sse_test.go` | SSE stream parsing |
| `internal/provider/registry_test.go` | Provider registration/activation |

### Tool Tests
| File | Tests |
|---|---|
| `internal/tools/bash_test.go` | Bash execution, timeout, output limits |
| `internal/tools/bash_security_test.go` | Shell injection prevention |
| `internal/tools/bash_kill_test.go` | Bash kill grace period |
| `internal/tools/edit_test.go` | File edit operations |
| `internal/tools/fileread_test.go` | File read operations |
| `internal/tools/filewrite_test.go` | File write operations |
| `internal/tools/glob_test.go` | Glob pattern matching |
| `internal/tools/grep_test.go` | Grep search |
| `internal/tools/grep_truncation_test.go` | Grep output truncation |
| `internal/tools/permissions_test.go` | Permission rule evaluation |
| `internal/tools/permission_timeout_test.go` | Permission modal timeout |
| `internal/tools/dispatcher_test.go` | Tool dispatch routing |
| `internal/tools/webfetch_test.go` | Web fetching |
| `internal/tools/webfetch_security_test.go` | SSRF protection |
| `internal/tools/webfetch_helpers_test.go` | WebFetch helper utilities |
| `internal/tools/subagent/manager_test.go` | Subagent lifecycle |
| `internal/tools/subagent/loop_parse_test.go` | Subagent tool call parsing |

### Workflow Tests
| File | Tests |
|---|---|
| `internal/workflow/engine_test.go` | Core engine behavior |
| `internal/workflow/engine_parse_test.go` | LLM response parsing |
| `internal/workflow/discuss_test.go` | Discuss phase |
| `internal/workflow/plan_test.go` | Plan phase |
| `internal/workflow/execute_test.go` | Execute phase |
| `internal/workflow/verify_test.go` | Verify phase |
| `internal/workflow/ship_test.go` | Ship phase |
| `internal/workflow/initialize_test.go` | Init phase |
| `internal/workflow/integration_test.go` | Cross-phase flows |
| `internal/workflow/plan_parser_test.go` | Plan parsing from LLM |
| `internal/workflow/phase_transition_test.go` | Phase transition validation |
| `internal/workflow/thinking_indicator_test.go` | Thinking indicator |
| `internal/workflow/self_heal_visibility_test.go` | Self-heal behavior |
| `internal/workflow/intermediate_progress_test.go` | Progress reporting |

### TUI Component Tests
| File | Tests |
|---|---|
| `internal/tui/components/message_test.go` | Message rendering |
| `internal/tui/components/permission_test.go` | Permission modal |
| `internal/tui/components/sparkline_test.go` | Sparkline rendering |
| `internal/tui/components/starfield_test.go` | Starfield animation |
| `internal/tui/components/thinking_test.go` | Thinking block |
| `internal/tui/components/toolcard_test.go` | Tool card rendering |
| `internal/tui/layout/responsive_test.go` | Responsive layout |
| `internal/tui/theme/registry_test.go` | Theme registry |
| `internal/tui/theme/theme_test.go` | Theme switching |
| `internal/tui/tui_test.go` | TUI integration |
