# Coding Conventions

**Analysis Date:** 2026-08-03

## Naming Patterns

**Files:**
- Snake_case for all Go files: `dispatcher.go`, `bash_test.go`, `edit_benchmark_test.go`
- Test files use `_test.go` suffix: `permissions_test.go`, `webfetch_security_test.go`
- Benchmark files use `_benchmark_test.go` suffix: `edit_benchmark_test.go`
- Helper files use `_helpers.go` or `_helpers_test.go` suffix: `helpers.go`, `test_helpers.go`

**Functions:**
- PascalCase for exported functions: `NewDispatcher()`, `DefaultDispatcher()`, `BuildToolDefs()`
- camelCase for unexported functions: `testDispatcher()`, `grepToolInput()`, `extractFromParams()`
- Test functions use `Test` prefix: `TestDispatcher_RegisterAndExecute()`, `TestBash_WorkdirValidation()`
- Benchmark functions use `Benchmark` prefix: `BenchmarkCascadingReplace()`
- Subtests use descriptive names: `t.Run("Permission request times out", func(t *testing.T) {...})`

**Variables:**
- PascalCase for exported variables: `ToolRateLimitBurst`, `MaxConcurrentTools`
- camelCase for unexported variables: `skipDirsCache`, `permissionRequestID`
- Constants use PascalCase: `RiskSafe`, `PhaseInitialize`, `MaxBackupsPerFile`
- Boolean variables use `is`, `has`, or `should` prefix: `hasProvider`, `isChild`

**Types:**
- PascalCase for all types: `Dispatcher`, `PermissionRequest`, `ToolResult`
- Interface suffix not used: `Tool` (not `ToolInterface`)
- Struct field names use PascalCase: `ToolName`, `RiskLevel`, `RequestID`

## Code Style

**Formatting:**
- Tool: `gofmt` + `goimports`
- Key settings: Standard Go formatting
- Run `make fmt` to format all files

**Linting:**
- Tool: `golangci-lint` (version 2)
- Config: `.golangci.yml`
- Enabled linters: govet (with shadow), staticcheck, errcheck, ineffassign, unused
- Test files excluded from errcheck and unused

**Import Organization:**
1. Standard library
2. Third-party packages
3. Project packages (github.com/eshanized/M31A/...)

Example:
```go
import (
    "context"
    "fmt"
    "testing"

    "github.com/bmatcuk/doublestar/v4"
    "github.com/eshanized/M31A/internal/core/config"
    m31errors "github.com/eshanized/M31A/internal/core/errors"
    "github.com/eshanized/M31A/internal/core/types"
)
```

**Path Aliases:**
- `m31errors` for `github.com/eshanized/M31A/internal/core/errors`
- No other aliases used

## Error Handling

**Patterns:**
- Return errors, never panic: `return nil, fmt.Errorf("%w", err)`
- Use sentinel errors: `var ErrPermissionDenied = errors.New("permission denied")`
- Wrap with context: `fmt.Errorf("tool %s: %w", toolName, err)`
- Check errors immediately: `if err != nil { return err }`
- Use `errors.Is()` for sentinel comparison: `errors.Is(err, ErrPermissionDenied)`
- Use `errors.As()` for type assertion: `errors.As(err, &toolErr)`

**Error Types:**
- `ToolError`: Wraps tool execution errors with tool name and operation
- `ProviderError`: Wraps API provider errors with HTTP status code
- `ConfigError`: Wraps configuration errors with key context

**User Messages:**
- Use `errors.UserMessage(err)` for user-facing error messages
- Provides actionable guidance: "Provider unreachable — check your internet connection"

## Comments

**When to Comment:**
- Exported functions/types need doc comments
- Complex algorithms need explanation
- Security-critical code needs comments
- TODO/FIXME for known issues

**JSDoc/TSDoc:**
- Not applicable (Go project)

**Comment Style:**
```go
// FunctionName does X and returns Y.
// It handles edge cases Z and W.
func FunctionName() {
```

## Function Design

**Size:** Functions are generally short (50-200 lines). Complex functions are split into smaller helpers.

**Parameters:** 
- Use struct for complex parameters: `types.ToolInput{Params: map[string]any{...}}`
- Use context.Context as first parameter
- Use options pattern for optional configuration

**Return Values:**
- Return `(result, error)` tuple
- Return zero values on error
- Use named return values sparingly

## Module Design

**Exports:**
- Export only what's needed
- Use unexported functions for internal logic
- Provide constructors: `NewDispatcher()`, `NewBash()`

**Barrel Files:**
- Not used (Go doesn't have barrel files)

## Key Patterns

**Interface Compliance:**
- Compile-time checks: `var _ types.Tool = (*Edit)(nil)`
- Ensures structs implement required interfaces

**Concurrency:**
- Use `sync.RWMutex` for shared state: `Dispatcher.mu`
- Use `sync.Map` for concurrent maps: `Dispatcher.pendingResponses`
- Use channels for communication: `Dispatcher.requestCh`
- Use atomic operations: `Dispatcher.pendingPermCount`
- Never mutate shared state from goroutines without synchronization

**Testing Helpers:**
- `t.Helper()` for test helper functions
- `t.TempDir()` for temporary directories
- `t.Cleanup()` for cleanup functions
- Table-driven tests for parameterized cases

**Constants:**
- Define in `internal/core/types/constants.go` for cross-cutting constants
- Define in package-specific `constants.go` for local constants
- Use descriptive names: `ToolRateLimitBurst`, `MaxConcurrentTools`

## Documentation

**AGENTS.md:**
- Contains quick commands and architecture overview
- Reference for AI agents working on the codebase

**TESTING.md:**
- Testing guide with commands and patterns
- Reference for writing tests

---

*Convention analysis: 2026-08-03*
