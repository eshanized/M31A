---
focus: quality
last_mapped: 2026-05-30
---

# M31A — Coding Conventions

## Code Style

- **Go 1.22+ idioms**: uses `for range` with int, `math/rand/v2` where applicable
- **Error handling**: returns errors with `fmt.Errorf("context: %w", err)` wrapping. No `errors.Wrap`.
- **Sentinel errors**: defined in `internal/errors/errors.go`. Compared with `errors.Is()`. Never type-asserted.
- **No panics**: except `NewEngine()` panics on prompt load failure (fatal at startup). Tool registration uses panic on duplicate.
- **Context propagation**: all blocking operations accept `context.Context` — HTTP calls, SSH streaming, tool execution.
- **Goroutine safety**: all shared state protected by `sync.RWMutex`. Never mutate `AppState` from goroutines.
- **Package layering**: `internal/types/` is the leaf package — zero internal imports. `internal/errors/` same.

## Naming

| Element | Convention | Example |
|---------|-----------|---------|
| Package names | lowercase, single word | `tui`, `provider`, `session` |
| Exported types | PascalCase | `AppState`, `ModelInfo`, `ToolResult` |
| Unexported fields | camelCase | `streaming`, `activeProvider` |
| Constants | PascalCase | `MaxFileSize`, `ErrInvalidKey` |
| Methods | PascalCase | `Init()`, `Update()`, `View()` |
| File names | snake_case | `bash_unix.go`, `modelselector.go` |
| Test files | `_test.go` suffix | `app_test.go`, `client_test.go` |
| Prompt files | snake_case.md | `base.md`, `tool-use.md` |

## Architectural Patterns

### Bubble Tea MVU

All TUI models follow the standard Bubble Tea pattern:

```go
type MyModel struct { /* state fields */ }

func (m MyModel) Init() tea.Cmd { /* initial commands */ }
func (m MyModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) { /* state mutations */ }
func (m MyModel) View() string { /* rendering */ }
```

### Message Types for Cross-Component Communication

Typed messages (structs implementing `tea.Msg`) are used for all cross-component events:

- `PhaseResultMsg{Phase, Tasks, Messages, Success, Error}`
- `StreamChunkMsg{Chunk}`
- `HealthCheckTickMsg`, `RefreshCacheMsg`
- `FallbackEventMsg{From, To, Reason}`
- `PermissionRequestMsg{Request}`, `PermissionResponseMsg{Response}`

### Command Pattern (Slash Commands)

All slash commands are pure functions returning `CommandResult`:

```go
type CommandHandler func(args []string, ctx CommandContext) CommandResult
```

Registered in `CommandRegistry` at startup. Never mutate `AppState` directly.

### Dispatcher Pattern (Tool System)

Tools implement `types.Tool` interface (`Name()`, `Description()`, `RiskLevel()`, `Execute()`).

`tools.Dispatcher` routes `ToolCall` → registered tool, applies permission gate for dangerous/destructive tools, returns `ToolResult`.

## Error Handling

```go
// Always wrap with context
if err != nil {
    return fmt.Errorf("read config file: %w", err)
}

// Sentinel error comparison
if errors.Is(err, m31errors.ErrRateLimited) {
    // trigger fallback
}

// Never expose raw stack traces to TUI — use styled messages
m.currentOperation = fmt.Sprintf("Error: %v", msg.Err)
```

## Logging

- Only structured `slog` calls — no `log.Printf` except in openrouter/zen client fallback paths
- Log levels: `debug`, `info`, `warn`, `error`
- Never write to stdout/stderr during TUI operation
- Key=value pairs for structured context: `logger.Info("session started", "id", id, "provider", provider)`

## Concurrency Safety

- Bubble Tea `Update()` is single-threaded — all state mutations happen here
- Goroutines emit `tea.Cmd` returning `tea.Msg`, never mutate state directly
- Stream readers use channels + select for non-blocking send
- `sync.RWMutex` for read-heavy concurrent access (model cache, dispatcher tools)

## File I/O

- All writes are atomic: write to `.m31a_tmp_<random>` temp file in same directory, then `os.Rename`
- Backup original files before overwrite to `~/.m31a/sessions/<id>/backups/`
- Binary detection: mime sniff on first 512 bytes (FileRead)
- Path safety: resolve symlinks, reject paths outside cwd

## Testing Patterns

- Stdlib `testing` only — no testify, no Ginkgo
- Mock HTTP servers via `httptest.NewServer` for provider tests
- Temp directories via `t.TempDir()` for filesystem tests
- Table-driven tests with anonymous structs
- Subtests with `t.Run()` for grouped test cases
