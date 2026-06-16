# Codebase Structure

**Analysis Date:** 2026-06-16

## Directory Layout

```
M31A/
├── cmd/m31a/              # CLI entry point
├── internal/              # Private packages (not importable by external code)
│   ├── codeintel/         # Project indexing, import graph, relevance scoring
│   ├── config/            # TOML config loading, hot-reload, env expansion
│   ├── errors/            # Sentinel errors + UserMessage() mapping
│   ├── fileutil/          # Atomic file writes (temp+rename)
│   ├── git/               # Shell-based git wrapper (satisfies types.GitClient)
│   ├── log/               # Structured slog logging, daily rotation, 7-day retention
│   ├── provider/          # LLM provider abstraction, SSE streaming, registry, fallback
│   │   ├── openrouter/    # OpenRouter API client
│   │   └── zen/           # Zen API client
│   ├── tokens/            # tiktoken-go + rune fallback, EMA calibration
│   ├── tools/             # Bash, FileRead, FileWrite, Glob, Grep, WebFetch, permissions
│   │   └── subagent/      # Parallel child agent management (worktrees, events)
│   ├── tui/               # Bubble Tea TUI (29 screens, page layout, themes)
│   │   ├── commands/      # Slash command registry and handlers
│   │   ├── components/    # 42 reusable UI components (badge, card, spinner, etc.)
│   │   ├── layout/        # Page chrome (header/footer), responsive breakpoints
│   │   ├── streaming/     # LLM streaming pipeline + agent loop
│   │   ├── theme/         # Theme manager, colors, borders, shadows
│   │   └── tuitypes/      # Shared types to break circular dependencies
│   ├── types/             # Core type definitions (WorkflowPhase, Task, Message, Tool, etc.)
│   └── workflow/          # Six-phase orchestration engine
│       └── prompts/       # 11 embedded markdown prompt templates
├── pkg/                   # Public packages (no internal/ imports — ideally)
│   ├── arbitrage/         # Model cost optimization, task complexity scoring
│   ├── autodream/         # Context window consolidation (message compression)
│   ├── bisect/            # Git bisect automation for finding breaking commits
│   ├── history/           # Frecent prompt history (frecency scoring)
│   ├── keychain/          # OS-native secure API key storage (Linux/macOS/Windows)
│   ├── ledger/            # Cross-session learning records (markdown tables)
│   ├── rollback/          # Git commit chain management, soft/hard/safe reset
│   ├── session/           # Session lifecycle, checkpoints, planning file I/O
│   └── taskrunner/        # Task dependency resolution (Kahn's) + bounded parallelism
├── docs/                  # 11 documentation files
├── scripts/               # verify_v1.sh acceptance suite
├── .github/workflows/     # CI/CD pipeline (ci.yml)
├── .planning/             # GSD planning artifacts
│   └── codebase/          # Generated codebase analysis documents
├── .m31a/                 # Runtime session data (project-local)
├── go.mod / go.sum        # Dependencies (Go 1.25)
├── Makefile               # Build/test/lint/cross-compile targets
├── .goreleaser.yaml       # Release config (GoReleaser v2)
└── install.sh             # One-liner installer
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: CLI entry point and usage/help output
- Contains: `main.go` (dependency wiring, signal handling, TUI launch), `usage.go` (help text)
- Key files: `cmd/m31a/main.go` — wires all dependencies, creates `AppState`, launches Bubble Tea

**`internal/tui/`:**
- Purpose: Full terminal UI with 29 screens, Elm architecture
- Contains: Screen models, app state management, view rendering, keybindings, helpers
- Key files:
  - `internal/tui/app_state.go` — `AppState` struct (top-level Bubble Tea model, 199 lines of fields)
  - `internal/tui/app.go` — `Init()`, `Shutdown()`, workflow engine init, file/config watchers
  - `internal/tui/app_update.go` — `Update()` dispatch (2752 lines, single mutation point)
  - `internal/tui/app_view.go` — `View()` rendering (838 lines, page chrome + screen content)
  - `internal/tui/repl.go` — REPL model (chat viewport, keyboard/mouse handling)
  - `internal/tui/types.go` — Re-exports from `tuitypes` sub-package

**`internal/tui/components/`:**
- Purpose: 42 reusable UI building blocks
- Contains: Badges, cards, spinners, code blocks, file trees, permission modals, sparklines, task graphs, etc.
- Key files: `permission.go` (permission modal), `message.go` (message renderer), `taskgraph.go` (task dependency visualization)

**`internal/tui/layout/`:**
- Purpose: Responsive page layout system (header + content + footer)
- Contains: Breakpoint detection, page chrome rendering, minimum screen guard
- Key files: `page.go` (unified `RenderPage()`), `responsive.go` (4 breakpoints: UltraNarrow/Compact/Standard/Full)

**`internal/tui/streaming/`:**
- Purpose: LLM streaming pipeline and autonomous agent loop
- Contains: SSE chunk processing, tool call accumulation, goroutine ownership model
- Key files: `streaming.go` (StartStreamCmd, goroutine lifecycle), `agent_loop.go` (multi-iteration tool-calling loop, max 50 iterations)

**`internal/tui/commands/`:**
- Purpose: Slash command registry and execution
- Contains: Command definitions, parsing, handlers
- Key files: Re-exported via `internal/tui/commands.go`

**`internal/tui/theme/`:**
- Purpose: Theme management (dark/light/auto), colors, borders, shadows
- Contains: Theme interface, registry, unicode decorations
- Key files: `theme.go` (Theme interface), `registry.go` (theme manager), `colors.go` (color definitions)

**`internal/tui/tuitypes/`:**
- Purpose: Shared types to break circular dependencies between tui and sub-packages
- Contains: `Screen` enum (29 values), 30+ message types, `WorkflowEngine` interface
- Key files: `tuitypes.go` (447 lines of shared type definitions)

**`internal/workflow/`:**
- Purpose: Six-phase workflow orchestration engine
- Contains: Phase implementations, engine core, plan parser, prompt registry, message types
- Key files:
  - `internal/workflow/engine.go` — `Engine` struct, `RunPhase()` dispatch, `NewEngine()`, model selection
  - `internal/workflow/initialize.go` — Project detection, git init, planning dir setup
  - `internal/workflow/discuss.go` — Clarifying questions via LLM streaming
  - `internal/workflow/plan.go` — Plan generation with LLM
  - `internal/workflow/plan_parser.go` — Markdown → Task extraction
  - `internal/workflow/execute.go` — Task runner integration, tool dispatch, self-heal
  - `internal/workflow/verify.go` — File checks, syntax validation, test execution
  - `internal/workflow/ship.go` — Final commit, ledger entry, session archival
  - `internal/workflow/engine_messages.go` — `MsgEmitter` interface and 15+ event types

**`internal/workflow/prompts/`:**
- Purpose: Embedded markdown prompt templates for LLM interactions
- Contains: 11 `.md` files loaded at startup via `embed.FS`
- Key files: `base.md`, `tool-use.md`, `plan-format.md`, `execute-task.md`, `discuss-questions.md`, `self-heal.md`, `demonstration-format.md`, `autonomous.md`, `context-awareness.md`, `code-quality.md`, `code-intelligence.md`

**`internal/provider/`:**
- Purpose: LLM provider abstraction with fallback, caching, health checks
- Contains: Provider interface, base client, registry, fallback logic, SSE parser, model cache
- Key files:
  - `internal/provider/interface.go` — `LLMProvider` interface (8 methods), `ChatRequest`, `ToolDefinition`
  - `internal/provider/base_client.go` — Shared HTTP transport, dual HTTP clients (streaming vs catalog)
  - `internal/provider/registry.go` — Thread-safe multi-provider management
  - `internal/provider/fallback.go` — Parallel health checks, automatic failover
  - `internal/provider/sse.go` — Server-sent events parser with watchdog timeout
  - `internal/provider/cache.go` — Model catalog cache with TTL (5 min active, 24h stale)

**`internal/tools/`:**
- Purpose: Tool implementations and permission-gated execution
- Contains: Dispatcher, 14+ tool implementations, permission system, rate limiter
- Key files:
  - `internal/tools/dispatcher.go` — Tool registry, permission gating, rate limiting, execution
  - `internal/tools/defaults.go` — `DefaultDispatcher()` registers all standard tools
  - `internal/tools/permissions.go` — Permission request/response channels, rule evaluation
  - `internal/tools/tooldefs.go` — `BuildToolDefs()` creates LLM tool schemas
  - `internal/tools/bash.go` — Shell command execution (dangerous risk, timeout, output capping)
  - `internal/tools/fileread.go` — File reading with size limits (50MB max)
  - `internal/tools/filewrite.go` — Atomic file writes with backup system
  - `internal/tools/edit.go` — File editing (find-and-replace)
  - `internal/tools/glob.go` — File pattern matching (doublestar, 1000 result limit)
  - `internal/tools/grep.go` — Content search (ripgrep when available, pure-Go fallback)
  - `internal/tools/webfetch.go` — URL fetching with SSRF protection
  - `internal/tools/websearch.go` — Privacy-respecting web search
  - `internal/tools/question.go` — Interactive user prompts
  - `internal/tools/agent.go` — Sub-agent spawning
  - `internal/tools/todo.go` — Task file I/O
  - `internal/tools/codemap.go` — Codebase mapping tool
  - `internal/tools/filedelete.go` — File deletion with backup
  - `internal/tools/filemove.go` — File renaming/moving
  - `internal/tools/filelist.go` — Directory listing

**`internal/tools/subagent/`:**
- Purpose: Parallel child agent management with isolated worktrees
- Contains: Manager, worktree lifecycle, event system, agent loop parsing
- Key files: `manager.go` (lifecycle management), `worktree.go` (git worktree creation), `loop.go` (child agent execution), `events.go` (event system)

**`internal/types/`:**
- Purpose: Core type definitions shared across all layers
- Contains: `WorkflowPhase`, `Task`, `Message`, `ToolCall`, `ModelInfo`, `Session`, constants
- Key files:
  - `internal/types/types.go` — All core types (237 lines): `Tool` interface, `Task`, `Message`, `ModelInfo`, `StreamChunk`, etc.
  - `internal/types/constants.go` — 45+ constants (cache TTLs, timeouts, limits, defaults)
  - `internal/types/git.go` — `GitClient` interface and `CommitInfo` type
  - `internal/types/plan.go` — `Plan` type for plan parsing

**`internal/config/`:**
- Purpose: TOML configuration loading, validation, hot-reload
- Contains: Config struct definitions, loader, hot-reload watcher, project context detection
- Key files:
  - `internal/config/types.go` — Config struct hierarchy (222 lines): Provider, Model, UI, Permissions, Features, etc.
  - `internal/config/loader.go` — `Load()`, `DefaultConfig()`, validation, hot-reload (1014 lines)
  - `internal/config/project_context.go` — Project type detection (Go, Node, Python, etc.)

**`internal/errors/`:**
- Purpose: Sentinel errors and user-friendly error messages
- Contains: 20+ sentinel errors, `UserMessage()` mapping, regex-based HTTP status detection
- Key files: `internal/errors/errors.go` (138 lines)

**`internal/git/`:**
- Purpose: Shell-based git wrapper satisfying `types.GitClient` interface
- Contains: All git operations (init, add, commit, log, diff, branch, etc.)
- Key files: `internal/git/git.go` (737 lines) — Compile-time interface check, sanitization, branching

**`internal/codeintel/`:**
- Purpose: Codebase intelligence (project indexing, import graph, symbol lookup, relevance scoring)
- Contains: Parser interface, import graph builder, symbol index, relevance scorer
- Key files: `internal/codeintel/codeintel.go` (lazy indexer), `graph.go` (import graph), `relevance.go` (scorer)

**`internal/tokens/`:**
- Purpose: Token estimation for context window management
- Contains: tiktoken-go integration, rune fallback, EMA calibration
- Key files: `internal/tokens/estimator.go`

**`internal/fileutil/`:**
- Purpose: Crash-safe file I/O utilities
- Contains: Atomic writes (temp + rename), permission preservation
- Key files: `internal/fileutil/atomic.go` (58 lines — simple and correct)

**`internal/log/`:**
- Purpose: Structured logging with daily rotation
- Contains: slog initialization, log rotation, 7-day retention
- Key files: `internal/log/log.go` (134 lines)

**`pkg/session/`:**
- Purpose: Session lifecycle management, checkpoint/restore, planning file I/O
- Contains: Manager (CRUD), session info, checkpoint management, planning files
- Key files:
  - `pkg/session/manager.go` — `Manager` struct, `NewManager()`, session CRUD (591 lines)
  - `pkg/session/checkpoint.go` — Checkpoint save/restore (max 2)
  - `pkg/session/planning.go` — PROJECT.md, STATE.md, TASKS.md I/O

**`pkg/taskrunner/`:**
- Purpose: Task execution with dependency resolution and bounded parallelism
- Contains: Runner (Kahn's algorithm), task scheduling, execution groups
- Key files: `pkg/taskrunner/runner.go` (374 lines) — Topological sort, parallel execution, retry

**`pkg/ledger/`:**
- Purpose: Cross-session learning records
- Contains: Markdown table persistence, stats, keyword search
- Key files: `pkg/ledger/ledger.go`

**`pkg/rollback/`:**
- Purpose: Git commit chain management
- Contains: Chain listing, soft/hard/safe reset with auto-stash and backup branches
- Key files: `pkg/rollback/rollback.go`

**`pkg/bisect/`:**
- Purpose: Automated git bisect
- Contains: Bisect runner with test function injection, timeout fallback
- Key files: `pkg/bisect/bisect.go`, `pkg/bisect/exec.go`

**`pkg/autodream/`:**
- Purpose: Context window consolidation (message compression)
- Contains: Protect/summarize logic, role-sampled compression
- Key files: `pkg/autodream/autodream.go`

**`pkg/arbitrage/`:**
- Purpose: Model cost optimization
- Contains: Task complexity scoring, token estimation per model, cheapest model recommendation
- Key files: `pkg/arbitrage/arbitrage.go`

**`pkg/keychain/`:**
- Purpose: OS-native secure API key storage
- Contains: Platform-specific implementations (Linux/macOS/Windows)
- Key files: `pkg/keychain/keychain.go` (interface), `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`

**`pkg/history/`:**
- Purpose: Frecent prompt history (frecency scoring)
- Contains: Score computation, JSON persistence, search
- Key files: `pkg/history/history.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: CLI entry — wires all dependencies, launches Bubble Tea program
- `cmd/m31a/usage.go`: Help text and command usage output

