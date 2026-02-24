# Conventions

> Code style, naming patterns, error handling, and architectural conventions.

## Go Style

- **Go 1.24+** — uses `slog` structured logging, `errors.Is` pattern matching
- **golangci-lint** enabled: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`
- Shadow checking enabled (`check-shadowing: true`)
- Test files excluded from `errcheck` and `unused` linting
- **No CGO** — `CGO_ENABLED=0` enforced for static binaries

## Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Packages | lowercase, single-word | `config`, `types`, `tools` |
| Types | PascalCase | `WorkflowPhase`, `ToolCall`, `StreamChunk` |
| Constants | PascalCase (exported), camelCase (unexported) | `MaxFileSize`, `bashTimeout` |
| Functions | PascalCase (exported), camelCase (unexported) | `NewDispatcher`, `userMessage` |
| Test functions | `Test<Type>_<Behavior>` | `TestBash_SimpleCommand` |
| Files | lowercase, underscore-separated | `bash_security_test.go` |

## Package Layout Rules

```
internal/  — private packages (not importable externally)
pkg/       — public packages (importable by external tools)
cmd/       — binary entry points only
```

**Dependency rules (strict):**
- `internal/types/` — zero internal imports (leaf package)
- `internal/errors/` — zero internal imports (leaf package)
- `internal/log/` — stdlib imports only
- `internal/config/` — may import `internal/types/`
- `internal/provider/` — may import `internal/types/`, `internal/errors/`
- `internal/tools/` — may import `internal/types/`, `internal/errors/`
- No circular dependencies

## Error Handling

### Sentinel Errors

Defined in `internal/errors/errors.go`. Always use `errors.Is()` for comparison:

```go
if errors.Is(err, errors.ErrRateLimited) {
    // handle rate limit
}
```

Never use type assertions on sentinel errors.

### Error Wrapping

Use `fmt.Errorf("context: %w", err)` for adding context. Unwrap with `errors.Is`/`errors.As`.

### User-Facing Messages

`errors.UserMessage(err)` returns actionable strings for TUI display. Falls back to generic message for unrecognized errors.

## Import Style

Standard library first, then external, then internal packages:

```go
import (
    "context"
    "encoding/json"

    "charmbracelet/bubbletea"

    "github.com/eshanized/M31A/internal/types"
)
```

## State Management

- **Bubble Tea single-threaded** — all state mutations via `Update()` only
- Never mutate state from goroutines — use `tea.Cmd` and `tea.Msg`
- Files split by concern: `*_model.go`, `*_view.go`, `*_tabs.go`, `*_state.go`, `*_keys.go`

## Concurrency

- `sync.RWMutex` for read-heavy shared state
- `sync.Map` for concurrent map access (tool permissions, pending responses)
- `sync.Once` for one-time initialization (stop logic)
- Channels for message passing between goroutines and main loop

## Configuration

- TOML config via `BurntSushi/toml`
- API key resolution: env var → OS keychain → config file (never plaintext)
- Atomic file writes: write to temp file, then rename
