# Codebase Structure

**Analysis Date:** 2026-07-04

## Directory Layout

```
M31A/
├── cmd/m31a/           # Binary entry point (CLI, flag parsing, TUI bootstrap)
├── internal/           # Private application packages (not importable outside module)
│   ├── codeintel/      # Code intelligence: AST parsing, relevance scoring, trie indexing
│   ├── config/         # TOML config loading, merging, type definitions
│   ├── context/        # Dynamic system context sources (datetime, env, git)
│   ├── decision/       # Decision logging with receipts and redaction
│   ├── errors/         # Sentinel error definitions
│   ├── fileutil/       # Atomic writes, platform file locking
│   ├── git/            # Git operations (commit, diff, branch, worktree)
│   ├── log/            # Structured slog logger (file + stderr)
│   ├── logging/        # Audit logging subsystem
│   ├── provider/       # LLM provider abstraction + implementations
│   │   ├── nvidia/     # NVIDIA NIM provider
│   │   ├── openrouter/ # OpenRouter provider
│   │   └── zen/        # Zen (opencode.ai) provider
│   ├── scaffold/       # Project scaffolding for website generation
│   ├── shell/          # Platform-specific shell detection
│   ├── testutil/       # Test environment helpers
│   ├── tokens/         # Token estimation with EMA calibration
│   ├── tools/          # Tool implementations + dispatcher
│   │   └── subagent/   # Parallel child agent orchestration
│   ├── tui/            # Bubble Tea TUI (Elm architecture)
│   │   ├── a11y/       # Accessibility announcements
│   │   ├── commands/   # Slash command implementations
│   │   ├── components/ # Reusable UI building blocks
│   │   ├── layout/     # Responsive layout system (Box/Stack/Page)
│   │   ├── streaming/  # LLM streaming + agent loop for TUI
│   │   ├── theme/      # Design tokens, light/dark themes
│   │   └── tuitypes/   # TUI-specific type definitions
│   ├── types/          # Shared domain types (Message, Task, ModelInfo, etc.)
│   ├── wiring/         # Dependency wiring helpers
│   └── workflow/       # 7-phase workflow engine
│       ├── prompts/    # Embedded prompt templates
│       │   └── models/ # Prompt model definitions
│       └── templates/  # Embedded website templates (nextjs, blog, portfolio, spa)
├── pkg/                # Public reusable packages (no internal/ imports)
│   ├── arbitrage/      # Model cost/quality arbitrage scoring
│   ├── autodream/      # Auto-suggestion consolidation
│   ├── bisect/         # Git bisect automation
│   ├── compaction/     # Session context compaction
│   ├── coordinator/    # Generic concurrency coordinator
│   ├── history/        # Frecent history tracking
│   ├── keychain/       # OS keychain integration (macOS/Linux/Windows)
│   ├── ledger/         # Markdown session ledger
│   ├── metrics/        # Metrics collector
│   ├── narrative/      # Narrative engine for progressive UI
│   ├── retry/          # Retry policies with backoff
│   ├── rollback/       # Git-based session rollback
│   ├── session/        # Session persistence (messages, tasks, checkpoints)
│   ├── skills/         # Skill discovery and loading
│   └── taskrunner/     # Parallel task execution with dependencies
├── .m31a/              # Project-local runtime data (sessions, backups)
├── .planning/          # Planning documents and phase artifacts
├── docs/               # Project documentation
├── scripts/            # Build and utility scripts
├── .github/            # GitHub Actions workflows and issue templates
├── go.mod              # Go module definition (go 1.25.0)
├── go.sum              # Dependency checksums
├── Makefile            # Build, test, lint commands
├── .golangci.yml       # Linter configuration
├── .goreleaser.yaml    # Cross-compilation and release config
├── Dockerfile          # Container build
└── AGENTS.md           # Agent instructions and codebase guide
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Single binary entry point
- Contains: `main.go` only
- Key files: `cmd/m31a/main.go`

**`internal/provider/`:**
- Purpose: LLM API abstraction with multiple backends
- Contains: Interface, registry, base client, per-provider implementations, caching, SSE streaming, fallback logic
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/base_client.go`, `internal/provider/fallback.go`, `internal/provider/sse.go`, `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`, `internal/provider/nvidia/client.go`

