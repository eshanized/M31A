# Codebase Structure

**Analysis Date:** 2026-07-13

## Directory Layout

```
M31A/
├── cmd/m31a/              # CLI entry point
│   └── main.go            # Flag parsing, config, provider init, TUI launch
├── internal/              # Private application code (cannot be imported externally)
│   ├── codeintel/         # Codebase intelligence: AST parsing, dependency graphs, indexing
│   ├── config/            # TOML config loading, merging, project context detection
│   ├── context/           # Dynamic context sources (datetime, env, git)
│   ├── decision/          # Decision logging, querying, redaction
│   ├── errors/            # Sentinel errors, HTTP error classification
│   ├── fileutil/          # File utility helpers
│   ├── git/               # Git operations wrapper (init, commit, diff, worktree)
│   ├── log/               # Structured logger with file rotation
│   ├── logging/           # Additional logging utilities
│   ├── provider/          # LLM provider abstraction + 3 backends
│   │   ├── openrouter/    # OpenRouter API client
│   │   ├── zen/           # Zen API client
│   │   ├── nvidia/        # NVIDIA NIM API client
│   │   └── mock/          # Mock provider for testing
│   ├── shell/             # Shell execution utilities
│   ├── testutil/          # Test helpers and fixtures
│   ├── tokens/            # Token estimation (tiktoken-go)
│   ├── tools/             # 18 built-in tools + dispatcher
│   │   └── subagent/      # Parallel sub-agent manager with worktrees
│   ├── tui/               # Bubble Tea TUI (164 files)
│   │   ├── a11y/          # Accessibility helpers
│   │   ├── commands/      # Slash command registry and handlers
│   │   ├── components/    # Reusable UI components (modals, etc.)
│   │   ├── layout/        # Layout engine (box, stack, page, solver)
│   │   ├── streaming/     # Stream processing, agent loop
│   │   ├── theme/         # Dark theme definition
│   │   └── tuitypes/      # Screen IDs, message types, constants
│   ├── types/             # Shared type vocabulary (Message, Task, Tool, etc.)
│   ├── wiring/            # Dependency wiring (regression tests only)
│   └── workflow/          # 7-phase workflow engine
│       ├── prompts/       # Prompt templates (embedded via go:embed)
│       │   └── models/    # Model-specific prompt overrides
│       └── templates/     # Website templates (Next.js, blog, portfolio, SPA)
│           └── external/  # User-extensible templates
├── pkg/                   # Reusable packages (must NOT import internal/)
│   ├── arbitrage/         # Model cost/value scoring
│   ├── autodream/         # Auto-dream message consolidation
│   ├── bisect/            # Git bisect automation for bug finding
│   ├── compaction/        # Session history compaction
│   ├── coordinator/       # Task coordination utilities
│   ├── history/           # Frecent history tracking
│   ├── keychain/          # OS keychain integration (D-Bus/Secret Service)
│   ├── ledger/            # Session record persistence (LEDGER.md)
│   ├── metrics/           # Session metrics collector
│   ├── narrative/         # Narrative event classification and templating
│   ├── retry/             # Retry with backoff
│   ├── rollback/          # Git-based rollback
│   ├── session/           # Session manager (CRUD, checkpoint, task persistence)
│   ├── skills/            # Skill system
│   └── taskrunner/        # Task dependency graph executor
├── .planning/             # GSD planning artifacts
│   └── codebase/          # Codebase analysis documents
├── .claude/               # Claude configuration (if present)
├── .agents/               # Agent configuration (if present)
├── go.mod                 # Module: github.com/eshanized/M31A (Go 1.25)
├── go.sum                 # Dependency checksums
├── Makefile               # Build, test, lint, cross-compile targets
├── AGENTS.md              # Quick reference for agents
├── README.md              # Project documentation
├── LICENSE                # License file
├── .golangci.yml          # Linter configuration
├── .goreleaser.yml        # Release automation config
├── .gitignore             # Git ignore rules
└── .env.example           # Example environment variables
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: CLI entry point — single `main.go` with flag parsing, config load, provider registration, TUI construction
- Contains: `main.go` (477 lines)
- Key files: `cmd/m31a/main.go`

**`internal/types/`:**
- Purpose: Shared type vocabulary used by all layers — the leaf package with zero internal imports
- Contains: `Message`, `ToolCall`, `Task`, `WorkflowPhase`, `ModelInfo`, `Tool` interface, `StreamChunk`, risk levels, status constants
- Key files: `internal/types/types.go`, `internal/types/constants.go`, `internal/types/toolcall.go`

**`internal/config/`:**
- Purpose: TOML configuration loading with merge, project context detection, instructions loading
- Contains: `Config` struct with 16 sub-configs, loader with 4-level prompt priority, `.env` loading
- Key files: `internal/config/types.go`, `internal/config/loader.go`, `internal/config/merge.go`

**`internal/provider/`:**
- Purpose: LLM provider abstraction — interface + 3 backends + registry + caching + capabilities
- Contains: `LLMProvider` interface, `Registry`, SSE streaming, model metadata, fallback logic, reasoning detection
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/base_client.go`

