# Coding Conventions

**Analysis Date:** 2026-06-07

## Naming Patterns

**Files:**
- Lowercase with underscores for multi-word files: `app_update.go`, `bash_security_test.go`
- Test files follow Go convention: `<source>_test.go` (e.g., `bash_test.go`, `cache_test.go`)
- Platform-specific files use build tags or suffixes: `bash_unix.go`, `bash_windows.go`
- Sub-package separation: `*_model.go`, `*_view.go`, `*_tabs.go`, `*_keys.go` for TUI component decomposition

**Functions:**
- PascalCase for exported functions: `NewBash()`, `NewDispatcher()`, `DefaultConfig()`
- camelCase for unexported functions: `nextPermissionRequestID()`, `bodyReader()`
- Constructors follow `New<Type>()` pattern: `NewBash()`, `NewFileWrite()`, `NewRegistry()`
- Test functions: `Test<Type>_<Method>_<Scenario>` (e.g., `TestBash_SimpleCommand`, `TestModelCache_Get_StaleFallback`)

**Variables:**
- camelCase for local variables and fields: `workDir`, `permissionTimeout`
- PascalCase for exported fields: `RiskLevel`, `WorkflowPhase`
- Constants: PascalCase for exported, camelCase for unexported: `MaxFileSize`, `permissionRequestID`

**Types:**
- PascalCase for all exported types: `LLMProvider`, `ChatRequest`, `ToolResult`
- Interfaces: noun or `-er` suffix for single-method interfaces: `LLMProvider`, `SchemaProvider`, `PermissionGate`
- Struct tags: JSON snake_case with `json:"field_name"` and optional `omitempty`

## Code Style

**Formatting:**
- Tool: `gofmt` (enforced in CI via `gofmt -l .`)
- Additional: `goimports` (run via `make fmt`)
- No `.prettierrc` or `biome.json` — pure Go formatting tools

**Linting:**
- Tool: `golangci-lint` with timeout 5m
- Enabled linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled: `govet.check-shadowing: true`
- Test file exclusions: `errcheck` and `unused` warnings suppressed in `_test.go` files

**Key Rules:**
- `CGO_ENABLED=0` enforced for all builds (static binary)
- No CSS-style animations — Bubble Tea uses Unicode spinners + frame redraws
- All state mutations through Bubble Tea's `Update()` only — never mutate from goroutines

## Import Organization

**Order:**
1. Standard library (`context`, `fmt`, `os`, `time`)
2. External packages (`github.com/charmbracelet/bubbletea`, `github.com/BurntSushi/toml`)
3. Internal packages (`github.com/eshanized/M31A/internal/...`)

**Path Aliases:**
- Import alias for errors package: `m31errors "github.com/eshanized/M31A/internal/errors"`
- Import alias for types package: `m31types "github.com/eshanized/M31A/internal/types"`
- Bubble Tea: `tea "github.com/charmbracelet/bubbletea"`

**Example Pattern:**
```go
import (
    "context"
    "fmt"

    "github.com/BurntSushi/toml"
    tea "github.com/charmbracelet/bubbletea"

    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)
```

## Error Handling

**Patterns:**
- Sentinel errors defined in `internal/errors/errors.go` as `var Err* = errors.New("...")`
- Use `errors.Is()` for comparison — never use type assertions on sentinel errors
- Wrap errors with context: `fmt.Errorf("git init: %w", err)`
- User-friendly messages via `errors.UserMessage(e error) string` function
- Tool errors returned in both `error` return and `ToolResult.Error` string field

**Error Types:**
- Provider errors: `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`
- Tool errors: `ErrToolExecution`, `ErrPermissionDenied`, `ErrToolInputTooLarge`
- Session errors: `ErrSessionCorrupted`, `ErrSessionNotFound`
- Phase errors: `ErrPhaseTransition`, `ErrTaskFailed`

