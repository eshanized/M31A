# CONVENTIONS.md — Code Style & Conventions

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## Go Code Style

M31A follows standard Go conventions (`gofmt`, `go vet`, `golangci-lint`):

### Linter Configuration (`.golangci.yml`)
```yaml
linters:
  enable:
    - govet       # Reports suspicious constructs
    - staticcheck # Advanced static analysis
    - errcheck    # Ensures errors are checked
    - ineffassign # Detects ineffectual assignments
    - unused      # Reports unused code
    - gosimple    # Suggests simplifications
```

- `govet` has `check-shadowing: true`
- `errcheck` skips type assertions and blank identifier checks
- Test files excluded from `errcheck` and `unused`

### Formatting
- Standard `go fmt` enforced via `make fmt` target
- `goimports` used for import ordering
- `CGO_ENABLED=0` for all builds
- Build flags: `-s -w` (strip debug info) for release builds

## Naming Conventions

### Packages
- All lowercase, single-word names preferred
- `internal/`, `pkg/` distinction: internal for private, pkg for potentially reusable
- Platform-specific suffix: `_unix.go`, `_windows.go`, `_darwin.go`, `_linux.go`
- Test files: `*_test.go` alongside source, `extra_test.go` for integration-style tests

### Types & Interfaces
- PascalCase for exported types, camelCase for unexported
- Interface names: `LLMProvider`, `PermissionGate`, `MsgEmitter`
- Error sentinels: `Err*` pattern (e.g., `ErrProviderUnreachable`, `ErrRateLimited`)
- Constants: PascalCase for exported, camelCase for unexported

### Functions & Methods
- PascalCase exported, camelCase unexported
- Constructors: `New*()` pattern (e.g., `NewRegistry()`, `NewSession()`)
- Getter methods: omit `Get` prefix (e.g., `Name()` not `GetName()`)

## Error Handling Patterns

### Sentinel Errors (`internal/errors/errors.go`)
```go
var ErrProviderUnreachable = errors.New("provider unreachable")
var ErrRateLimited = errors.New("rate limited")
```
- All sentinel errors defined in `internal/errors/errors.go`
- User-friendly messages via `UserMessage(err) string` — converts internal errors to actionable messages
- Pattern matching for unwrapped errors (HTTP status codes, connection errors, TLS errors)

### Error Wrapping
```go
return fmt.Errorf("provider %q not registered: %w", name, m31errors.ErrProviderNotFound)
```
- Uses `%w` for wrapping with `errors.Is` compatibility
- Context added to errors before returning them

### Validation Pattern
```go
func validateSessionID(id string, expectedLen int) error {
    if expectedLen <= 0 { expectedLen = types.SessionIDLength }
    if len(id) != expectedLen {
        return fmt.Errorf("session ID must be %d chars, got %d", expectedLen, len(id))
    }
    for _, c := range id {
        if !unicode.IsDigit(c) && !(c >= 'a' && c <= 'f') {
            return fmt.Errorf("session ID must contain only lowercase hex chars [a-f0-9], got %q", id)
        }
    }
    return nil
}
```

## Logging

- Uses `log/slog` (structured logging, Go 1.21+)
- Logger initialized once in `main.go`, set as default via `slog.SetDefault()`
- Contextual attributes always included (e.g., `"version"`, `"error"`, `"model"`)
- Log levels: `Info` for startup, `Warn` for recoverable issues, `Error` for failures
- Keychain errors logged as warnings (non-fatal)
- Log file managed by `internal/log/log.go`

## Concurrency Patterns

### Thread Safety
- `sync.RWMutex` for read-heavy structures (provider registry, config)
- `sync.Mutex` for write-heavy operations
- `sync.Once` for lazy initialization (`SkipDirsMap()`)
- `sync/atomic` for counters (`permissionRequestID`)
- `singleflight` (`golang.org/x/sync`) for deduplicating concurrent model fetches

### Channel Usage
- Bubble Tea's message passing via `tea.Cmd` channels
- `ChannelSendTimeout = 500ms` to prevent goroutine leaks on channel sends
- Signal handling via dedicated goroutine with `sigDone` channel for clean shutdown

## Configuration Defaults Pattern

- `DefaultConfig()` in `internal/config/loader.go:28` returns a Config with all sane defaults
- Missing config file causes Load to return DefaultConfig without error
- Hot-reload supported via `fsnotify` file watcher
- Config values overridable via environment variable `M31A_CONFIG`

## Security Conventions

### SSRF Protection (`internal/errors/errors.go:35`)
```go
ErrPrivateIPBlocked = errors.New("access to private IP is blocked (SSRF protection)")
```
WebFetch tool blocks requests to private IP ranges, loopback, and link-local addresses.

### API Key Handling
- Keys stored in OS keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager)
- Config file `api_key` fields also supported as fallback
- Keys resolved at startup via `cfg.ResolveAPIKeys(kc)` in `main.go:116`
- Provider errors sanitized (max 200 chars) to avoid leaking keys in logs

### File Size Limits
```go
MaxFileSize = 5 * 1024 * 1024        // 5MB file read limit
MaxSessionFileSize = 50 * 1024 * 1024 // 50MB session file limit
MaxLLMResponseBytes = 1 << 20         // 1MB LLM response limit
```

### Permission System (`internal/config/types.go:163-183`)
- `PermissionsConfig` with rules: tool, pattern, risk_level, action (allow/deny/ask)
- Per-agent permission profiles via `PermissionsAgentConfig`
- Default modes: allow-all, ask-first, deny-all
- Timeout-based permission modal (default 300 seconds)

## Code Organization Conventions

### Import Aliasing
- External packages with long names: aliased for readability
- Internal packages with naming conflicts: prefixed (e.g., `m31errors`, `m31types`)
- Standard library imports grouped first, then external, then internal

### Compile-time Interface Checks
```go
var _ provider.LLMProvider = (*Client)(nil)
```
Used in provider implementations to ensure interface compliance at compile time.

### Documentation Comments
- Exported types and functions have doc comments
- Inline comments explain "why" not "what"
- Phase transition validation documented in `engine_messages.go`
- Constants have immediate comments explaining their purpose