**`internal/tools/`:**
- Purpose: 18 built-in tools for file I/O, code analysis, bash execution, web fetching, questions
- Contains: Tool implementations, `Dispatcher` (permissions, rate limiting, concurrency), `OutputStore`, `PersistentPermissions`
- Key files: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`, `internal/tools/interface.go`

**`internal/tools/subagent/`:**
- Purpose: Parallel sub-agent manager — each subagent gets its own dispatcher + git worktree
- Contains: `Manager`, `loop`, `worktree`, `profile`, `events`
- Key files: `internal/tools/subagent/manager.go`, `internal/tools/subagent/loop.go`

**`internal/tui/`:**
- Purpose: Bubble Tea TUI — 164 files implementing 30+ screens, REPL, sidebar, workflow views
- Contains: `AppState` (top model), screen models, handlers, streaming, command system, layout engine
- Key files: `internal/tui/app.go`, `internal/tui/app_state.go`, `internal/tui/app_update.go`, `internal/tui/repl.go`

**`internal/workflow/`:**
- Purpose: 7-phase workflow engine — Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship
- Contains: `Engine`, `StateMachine`, `PhaseCoordinator`, `ContextBuilder`, `PromptBuilder`, phase implementations
- Key files: `internal/workflow/engine.go`, `internal/workflow/state_machine.go`, `internal/workflow/execute.go`

**`pkg/session/`:**
- Purpose: Session persistence — CRUD operations, checkpoint save/restore, task persistence
- Contains: `Manager`, `Session`, checkpoint handling, project state, planning files
- Key files: `pkg/session/manager.go`, `pkg/session/session.go`, `pkg/session/checkpoint.go`

**`pkg/taskrunner/`:**
- Purpose: Task dependency graph executor — runs tasks in parallel with dependency ordering
- Contains: `Runner` with `MaxParallel` configuration
- Key files: `pkg/taskrunner/runner.go`

**`pkg/rollback/`:**
- Purpose: Git-based rollback — revert to specific commits or undo changes
- Contains: `Rollback` struct wrapping `git.Git`
- Key files: `pkg/rollback/rollback.go`

**`pkg/bisect/`:**
- Purpose: Git bisect automation — find which commit introduced a bug
- Contains: `Bisect` with exec function abstraction
- Key files: `pkg/bisect/bisect.go`

**`pkg/compaction/`:**
- Purpose: Session history compaction — summarize old messages to stay within context window
- Contains: `Compactor`, template-based summarization, `SplitMessages()`
- Key files: `pkg/compaction/compaction.go`, `pkg/compaction/template.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: CLI entry point — `main()` → `run()` → `tea.NewProgram(app).Run()`
- `internal/tui/app.go:24`: `AppState.Init()` — starts health ticker, permission listener
- `internal/tui/app_update.go:19`: `AppState.Update()` — single dispatch for all messages

**Configuration:**
- `internal/config/types.go`: `Config` struct with all TOML fields
- `internal/config/loader.go`: `Load()` — TOML parsing, `.env` loading, keychain resolution
- `internal/config/merge.go`: `Merge()` — project + global config merging
- `.env.example`: Example environment variables (gitignored except this file)

**Core Logic:**
- `internal/workflow/engine.go`: `Engine.RunPhase()` — 7-phase orchestration
- `internal/workflow/state_machine.go`: `StateMachine.Transition()` — phase transition validation
- `internal/tools/dispatcher.go`: `Dispatcher.Execute()` — tool execution with permissions
- `internal/provider/registry.go`: `Registry` — provider management and active tracking

**Testing:**
- `*_test.go` files co-located with source files throughout `internal/` and `pkg/`
- `internal/tools/subagent/*_test.go`: Sub-agent unit tests
- `cmd/m31a/*_test.go`: E2E tests (compile and run binary)
- `internal/testutil/`: Shared test helpers

## Naming Conventions

