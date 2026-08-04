# Codebase Structure

**Analysis Date:** 2026-08-04

## Directory Layout

```
M31A/
├── cmd/m31a/                  # CLI entry point
│   └── main.go                # Flag parsing, config, TUI launch
├── internal/                  # Private application code (88K+ LOC)
│   ├── core/                  # Shared vocabulary & config
│   │   ├── config/            # TOML config loading, validation, merge
│   │   ├── errors/            # Sentinel errors, typed wrappers
│   │   └── types/             # Phase, Task, Message, Tool, Model types
│   ├── engine/                # Business logic
│   │   ├── workflow/          # 7-phase workflow engine (1832-line engine.go)
│   │   ├── taskrunner/        # Dependency-aware task scheduling
│   │   ├── bisect/            # Git bisect for failure isolation
│   │   ├── rollback/          # Git-based session rollback
│   │   ├── compaction/        # Session context compression
│   │   ├── coordinator/       # Phase coordination helpers
│   │   ├── decision/          # Non-blocking decision logging
│   │   ├── narrative/         # Event classification & rendering
│   │   ├── session/           # Session persistence & checkpoint
│   │   └── tokens/            # Token estimation via tiktoken
│   ├── infrastructure/        # Low-level utilities
│   │   ├── fileutil/          # Atomic writes, file locking
│   │   └── retry/             # Configurable retry policies
│   ├── integrations/          # External system adapters
│   │   ├── provider/          # LLM providers (OpenRouter, Zen, Nvidia)
│   │   ├── git/               # Git operations wrapper
│   │   ├── keychain/          # OS keychain for API keys
│   │   ├── ledger/            # Session history in markdown
│   │   ├── context/           # Dynamic context source registry
│   │   ├── metrics/           # Tool/LLM/phase metrics collection
│   │   ├── codeintel/         # Source code indexing & analysis
│   │   ├── autodream/         # Context consolidation
│   │   ├── skills/            # Composable slash commands
│   │   ├── shell/             # Platform-specific shell execution
│   │   ├── history/           # Frecency prompt history
│   │   └── arbitrage/         # Model selection optimization
│   ├── tools/                 # Tool execution framework
│   │   ├── fileops/           # FileRead, FileWrite, Edit, FileList, etc.
│   │   ├── exec/              # Bash, DevServer, OutputStore
│   │   ├── search/            # WebFetch, WebSearch, Glob, Grep
│   │   ├── ai/                # AskUserQuestion, Agent tool
│   │   ├── codeanalysis/      # CodeMap, CodeComplexity
│   │   ├── git/               # Git tool for LLM context
│   │   ├── network/           # HTTPCheck
│   │   ├── subagent/          # Subagent manager, loop, worktree
│   │   └── todo/              # TodoWrite, TodoRead
│   ├── tests/                 # Internal test helpers
│   │   ├── tools/             # Tool-specific test utilities
│   │   └── tui/               # TUI test harnesses
│   ├── testutil/              # CI and test timeout helpers
│   │   ├── ci/                # CI-specific utilities
│   │   └── testtimeout/       # Test timeout management
│   └── ui/                    # Terminal user interface
│       └── tui/               # Bubble Tea application
│           ├── commands/      # Slash command definitions
│           ├── components/    # Reusable UI primitives (51 files)
│           ├── layout/        # Header/footer, responsive constraints
│           ├── streaming/     # LLM streaming to TUI bridge
│           ├── theme/         # Dark theme management
│           └── tuitypes/      # Shared TUI types (breaks cycles)
├── pkg/                       # Public library code (currently empty)
├── tests/                     # External test suites
│   ├── e2e/                   # End-to-end tests (binary compilation)
│   └── testutil/              # Test utilities
│       ├── e2e/               # E2E test helpers
│       ├── integration/       # Integration test helpers
│       └── mocks/             # Mock implementations
├── dist/                      # Cross-compiled binaries
├── docs/                      # Documentation
├── scripts/                   # Build/release scripts
├── .github/                   # GitHub workflows & issue templates
├── .m31a/                     # Project-local M31A config
├── .planning/                 # GSD planning artifacts
│   └── codebase/              # Codebase analysis documents
├── go.mod                     # Go module definition
├── go.sum                     # Dependency checksums
├── Makefile                   # Build, test, lint, cross-compile targets
├── .golangci.yml              # Linter configuration
├── .goreleaser.yaml           # Release configuration
├── AGENTS.md                  # Agent instructions & conventions
└── README.md                  # Project documentation
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Application entry point — the only `package main` in the project
- Contains: `main.go` with `run()` function (588 lines), flag parsing, provider setup, TUI launch
- Key files: `main.go` (entry point), `.m31a/` (project-local config)

**`internal/core/config/`:**
- Purpose: Configuration management — loading, validation, merging, hot-reload
- Contains: TOML config types, loader, validator, merge logic, project context
- Key files: `types.go` (Config struct with 16 sub-configs), `loader.go` (TOML loading), `config_validate.go`

**`internal/core/types/`:**
- Purpose: Shared type vocabulary — the foundation that all other packages import
- Contains: WorkflowPhase, Task, Message, ModelInfo, Tool, ToolDefinition, RiskLevel, IntentType
- Key files: `types.go` (368 lines of shared types), `constants.go`, `toolcall.go`, `plan.go`

**`internal/core/errors/`:**
- Purpose: Error definitions and user-facing error messages
- Contains: 20+ sentinel errors, 3 typed error wrappers, `UserMessage()` function
- Key files: `errors.go` (258 lines)

**`internal/engine/workflow/`:**
- Purpose: Core workflow orchestration — the 7-phase pipeline engine
- Contains: Engine struct, phase implementations, state machine, context builder, prompt builder
- Key files: `engine.go` (1832 lines), `state_machine.go`, `phase_coordinator.go`, `execute.go` (870 lines)

**`internal/engine/session/`:**
- Purpose: Session persistence and resume
- Contains: Manager, Session struct, checkpoint, planning state
- Key files: `manager.go`, `session.go`, `checkpoint.go`, `planning.go`

**`internal/engine/taskrunner/`:**
- Purpose: Dependency-aware task scheduling with parallelism
- Contains: Runner with topological sort, parallel group execution
- Key files: `runner.go` (364 lines)

**`internal/tools/`:**
- Purpose: Tool execution framework — dispatcher, permissions, rate limiting, 18+ built-in tools
- Contains: Dispatcher, tool definitions, permission system, rate limiter
- Key files: `dispatcher.go` (535 lines), `defaults.go`, `permissions.go`, `constants.go`, `tools_reexport.go`

**`internal/integrations/provider/`:**
- Purpose: LLM provider abstraction with three implementations
- Contains: LLMProvider interface, Registry, base client, SSE parser, capability detection
- Key files: `interface.go`, `registry.go`, `base_client.go`, `sse.go`, `capabilities.go`

**`internal/integrations/provider/openrouter/`:**
- Purpose: OpenRouter LLM provider implementation
- Contains: Provider struct implementing LLMProvider
- Key files: Provider implementation

**`internal/integrations/provider/zen/`:**
- Purpose: Zen LLM provider implementation
- Contains: Provider struct implementing LLMProvider
- Key files: Provider implementation

**`internal/integrations/provider/nvidia/`:**
- Purpose: Nvidia LLM provider implementation
- Contains: Provider struct implementing LLMProvider
- Key files: Provider implementation

**`internal/ui/tui/`:**
- Purpose: Terminal user interface — Bubble Tea application with 25+ screens
- Contains: AppState (top model), REPL, per-screen models, handlers, streaming
- Key files: `app.go` (Init), `app_state.go` (AppState struct), `app_update.go` (Update dispatch), `repl_model.go`

**`internal/ui/tui/components/`:**
- Purpose: Reusable UI primitives — 51 component files
- Contains: Badge, Card, Progress, Spinner, ToolCard, TaskGraph, etc.
- Key files: `badge.go`, `card.go`, `progress.go`, `spinner.go`, `toolcard.go`, `taskgraph.go`

**`internal/ui/tui/layout/`:**
- Purpose: Layout system — header/footer chrome, responsive constraints
- Contains: Box model, constraints, page chrome, stack layout
- Key files: `page.go` (HeaderInfo, FooterInfo, PageChrome), `constraints.go`, `box.go`

**`internal/ui/tui/streaming/`:**
- Purpose: Bridge between LLM streaming and TUI rendering
- Contains: Agent loop integration, streaming message types
- Key files: `agent_loop.go`, `streaming.go`

**`internal/ui/tui/tuitypes/`:**
- Purpose: Shared TUI types to break circular dependencies
- Contains: Screen constants, message types, key binding types, WorkflowEngine interface
- Key files: `tuitypes.go` (904 lines)

**`tests/`:**
- Purpose: External test suites separate from internal test files
- Contains: E2E tests, test utilities, mocks
- Key files: `e2e/e2e_test.go`, `testutil/mocks/`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Application entry point — `run()` function returns exit code
- `internal/ui/tui/app.go:Init()`: TUI initialization — session setup, screen routing
- `internal/ui/tui/app_update.go:Update()`: Central message dispatch

**Configuration:**
- `internal/core/config/types.go`: Config struct with all 16 sub-configs
- `internal/core/config/loader.go`: TOML loading and merging
- `internal/core/config/config_validate.go`: Validation rules
- `.m31a/config.toml`: Project-local config (not in repo, gitignored)

**Core Logic:**
- `internal/engine/workflow/engine.go`: Workflow engine — 7-phase orchestration
- `internal/engine/workflow/state_machine.go`: Phase transition validation
- `internal/engine/workflow/execute.go`: Task execution with tool dispatch
- `internal/tools/dispatcher.go`: Tool execution hub with permissions

**Testing:**
- `tests/e2e/e2e_test.go`: End-to-end binary compilation tests
- `internal/tools/*_test.go`: Tool unit tests (co-located)
- `internal/engine/workflow/*_test.go`: Workflow engine tests (co-located)
- `internal/ui/tui/*_test.go`: TUI tests (co-located)

## Naming Conventions

**Files:**
- Go files use `snake_case.go` (e.g., `app_update.go`, `state_machine.go`)
- Test files use `_test.go` suffix (co-located with source)
- Platform-specific files use `_unix.go` / `_windows.go` / `_darwin.go` suffix
- Extra test files use `_extra_test.go` suffix for supplementary test cases
- Benchmark files use `_test.go` with `Benchmark` prefix

**Directories:**
- All lowercase with underscores (e.g., `taskrunner`, `codeanalysis`)
- Package names match directory names
- Test utilities in `testutil/` or `*_test.go` files

**Types:**
- Exported types use PascalCase (e.g., `WorkflowPhase`, `AppState`, `Dispatcher`)
- Interface types use `-er` suffix when single-method (e.g., `WorktreeOps`, `TokenEstimator`)
- Multi-method interfaces use descriptive names (e.g., `LLMProvider`, `Screenable`)
- Message types use `Msg` suffix (e.g., `StreamChunkMsg`, `PhaseResultMsg`)

**Functions:**
- Exported functions use PascalCase (e.g., `NewRegistry`, `DefaultDispatcher`)
- Constructor functions use `New` prefix (e.g., `NewManager`, `NewStateMachine`)
- Internal functions use camelCase (e.g., `detectProjectType`, `parseQuestions`)
- Phase run functions use `run` prefix (e.g., `runInitialize`, `runExecute`)

**Constants:**
- Exported constants use PascalCase (e.g., `MaxConcurrentTools`, `ToolRateLimitBurst`)
- Unexported constants use camelCase (e.g., `defaultCapacity`, `maxHistorySize`)
- Type constants use the type name as prefix (e.g., `PhaseInitialize`, `StatusPending`)

## Where to Add New Code

**New Workflow Phase:**
1. Add phase constant to `internal/core/types/types.go` (WorkflowPhase)
2. Add transition rules to `internal/engine/workflow/state_machine.go`
3. Create `internal/engine/workflow/<phase>.go` with `run<Phase>()` method
4. Add case to `Engine.RunPhase()` in `internal/engine/workflow/engine.go`
5. Add screen model in `internal/ui/tui/<phase>_model.go`
6. Add screen constant to `internal/ui/tui/tuitypes/tuitypes.go`

**New Tool:**
1. Create package under `internal/tools/<category>/` (e.g., `internal/tools/mytool/`)
2. Implement `types.Tool` interface (Name, Description, Execute) and optionally `types.SchemaProvider`
3. Register in `internal/tools/defaults.go:DefaultDispatcher()`
4. Add re-exports to `internal/tools/tools_reexport.go` if needed
5. Add tests in `internal/tools/<category>/<tool>_test.go`

**New LLM Provider:**
1. Create package under `internal/integrations/provider/<name>/`
2. Implement `provider.LLMProvider` interface (7 methods)
3. Register in `cmd/m31a/main.go` provider registration block
4. Add config struct to `internal/core/config/types.go:ProviderConfig`

**New Integration:**
1. Create package under `internal/integrations/<name>/`
2. Keep dependencies minimal — import only `internal/core/` if possible
3. Wire into engine or TUI via dependency injection

**New TUI Screen:**
1. Create `internal/ui/tui/<name>_model.go` implementing `tuitypes.Screenable`
2. Add screen constant to `internal/ui/tui/tuitypes/tuitypes.go`
3. Add routing in `internal/ui/tui/app_routing.go`
4. Add key bindings in `internal/ui/tui/keybindings.go`
5. Add view in `<name>_view.go` or inline in model file

**New Config Field:**
1. Add field to appropriate sub-config in `internal/core/config/types.go`
2. Add TOML tag and default in `internal/core/config/loader.go`
3. Add validation in `internal/core/config/config_validate.go`
4. Update `.env.example` if env var override is needed

**New Slash Command:**
1. Add to `internal/ui/tui/commands/` in appropriate category file
2. Register in `internal/ui/tui/commands/commands.go`

**New Component:**
1. Create `internal/ui/tui/components/<name>.go`
2. Keep stateless if possible — prefer functions over structs
3. Follow existing component patterns (see `badge.go`, `card.go`)

## Special Directories

**`dist/`:**
- Purpose: Cross-compiled binaries for all platforms
- Generated: Yes (by `make cross` or goreleaser)
- Committed: Yes (pre-built binaries for releases)

**`.m31a/`:**
- Purpose: Project-local M31A configuration and session data
- Generated: Yes (created at runtime)
- Committed: No (gitignored)

**`.planning/`:**
- Purpose: GSD planning artifacts and codebase analysis
- Generated: Yes (by GSD commands)
- Committed: No (gitignored)

**`tests/e2e/`:**
- Purpose: End-to-end tests that compile and run the binary
- Generated: No
- Committed: Yes

**`internal/engine/workflow/templates/`:**
- Purpose: Embedded website templates for project scaffolding
- Generated: No (static templates)
- Committed: Yes
- Note: Uses `//go:embed` directive in `engine.go`

**`internal/engine/workflow/prompts/`:**
- Purpose: Embedded prompt templates for workflow phases
- Generated: No (static prompts)
- Committed: Yes
- Note: Uses `//go:embed` directive

**`M31A.wiki/`:**
- Purpose: GitHub wiki content (separate git repo)
- Generated: No
- Committed: Yes (in wiki repo)

**`.github/`:**
- Purpose: GitHub Actions workflows and issue templates
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-08-04*