**Configuration:**
- `internal/config/types.go`: Config struct definitions (10 top-level sections)
- `internal/config/loader.go`: Config loading, validation, hot-reload, defaults
- `~/.m31a/config.toml`: Global config file location
- `m31a.toml`: Project-local config (walks up 3 parent directories)

**Core Logic:**
- `internal/tui/app_state.go`: Top-level Bubble Tea model with all state fields
- `internal/tui/app_update.go`: Single dispatch point for all messages (2752 lines)
- `internal/tui/app_view.go`: Full terminal frame rendering (838 lines)
- `internal/workflow/engine.go`: Six-phase workflow orchestrator (902 lines)
- `internal/workflow/execute.go`: Task execution with tool dispatch (614 lines)
- `internal/tools/dispatcher.go`: Tool registration, permission gating, execution (332 lines)

**Type Definitions:**
- `internal/types/types.go`: All core types and interfaces (237 lines)
- `internal/types/constants.go`: 45+ application constants (145 lines)
- `internal/tui/tuitypes/tuitypes.go`: TUI-specific types and message definitions (447 lines)

**Testing:**
- Tests are co-located with source files (`*_test.go` pattern)
- `*_extra_test.go` files contain additional test cases (not present in all packages)
- `internal/tui/test_helpers_test.go`: Shared test utilities for TUI tests

