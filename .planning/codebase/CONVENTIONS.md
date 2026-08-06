# Coding Conventions

**Analysis Date:** 2026-08-06

## Code Style

**Formatting:**
- All code must be `gofmt`-clean
- `goimports` for import organization (run via `make fmt`)
- Imports: group stdlib / third-party / project
- 4-space indentation (Go standard)
- Max line length: ~120 characters (soft limit)

**Linting:**
- `golangci-lint` with 5-minute timeout
- Enabled linters: `govet` (with shadow), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- Test files excluded from `errcheck` and `unused`
- Config: `golangci-lint run ./... --timeout=5m`

**General Rules:**
- Return errors, never panic. Wrap with `fmt.Errorf("%w", err)`
- Exported functions/types need doc comments
- No emojis in code or docs

## File Organization

**Engine Files (engine_<concern>.go pattern):**
- Each file handles one concern within the `workflow` package
- All engine files share the `Engine` receiver
- No new packages for small amounts of code; prefer same-package splitting
- Naming: `engine_<concern>.go` (e.g., `engine_pause.go`, `engine_streaming.go`)

**General File Naming:**
- Snake_case for Go files: `engine.go`, `dispatcher_test.go`, `runner.go`
- Test files use `_test.go` suffix: `engine_test.go`, `permissions_test.go`
- Package directories use lowercase: `workflow/`, `taskrunner/`, `fileops/`

**Naming Patterns:**
- CamelCase for exported functions: `NewDispatcher()`, `BuildToolDefs()`, `RunPhase()`
- camelCase for unexported functions: `ensurePermission()`, `extractCommandString()`
- PascalCase for exported types: `Dispatcher`, `Engine`, `TaskRunner`
- Interface names describe capability: `Tool`, `SchemaProvider`, `LLMProvider`
- Error types suffix with `Error`: `ToolError`, `ProviderError`, `ConfigError`

## Concurrency

**Lock Ordering (single authoritative source: `engine_concurrency.go`):**
```
transitionMu > planMu > messagesMu > intentResultMu > cachedFullPromptsMu > checkpointMu
```
- When acquiring multiple locks, always acquire in the documented order
- Never acquire a lock that is earlier in the order while holding a later lock
- Each accessor method in WorkflowState acquires exactly one lock
- Code that needs multiple fields must acquire locks in order or use snapshot methods

**Bubble Tea Contract:**
- All state mutations go through `Update()` only
- Goroutines communicate via `tea.Cmd` / `tea.Msg` channels
- Never mutate `AppState` from a goroutine
- Use `MsgEmitter` to bridge goroutine output to TUI

**Pattern:**
- Use `sync.RWMutex` for read-heavy data
- Use `sync.Map` for concurrent maps
- Use channels for goroutine communication
- Use `sync.Once` for initialization

## Logging

**Structured Logging with slog:**
- Use `log/slog` for all logging (stdlib, zero deps)
- Structured fields with consistent naming (camelCase)
- Component-specific loggers via `slog.Default().With("component", "name")`
- Debug level for internal diagnostics, Info for significant events, Warn for recoverable errors, Error for failures

**Field Naming Convention:**
- Use camelCase for field names (Go convention)
- Common fields: `phase`, `model`, `error`, `elapsed`, `session_id`, `component`
- Never log API keys or secrets

**Example:**
```go
logger := slog.Default().With("component", "workflow-engine")
logger.Info("phase started", "phase", phase, "model", modelID)
logger.Error("phase failed", "phase", phase, "error", err, "elapsed", time.Since(start))
```

## Interface Boundaries

- Go module system for hard boundaries (`pkg/` must NOT import `internal/`)
- Interfaces at consumer boundary for soft boundaries
- Constructor injection via options structs (`EngineOptions` pattern)
- No runtime DI containers (per D-06)
- Small interfaces: "accept interfaces, return structs"

## Error Handling

**Patterns:**
- Sentinel errors in `internal/core/errors/errors.go`
- Typed wrappers with `Unwrap()` for `errors.Is`/`errors.As`
- User-facing messages via `errors.UserMessage()`
- All errors wrapped with context: `fmt.Errorf("context: %w", err)`
- Workflow phases return `(*PhaseResult, error)`
- Tool execution returns `ToolError` with tool name and operation context

**Example:**
```go
// Sentinel errors
var ErrPermissionDenied = errors.New("permission denied")

// Custom error type
type ToolError struct {
    Tool string
    Op   string
    Err  error
}

func (e *ToolError) Error() string {
    if e.Op != "" {
        return "tool " + e.Tool + ": " + e.Op + ": " + e.Err.Error()
    }
    return "tool " + e.Tool + ": " + e.Err.Error()
}

func (e *ToolError) Unwrap() error { return e.Err }
```

## Testing

**Table-Driven Tests:**
```go
func TestSomething(t *testing.T) {
    tests := []struct {
        name    string
        input   string
        expected string
    }{
        {"case1", "input1", "expected1"},
        {"case2", "input2", "expected2"},
    }

    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) {
            t.Parallel() // when safe
            got := FunctionUnderTest(tt.input)
            if got != tt.expected {
                t.Errorf("got %q, want %q", got, tt.expected)
            }
        })
    }
}
```

**Hand-Written Mocks:**
- No mocking library — write mocks by hand
- Use `t.Helper()` in test helper functions
- Use `t.Cleanup()` for resource cleanup

**Race Detector:**
- Always test with `-race` flag (`make test`)
- Quick smoke test: `make test-fast` (no race detector)

**Integration Over Unit:**
- Prefer integration tests for workflow logic
- Unit tests for pure functions and edge cases

**Parallel Testing:**
- Use `t.Parallel()` for independent tests
- Avoid for tests with shared mutable state
- Use `t.TempDir()` for isolated test directories

## Commit Messages

**Conventional Commits:**
- `feat:` — New feature, endpoint, component
- `fix:` — Bug fix, error correction
- `docs:` — Documentation only
- `test:` — Test-only changes
- `refactor:` — Code cleanup, no behavior change
- `chore:` — Config, tooling, dependencies

**Format:**
```
type(scope): concise description

- Detail 1
- Detail 2
```

## Import Organization

**Order:**
1. Standard library (`context`, `fmt`, `os`, `sync`)
2. Third-party packages (`github.com/charmbracelet/bubbletea`, `github.com/bmatcuk/doublestar/v4`)
3. Project imports (`github.com/eshanized/M31A/internal/...`)

**Path Aliases:**
- `m31errors` for `github.com/eshanized/M31A/internal/core/errors`
- `m31types` for `github.com/eshanized/M31A/internal/core/types`
- `toolsExec` for `github.com/eshanized/M31A/internal/tools/exec`

## Configuration

**Pattern:**
- TOML for main config: `~/.m31a/config.toml`
- Environment variables for secrets: `OPENROUTER_API_KEY`
- Keychain for API keys: `pkg/keychain/`
- Never commit secrets to git

## Key Gotchas

- **No CGO:** Build with `CGO_ENABLED=0` (hard constraint)
- **Bubble Tea:** Single-threaded UI framework; never mutate state from goroutines
- **Provider models:** Dynamic from APIs; never hardcode model names
- **API keys:** Use OS keychain; never write to disk in plaintext
- **Error wrapping:** Use `%w` verb for `errors.Is()`/`errors.As()` compatibility
- **Engine splitting:** All engine split files remain in the same package (`workflow`)

---

*Convention analysis: 2026-08-06*
