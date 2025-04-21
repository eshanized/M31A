# Contributing to M31A

## Development Setup

### Requirements

- Go 1.22+
- golangci-lint (optional, for linting)

### Clone and Build

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
go mod tidy
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

### Run Tests

```bash
go test -race -cover ./...
```

Coverage targets: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`.

### Lint

```bash
golangci-lint run ./...
go vet ./...
```

## Architecture Rules

M31A enforces strict package dependency rules. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full dependency graph. Key rules:

- **Bubble Tea is single-threaded.** All state mutations go through `Update()` only. Never mutate `AppState` from a goroutine. Use `tea.Cmd` and `tea.Msg`.
- **No CGO.** Binary must be static (`CGO_ENABLED=0`).
- **No telemetry.** No analytics. No external calls except OpenRouter/Zen APIs.
- **No hardcoded model lists.** Models discovered dynamically from provider APIs.
- **V1 tools:** Bash, FileRead, FileWrite, Glob, Grep only.
- **V1 task execution is SEQUENTIAL.** No concurrency.

## Adding a New Tool

1. Create `internal/tools/mytool.go` implementing the `types.Tool` interface:
   - `Name() string`
   - `Description() string`
   - `RiskLevel() types.RiskLevel`
   - `Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)`
2. Register it in `DefaultDispatcher()` in `internal/tools/dispatcher.go`
3. Write tests in `internal/tools/mytool_test.go`
4. Update this doc and `docs/SLASH_COMMANDS.md` if relevant

## Adding a New TUI Screen

1. Add a `Screen*` constant in `internal/tui/types.go`
2. Create a model struct with `View()`, `Update()`, and optionally `Init()` methods
3. Add the model field to `AppState` in `internal/tui/app.go`
4. Initialize the model in `NewApp()`
5. Add a case in `Update()` and `View()` switch statements
6. Write tests

## Adding a New Workflow Phase

1. Create `internal/workflow/myphase.go` with `runMyPhase()` method on `Engine`
2. Add the phase to the switch in `Engine.RunPhase()`
3. Add the phase constant to `internal/types/enums.go`
4. Update `handlePhase` in `internal/tui/commands.go` if needed
5. Write tests in `internal/workflow/engine_test.go`

## Pull Request Conventions

- **Commit messages:** Follow conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`)
- **Tests:** All PRs must pass `go test -race ./...` with no regressions
- **Coverage:** New code should meet the 75% threshold (90% for critical packages)
- **No breaking changes** without discussion in an issue first
- **One concern per PR** — keep changes focused and reviewable

## Key Files

| File | Purpose |
|------|---------|
| `cmd/m31a/main.go` | Binary entry point, flag parsing, TUI launch |
| `internal/tui/app.go` | Bubble Tea app, state management, message routing |
| `internal/tui/commands.go` | Slash command handlers |
| `internal/workflow/engine.go` | Six-phase workflow orchestration |
| `internal/provider/` | LLM provider interface + OpenRouter/Zen clients |
| `internal/tools/` | Core tool implementations |
| `pkg/session/` | Session lifecycle and file persistence |