## Naming Conventions

**Files:**
- Screen models: `{feature}_model.go` + `{feature}_view.go` (e.g., `repl_model.go`, `plan_view.go`)
- Screen with extra tests: `{feature}_extra_test.go` (e.g., `sidebar_extra_test.go`)
- Platform-specific: `{name}_{os}.go` (e.g., `bash_unix.go`, `bash_windows.go`, `keychain_linux.go`)
- Re-exports: `types.go` or `{name}.go` at package root (e.g., `tui/types.go`, `tui/commands.go`)
- Prompts: `{purpose}.md` (e.g., `base.md`, `tool-use.md`)

**Directories:**
- Lowercase, single-word or hyphen-free (e.g., `codeintel`, `fileutil`, `tuitypes`)
- Sub-packages by feature (e.g., `provider/openrouter/`, `tools/subagent/`, `tui/components/`)
- Platform-specific code via build tags, not directory separation

**Types:**
- PascalCase for exported types: `AppState`, `ReplModel`, `WorkflowPhase`, `ToolInput`
- `-er` suffix for interfaces: `LLMProvider`, `GitClient`, `Keychain`, `MsgEmitter`, `SchemaProvider`
- `-er` suffix for functions returning interfaces: `NewDispatcher()` returns `*Dispatcher`
- Compile-time checks: `var _ Interface = (*Concrete)(nil)` at top of implementation files