**Example Pattern:**
```go
func (t *Bash) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    command, ok := input.Params["command"].(string)
    if !ok {
        return types.ToolResult{}, fmt.Errorf("missing parameter: command: %w", m31errors.ErrToolExecution)
    }
    // ... implementation
}
```

## Logging

**Framework:** `log/slog` (structured logger)

**Patterns:**
- Logger initialized in `cmd/m31a/main.go` with file rotation
- Log level controlled via `M31A_LOG_LEVEL` env var (default: `info`)
- Log format controlled via `M31A_LOG_FORMAT` env var (default: `json`)
- Never log to stdout/stderr during TUI operation — only to `~/.m31a/m31a.log`
- Use structured fields: `logger.Info("starting", "version", Version, "os", runtime.GOOS)`

**Log Levels:**
- `logger.Info()` for normal operations
- `logger.Warn()` for recoverable issues (keychain unavailable, provider not registered)
- `logger.Error()` for fatal issues (cannot create config directory, TUI initialization failed)

## Comments

**When to Comment:**
- Exported functions and types must have doc comments
- Complex algorithms (topological sort, context pruning, reasoning normalization)
- Known violations or workarounds (reference ticket IDs: `// Fix C-4:`, `// BUG-04 fix:`)
- Thread safety concerns (goroutine boundaries, channel usage)

**JSDoc/TSDoc:**
- Not applicable — Go uses `//` comment style for documentation
- Godoc-style comments for exported types and functions

**Example Pattern:**
```go
// RunPhaseCmd returns a tea.Cmd that executes the given workflow phase in a
// goroutine and emits a PhaseResultMsg on completion. It also sets up a
// MsgEmitter on the engine so that TaskStartMsg and TaskUpdateMsg are emitted
// during execution.
func RunPhaseCmd(app *AppState, phase types.WorkflowPhase, goal string) tea.Cmd {
```

## Function Design

**Size:** No explicit limit, but functions tend to be focused (20-50 lines typical)

**Parameters:**
- Use struct types for complex inputs: `types.ToolInput`, `provider.ChatRequest`
- Use context as first parameter: `ctx context.Context`
- Options pattern for constructors: `openrouter.Options{BaseURL: ..., CacheTTL: ...}`

**Return Values:**
- Multiple return values for success/error: `(types.ToolResult, error)`
- Pointer returns for optional data: `*types.ModelInfo`
- Boolean flags for existence checks: `(types.ModelInfo, bool)` for cache lookups

## Module Design

**Exports:**
- Exported functions follow `New<Type>()` pattern for constructors
- Interfaces defined in dedicated `interface.go` files
- Types organized by domain: `internal/types/types.go` for shared types

**Barrel Files:**
- Not used — each package has focused files
- No `index.go` or re-export files

## Constants Organization

**Location:**
- Shared constants in `internal/types/constants.go`
- Tool-specific constants in `internal/tools/constants.go` (duplicated to avoid import cycles)
- Comment explains duplication: `// NOTE: Some constants below are intentionally duplicated`

**Naming:**
- PascalCase for exported constants: `MaxFileSize`, `BashTimeout`
- camelCase for unexported constants: `permissionRequestID`
- Group related constants together with comments

## JSON Struct Tags

**Pattern:**
```go
type ModelInfo struct {
    ID            string   `json:"id"`
    ContextLength int64    `json:"context_length"`
    Pricing       Pricing  `json:"pricing"`
    Capabilities  CapFlags `json:"capabilities"`
    Variant       *string  `json:"variant,omitempty"` // nil by default
}
```

**Rules:**
- Always use `json:"field_name"` with snake_case
- Use `omitempty` for optional fields
- Pointer types for truly optional fields: `*string`
- Consistent across all packages — no mixed casing

## Build Tags

**Usage:**
- Platform-specific implementations: `bash_unix.go`, `bash_windows.go`
- No explicit build tags in filenames — relies on Go's `_GOOS.go` convention
- CI builds for: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

---

*Convention analysis: 2026-06-07*
