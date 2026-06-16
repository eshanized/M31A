# Coding Conventions

**Analysis Date:** 2026-06-16

## Naming Patterns

**Files:**
- Source files: `snake_case.go` — e.g., `base_client.go`, `plan_parser.go`, `atomic.go`
- Test files: `{source}_test.go` or `{source}_extra_test.go` for supplementary tests
- Screen models: `{feature}_model.go` / `{feature}_view.go` — e.g., `repl_model.go`
- Doc tests: `doc_test.go` (empty package-level coverage test)

**Functions:**
- Exported: `PascalCase` — e.g., `Load()`, `AtomicWrite()`, `ParsePlan()`
- Unexported: `camelCase` — e.g., `validateConfig()`, `mergeConfig()`, `substituteVars()`
- Test helpers: `t.Helper()`-prefixed, prefixed with `setup` or `new` — e.g., `setupRepo(t)`, `newTestManager(t)`

**Variables:**
- Exported constants: `PascalCase` — e.g., `MaxToolOutputChars`, `BashTimeout`
- Unexported constants: `camelCase` — e.g., `dirPermission`, `filePermission`
- Package-level vars: `camelCase` — e.g., `sharedTransport`, `knownKeysMap`
- Regex compiled: `re` prefix — e.g., `reHTTP401`, `reHTTP429`, `varRe`

**Types:**
- Structs: `PascalCase` — e.g., `BaseClient`, `Config`, `RollbackResult`
- Interfaces: `-er` suffix — e.g., `LLMProvider`, `Keychain`, `GitClient`
- Error sentinels: `Err` prefix — e.g., `ErrProviderUnreachable`, `ErrRateLimited`
- Type aliases: `PascalCase` with comment — e.g., `type CommitInfo = types.CommitInfo`

## Code Style

**Formatting:**
- Tool: `gofmt` (enforced in CI)
- Additional: `goimports` for import ordering
- Key settings: default gofmt formatting

**Linting:**
- Tool: `golangci-lint` v2 with `.golangci.yml`
- Enabled linters: `govet` (with shadow), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- Test files: `errcheck` and `unused` linters disabled
- Timeout: 5 minutes

## Import Organization

**Order:**
1. Standard library — `fmt`, `os`, `sync`, `testing`
2. Third-party — `github.com/BurntSushi/toml`, `github.com/fsnotify/fsnotify`
3. Internal — `github.com/eshanized/M31A/internal/*`, `github.com/eshanized/M31A/pkg/*`

**Path Aliases:**
- Error package aliased as `m31errors` — e.g., `m31errors "github.com/eshanized/M31A/internal/errors"`
- No other path aliases; use full import paths

**Import Style:**
- Alphabetically sorted within each group
- Blank line between groups

## Error Handling

**Patterns:**
- Sentinel errors: `errors.New()` in `internal/errors/errors.go` — 20+ sentinels
- Error wrapping: `fmt.Errorf("context: %w", err)` — always lowercase messages
- Error checking: `errors.Is()` for sentinel matching, not string comparison
- Error messages: lowercase, no punctuation — e.g., `"provider unreachable"`
- User messages: `UserMessage(error)` maps sentinels to actionable strings
- Multi-error collection: `var errs []error` then `errors.Join(errs...)` or custom join

**Example Pattern:**
```go
func Load(path string) (*Config, error) {
    if err := someOp(); err != nil {
        return nil, fmt.Errorf("decode config %s: %w", path, err)
    }
    return cfg, nil
}
```

## Logging

**Framework:** `log/slog` only — no `fmt.Println` or `log.Printf`

**Patterns:**
- Structured logging: `slog.Info("msg", "key", value)`
- Warn level for non-fatal: `slog.Warn("config watcher error", "error", err)`
- Debug level for development: `slog.Debug("cache hit", "model", id)`
- JSON format default, text via `M31A_LOG_FORMAT=text`
- Log level via `M31A_LOG_LEVEL` env var

## Comments

**When to Comment:**
- Package-level: brief purpose doc in `doc.go`
- Exported functions: doc comment explaining purpose
- Complex logic: inline comments explaining "why" not "what"
- Test sections: `// ── Section Name ──` separators
- Bug references: `// BUG-XX:` or `// M-XX:` prefixes

**JSDoc/TSDoc:**
- Go doc comments: `// FuncName does X.` format
- No JSDoc; this is Go code

## Function Design

**Size:** No explicit limit; functions are kept focused and readable

**Parameters:**
- Context as first parameter: `func Do(ctx context.Context, ...)`
- Options pattern not used; direct parameters preferred
- Callbacks as function parameters: `func(ctx context.Context, task types.Task) TaskResult`

**Return Values:**
- Error as last return value: `result, error`
- Pointer receivers for methods that modify state
- Value receivers for methods that only read state

## Module Design

**Exports:**
- Exported: public API, testable, documented
- Unexported: internal implementation, helpers
- Interface compliance: compile-time checks via `var _ Interface = (*Type)(nil)`

**Barrel Files:**
- Not used; packages export directly from their source files

## Concurrency Patterns

**Mutex:** `sync.RWMutex` for read-heavy maps (e.g., `ModelCache`)

**Once:** `sync.Once` for initialization — e.g., `knownKeysOnce`, `sharedTransportOnce`

**Channels:** Per-request channels via `sync.Map` for tool permission gating

**Semaphore:** Bounded goroutine pool via channel — e.g., task runner parallelism

## File I/O Patterns

**Atomic Writes:** `internal/fileutil/atomic.go` — temp file + `os.Rename()`

**Permissions:** `0644` for files, `0755` for directories

**Size Limits:** `readFileLimited()` enforces 50MB max for session files

**Skip Directories:** `node_modules`, `vendor`, `.next`, `dist`, `build`, `target`, `.venv`, `__pycache__`

## Configuration Patterns

**Config Loading Order:**
1. Defaults (`DefaultConfig()`)
2. Global TOML (`~/.m31a/config.toml`)
3. Environment overrides (`M31A_*`)
4. Project config (`m31a.toml`)
5. Variable substitution (`${VAR}`)
6. Validation

**Hot-Reload:**
- Primary: `fsnotify` file watcher
- Fallback: Polling every 5s
- Debouncing: 50ms delay

## Tool Interface Pattern

```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

## TUI Model Pattern

```go
type Model struct { /* state */ }
func NewXxxModel(...) Model { return Model{...} }
func (m Model) Init() tea.Cmd { return nil }
func (m Model) Update(msg tea.Msg) (Model, tea.Cmd) { ... }
func (m Model) View() string { ... }
```

---

*Convention analysis: 2026-06-16*
