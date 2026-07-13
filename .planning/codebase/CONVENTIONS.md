# Coding Conventions

**Analysis Date:** 2026-07-13

## Naming Patterns

**Files:**
- Lowercase with underscores for multi-word files: `cache_refresh_test.go`, `permission_timeout_test.go`, `extra_test.go`
- Source and test files live in the same package directory (co-located)
- Platform-specific files use suffixes: `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`
- `doc.go` files provide package-level documentation in `pkg/` subdirectories

**Functions:**
- CamelCase for all functions: `checkPermission`, `extractCommandString`, `matchToolName`
- Exported functions use PascalCase: `New`, `Run`, `SetGit`, `ApproveBatch`
- Test functions use `Test` prefix with underscore-separated description: `TestBisect_Successful`, `TestBatchApproval_ConcurrentAccess`
- Helper functions in tests use descriptive names: `setupBisectRepo`, `runGit`, `writeFile`, `commitHash`
- Private/unexported helper functions use camelCase: `parseBisectLog`, `batchKey`, `riskLevelValue`

**Variables:**
- camelCase for local variables: `sessionStartHash`, `headHash`, `checkCalls`
- PascalCase for exported constants: `RiskSafe`, `PhaseInitialize`, `StatusPending`
- Package-level constants use PascalCase: `MaxHealAttempts`, `BashOutputLimit`, `DirPermission`
- Error sentinels prefixed with `Err`: `ErrPermissionDenied`, `ErrProviderUnreachable`, `ErrBisectResetFailed`

**Types:**
- PascalCase for exported types: `BisectResult`, `ToolCall`, `PermissionRequest`
- Interfaces use `-er` suffix or descriptive name: `GitRunner`, `SchemaProvider`, `Tool`
- Struct fields use PascalCase with JSON tags: `ID string \`json:"id"\``
- Config structs use `Config` suffix: `ProviderConfig`, `UIConfig`, `ToolsConfig`

## Code Style

**Formatting:**
- Tool: `gofmt` (mandatory) + `goimports` (recommended)
- All code must be `gofmt`-clean before commit
- Run `make fmt` to auto-format

**Linting:**
- Tool: `golangci-lint` with 5-minute timeout
- Enabled linters: `govet` (with shadow), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- Test files excluded from `errcheck` and `unused` checks
- Run `make lint` to verify

**Build Constraints:**
- `CGO_ENABLED=0` is a hard constraint — binary must be statically linked
- Use `-trimpath` for reproducible builds
- Cross-compile targets: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64

## Import Organization

**Order:**
1. Standard library packages
2. Third-party packages (charmbracelet, BurntSushi, etc.)
3. Project packages (`github.com/eshanized/M31A/...`)

**Style:**
- Group imports with blank lines between groups
- Use `goimports` to auto-organize
- Avoid unnecessary imports; keep import blocks clean

**Example from `cmd/m31a/main.go`:**
```go
import (
    "context"
    "flag"
    "fmt"
    "log/slog"
    "os"
    // ... stdlib

    tea "github.com/charmbracelet/bubbletea"
    // ... third-party

    "github.com/eshanized/M31A/internal/config"
    "github.com/eshanized/M31A/internal/git"
    // ... project
)
```

**Path Aliases:**
- Error package imported as `m31errors`: `m31errors "github.com/eshanized/M31A/internal/errors"`
- Bubble Tea imported as `tea`: `tea "github.com/charmbracelet/bubbletea"`

## Error Handling

**Patterns:**

1. **Return errors, never panic:**
```go
func New(workDir string, logger *slog.Logger) *Bisect {
    return &Bisect{workDir: workDir, logger: logger}
}
```

2. **Wrap errors with context using `fmt.Errorf("%w", err)`:**
```go
out, err := b.git.Run(args...)
if err != nil {
    return out, fmt.Errorf("git %s: %w", strings.Join(args, " "), err)
}
```

3. **Use sentinel errors from `internal/errors/errors.go`:**
```go
if !resp.Allowed {
    return m31errors.ErrPermissionDenied
}
```

4. **Check errors with `errors.Is` and `errors.As`:**
```go
switch {
case errors.Is(e, ErrProviderUnreachable):
    return "Provider unreachable — check your internet connection"
// ...
}
```

5. **Typed error structs for structured context:**
```go
type ToolError struct {
    Tool string
    Op   string
    Err  error
}

type ProviderError struct {
    Provider   string
    Model      string
    StatusCode int
    Err        error
}
```

6. **User-friendly error messages via `UserMessage()`:**
```go
func UserMessage(e error) string {
    // Maps sentinel errors and patterns to actionable messages
}
```

**Do NOT:**
- Use `panic()` in production code
- Ignore errors with `_ = err` (except in deferred cleanup)
- Use bare `return err` without wrapping context

## Comments

**When to Comment:**
- Package-level `doc.go` files for `pkg/` packages
- Exported functions/types must have doc comments
- Complex algorithms or non-obvious logic
- Workaround or technical debt markers (e.g., `// M-29: ...`)

**Doc Comment Style:**
```go
// Package bisect wraps git bisect to automatically identify the commit that
// introduced a regression. It runs a user-supplied check function across a
// commit range and returns the offending commit with its diff.
package bisect
```

```go
// Run performs a git bisect between sessionStartHash (good) and headHash (bad)
// using checkFn to determine pass/fail at each step.
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (result *BisectResult, err error) {
```

**No emojis** in code or documentation.

## Function Design

**Size:** Keep functions focused and under ~100 lines. Split complex logic into helpers.

**Parameters:** Use struct parameters for functions with many options:
```go
type ManagerOpts struct {
    CoordinatorTimeoutSecs int
}
sessionMgr := session.NewManager(globalConfigDir, workDir, session.ManagerOpts{...})
```

**Return Values:** Return `(result, error)` tuples. Use named return values for complex functions:
```go
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (result *BisectResult, err error) {
```

## Module Design

**Exports:** Export only what's needed. Use interfaces for testability:
```go
type GitRunner interface {
    Run(args ...string) (string, error)
}
```

**Barrel Files:** Not used. Each file is self-contained within its package.

**Dependency Rule:** `pkg/` must NOT import `internal/`. Enforced by Go module system.

## Conventional Commits

Format: `type(scope): description`

Types:
- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation
- `test:` — Tests
- `refactor:` — Code restructuring
- `chore:` — Maintenance

Examples:
```
feat: add bisect reset error sentinel (M-29)
fix: prevent permission response deadlock on concurrent requests
test: add race condition tests for DNS cache
```

## Special Patterns

**Constants:**
- Centralized in `internal/types/constants.go` for cross-package constants
- Package-local constants defined at file top
- Use `const` blocks with descriptive names:
```go
const (
    MaxHealAttempts = 2
    MaxPlanRetries  = 3
    BashTimeout     = 30 * time.Minute
)
```

**Configuration:**
- TOML-based config with nested structs
- All config fields have `toml:""` tags
- Defaults documented in comments, applied at load time
- Config hierarchy: embedded defaults < global < project < env vars

---

*Convention analysis: 2026-07-13*
