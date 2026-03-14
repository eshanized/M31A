# Coding Conventions

**Analysis Date:** 2026-06-12

## Naming Patterns

**Files:**
- Lowercase with underscores: `engine_test.go`, `bash_security_test.go`, `context_warning_test.go`
- Platform-specific suffixes: `_unix.go`, `_windows.go`, `_darwin.go` (e.g., `bash_unix.go`, `bash_windows.go`)
- Test files: `*_test.go` (standard Go convention)
- Doc files: `doc.go` in each `pkg/` subdirectory for package documentation

**Functions:**
- MixedCaps (exported): `NewBash()`, `NewSession()`, `LoadPrompts()`
- camelCase (unexported): `setupTestEngine()`, `gitConfig()`, `modelForPhase()`
- Constructor pattern: `New*()` or `NewXxx()` (e.g., `NewBash()`, `NewSession()`, `New()`)
- Setup helpers in tests: `setupTestEngine()`, `setupBisectRepo()`, `setupRepo()`

**Variables:**
- MixedCaps for exported: `CurrentSchemaVersion`, `ErrProviderUnreachable`
- camelCase for unexported: `sessionID`, `workDir`, `backupDir`
- Constants: MixedCaps for exported (`RiskSafe`, `PhaseIdle`), camelCase for unexported (`fileActionCreate`)

**Types:**
- MixedCaps for exported types: `Engine`, `Session`, `Bash`, `Rollback`
- Interface naming: Single-method interfaces named after method (`Name()`, `Description()`), larger interfaces descriptive (`LLMProvider`, `Tool`)
- Struct field tags: `json:"field_name"` for JSON, `toml:"field_name"` for TOML

**Packages:**
- Lowercase single words: `session`, `tools`, `workflow`, `provider`
- Internal packages: `internal/` prefix for private code
- Public packages: `pkg/` prefix for reusable code

## Code Style

**Formatting:**
- Tool: `gofmt` (standard Go formatter)
- Run `gofmt -w .` before committing
- Run `goimports` for import sorting

**Linting:**
- Tool: `golangci-lint` with configuration in `.golangci.yml`
- Enabled linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled: `check-shadowing: true`
- Test files excluded from: `errcheck`, `unused` linters

**Line Length:**
- No explicit limit, but prefer readability
- Long lines broken with proper Go formatting

## Import Organization

**Order:**
1. Standard library (`context`, `fmt`, `os`, `time`)
2. Third-party packages (`github.com/charmbracelet/bubbletea`, `github.com/eshanized/M31A/...`)
3. Project internal packages (`internal/...`, `pkg/...`)

**Separation:**
- Blank line between groups
- Use `goimports` for automatic sorting

**Path Aliases:**
- Common aliases for internal packages:
  - `m31types "github.com/eshanized/M31A/internal/types"`
  - `m31errors "github.com/eshanized/M31A/internal/errors"`
  - `tea "github.com/charmbracelet/bubbletea"`

**Example:**
```go
import (
    "context"
    "fmt"
    "os"

    "github.com/charmbracelet/bubbletea"
    m31errors "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
)
```

## Error Handling

**Patterns:**
- Return errors rather than panicking
- Use `fmt.Errorf` with `%w` for wrapping: `fmt.Errorf("chain: %w", err)`
- Sentinel errors defined in `internal/errors/errors.go`:
  - `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, etc.
  - Use `errors.Is()` for checking
- User-friendly messages via `errors.UserMessage()` function

**Error Types:**
- Sentinel errors: `var ErrProviderUnreachable = errors.New("provider unreachable")`
- Wrapped errors: `fmt.Errorf("load prompt %s: %w", path, err)`
- Pattern matching: `strings.Contains(errStr, "connection refused")`

**Example:**
```go
if err != nil {
    return nil, fmt.Errorf("load prompt %s: %w", path, err)
}
```

## Comments

**When to Comment:**
- Exported functions, types, and packages must have doc comments
- Inline comments explain *why*, not *what*
- Complex logic blocks get explanatory comments
- TODO/FIXME for known issues

**Doc Comment Style:**
```go
// Session wraps types.Session with additional runtime state fields.
type Session struct {
    // ...
}

// NewSession creates a new Session with default values.
func NewSession(id, model, provider string) *Session {
    // ...
}
```

**Package Documentation:**
- Each `pkg/` package has a `doc.go` file
- Example: `pkg/session/doc.go`
```go
// Package session manages the lifecycle of agent sessions, including
// creation, persistence, checkpointing, and archival. Sessions are stored
// as JSON and Markdown files under ~/.m31a/sessions/.
package session
```

## Function Design

**Size:** Functions are focused and reasonably sized (typically <100 lines)

**Parameters:**
- Use structs for complex inputs: `ManagerOpts{}`, `ChatRequest{}`
- Context as first parameter for cancellation support
- Return multiple values: `(result, error)` pattern

**Return Values:**
- Always return error as last value
- Use named return values for clarity in complex functions
- Zero values for error cases

**Example:**
```go
func (r *Rollback) Chain(limit int) ([]RollbackEntry, error) {
    if limit <= 0 {
        limit = 20
    }
    // ...
    return entries, nil
}
```

## Module Design

**Exports:**
- Export only what's needed for external use
- Use interfaces for abstraction: `LLMProvider`, `Tool`
- Keep implementation details unexported

**Barrel Files:**
- Not used (Go convention)
- Each file exports specific types/functions

**Package Boundaries:**
- `internal/` packages: Private to the module
- `pkg/` packages: Reusable across projects
- Strict dependency rules enforced (see `docs/ARCHITECTURE.md`)

## Configuration

**File Format:** TOML (`config.toml`)
- Config types defined in `internal/config/types.go`
- Use `toml:"field_name"` struct tags
- Default values via `DefaultConfig()` functions

**Environment Variables:**
- Load via `config.LoadDotEnv()` before logger initialization
- Prefix: `M31A_` for project-specific vars
- Sensitive values: API keys resolved via keychain → env → config file

## Concurrency

**Bubble Tea Model:**
- Single-threaded event loop
- All state mutations through `Update()` only
- Never mutate `AppState` from goroutines
- Use `tea.Cmd` and `tea.Msg` for async operations

**Goroutines:**
- Use `sync.Once` for one-time initialization
- Use channels for communication
- Context cancellation for cleanup
- `sync.Mutex` for shared state protection

**Example:**
```go
var killOnce sync.Once
go func() {
    select {
    case <-ctx.Done():
        killOnce.Do(func() {
            processKill(cmd.Process.Pid, sigInt)
        })
    case <-cmdDone:
        return
    }
}()
```

## Documentation

**Architecture:**
- `docs/ARCHITECTURE.md` — Package dependency graph, data flow
- `CONTRIBUTING.md` — Development setup, code style, PR conventions
- `AGENTS.md` — Architecture rules, absolute prohibitions

**Code Documentation:**
- Godoc comments on all exported symbols
- Inline comments for complex logic
- README files in key directories

---

*Convention analysis: 2026-06-12*