**`internal/tools/`:**
- Purpose: 18+ built-in tools with dispatcher managing permissions, rate limiting, concurrency
- Contains: Tool implementations, dispatcher, permission system, output store, constants
- Key files: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`, `internal/tools/interface.go`, `internal/tools/constants.go`, `internal/tools/bash.go`, `internal/tools/edit.go`, `internal/tools/filewrite.go`, `internal/tools/grep.go`, `internal/tools/glob.go`, `internal/tools/webfetch.go`, `internal/tools/agent.go`, `internal/tools/permissions.go`

**`internal/workflow/`:**
- Purpose: 7-phase workflow engine orchestrating the full development lifecycle
- Contains: Engine, state machine, phase coordinator, per-phase logic, prompt/context builders, intent classification, self-healing, plan parsing
- Key files: `internal/workflow/engine.go`, `internal/workflow/state_machine.go`, `internal/workflow/initialize.go`, `internal/workflow/discuss.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`, `internal/workflow/verify.go`, `internal/workflow/runtime.go`, `internal/workflow/ship.go`, `internal/workflow/prompt_builder.go`, `internal/workflow/context_builder.go`, `internal/workflow/intent.go`

**`internal/tui/`:**
- Purpose: Full-screen terminal UI following Elm architecture
- Contains: Top-level AppState, screen routing, REPL, sidebar, streaming, handlers, screen-specific models
- Key files: `internal/tui/app.go`, `internal/tui/app_state.go`, `internal/tui/app_update.go`, `internal/tui/app_view.go`, `internal/tui/app_routing.go`, `internal/tui/repl.go`, `internal/tui/repl_model.go`, `internal/tui/repl_view.go`, `internal/tui/sidebar_model.go`, `internal/tui/commands.go`, `internal/tui/keybindings.go`

**`internal/types/`:**
- Purpose: Shared type vocabulary across all layers
- Contains: Core domain types, constants, Tool interface, enums
- Key files: `internal/types/types.go`, `internal/types/constants.go`, `internal/types/toolcall.go`, `internal/types/plan.go`, `internal/types/git.go`

**`pkg/session/`:**
- Purpose: Session persistence and management
- Contains: Session create/load/save, message persistence, task tracking, checkpoint resume
- Key files: `pkg/session/manager.go`, `pkg/session/session.go`, `pkg/session/planning.go`, `pkg/session/checkpoint.go`

**`pkg/keychain/`:**
- Purpose: OS keychain integration for API key storage
- Contains: Platform-specific implementations (macOS, Linux, Windows), cached wrapper
- Key files: `pkg/keychain/keychain.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_windows.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point, CLI parsing, TUI bootstrap
- `e2e_test.go`: E2E test runner (compiles and runs binary)

**Configuration:**
- `internal/config/types.go`: All config struct definitions
- `internal/config/loader.go`: Config loading from TOML
- `internal/config/merge.go`: Config merge logic
- `internal/config/instructions.go`: AGENTS.md discovery
- `.golangci.yml`: Linter configuration
- `.goreleaser.yaml`: Release and cross-compilation

**Core Logic:**
- `internal/workflow/engine.go`: Workflow engine (1423 lines, central orchestration)
- `internal/workflow/state_machine.go`: Phase transition validation
- `internal/tools/dispatcher.go`: Tool execution dispatcher (480 lines)
- `internal/provider/interface.go`: LLM provider interface
- `internal/tui/app_state.go`: Top-level TUI state (418 lines)
- `internal/tui/app_update.go`: Message dispatch (569 lines)

**Testing:**
- `*_test.go` files co-located throughout all packages
- `e2e_test.go`: Root-level E2E tests
- `internal/testutil/envtest.go`: Test environment helpers

## Naming Conventions

**Files:**
- `snake_case.go` for all Go files
- `*_test.go` for test files (co-located with source)
- `*_extra_test.go` for supplementary test files
- `*_bench_test.go` for benchmark files
- Platform-specific: `*_unix.go`, `*_windows.go`, `*_darwin.go`
- Screen models: `{screen}_model.go`, `{screen}_view.go`
- Handlers: `handler_{concern}.go` (e.g., `handler_config.go`, `handler_modal.go`)
- App handlers: `app_handlers_{concern}.go`

