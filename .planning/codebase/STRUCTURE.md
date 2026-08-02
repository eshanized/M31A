# Codebase Structure

**Analysis Date:** 2026-08-03

## Directory Layout

```
M31A/
├── cmd/m31a/              # CLI entry point (main.go, usage.go)
├── internal/
│   ├── core/              # Shared types, config, errors (leaf layer)
│   │   ├── config/        # TOML config loading, validation, merging, hot-reload
│   │   ├── errors/        # Sentinel errors, structured error types, user messages
│   │   └── types/         # Shared vocabulary: Message, ToolCall, Task, WorkflowPhase, etc.
│   ├── engine/            # Business logic layer
│   │   ├── workflow/      # 7-phase workflow engine (100+ files)
│   │   ├── session/       # Session persistence, checkpoint save/restore
│   │   ├── tokens/        # Token estimation (tiktoken-go)
│   │   ├── bisect/        # Git bisect automation
│   │   ├── rollback/      # Git rollback operations
│   │   ├── taskrunner/    # Task execution and dependency resolution
│   │   ├── compaction/    # Context window auto-compaction
│   │   ├── decision/      # Decision logging subsystem
│   │   └── narrative/     # Narrative engine for workflow storytelling
│   ├── tools/             # 18 built-in LLM tools
│   │   ├── fileops/       # FileRead, FileWrite, Edit, FileList, FileDelete, FileMove
│   │   ├── exec/          # Bash, DevServer, output bounding
│   │   ├── search/        # WebFetch, WebSearch, Glob, Grep
│   │   ├── ai/            # AskUserQuestion tool
│   │   ├── git/           # Git tool wrapper
│   │   ├── network/       # HTTPCheck tool
│   │   ├── codeanalysis/  # CodeMap, CodeComplexity tools
│   │   ├── todo/          # TodoWrite, TodoRead tools
│   │   └── subagent/      # Parallel subagent management, worktrees, profiles
│   ├── integrations/      # External service adapters
│   │   ├── provider/      # LLM provider interface + 3 implementations
│   │   │   ├── openrouter/ # OpenRouter provider
│   │   │   ├── zen/       # Zen provider
│   │   │   └── nvidia/    # NVIDIA provider
│   │   ├── git/           # Git operations wrapper
│   │   ├── keychain/      # OS keychain (macOS/Linux/Windows)
│   │   ├── ledger/        # Session record persistence (LEDGER.md)
│   │   ├── log/           # Structured logging setup
│   │   ├── metrics/       # Session metrics collector
│   │   ├── codeintel/     # Codebase intelligence indexer
│   │   ├── context/       # Dynamic system context sources
│   │   ├── autodream/     # Auto-consolidation of long conversations
│   │   ├── arbitrage/     # Model cost arbitrage scoring
│   │   ├── shell/         # Shell integration
│   │   └── skills/        # Skill management
│   ├── infrastructure/    # Cross-cutting utilities
│   │   ├── fileutil/      # File operation helpers
│   │   └── retry/         # Retry logic
│   ├── ui/tui/            # Terminal UI (Bubble Tea)
│   │   ├── commands/      # Command implementations
│   │   ├── components/    # Reusable UI components (PermissionModal, QuestionModel)
│   │   ├── layout/        # Responsive layout system
│   │   ├── streaming/     # LLM streaming display
│   │   ├── theme/         # Theme management (dark mode)
│   │   └── tuitypes/      # TUI-specific types
│   ├── testutil/          # Test utilities (CI helpers, timeout helpers)
│   └── tests/             # Integration tests, mocks
├── tests/
│   ├── e2e/               # End-to-end tests (compiles and runs binary)
│   └── testutil/          # Test utilities (mocks, env helpers)
├── pkg/                   # Public packages (currently empty)
├── docs/                  # Documentation
├── scripts/               # Build/release scripts
├── .github/               # GitHub workflows
├── dist/                  # Cross-compiled binaries (gitignored)
├── Makefile               # Build, test, lint, cross-compile targets
├── go.mod                 # Go module definition (go 1.25.12)
├── go.sum                 # Dependency checksums
├── .golangci.yml          # Linter configuration
├── .goreleaser.yaml       # Release automation config
└── m31a.json              # Default project configuration
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: CLI entry point — flag parsing, config load, provider registration, TUI/headless launch
- Contains: `main.go` (588 lines), `usage.go`, tests
- Key files: `main.go:run()` is the canonical entry point

**`internal/core/`:**
- Purpose: Shared vocabulary and configuration — the leaf dependency layer
- Contains: Type definitions, config loading, error sentinels
- Key files: `types/types.go` (368 lines — all shared types), `config/loader.go`, `errors/errors.go`

**`internal/engine/workflow/`:**
- Purpose: Seven-phase workflow engine — the heart of M31A
- Contains: Phase implementations, state machine, prompt building, context building, streaming
- Key files: `engine.go` (1600+ lines), `state_machine.go`, `execute.go`, `plan.go`, `discuss.go`, `verify.go`, `ship.go`

**`internal/tools/`:**
- Purpose: 18 built-in tools with permission enforcement, rate limiting, and concurrency control
- Contains: Tool implementations, dispatcher, permission system
- Key files: `dispatcher.go` (535 lines), `defaults.go` (registration), `permissions.go`, `interface.go`

**`internal/integrations/provider/`:**
- Purpose: LLM provider abstraction with 3 implementations
- Contains: Interface definition, registry, per-provider clients, model caching, fallback
- Key files: `interface.go` (24 lines), `registry.go`, `base_client.go`, `cache.go`

**`internal/ui/tui/`:**
- Purpose: Terminal UI using Bubble Tea (Elm architecture)
- Contains: Root AppState model, per-screen models, handlers, views, streaming, themes
- Key files: `app_state.go` (482 lines), `app.go` (787 lines), `repl.go`, `sidebar_model.go`

**`internal/engine/session/`:**
- Purpose: Session persistence — save/load sessions, checkpoints, tasks, projects
- Contains: Manager, session files, checkpoint logic, planning helpers
- Key files: `manager.go`, `session.go`, `checkpoint.go`, `planning.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: CLI entry point (flag parsing, config, provider init, TUI launch)
- `internal/ui/tui/app.go:Init()`: Bubble Tea initialization (health ticker, permission listener)
- `internal/tools/defaults.go:DefaultDispatcher()`: Tool registration entry point

