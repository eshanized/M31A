# Coding Conventions

**Analysis Date:** 2026-07-10

## Language and Style

**Primary Language:** Go 1.25+

**Formatting:**
- `gofmt` (enforced via `make fmt`)
- `goimports` for import grouping and ordering
- All code must pass `gofmt` before commit

**Linting:**
- `golangci-lint` with 5-minute timeout (`make lint`)
- Enabled linters: `govet` (with shadow detection), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- Test files excluded from `errcheck` and `unused` checks (see `.golangci.yml`)

**No emojis** in code, comments, or documentation. Use ASCII-only text.

## Naming Patterns

**Files:**
- Source files: `snake_case.go` (e.g., `keychain_linux.go`, `bash_security_test.go`)
- Platform-specific: `<name>_<os>.go` (e.g., `keychain_darwin.go`, `keychain_windows.go`, `bash_windows.go`)
- Test files: `<name>_test.go` (standard Go convention)
- Extra test files: `<name>_extra_test.go` for edge-case coverage (e.g., `manager_extra_test.go`, `compaction_extra_test.go`)
- Benchmark files: `<name>_bench_test.go` or inline `BenchmarkXxx` functions in `<name>_test.go`
- Doc files: `doc.go` in each `pkg/` package with a single package doc comment

**Directories:**
- Source packages: `snake_case` (e.g., `fileutil/`, `codeintel/`, `taskrunner/`)
- All lowercase, no hyphens

**Functions:**
- Exported: `PascalCase` (e.g., `New`, `Run`, `Schedule`, `ShouldCompact`)
- Unexported: `camelCase` (e.g., `createCommits`, `stashIfDirty`, `countCommitsBetween`)
- Test functions: `TestStructName_MethodOrBehavior` (e.g., `TestSoftReset_WithUncommitted`, `TestCompactor_ShouldCompact`)
- Subtest names: lowercase descriptive strings (e.g., `"clean"`, `"dirty"`, `"modified"`)

**Types:**
- Structs: `PascalCase` (e.g., `BisectResult`, `Compactor`, `LedgerEntry`)
- Interfaces: `PascalCase` (e.g., `GitRunner`, `Keychain`)
- Constants: `PascalCase` (e.g., `PhaseIdle`, `RiskSafe`, `StatusDone`)
- String enums: Use typed constants with `string` type (e.g., `type WorkflowPhase string`)

**Variables:**
- Exported vars: `PascalCase` (e.g., `ErrInvalidKey`, `SkipDirs`)
- Package-level vars: `camelCase` (e.g., `skipDirsCache`, `loadTestDotEnvOnce`)
- Local vars: `camelCase` (e.g., `cfg`, `logger`, `dispatcher`)

## Import Organization

**Order (enforced via `goimports`):**
1. Standard library
2. Third-party packages
3. Project packages (`github.com/eshanized/M31A/...`)

**Example from `cmd/m31a/main.go`:**
```go
import (
    "context"
    "flag"
    "fmt"
    // ... stdlib

    tea "github.com/charmbracelet/bubbletea"
    // ... third-party

    "github.com/eshanized/M31A/internal/config"
    "github.com/eshanized/M31A/internal/git"
    // ... project
)
```

**Path Aliases:**
- Use full import paths; no aliases except for `tea` (`github.com/charmbracelet/bubbletea` → `tea`)
- Error package aliased as `m31errors` when imported alongside standard `errors`: `m31errors "github.com/eshanized/M31A/internal/errors"`

## Error Handling

**Strategy:** Return errors, never panic. Use sentinel errors and typed error structs.

**Sentinel errors** (defined in `internal/errors/errors.go`):
```go
var (
    ErrProviderUnreachable = errors.New("provider unreachable")
    ErrRateLimited         = errors.New("rate limited")
    ErrInvalidKey          = errors.New("invalid API key")
    // ... ~30+ sentinels total
)
```

**Typed error structs** (in `internal/errors/errors.go`):
- `ToolError` — wraps tool execution errors with `Tool`, `Op`, `Err` fields
- `ProviderError` — wraps API errors with `Provider`, `Model`, `StatusCode`, `Err` fields
- `ConfigError` — wraps config errors with `Key`, `Err` fields
- All implement `Error() string` and `Unwrap() error` for `errors.Is`/`errors.As` compatibility