**Directories:**
- `lowercase` (no underscores or hyphens)
- Package name matches directory name
- `internal/` for private, `pkg/` for public

**Types:**
- `PascalCase` for exported types
- `camelCase` for unexported types
- Interfaces: noun-based (e.g., `LLMProvider`, `Tool`, `WorktreeOps`)
- Message types: `{Name}Msg` suffix (e.g., `PhaseResultMsg`, `StreamChunkMsg`)
- Screen constants: `Screen{Name}` (e.g., `ScreenREPL`, `ScreenPlan`)

**Functions:**
- `PascalCase` for exported
- `camelCase` for unexported
- Handler functions: `handle{Name}Msg()` or `handle{Name}Action()`
- Bubble Tea: `Init()`, `Update()`, `View()` on tea.Model implementations
- Channel commands: `{name}Cmd()` pattern (e.g., `permListenerCmd()`, `startNewSession()`)

## Where to Add New Code

**New Workflow Phase:**
- Phase logic: `internal/workflow/{phase_name}.go`
- Phase type constant: `internal/types/types.go` (add to `WorkflowPhase` enum)
- State machine transitions: `internal/workflow/state_machine.go` (update `validTransitions`)
- TUI screen model: `internal/tui/{phase}_model.go` + `internal/tui/{phase}_view.go`
- Screen constant: `internal/tui/tuitypes/tuitypes.go` (add `Screen{Name}`)
- Engine switch case: `internal/workflow/engine.go` line 635 (`RunPhase()` switch)
- Config agent override: `internal/config/types.go` (`AgentsConfig`)

**New Tool:**
- Tool implementation: `internal/tools/{tool_name}.go`
- Register in: `internal/tools/defaults.go` (`DefaultDispatcher()`)
- Tool definition: `internal/tools/tooldefs.go` (JSON schema)
- Tests: `internal/tools/{tool_name}_test.go`

**New Provider:**
- Provider implementation: `internal/provider/{name}/client.go`
- Provider interface compliance: embed `provider.BaseClient`
- Register in: `cmd/m31a/main.go` (provider registration block)
- Config: `internal/config/types.go` (`ProviderConfig`)

**New TUI Screen:**
- Screen model: `internal/tui/{screen}_model.go`
- Screen view: `internal/tui/{screen}_view.go`
- Screen constant: `internal/tui/tuitypes/tuitypes.go`
- Screen routing: `internal/tui/app_routing.go` (add to `initScreenUpdaters()`)
- Key bindings: `internal/tui/keybindings.go`
- View rendering: `internal/tui/app_view.go` (add to `renderScreenContent()`)
- Update dispatch: `internal/tui/app_update.go` (add message case)

**New Slash Command:**
- Command definition: `internal/tui/commands/commands_{category}.go`
- Register in: `internal/tui/commands.go` (`DefaultCommands()`)

**New Public Package:**
- Package: `pkg/{package_name}/`
- Tests: `pkg/{package_name}/*_test.go`
- Must NOT import `internal/`

**New Config Field:**
- Add to struct: `internal/config/types.go`
- Add defaults: `internal/config/loader.go`
- Document in: `m31a.json` (JSON schema reference)

**New Embedded Prompt:**
- Prompt file: `internal/workflow/prompts/{name}.txt`
- Load in: `internal/workflow/prompt_builder.go`

**New TUI Component:**
- Component: `internal/tui/components/{name}.go`
- Theme integration: Use `theme.Manager` for styles

## Special Directories

**`.m31a/`:**
- Purpose: Project-local runtime data (sessions, backups, tool output, config overrides)
- Generated: Yes (created at runtime)
- Committed: No (gitignored)

**`.planning/`:**
- Purpose: Planning documents, phase artifacts, codebase maps
- Generated: Yes (by GSD commands)
- Committed: Partially (codebase maps yes, phase artifacts may be gitignored)

**`internal/workflow/templates/`:**
- Purpose: Embedded website templates (nextjs, blog, portfolio, spa) for website generation workflow
- Generated: No (bundled via `embed.FS`)
- Committed: Yes

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates for each workflow phase
- Generated: No (bundled via `embed.FS`)
- Committed: Yes

---

*Structure analysis: 2026-07-04*
