# Contributing to M31 Autonomous

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

## Code Style

- **Formatting**: All code must be `gofmt`-clean. Run `gofmt -w .` before committing.
- **Imports**: Group standard library, third-party, and project imports with blank lines between groups. Use `goimports` for sorting.
- **Naming**: Follow Go naming conventions. Package names are lowercase single words. Exported identifiers use MixedCaps.
- **Error handling**: Return errors rather than panicking. Use `fmt.Errorf` with `%w` for wrapping.
- **Comments**: Exported functions, types, and packages must have doc comments. Inline comments should explain *why*, not *what*.
- **No emojis** in code or documentation.

## Architecture Rules

For the full package layout and architectural constraints, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Key highlights:

- **Three providers** — OpenRouter, Zen, and Nvidia. No direct Anthropic or OpenAI.
- **18 built-in tools** — Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch, WebSearch, CodeMap, CodeComplexity, FileDelete, FileMove, FileList, TodoWrite, TodoRead, DevServer, HTTPCheck, AskUserQuestion.
- API keys resolved in order: environment variable → OS keychain → config file. Never stored in plaintext.
- No telemetry, analytics, or phone-home behavior.
- **Bubble Tea is single-threaded.** All state mutations go through `Update()` only. Never mutate `AppState` from a goroutine. Use `tea.Cmd` and `tea.Msg`.
- **No CGO.** Binary must be static (`CGO_ENABLED=0`).
- **No hardcoded model lists.** Models discovered dynamically from provider APIs.
- **Bounded parallelism.** Task runner uses Kahn's algorithm with 4 concurrent tasks.

## Package Layout

```
cmd/m31a/          Entry point (flag parsing, config, provider registration, TUI launch)
internal/          Private packages (not importable)
  codeintel/       4-language parser, import graph, relevance scoring
  config/          TOML loader, hot-reload, project context detection
  context/         Dynamic context system, registry, estimation
  decision/        Decision logging and receipts
  errors/          Sentinel errors with user-friendly messages
  fileutil/        Atomic file operations
  git/             Git operations (commit, rollback, diff, stash, branch)
  log/             Structured logging with daily rotation
  provider/        LLM provider abstraction (OpenRouter, Zen, Nvidia)
  tokens/          Token estimation (tiktoken + EMA calibration)
  tools/           18 tools + dispatcher + permissions + subagents
  tui/             Bubble Tea TUI (33 screens)
  types/           Shared types, constants, workflow phases
  workflow/        Seven-phase orchestration engine
pkg/               Public packages (importable)
  autodream/       Context consolidation with reentrancy guard
  arbitrage/       Model-cost optimizer with task classification
  bisect/          Git-bisect wrapper for model comparison
  compaction/      Context compaction utilities
  coordinator/     Drain management for workflow→TUI communication
  history/         Frecent prompt history with scoring
  keychain/        OS keychain abstraction (Linux/macOS/Windows)
  ledger/          Cross-session learning store (markdown-backed)
  metrics/         Metrics collection and reporting
  retry/           Retry logic with exponential backoff
  rollback/        Commit-chain manager (soft/hard/safe reset)
  session/         Session lifecycle, persistence, checkpointing
  skills/          Skill discovery and management
  taskrunner/      Kahn's algorithm, bounded parallelism
```

## Adding a New Tool

1. Create `internal/tools/mytool.go` implementing the `types.Tool` interface:
   - `Name() string`
   - `Description() string`
   - `RiskLevel() types.RiskLevel`
   - `Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)`
2. Register it in `DefaultDispatcher()` in `internal/tools/defaults.go`
3. Write tests in `internal/tools/mytool_test.go`
4. Update `docs/TOOLS.md` if relevant

## Adding a New TUI Screen

1. Add a `Screen*` constant in `internal/tui/tuitypes/tuitypes.go`
2. Create a model struct with `View()`, `Update()`, and optionally `Init()` methods
3. Add the model field to `AppState` in `internal/tui/app.go`
4. Initialize the model in `NewApp()`
5. Add a case in `Update()` and `View()` switch statements
6. Write tests

## Adding a New Workflow Phase

1. Add the phase constant to `internal/types/types.go`
2. Add to `validPhaseTransitions` map in `internal/workflow/engine.go`
3. Create `internal/workflow/myphase.go` with `runMyPhase()` method on `Engine`
4. Add to `RunPhase()` dispatch in `internal/workflow/engine.go`
5. Add phase model slot to `modelForPhase()`
6. Add TUI screen in `internal/tui/tuitypes/tuitypes.go`
7. Write tests in `internal/workflow/engine_test.go`

## Pull Request Conventions

- **Commit messages:** Follow conventional commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`)
- **Tests:** All PRs must pass `go test -race ./...` with no regressions
- **Coverage:** New code should meet the 75% threshold (90% for critical packages)
- **No breaking changes** without discussion in an issue first
- **One concern per PR** — keep changes focused and reviewable

## Key Files

| File | Purpose |
|------|---------|
| `cmd/m31a/main.go` | Binary entry point, flag parsing, TUI launch |
| `internal/tui/app.go` | Bubble Tea app, state management, message routing |
| `internal/tui/commands/` | Slash command registry and handlers |
| `internal/workflow/engine.go` | Seven-phase workflow orchestration |
| `internal/provider/` | LLM provider interface + OpenRouter/Zen/Nvidia clients |
| `internal/tools/` | Core tool implementations and dispatcher |
| `pkg/session/` | Session lifecycle and file persistence |