**Files:**
- Source: `snake_case.go` (e.g., `app_state.go`, `state_machine.go`, `toolcall.go`)
- Tests: `snake_case_test.go` co-located with source
- Models: `<name>_model.go` (e.g., `repl_model.go`, `sidebar_model.go`, `plan_model.go`)
- Views: `<name>_view.go` (e.g., `repl_view.go`, `plan_view.go`)
- Handlers: `handler_<category>.go` (e.g., `handler_config.go`, `handler_stream.go`)
- Platform: `<name>_<os>.go` (e.g., `bash_sandbox_linux.go`, `ship_lock_unix.go`)

**Directories:**
- Kebab-case for sub-packages (e.g., `subagent/`, `codeintel/`)
- Plural for package names (e.g., `tools/`, `providers/`, `types/`)

**Types:**
- Interfaces: `PascalCase` with `-er` suffix (e.g., `LLMProvider`, `Tool`, `GitClient`)
- Structs: `PascalCase` (e.g., `AppState`, `Dispatcher`, `Engine`)
- Constants: `PascalCase` for exported, `camelCase` for unexported
- Error sentinels: `Err` prefix (e.g., `ErrProviderUnreachable`, `ErrPhaseTransition`)

**Functions:**
- Exported: `PascalCase` (e.g., `NewEngine`, `RunPhase`, `ChatCompletionStream`)
- Unexported: `camelCase` (e.g., `runExecute`, `buildDiscussContext`, `modelForPhase`)
- Constructors: `New` prefix (e.g., `NewEngine`, `NewDispatcher`, `NewRegistry`)
- Commands: `<Verb><Noun>` (e.g., `StartStreamCmd`, `NextHealthTick`)

## Where to Add New Code

**New Tool:**
- Implementation: `internal/tools/<tool_name>.go` implementing `types.Tool` interface
- Registration: `internal/tools/defaults.go` — add `d.Register(New<Tool>(...))` call
- Tests: `internal/tools/<tool_name>_test.go`
- Permission risk: Set in `RiskLevel()` method — `safe`, `medium`, `dangerous`, `destructive`

**New Workflow Phase:**
- Add constant: `internal/types/types.go` — new `WorkflowPhase` constant
- Add transition: `internal/workflow/state_machine.go` — update `validTransitions` map
- Add handler: `internal/workflow/<phase>.go` — implement `run<Phase>()`
- Add TUI screen: `internal/tui/<phase>_model.go` + `<phase>_view.go`
- Add screen constant: `internal/tui/tuitypes/` — new `Screen<Phase>` constant

**New Provider:**
- Implementation: `internal/provider/<name>/` — implement `LLMProvider` interface
- Registration: `cmd/m31a/main.go` — add `tui.RegisterProvider(registry, cfg, "<name>", apiKey, version)`
- Config: `internal/config/types.go` — add `<Name>Config` to `ProviderConfig`

**New TUI Screen:**
- Model: `internal/tui/<name>_model.go` — struct + `Init()`, `Update()`, `View()`
- View: `internal/tui/<name>_view.go` — rendering logic (optional, can be in model)
- Screen constant: `internal/tui/tuitypes/` — add `Screen<Name>` constant
- Registration: `internal/tui/app_state.go` — add field to `AppState`
- Routing: `internal/tui/app_routing.go` — add case to `screenUpdaters` map

**New PKG Package:**
- Location: `pkg/<package_name>/`
- Constraint: Must NOT import `internal/` — only `internal/types` for type definitions
- Pattern: Export `<Type>` struct, `<NewType>()` constructor, `<Method>()` functions

**New Config Field:**
- Type: `internal/config/types.go` — add field to appropriate `*Config` struct
- TOML tag: Add `toml:"field_name"` tag
- Default: Handle in `loader.go` or `merge.go` with safe defaults
- Validation: Add validation in `loader.go` if needed

## Special Directories

**`.planning/`:**
- Purpose: GSD planning artifacts — codebase analysis, phase plans, requirements
- Generated: Yes (by GSD commands)
- Committed: No (gitignored)

**`.m31a/` (runtime):**
- Purpose: Runtime data — sessions, checkpoints, backups, tool output, logs
- Generated: Yes (at runtime)
- Committed: No (gitignored)
- Location: `<workDir>/.m31a/` for project-local, `~/.m31a/` for global

**`internal/workflow/templates/`:**
- Purpose: Website build templates (Next.js, blog, portfolio, SPA)
- Generated: No (static embedded via `go:embed`)
- Committed: Yes

**`internal/workflow/prompts/`:**
- Purpose: Prompt templates for each workflow phase
- Generated: No (static embedded via `go:embed`)
- Committed: Yes

---

*Structure analysis: 2026-07-13*
