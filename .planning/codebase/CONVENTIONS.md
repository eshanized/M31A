# CONVENTIONS.md — M31A Code Conventions

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Code Style

### Formatting
- All code must be `gofmt`-clean
- Use `goimports` for import ordering
- Import groups: stdlib / third-party / project (`github.com/eshanized/M31A/...`)

### Naming
- **Packages**: Short, lowercase, single-word (`tools`, `session`, `provider`)
- **Types**: PascalCase (`Dispatcher`, `PermissionRequest`, `ToolDefinition`)
- **Functions**: PascalCase for exported, camelCase for unexported (`newDispatcher`, `isRuleExpired`)
- **Constants**: PascalCase or camelCase (`ToolRateLimitBurst`, `DefaultTimeoutSecs`)
- **Interfaces**: `-er` suffix for single-method (`LLMProvider`), descriptive for multi-method
- **Test files**: `*_test.go` suffix, same package (white-box tests)

### Comments
- Exported functions/types need doc comments (godoc format)
- No unnecessary inline comments — code should be self-documenting
- Comments explain *why*, not *what*

## Error Handling

### Pattern: Return errors, never panic
```go
func DoSomething() error {
    if err := validate(); err != nil {
        return fmt.Errorf("validate: %w", err)
    }
    return nil
}
```

### Custom Error Types (`internal/core/errors/`)
- Use structured error codes (`m31errors.Error`)
- Wrap with `fmt.Errorf("%w", err)` for error chains
- Log errors with `slog.Error()` at call sites

### Error Returns
- Most functions return `error` as last value
- Use named returns only when necessary for deferred cleanup
- Never silently ignore errors (use `_` only with explicit comment)

## Patterns

### Dependency Injection
- Dependencies passed via constructor functions:
```go
func NewDispatcher(workDir string, cfg *config.PermissionsConfig) *Dispatcher
```
- Interfaces for testability (`LLMProvider`, `ToolDispatcher`)

### Permission System
- Tools classified by `RiskLevel`: Safe → Low → Medium → High → Dangerous → Destructive
- Permission rules: allow/deny/ask with pattern matching
- Batch approval for repeated calls (task-scoped)
- Persistent rules saved to `.m31a/permissions.json`

### Rate Limiting
- Token bucket algorithm for tool execution
- Separate buckets for normal vs. dangerous tools
- Concurrency semaphore (`MaxConcurrentTools = 8`)

### Session Management
- Project-local sessions in `<workDir>/.m31a/sessions/`
- Checkpoint system for crash recovery
- Planning state tracked separately

## File Organization

### Package Structure
- One concern per package (`keychain`, `rollback`, `bisect`)
- Large packages split into sub-files (`app_*.go`, `repl_*.go`)
- Platform-specific code: `*_linux.go`, `*_darwin.go`, `*_windows.go`

### Test Organization
- Unit tests in same package (white-box)
- Integration tests in `internal/tests/`
- E2E tests in `tests/e2e/` (compile and run binary)
- Test helpers in `internal/testutil/`

### Constants
- Package-level constants in `constants.go`
- Centralized type constants in `internal/core/types/constants.go`
- Tool-specific limits in `internal/tools/constants.go`

## Configuration

### TOML Config (`~/.m31a/config.toml`)
- Sections: `[provider]`, `[model]`, `[permissions]`, `[features]`, `[tools]`
- Environment variable overrides via `.env` files
- API keys resolved through OS keychain (never plaintext)

### Config Loading
- `config.Load(path)` — primary loader
- `config.Merge()` — layer project config over global
- `config.LoadDotEnv()` — load `.env` before logger init

## Logging

### Structured Logging (slog)
- Use `slog.Info()`, `slog.Error()`, `slog.Warn()` with key-value pairs
- Logger initialized in `main.go` with version context
- Default logger set via `slog.SetDefault()`

## Build

### Makefile Targets
- `make build` — optimized release binary
- `make test` — race detector + coverage
- `make lint` — golangci-lint (govet, staticcheck, errcheck, ineffassign, unused)
- `make check` — full CI pipeline (fmt → tidy → vet → lint → test)

### Linter Config (`.golangci.yml`)
- Enabled: govet (with shadow), staticcheck, errcheck, ineffassign, unused
- Test files excluded from errcheck and unused
- 5-minute timeout for lint runs
