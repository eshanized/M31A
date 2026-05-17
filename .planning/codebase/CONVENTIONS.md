# Coding Conventions

**Analysis Date:** 2026-06-14

## Naming Patterns

**Files:**
- Lowercase with underscores for multi-word names: `loader.go`, `bash_test.go`, `permission_timeout_test.go`
- Platform-specific suffixes: `bash_unix.go`, `bash_windows.go`
- `_extra_test.go` suffix for supplemental tests added after initial implementation: `extra_test.go`
- Test files always end `_test.go` and live in the same package as the source (white-box testing)

**Functions:**
- Exported: MixedCaps (`NewBash`, `DefaultDispatcher`, `LoadPrompts`)
- Unexported: camelCase (`setupTestEngine`, `rotateLogFiles`, `applyVarSubstitution`)
- Constructors: `New<StructName>()` pattern — `NewBash(workDir)`, `NewEdit(workDir, backupDir)`, `NewDispatcher(cfg)`
- Test helpers: `test<Thing>()` — `testTheme()`, `testAppState()`
- Setup helpers: `setupTest<Thing>()` — `setupTestEngine(t)`

**Variables:**
- Package-level constants: ALL_CAPS for domain constants (`MaxHealAttempts`, `BashOutputLimit`) or PascalCase for non-const values
- Local variables: camelCase (`timeoutSec`, `cfgCopy`, `logDir`)
- Receiver names: single letter matching type initial: `t *Bash`, `m *mockProvider`, `lw *limitWriter`
- Error variables: `Err<Name>` prefix (`ErrProviderUnreachable`, `ErrValidation`, `ErrBisectResetFailed`)
- Sentinel errors defined in `internal/errors/errors.go`

**Types:**
- Structs: PascalCase (`Bash`, `Dispatcher`, `ToolResult`)
- Interfaces: PascalCase, often without `-er` suffix for non-single-method interfaces (`LLMProvider`, `PermissionGate`)
- Enums (string types): PascalCase type with `const` block using PascalCase values (`RiskLevel` → `RiskSafe`, `RiskDangerous`)
- Config structs: `<Name>Config` suffix (`ProviderConfig`, `UIConfig`, `FeaturesConfig`)
- TOML tags use snake_case: `api_key`, `default_mode`, `context_warning_threshold`

## Code Style

**Formatting:**
- Tool: `gofmt` (enforced by `make fmt`)
- Additional: `goimports` for import sorting (run after `gofmt`)
- All code must be `gofmt`-clean before committing

**Linting:**
- Tool: `golangci-lint` with config at `.golangci.yml`
- Enabled linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled: `check-shadowing: true`
- Test files exempt from `errcheck` and `unused` linters
- Timeout: 5 minutes

**Key style rules from `.golangci.yml` (`issues.exclude-rules`):**
- `_test.go` files skip `errcheck` and `unused` linters

## Import Organization

**Order (3 groups separated by blank lines):**
1. Standard library (`context`, `fmt`, `log/slog`, `os`, etc.)
2. Third-party packages (`github.com/charmbracelet/*`, `github.com/BurntSushi/toml`, etc.)
3. Project packages (`github.com/eshanized/M31A/internal/...`, `github.com/eshanized/M31A/pkg/...`)

**Path Aliases:**
- `m31errors "github.com/eshanized/M31A/internal/errors"` — alias for error package to avoid collision with stdlib `errors`
- `m31types "github.com/eshanized/M31A/internal/types"` — alias when `types` collides with stdlib
- `tea "github.com/charmbracelet/bubbletea"` — third-party alias

**Pattern:** Always use full module path `github.com/eshanized/M31A/internal/...` or `github.com/eshanized/M31A/pkg/...`. No relative imports.

## Error Handling

**Core rules (from `CONTRIBUTING.md`):**
- Return errors, never panic
- Wrap errors with `fmt.Errorf("context: %w", err)`
- Use sentinel errors from `internal/errors/errors.go` for known failure modes

**Sentinel error pattern** (`internal/errors/errors.go`):
```go
var ErrToolExecution = errors.New("tool execution failed")
```

**Wrapping pattern:**
```go
return types.ToolResult{}, fmt.Errorf("missing parameter: command: %w", m31errors.ErrToolExecution)
```

**User-facing messages** (`internal/errors/errors.go:UserMessage()`):
- Every sentinel error has a user-friendly message in `UserMessage()`
- Switch on `errors.Is()` for sentinels, then fall back to string pattern matching
- Use `UserMessage(err)` to convert technical errors to actionable messages

