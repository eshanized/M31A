# Coding Conventions

**Analysis Date:** 2026-08-04

## Naming Patterns

**Files:**
- Snake_case for Go files: `engine.go`, `dispatcher_test.go`, `runner.go`
- Test files use `_test.go` suffix: `engine_test.go`, `permissions_test.go`
- Package directories use lowercase: `workflow/`, `taskrunner/`, `fileops/`

**Functions:**
- CamelCase for exported functions: `NewDispatcher()`, `BuildToolDefs()`, `RunPhase()`
- camelCase for unexported functions: `ensurePermission()`, `extractCommandString()`
- Test functions use `Test` prefix: `TestDispatcher_RegisterAndExecute()`
- Benchmark functions use `Benchmark` prefix: `BenchmarkCascadingReplace()`

**Variables:**
- camelCase for local variables: `workDir`, `sessionMgr`, `dispatcher`
- PascalCase for exported fields: `Name_`, `RiskLevel_`, `ExecFunc`
- Constants in PascalCase: `MaxConcurrentTools`, `ToolRateLimitBurst`
- Private fields with underscore suffix for mock fields: `Name_`, `Description_`

**Types:**
- PascalCase for exported types: `Dispatcher`, `Engine`, `TaskRunner`
- Interface names describe capability: `Tool`, `SchemaProvider`, `LLMProvider`
- Error types suffix with `Error`: `ToolError`, `ProviderError`, `ConfigError`

## Code Style

**Formatting:**
- `gofmt` for standard formatting
- `goimports` for import organization (run via `make fmt`)
- 4-space indentation (Go standard)
- Max line length: ~120 characters (soft limit)

**Linting:**
- `golangci-lint` with 5-minute timeout
- Enabled linters: `govet` (with shadow), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- Test files excluded from `errcheck` and `unused`
- Config: `golangci-lint run ./... --timeout=5m`

## Import Organization

**Order:**
1. Standard library (`context`, `fmt`, `os`, `sync`)
2. Third-party packages (`github.com/charmbracelet/bubbletea`, `github.com/bmatcuk/doublestar/v4`)
3. Project imports (`github.com/eshanized/M31A/internal/...`)

**Path Aliases:**
- `m31errors` for `github.com/eshanized/M31A/internal/core/errors`
- `m31types` for `github.com/eshanized/M31A/internal/core/types`
- `toolsExec` for `github.com/eshanized/M31A/internal/tools/exec`

## Error Handling

**Patterns:**
- Always return errors, never panic
- Wrap errors with `fmt.Errorf("%w", err)` for context
- Use sentinel errors for known error conditions
- Custom error types implement `Error()` and `Unwrap()` methods

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

## Comments

**When to Comment:**
- Exported functions/types need doc comments
- Complex algorithms or business logic
- TODO/FIXME for known issues (use `TODO:` prefix)
- Avoid obvious comments

**JSDoc/TSDoc:**
- Go doc comments for exported symbols
- Format: `// FunctionName does X`

## Function Design

**Size:** Functions should be focused and small (typically <100 lines)

**Parameters:**
- Use struct types for >3 parameters
- Context as first parameter for cancellable operations
- Options pattern for complex configurations

**Return Values:**
- Multiple return values for error handling
- Named return values for clarity in complex functions
- Zero values for errors

## Module Design

**Exports:**
- Export only what's needed by other packages
- Use unexported types for internal implementation
- Re-export from parent packages when needed

**Barrel Files:**
- `tools_reexport.go` re-exports tool constructors for backward compatibility
- Provides clean API surface for consumers

## Concurrency

**Pattern:**
- Use `sync.RWMutex` for read-heavy data
- Use `sync.Map` for concurrent maps
- Use channels for goroutine communication
- Use `sync.Once` for initialization

**Example:**
```go
type Dispatcher struct {
    mu                sync.RWMutex
    tools             map[string]types.Tool
    pendingResponses  sync.Map
    // ...
}

func (d *Dispatcher) List() []string {
    d.mu.RLock()
    defer d.mu.RUnlock()
    // ...
}
```

## Testing Conventions

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

**Parallel Testing:**
- Use `t.Parallel()` for independent tests
- Avoid for tests with shared mutable state
- Use `t.TempDir()` for isolated test directories

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

---

*Convention analysis: 2026-08-04*