**Constants:**
- PascalCase exported: `RiskDangerous`, `PhaseExecute`, `StatusPending`, `MaxHealAttempts`
- camelCase unexported: `permissionRequestID`, `sharedTransport`, `skipDirsCache`

**Error Package:**
- Aliased as `m31errors` throughout codebase: `m31errors "github.com/eshanized/M31A/internal/errors"`
- Types package aliased as `m31types` in workflow: `m31types "github.com/eshanized/M31A/internal/types"`

## Where to Add New Code

**New Tool:**
- Implementation: `internal/tools/{toolname}.go`
- Compile-time check: `var _ types.Tool = (*ToolName)(nil)` at top
- Register in: `internal/tools/defaults.go` (add to `DefaultDispatcher()`)
- Test: `internal/tools/{toolname}_test.go`

**New Workflow Phase:**
- Phase implementation: `internal/workflow/{phase}.go`
- Prompt template: `internal/workflow/prompts/{phase}.md`
- Add to switch in: `internal/workflow/engine.go:278` (`RunPhase`)
- Add to `WorkflowPhase` enum: `internal/types/types.go:18`
- Add screen: `internal/tui/tuitypes/tuitypes.go` (new `Screen` constant)
- Add screen model: `internal/tui/{phase}_model.go` + `{phase}_view.go`

