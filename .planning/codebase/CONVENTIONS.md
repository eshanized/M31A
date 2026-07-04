# Coding Conventions

**Analysis Date:** 2026-07-04

## Naming Patterns

**Files:**
- snake_case.go: `engine.go`, `state_machine.go`, `dispatcher.go`, `base_client.go`
- Test files: `*_test.go` (same package, not external test package)
- Extra test files: `*_extra_test.go` for additional coverage tests
- Doc files: `doc.go` for package-level documentation

**Packages:**
- Lowercase, single-word: `workflow`, `tools`, `provider`, `taskrunner`, `bisect`, `rollback`
- Internal packages: `internal/types`, `internal/errors`, `internal/config`
- Public packages: `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`, `pkg/session`

**Functions:**
- PascalCase for exported: `NewEngine()`, `RunPhase()`, `Schedule()`, `Chain()`
- camelCase for unexported: `setupTestEngine()`, `parseQuestions()`, `buildDiscussContext()`
- Setup helpers: `setupXxx(t *testing.T)` pattern returning (thing, cleanup)
- Factory functions: `New()` prefix for constructors

**Variables:**
- camelCase: `callCount`, `multiResponses`, `planMarkdown`
- Constants: PascalCase for exported, camelCase for unexported
- Package-level errors: `Err` prefix (e.g., `ErrCircularDependency`, `ErrProviderUnreachable`)

**Types:**
- PascalCase: `Engine`, `WorkflowState`, `Runner`, `TaskResult`
- Interfaces: PascalCase, often single-method or small method sets
- Error types: `ToolError`, `ProviderError`, `ConfigError`

## Code Style

**Formatting:**
- Tool: `gofmt` (mandatory)
- Import tool: `goimports` (group stdlib / third-party / project)
- No tabs in source (Go standard uses tabs)

**Linting:**
- Tool: `golangci-lint` with 5m timeout
- Enabled linters: govet (with shadow), staticcheck, errcheck, ineffassign, unused
- Test files excluded from errcheck and unused

## Import Organization

**Order:**
1. Standard library (`context`, `fmt`, `os`, `testing`)
2. Third-party packages (`github.com/charmbracelet/bubbletea`, `github.com/godbus/dbus/v5`)
3. Project internal packages (`github.com/eshanized/M31A/internal/...`)

**Aliases:**
- `m31errors "github.com/eshanized/M31A/internal/errors"` — for error package
- `m31types "github.com/eshanized/M31A/internal/types"` — for types package
- `provider "github.com/eshanized/M31A/internal/provider"` — when ambiguous
- `ctxsrc "github.com/eshanized/M31A/internal/context"` — to avoid name collision

**Path Aliases:**
- None (standard Go module paths)

## Error Handling

**Patterns:**
- Return errors, never panic
- Wrap with `fmt.Errorf("context: %w", err)` for error chains
- Use sentinel errors for known conditions
- Implement `Unwrap() error` on custom error types

**Sentinel Errors:**
```go
// Define in internal/errors/errors.go
var ErrCircularDependency = errors.New("circular dependency in task graph")
var ErrProviderUnreachable = errors.New("provider unreachable")
```

**Error Types:**
```go
// Structured error with context
type ToolError struct {
    Tool string
    Op   string
    Err  error
}

func (e *ToolError) Error() string { ... }
func (e *ToolError) Unwrap() error { return e.Err }
```

**User-Facing Errors:**
- `UserMessage(e error) string` function in `internal/errors/errors.go`
- Maps internal errors to user-friendly messages
- Handles sentinel errors, typed errors, and pattern matching

## Logging

**Framework:** `log/slog` (Go standard library)

**Patterns:**
- Use `slog.Default()` or injected logger
- Structured logging with key-value pairs
- Log errors with context, not raw error values

## Comments

**When to Comment:**
- All exported functions and types need doc comments
- Complex algorithms or non-obvious logic
- Package-level documentation in `doc.go`

**JSDoc/TSDoc:** Not applicable (Go)

**Package Documentation:**
```go
// Package bisect wraps git bisect to automatically identify the commit that
// introduced a regression. It runs a user-supplied check function across a
// commit range and returns the offending commit with its diff.
package bisect
```

## Function Design

**Size:** Functions are generally focused and under 50 lines

**Parameters:**
- Use functional options for complex constructors
- Context as first parameter for cancellable operations
- Configuration structs for multiple options

**Return Values:**
- `(result, error)` pattern
- Named return values for documentation
- Empty slices over nil for collections

## Module Design

**Exports:**
- Minimal public API surface
- Exported types and functions only when needed by other packages
- Unexported helper functions within packages

**Barrel Files:** Not used (standard Go package imports)

## Key Gotchas

- **No emojis** in code or docs (per AGENTS.md)
- **No CGO** — `CGO_ENABLED=0` is a hard constraint
- **Bubble Tea is single-threaded** — use channels, never shared mutable state from goroutines
- **Provider model lists are dynamic** — never hardcode model names
- **API keys through OS keychain** — never written to disk in plaintext
- **`pkg/` must NOT import `internal/`** — enforced by Go module system

---

*Convention analysis: 2026-07-04*