**Configuration:**
- `internal/core/config/loader.go`: TOML config loading and validation
- `internal/core/config/types.go`: Config struct definitions
- `internal/core/config/merge.go`: Config merging logic
- `internal/core/config/project_context.go`: Project context detection
- `internal/core/config/instructions.go`: System instructions loading
- `.golangci.yml`: Linter configuration

**Core Logic:**
- `internal/engine/workflow/engine.go`: Workflow engine orchestration
- `internal/engine/workflow/state_machine.go`: Phase transition validation
- `internal/engine/workflow/execute.go`: Execute phase implementation
- `internal/engine/workflow/plan.go`: Plan phase implementation
- `internal/engine/workflow/discuss.go`: Discuss phase implementation
- `internal/tools/dispatcher.go`: Tool execution orchestration
- `internal/core/types/types.go`: All shared type definitions

**Testing:**
- `tests/e2e/e2e_test.go`: End-to-end tests (compiles binary, runs prompts)
- `internal/ui/tui/*_test.go`: TUI unit tests (co-located with source)
- `internal/engine/workflow/*_test.go`: Workflow engine tests
- `tests/testutil/mocks/`: Mock implementations for provider, tool, dispatcher
- `tests/testutil/envtest.go`: Environment test helpers

## Naming Conventions

**Files:**
- Go files: `snake_case.go` (e.g., `app_state.go`, `state_machine.go`)
- Test files: `*_test.go` (co-located with source)
- Extra test files: `*_extra_test.go` (additional test coverage)
- Platform-specific: `*_linux.go`, `*_darwin.go`, `*_windows.go`

**Directories:**
- Lowercase, single words or concatenated words (e.g., `fileops`, `codeanalysis`, `subagent`)
- Test directories: `tests/`, `internal/tests/`

**Types:**
- Exported types: `PascalCase` (e.g., `AppState`, `WorkflowPhase`, `ToolCall`)
- Interfaces: `PascalCase` (e.g., `LLMProvider`, `Tool`, `SchemaProvider`)
- Constants: `PascalCase` for exported, `camelCase` for unexported

**Functions:**
- Exported: `PascalCase` (e.g., `NewEngine`, `DefaultDispatcher`)
- Unexported: `camelCase` (e.g., `runPhase`, `ensurePermission`)
- Methods: `PascalCase` for exported, `camelCase` for unexported

## Where to Add New Code

**New Workflow Phase:**
1. Add phase constant to `internal/core/types/types.go` (WorkflowPhase enum)
2. Add transition rules to `internal/engine/workflow/state_machine.go`
3. Implement phase in `internal/engine/workflow/` (new file `phase_name.go`)
4. Add case to `engine.go:RunPhase()` switch
5. Add TUI model in `internal/ui/tui/phase_name_model.go`

**New Tool:**
1. Create implementation in `internal/tools/` (new directory or file)
2. Implement `types.Tool` interface (`Name()`, `Description()`, `RiskLevel()`, `Execute()`)
3. Optionally implement `types.SchemaProvider` for parameter schemas
4. Register in `internal/tools/defaults.go:DefaultDispatcher()`
5. Add tests in `internal/tools/` (co-located `_test.go`)

**New LLM Provider:**
1. Create directory `internal/integrations/provider/newprovider/`
2. Implement `provider.LLMProvider` interface
3. Add registration in `cmd/m31a/main.go` (config-based)
4. Add config fields in `internal/core/config/types.go`

**New TUI Screen:**
1. Create model in `internal/ui/tui/newscreen_model.go`
2. Add screen constant to `internal/ui/tui/constants.go` (Screen enum)
3. Add to `AppState` fields in `internal/ui/tui/app_state.go`
4. Register in `internal/ui/tui/app_routing.go` and `app_screens.go`
5. Add view method in `internal/ui/tui/newscreen_view.go`

**New Integration:**
1. Create directory `internal/integrations/newintegration/`
2. Implement adapter interface
3. Wire into `cmd/m31a/main.go` or relevant engine component

**Utilities:**
- Shared helpers: `internal/infrastructure/fileutil/` or `internal/infrastructure/retry/`
- TUI helpers: `internal/ui/tui/helpers_*.go`
- Config helpers: `internal/core/config/`

## Special Directories

**`internal/tests/`:**
- Purpose: Integration tests that span multiple packages
- Generated: No
- Committed: Yes

**`tests/testutil/mocks/`:**
- Purpose: Mock implementations for provider, tool, dispatcher
- Generated: No (hand-written)
- Committed: Yes

**`dist/`:**
- Purpose: Cross-compiled binaries for release
- Generated: Yes (by `make cross` or goreleaser)
- Committed: No (gitignored)

**`.planning/`:**
- Purpose: GSD planning state — codebase maps, phase plans, decisions
- Generated: Yes (by GSD commands)
- Committed: Yes

**`.m31a/`:**
- Purpose: Runtime data — sessions, backups, tool output, logs
- Generated: Yes (at runtime)
- Committed: No (gitignored)

---

*Structure analysis: 2026-08-03*
