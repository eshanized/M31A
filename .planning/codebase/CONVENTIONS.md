# Coding Conventions — M31A

> Mapped: 2026-07-09

## Language & Build

- **Go 1.25** — all code must compile with `go 1.25.0`
- **No CGO** — `CGO_ENABLED=0` hard constraint. Static binary only.
- **gofmt** — all code must be `gofmt`-clean (enforced in CI)
- **goimports** — import groups: stdlib / third-party / project

## Code Style

### Imports

```
import (
    "fmt"
    "os"

    "github.com/charmbracelet/bubbletea"
    "github.com/example/lib"

    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/pkg/session"
)
```

Three groups: stdlib, third-party, project. Separated by blank lines.

### Error Handling

- Return errors, never panic
- Wrap errors with `fmt.Errorf("%w", err)` for proper unwrapping
- Use sentinel errors from `internal/errors/` where appropriate
- `errcheck` linter enforces error checking (excluded for test files)

### Naming

- Exported functions/types need doc comments (enforced by `golint` / style checks)
- Unexported functions do NOT require doc comments
- Acronyms are all-caps: `SSE`, `HTTP`, `URL`, `API`, `TTL`, `TUI`, `DBus`
- Test files: `<name>_test.go`, test helpers `<name>_test_helper.go`
- Test functions: `Test<Package>_<Function>` pattern (e.g., `TestApp_Update`)

### Documentation

- Doc comments on all exported symbols
- No emojis in code or docs
- Markdown documentation in `docs/` directory
- AGENTS.md for build/test/lint conventions

## Project Patterns

### Error Pattern: Sentinel Errors

Sentinel errors are defined in `internal/errors/` and used with `errors.Is()` / `errors.As()`. Example:

```go
var ErrNotFound = errors.New("not found")
```

### Configuration Pattern

Config is loaded from TOML (`~/.m31a/config.toml`) with:
- `internal/config/types.go` — Config struct definition
- `internal/config/loader.go` — TOML loading
- `internal/config/merge.go` — Default overlay
- `internal/config/project_context.go` — Auto-detection

### Provider Pattern

Each provider implements the `Provider` interface from `internal/provider/interface.go`. Common base logic in `base_client.go`. Providers:
- Register via `internal/provider/registry.go`
- Cache model lists via `internal/provider/cache.go` (TTL-based)
- Support SSE streaming via `internal/provider/sse.go`

### Tool Pattern

Each tool implements `Tool` interface from `internal/tools/interface.go`:
- Registered in `internal/tools/defaults.go`
- Dispatched via `internal/tools/dispatcher.go`
- Permission-gated via `internal/tools/permissions.go`
- Rate-limited via `internal/tools/concurrency.go`

### Workflow Pattern

The seven-phase engine follows a state machine pattern:
- `engine.go` — Orchestrator
- `state_machine.go` — State transitions
- `phase_coordinator.go` — Phase lifecycle
- Each phase has its own file/package with `Execute` or `Run` methods

### Test Pattern

- Standard Go `testing` package (no third-party test frameworks)
- Test files adjacent to implementation files
- Table-driven tests where applicable
- Test helpers in `testutil/` and `_test.go` files
- E2E tests in root `e2e_test.go` (compiles binary, runs with args)
- Race detector enabled in CI

## Commit Conventions

Conventional commits:
- `feat:` for features
- `fix:` for bug fixes
- `docs:` for documentation
- `test:` for tests
- `refactor:` for refactoring
- `chore:` for maintenance tasks

## Linter Config

From `.golangci.yml` — version 2 config:
- **govet** with shadow detection enabled
- **staticcheck** — comprehensive static analysis
- **errcheck** — error return checking (disabled for test files)
- **ineffassign** — ineffectual assignments
- **unused** — unused code detection (disabled for test files)

## Security Conventions

- API keys go through OS keychain (`pkg/keychain/`) — never plaintext on disk
- `.env` files are gitignored except `.env.example`
- All file paths are sanitized for traversal attacks
- WebFetch blocks private/loopback/link-local IPs
- WebSearch uses DNS cache (5min TTL) for rebinding protection
- Bash commands gated by modal permission system
- Subagent nesting limited to depth 2

## Key Architectural Rules

- `pkg/` must NOT import `internal/` (Go module convention)
- Bubble Tea is strictly single-threaded — use channels, never shared mutable state from goroutines
- Provider model lists are dynamic — never hardcode model names
- All state mutations go through `tea.Update()` only