**New TUI Screen:**
- Screen constant: `internal/tui/tuitypes/tuitypes.go` (add to `Screen` enum)
- Model file: `internal/tui/{feature}_model.go` (struct, `New{Feature}Model()`)
- View file: `internal/tui/{feature}_view.go` (optional, can be in model file)
- Wire into: `internal/tui/app_view.go` (`renderActiveScreen()`)
- Wire into: `internal/tui/app_update.go` (message handling)

**New LLM Provider:**
- Implementation: `internal/provider/{providername}/client.go`
- Embed `provider.BaseClient`
- Implement `provider.LLMProvider` interface
- Register in: `internal/tui/provider_registration.go` (`RegisterProvider()`)

**New pkg/ Package:**
- Create: `pkg/{packagename}/`
- Follow: Only import `internal/types` and `internal/errors` (no other `internal/` imports)
- Add doc.go with package documentation
- Add `_test.go` files for testing

**New Slash Command:**
- Register in: `internal/tui/commands/` package
- Command definitions, parsing, handlers

## Special Directories

**`internal/tui/components/`:**
- Purpose: 42 reusable UI building blocks (badge, card, codeblock, spinner, etc.)
- Generated: No (hand-written)
- Committed: Yes
- Pattern: Each component is a self-contained file with its own View/Render function

**`internal/workflow/prompts/`:**
- Purpose: Markdown templates for LLM system prompts (11 files)
- Generated: No (hand-written markdown)
- Committed: Yes
- Loaded: Via `embed.FS` at startup (compile-time embedding)

**`.m31a/`:**
- Purpose: Runtime session data (project-local)
- Generated: Yes (by running M31A)
- Committed: No (in `.gitignore`)
- Contains: `session.json`, `messages.json`, `checkpoint.json`, `backups/`, `TASKS.md`, `STATE.md`, `PROJECT.md`

**`~/.m31a/`:**
- Purpose: Global config and state (user home)
- Generated: Yes (on first run)
- Committed: No
- Contains: `config.toml`, `m31a.log`, `LEDGER.md`, `history.json`

**`dist/`:**
- Purpose: Cross-compiled binaries (from `make cross` or GoReleaser)
- Generated: Yes
- Committed: No (in `.gitignore`)

---

*Structure analysis: 2026-06-16*