**Error collection pattern** (config validation):
```go
var errs []error
// ... append validation errors ...
if len(errs) > 0 {
    return fmt.Errorf("config validation:\n%s", joinedErrors)
}
```

**Tool result errors:**
- Tools return `(types.ToolResult, error)` — the error is for caller-level failures
- `ToolResult.Error` field holds tool-level error strings (non-fatal to the workflow)
- Example: `bash.go` returns timeout as `ToolResult{Error: "timeout after 30s"}` not as `error`

## Logging

**Framework:** `log/slog` (stdlib structured logging)

**Logger setup** (`internal/log/log.go`):
- JSON format by default, text via `M31A_LOG_FORMAT=text`
- Log level via `M31A_LOG_LEVEL` env var (debug/info/warn/error, default: info)
- Logs written to `~/.m31a/m31a.log` with daily rotation, 7-day retention
- Singleton via `sync.Once`: `defaultLogger`
- Initialized in `cmd/m31a/main.go` before any goroutines start

**Logging patterns:**
```go
slog.Info("M31A starting", "version", Version, "go_version", runtime.Version())
slog.Warn("keychain initialization failed", "error", kcErr)
logger.Error("cannot determine working directory", "error", err)
```

**Structured fields convention:**
- Always include contextual key-value pairs: `"error", err`, `"path", path`, `"key", key`
- Never use `fmt.Sprintf` in log messages — use structured fields instead
- Log before early returns and error paths

## Configuration

**Layered loading** (`internal/config/loader.go:Load()`):
1. `DefaultConfig()` — zero-valued defaults
2. Global TOML (`~/.m31a/config.toml`)
3. Environment variable overrides (`M31A_*`)
4. Project-level `m31a.toml` (walked up from cwd, max 3 levels)
5. Variable substitution (`${VAR}` → env value)
6. Validation

**Config type conventions:**
- All config structs use TOML tags: `toml:"field_name"`
- Nested structs for sections: `ProviderConfig`, `ModelConfig`, `UIConfig`
- Validation errors collected in `[]error` then joined
- Unknown TOML keys trigger a warning log (typo detection)

**Saving config:**
- Always use `fileutil.AtomicWrite()` for persistence (temp file + rename)
- API keys excluded from project config files — stored in keychain or env vars
- Deep-copy slices/maps before writing to prevent data races with `WatchConfig`

## Comments

**When to Comment:**
- All exported functions, types, and packages MUST have doc comments (Go convention)
- Inline comments explain *why*, not *what*
- Reference bug fix IDs: `// BUG-18:`, `// G-4 fix:`, `// H-12:`, `// WP-H05`
- Reference design rules: `// DEP-3:`, `// V1.1:`, `// WP-C03`

**Doc comment patterns:**
```go
// Load reads a TOML config file from the given path, applies multi-layer
// merging (global TOML → env vars → project m31a.toml), validation, and
// variable substitution, then returns the resulting Config.
func Load(path string) (*Config, error) {
```

**Code organization markers:**
```go
// ═══ countDone ═══
// ═══ plan_model.go ═══
```

## Function Design

**Size:** Functions tend to be 20-80 lines. Long functions are broken into clearly named helpers.

**Parameters:** Use struct types for more than 2-3 params. Tool constructors take explicit dependencies:
```go
func NewBash(workDir string) *Bash
func NewEdit(workDir, backupDir string) *Edit
func NewDispatcher(cfg *config.PermissionsConfig) *Dispatcher
```

**Return Values:**
- `(Result, error)` tuple for fallible operations
- `(value)` single return for pure functions
- Tool `Execute` always returns `(types.ToolResult, error)` — ToolResult carries partial results even on tool-level errors

## Module Design

**Package layout:**
- `cmd/` — Binary entry points only
- `internal/` — Private packages (config, errors, fileutil, git, log, provider, tokens, tools, tui, types, workflow)
- `pkg/` — Public/reusable packages (arbitrage, autodream, bisect, keychain, ledger, rollback, session, taskrunner)

**Exports:**
- Export only what's needed by other packages
- Keep implementation details unexported
- Use interfaces for decoupling: `provider.LLMProvider`, `types.Tool`, `tools.PermissionGate`

**Barrel files:** Not used. Each file exports types/functions directly.

**Dependency rules:**
- `internal/` packages cannot import `pkg/` (enforced by convention, not build tags)
- `pkg/` packages are standalone and testable independently
- No circular imports between internal packages

---

*Convention analysis: 2026-06-14*