**Error wrapping pattern:**
```go
// Always use fmt.Errorf with %w for wrapping
return fmt.Errorf("bisect start: %w", startErr)

// Multi-error wrapping for context + sentinel
err = fmt.Errorf("bisect reset: %w: %w", m31errors.ErrBisectResetFailed, resetErr)
```

**User-facing errors** (via `errors.UserMessage(e error) string`):
- Maps sentinels and typed errors to actionable user messages
- Pattern-matched fallbacks for HTTP status codes, connection errors, etc.
- Returns generic "An unexpected error occurred" for unrecognized errors

**Error handling in deferred calls:**
```go
// Use //nolint:errcheck for deferred Close() calls where error is non-actionable
defer stream.Close()   //nolint:errcheck
defer f.Close()        //nolint:errcheck
defer resp.Body.Close() //nolint:errcheck
```

## Documentation

**Package docs:** Every `pkg/` package has a `doc.go` with a single package comment:
```go
// Package rollback provides a commit chain browser for M31A sessions. It
// lists session commits, supports soft and hard resets to any point in the
// chain, and creates backup branches before destructive operations.
package rollback
```

**Exported functions/types:** Must have doc comments:
```go
// Run performs a git bisect between sessionStartHash (good) and headHash (bad)
// using checkFn to determine pass/fail at each step.
func (b *Bisect) Run(...) (*BisectResult, error) {
```

**Internal comments:** Use `//` for inline explanations, reference feature IDs:
```go
// H-24: Hard fallback — force exit after 5 seconds if TUI doesn't quit.
// WP-C03: fail fast if Getwd fails
// F-011, F-012: Model capability detection from config
```

**Comment style:**
- Single-line `//` comments for brief notes
- Block `/* */` not used
- Comments start with capital letter
- No trailing periods on short comments

## Module and Package Design

**Dependency rule:** `pkg/` must NOT import `internal/`. Enforced by Go module system.

**Package structure:**
- `cmd/m31a/` — entry point only (flag parsing, wiring)
- `internal/` — private implementation packages
- `pkg/` — reusable public packages (no `internal/` imports)

**Constructor pattern:**
```go
// New creates a Compactor with the given config and token estimator.
func New(cfg Config, tokenEst *tokens.Estimator) *Compactor {
    return &Compactor{
        cfg:      cfg,
        tokenEst: tokenEst,
    }
}
```

**Interface design:** Use small, focused interfaces (e.g., `GitRunner` with just `Run(args ...string) (string, error)`) to enable test doubles without pulling in full implementations.

## Concurrency

**Bubble Tea TUI:** All state mutations go through `Update()` only. Never mutate `AppState` from a goroutine. Use `tea.Cmd` / `tea.Msg` for async work.

**Shared state:** Use `sync.RWMutex` for read-heavy data (e.g., `Ledger.mu`, `Compactor.mu`). Use `atomic.Bool` for simple flags (e.g., `programExited`).

**Signal handling:** Send messages through channels (`tea.QuitMsg{}`) instead of calling functions directly from goroutines.

## Configuration

**Config loading order** (later overrides earlier):
1. Defaults (`DefaultConfig()`)
2. TOML file (`~/.m31a/config.toml` or `$M31A_CONFIG`)
3. Environment variables (`M31A_THEME`, `M31A_DEFAULT_MODEL`, etc.)

**Env var convention:** `M31A_<SECTION>_<KEY>` (e.g., `M31A_LOG_FORMAT`, `M31A_CONFIG`)

**API keys:** Stored in OS keychain via `pkg/keychain/` — never written to disk in plaintext. `.env` files are gitignored except `.env.example`.

## Conventional Commits

**Format:** `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`

## Constants

**Pattern:** All constants centralized in `internal/types/constants.go` with descriptive names:
```go
const (
    BashTimeout         = 30 * time.Minute
    BashOutputLimit     = 50_000
    DefaultContextLength = 128_000
)
```

**File permissions:** Use `types.DirPermission` (0755) and `types.FilePermission` (0644) constants.

---

*Convention analysis: 2026-07-10*
