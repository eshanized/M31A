# Codebase Structure

**Analysis Date:** 2026-06-07

## Directory Layout

```
M31A/
├── cmd/m31a/                  # Binary entry point (no logic)
│   ├── main.go                # CLI flags, config load, provider init, TUI launch
│   └── usage.go               # Help text and usage formatting
├── internal/                  # Private application packages
│   ├── config/                # TOML config parsing, env overrides, hot-reload
│   │   ├── types.go           # Config struct definitions
│   │   └── loader.go          # Config loading, env var resolution, file watcher
│   ├── errors/                # Sentinel errors and user-friendly messages
│   │   └── errors.go          # All var Err* definitions + UserMessage()
│   ├── git/                   # Git operations wrapper
│   │   └── git.go             # init, add, commit, log, diff, reset, stash, status
│   ├── log/                   # Structured logger with rotation
│   │   └── log.go             # slog logger → ~/.m31a/m31a.log, daily rotation
│   ├── provider/              # LLM provider abstraction layer
│   │   ├── interface.go       # LLMProvider interface, ChatRequest, ToolDefinition
│   │   ├── registry.go        # ProviderRegistry (Register, SetActive, Get, List)
│   │   ├── sse.go             # SSE stream parser (StreamIterator)
│   │   ├── cache.go           # Model cache with TTL
│   │   ├── fallback.go        # Auto-fallback logic (429/503 → switch provider)
│   │   ├── reasoning.go       # Reasoning/thinking segment normalization
│   │   ├── common.go          # Shared HTTP client helpers
│   │   ├── capabilities.go    # Model capability detection
│   │   ├── openrouter/        # OpenRouter client implementation
│   │   │   └── client.go      # GET /models, POST /chat/completions
│   │   └── zen/               # Zen (OpenCode) client implementation
│   │       └── client.go      # Identical interface, different base URL
│   ├── tokens/                # Token estimation for context budgeting
│   │   └── estimator.go       # tiktoken-go for GPT/Claude, char fallback, EMA
│   ├── tools/                 # Tool implementations and dispatcher
│   │   ├── interface.go       # PermissionRequest/Response, PermissionGate
│   │   ├── dispatcher.go      # Tool registry, permission gate, Execute()
│   │   ├── defaults.go        # DefaultDispatcher factory (registers all tools)
│   │   ├── permissions.go     # Rule-based permission checking
│   │   ├── constants.go       # Tool-specific constants (avoids import cycle)
│   │   ├── bash.go            # Bash tool (PTY on Unix)
│   │   ├── bash_unix.go       # PTY allocation (creack/pty)
│   │   ├── bash_windows.go    # Plain pipe fallback
│   │   ├── fileread.go        # FileRead tool (encoding detection, binary check)
│   │   ├── filewrite.go       # FileWrite tool (atomic write, backup)
│   │   ├── glob.go            # Glob tool (doublestar pattern matching)
│   │   ├── grep.go            # Grep tool (rg or pure-Go fallback)
│   │   ├── edit.go            # Edit tool (search/replace)
│   │   ├── webfetch.go        # WebFetch tool (HTTP GET, SSRF protection)
│   │   ├── todo.go            # TodoWrite tool (session task tracking)
│   │   └── question.go        # AskUserQuestion tool (interactive modal)
│   ├── tui/                   # Bubble Tea application (all screens)
│   │   ├── app.go             # Top-level AppState, Init(), phase management
│   │   ├── app_state.go       # AppState struct, NewApp() constructor
│   │   ├── app_view.go        # View() — screen routing
│   │   ├── app_update.go      # Update() — message handling
│   │   ├── app_update_screen.go   # Screen-specific update handlers
│   │   ├── app_update_slash.go    # Slash command routing
│   │   ├── app_update_workflow.go  # Workflow message handling
│   │   ├── app_update_permission.go # Permission modal handling
│   │   ├── app_channel.go     # Channel-based workflow message bus
│   │   ├── app_workflow.go    # Workflow engine initialization
│   │   ├── types.go           # Screen enum, message types
│   │   ├── commands.go        # Command registry (DefaultCommands)
│   │   ├── commands_core.go   # /help, /status, /clear, /quit
│   │   ├── commands_session.go # /fork, /prev, /next, /sessions
│   │   ├── commands_workflow.go # /workflow start, /workflow resume
│   │   ├── commands_config.go  # /settings, /theme, /model
│   │   ├── commands_git.go    # /git status, /git log, /git diff
│   │   ├── commands_ai.go     # /compress, /optimize, /ledger
│   │   ├── repl.go            # REPL screen (main chat view)
│   │   ├── repl_model.go      # ReplModel struct and methods
│   │   ├── repl_view.go       # REPL View() rendering
│   │   ├── repl_state.go      # REPL message history management
│   │   ├── repl_stream.go     # Streaming token rendering
│   │   ├── repl_thinking.go   # Thinking block rendering
│   │   ├── repl_keys.go       # REPL key bindings
│   │   ├── repl_commands.go   # REPL-specific commands
│   │   ├── repl_welcome.go    # Welcome message
│   │   ├── repl_quickactions.go # Quick action buttons
│   │   ├── header.go          # Header bar (provider, model, context)
│   │   ├── sidebar.go         # Sidebar (git status, file tree)
│   │   ├── statusbar.go       # Status bar (current operation, time)
│   │   ├── health.go          # Health check ticker
│   │   ├── streaming.go       # Streaming infrastructure
│   │   ├── backup.go          # Auto-backup feature
│   │   ├── history.go         # Command history
│   │   ├── helpers.go         # Shared utility functions
│   │   ├── truncate.go        # Text truncation helpers
│   │   ├── metrics.go         # MetricsModel (session analytics)
│   │   ├── goalinput.go       # GoalInputModel (full-screen goal entry)
│   │   ├── plan_model.go      # PlanModel (task list + cost panel)
│   │   ├── plan_view.go       # Plan screen rendering
│   │   ├── execute_model.go   # ExecuteModel (task progress)
│   │   ├── execute_view.go    # Execute screen rendering
│   │   ├── verify.go          # Verify screen (pass/fail checklist)
│   │   ├── ship_model.go      # ShipModel (summary banner)
│   │   ├── ship_view.go       # Ship screen rendering
│   │   ├── diff_model.go      # DiffModel (diff preview)
│   │   ├── diff_view.go       # Diff view rendering
│   │   ├── firstrun_model.go  # FirstRunModel (provider setup)
│   │   ├── firstrun_view.go   # First-run screen rendering
│   │   ├── resume_model.go    # ResumeModel (session browser)
│   │   ├── resume_view.go     # Resume screen rendering
│   │   ├── settings_model.go  # SettingsModel (6-tab config editor)
│   │   ├── settings_view.go   # Settings screen rendering
│   │   ├── settings_tabs.go   # Tab navigation for settings
│   │   ├── settings_edit.go   # Inline config editing
│   │   ├── modelselector.go   # Model selector (fuzzy search, provider filter)
│   │   ├── modelselector_list.go  # Model list rendering
│   │   ├── modelselector_view.go  # Model selector view
│   │   ├── ledger.go          # LedgerModel (learning journal browser)
│   │   ├── rollback.go        # RollbackModel (commit time machine)
│   │   ├── discuss.go         # DiscussModel (Q&A flow)
│   │   ├── cmdpalette.go      # Command palette (Ctrl+X)
│   │   ├── keybindings.go     # Global key bindings
│   │   ├── keybindings_screens.go # Screen-specific key bindings
│   │   ├── providerbadge.go   # Provider badge rendering
│   │   ├── spinners.go        # Spinner animations
│   │   ├── cache_refresh.go   # Model cache refresh ticker
│   │   ├── components/        # Reusable TUI widgets
│   │   │   ├── toolcard.go    # Tool execution card
│   │   │   ├── thinking.go    # Collapsible thinking block
│   │   │   ├── permission.go  # Permission modal overlay
│   │   │   ├── message.go     # Message bubble renderer
│   │   │   ├── badge.go       # Status badges
│   │   │   ├── progress.go    # Progress bars
│   │   │   ├── sparkline.go   # Activity sparkline
│   │   │   ├── metriccard.go  # Metric display card
│   │   │   ├── statrow.go     # Stat row component
│   │   │   ├── filterchips.go # Filter chip buttons
│   │   │   ├── search.go      # Search input
│   │   │   ├── question.go    # Question display
│   │   │   ├── logo.go        # ASCII art logo
│   │   │   ├── starfield.go   # Starfield animation
│   │   │   ├── bash_renderer.go      # Bash output renderer
│   │   │   ├── file_renderers.go     # File content renderer
│   │   │   ├── special_renderers.go  # Special content renderer
│   │   │   └── toolrenderers.go      # Tool-specific renderers
│   │   └── theme/             # Color palette and styles
│   │       ├── theme.go       # Theme struct, Manager (dark/light/auto)
│   │       └── colors.go      # Named color constants
│   ├── workflow/              # Six-phase workflow engine
│   │   ├── engine.go          # Engine struct, RunPhase(), Transition()
│   │   ├── engine_parse.go    # LLM response parsing (JSON task extraction)
│   │   ├── engine_messages.go # Message types (PhaseResultMsg, etc.)
│   │   ├── engine_verify.go   # Task verification logic
│   │   ├── initialize.go      # Phase: project detection, git init
│   │   ├── discuss.go         # Phase: clarifying questions via LLM
│   │   ├── plan.go            # Phase: task list generation
│   │   ├── execute.go         # Phase: task execution with tool dispatch
│   │   ├── verify.go          # Phase: acceptance checks + self-heal + bisect
│   │   ├── ship.go            # Phase: final commit + ledger + archive
│   │   └── prompts/           # Embedded prompt templates (//go:embed)
│   │       ├── base.md        # System prompt foundation
│   │       ├── tool-use.md    # Tool usage instructions
│   │       ├── plan-format.md # Task list format specification
│   │       ├── execute-task.md # Task execution instructions
│   │       ├── discuss-questions.md # Question generation instructions
│   │       └── self-heal.md   # Self-healing instructions
│   └── types/                 # Shared core types (leaf package)
│       ├── types.go           # Message, Task, ToolCall, ModelInfo, etc.
│       └── constants.go       # All application constants
├── pkg/                       # Public/shared packages
│   ├── session/               # Session lifecycle and persistence
│   │   ├── session.go         # Session struct, NewSession(), validation
│   │   ├── manager.go         # Manager: CRUD, fork, archive, recent models
│   │   ├── planning.go        # Planning file I/O (PROJECT.md, TASKS.md, STATE.md)
│   │   ├── checkpoint.go      # Checkpoint save/load
│   │   └── session_info.go    # SessionInfo struct for listing
│   ├── taskrunner/            # Task scheduling and execution
│   │   └── runner.go          # Topological sort (Kahn's), sequential execution
│   ├── ledger/                # Cross-session learning ledger
│   │   └── ledger.go          # Append, query, stats, truncation
│   ├── bisect/                # Git bisect wrapper
│   │   ├── bisect.go          # Run(), parseBisectLog()
│   │   └── exec.go            # execCommand fallback
│   ├── rollback/              # Commit rollback chain
│   │   └── rollback.go        # Chain, SoftReset, HardReset, SafeReset
│   ├── arbitrage/             # Cost-aware model arbitrage
│   │   └── arbitrage.go       # Scorer, CompareModels, Recommend
│   ├── autodream/             # Context consolidation
│   │   └── autodream.go       # Consolidator, protected set, summary
│   └── keychain/              # OS keychain abstraction
│       ├── keychain.go        # Keychain interface
│       ├── keychain_linux.go  # secret-service via godbus
│       ├── keychain_darwin.go # macOS Keychain Services
│       ├── keychain_windows.go # Windows Credential Manager
│       └── errors.go          # Keychain-specific errors
├── docs/                      # Project documentation
│   ├── ARCHITECTURE.md        # Package dependency graph, data flow
│   ├── INTERFACES.md          # All Go interface definitions
│   ├── TYPES.md               # Type reference, constants, enums
│   └── (other docs)
├── adrenaline/                # Planning and roadmap
│   ├── ROADMAP.md             # Implementation roadmap (12 phases)
│   ├── idea.md                # Original product idea
│   └── REFERENCE.md           # Reference specification
├── .planning/                 # GSD planning artifacts
├── scripts/                   # Build and utility scripts
├── .github/                   # GitHub Actions CI/CD
├── go.mod                     # Go module definition (go 1.24)
├── go.sum                     # Dependency checksums
├── Makefile                   # Build targets (build, test, lint, clean)
├── .golangci.yml              # Linter configuration
├── .goreleaser.yaml           # Release automation config
├── AGENTS.md                  # Agent instructions (build, test, architecture rules)
├── CONTRIBUTING.md            # Contribution guidelines
├── README.md                  # Project overview
├── LICENSE                    # MIT License
├── CHANGELOG.md               # Release history
└── install.sh                 # Quick install script
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Binary entry point only — no business logic
- Contains: `main.go` (CLI parsing, config load, provider init, TUI launch), `usage.go` (help text)
- Key files: `main.go`

**`internal/config/`:**
- Purpose: TOML configuration parsing with env var overrides and hot-reload
- Contains: Config struct types, loader, env resolution, file watcher
- Key files: `types.go` (all config structs), `loader.go` (Load, DefaultConfig, WatchConfig)

**`internal/errors/`:**
- Purpose: Sentinel error definitions and user-friendly error messages
- Contains: All `var Err*` definitions, `UserMessage()` function
- Key files: `errors.go`

**`internal/git/`:**
- Purpose: Git operations wrapper for all git commands used by the application
- Contains: init, add, commit, log, diff, reset, stash, branch, status (porcelain parser)
- Key files: `git.go`

**`internal/log/`:**
- Purpose: Structured logger with file rotation
- Contains: slog-based logger, daily rotation, 7-day retention
- Key files: `log.go`

**`internal/provider/`:**
- Purpose: LLM provider abstraction layer with streaming support
- Contains: `LLMProvider` interface, registry, SSE parser, model cache, auto-fallback, reasoning normalization
- Key files: `interface.go`, `registry.go`, `sse.go`, `cache.go`, `fallback.go`

**`internal/provider/openrouter/` and `internal/provider/zen/`:**
- Purpose: Concrete LLM provider implementations
- Contains: HTTP client, model fetching, chat completion streaming, health checks
- Key files: `client.go`

**`internal/tokens/`:**
- Purpose: Token estimation for context budget enforcement
- Contains: tiktoken-go integration, character fallback, EMA calibration
- Key files: `estimator.go`

**`internal/tools/`:**
- Purpose: Tool implementations and permission-gated dispatcher
- Contains: Bash (PTY), FileRead, FileWrite, Glob, Grep, Edit, WebFetch, TodoWrite, AskUserQuestion
- Key files: `dispatcher.go`, `bash.go`, `fileread.go`, `filewrite.go`, `glob.go`, `grep.go`

**`internal/tui/`:**
- Purpose: Bubble Tea application — all screens, routing, message bus
- Contains: AppState, screen models, views, key bindings, command system
- Key files: `app.go`, `app_state.go`, `app_view.go`, `app_update.go`

**`internal/tui/components/`:**
- Purpose: Reusable TUI widgets
- Contains: Tool cards, thinking blocks, permission modal, badges, progress bars, sparklines
- Key files: `toolcard.go`, `thinking.go`, `permission.go`, `message.go`

**`internal/tui/theme/`:**
- Purpose: Color palette and theme definitions
- Contains: Dark/light/auto theme modes via lipgloss styles
- Key files: `theme.go`, `colors.go`

**`internal/workflow/`:**
- Purpose: Six-phase workflow engine
- Contains: Engine struct, phase implementations, embedded prompt templates
- Key files: `engine.go`, `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `ship.go`

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates for each workflow phase
- Contains: Markdown files loaded via `//go:embed`
- Key files: `base.md`, `tool-use.md`, `plan-format.md`, `execute-task.md`, `discuss-questions.md`, `self-heal.md`

**`internal/types/`:**
- Purpose: Shared core types (leaf package, zero internal imports)
- Contains: Message, Task, ToolCall, ModelInfo, WorkflowPhase, RiskLevel, constants
- Key files: `types.go`, `constants.go`

**`pkg/session/`:**
- Purpose: Session lifecycle management and file-based persistence
- Contains: Manager (CRUD, fork, archive), session structs, planning file I/O, checkpoints
- Key files: `manager.go`, `session.go`, `planning.go`, `checkpoint.go`

**`pkg/taskrunner/`:**
- Purpose: Task scheduling with dependency resolution
- Contains: Topological sort (Kahn's algorithm), sequential group execution
- Key files: `runner.go`

**`pkg/ledger/`:**
- Purpose: Cross-session learning ledger
- Contains: Append-only markdown file, filtered queries, aggregate stats
- Key files: `ledger.go`

**`pkg/bisect/`:**
- Purpose: Git bisect wrapper for automated regression detection
- Contains: `Run()` with check function, bisect log parsing
- Key files: `bisect.go`

**`pkg/rollback/`:**
- Purpose: Commit rollback chain with safety
- Contains: Soft/hard reset, backup branches, diff preview
- Key files: `rollback.go`

**`pkg/arbitrage/`:**
- Purpose: Cost-aware model arbitrage
- Contains: Complexity scoring, cost comparison, recommendation engine
- Key files: `arbitrage.go`

**`pkg/autodream/`:**
- Purpose: Context consolidation (AutoDream)
- Contains: Protected set logic, oldest 50% summarization, pause/resume
- Key files: `autodream.go`

**`pkg/keychain/`:**
- Purpose: OS keychain abstraction for API key storage
- Contains: Platform-specific implementations (Linux/macOS/Windows)
- Key files: `keychain.go`, `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point — CLI flags, config, providers, TUI launch
- `internal/tui/app.go:Init()`: Bubble Tea initialization — health check ticker, cache refresh, listener commands
- `internal/tui/app_state.go:NewApp()`: Application constructor — wires all components together

**Configuration:**
- `~/.m31a/config.toml`: User configuration file (TOML format)
- `internal/config/types.go`: All config struct definitions
- `internal/config/loader.go`: Config loading, env var resolution, file watcher
- `internal/types/constants.go`: All application constants

**Core Logic:**
- `internal/workflow/engine.go`: Workflow engine — `RunPhase()`, `Transition()`, `streamLLM()`
- `internal/tools/dispatcher.go`: Tool dispatcher — `Execute()`, permission gate
- `internal/provider/interface.go`: `LLMProvider` interface definition
- `internal/provider/registry.go`: Provider registry — `Register()`, `SetActive()`, `Get()`

**Testing:**
- `*_test.go` files co-located with source files
- `internal/tools/*_test.go`: Tool unit tests
- `internal/workflow/*_test.go`: Workflow phase tests
- `pkg/taskrunner/runner_test.go`: Task runner tests
- `pkg/ledger/ledger_test.go`: Ledger tests

## Naming Conventions

**Files:**
- Snake_case for all Go files: `app_state.go`, `repl_model.go`, `toolcard.go`
- Test files: `*_test.go` suffix (e.g., `dispatcher_test.go`)
- Screen-specific files: `{screen}_model.go`, `{screen}_view.go` pattern (e.g., `plan_model.go`, `plan_view.go`)

**Directories:**
- Lowercase, no separators: `internal/tui/`, `pkg/session/`
- Platform-specific: `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go`

**Types:**
- PascalCase for exported types: `AppState`, `ReplModel`, `LLMProvider`
- Interfaces: Verb-noun or noun pattern: `LLMProvider`, `Tool`, `PermissionGate`, `SchemaProvider`
- Enums: PascalCase type with `const` block: `WorkflowPhase`, `RiskLevel`, `TaskStatus`

**Functions:**
- PascalCase for exported: `NewEngine()`, `RunPhase()`, `Execute()`
- camelCase for unexported: `streamLLM()`, `buildToolDefinitions()`, `preflightContextCheck()`
- Phase handlers: `run{Phase}()` pattern (e.g., `runInitialize()`, `runDiscuss()`)

**Constants:**
- PascalCase for exported: `MaxFileSize`, `BashTimeout`, `PhaseExecute`
- SCREAMING_SNAKE for unexported or platform-specific: `DirPermission`, `FilePermission`

## Where to Add New Code

**New Workflow Phase:**
- Phase implementation: `internal/workflow/{phase}.go`
- Phase handler in engine: `internal/workflow/engine.go:RunPhase()` switch case
- Prompt template: `internal/workflow/prompts/{phase}.md`
- TUI screen model: `internal/tui/{phase}_model.go`
- TUI screen view: `internal/tui/{phase}_view.go`
- Screen constant: `internal/tui/types.go` (add to `Screen` enum)
- View routing: `internal/tui/app_view.go` switch case

**New Tool:**
- Tool implementation: `internal/tools/{tool}.go`
- Register in: `internal/tools/defaults.go`
- Risk level: define in tool's `RiskLevel()` method
- Schema: implement `SchemaProvider` interface if tool has parameters

**New TUI Screen:**
- Screen model: `internal/tui/{screen}_model.go`
- Screen view: `internal/tui/{screen}_view.go`
- Screen constant: `internal/tui/types.go` (add to `Screen` enum)
- AppState field: `internal/tui/app_state.go` (add model field)
- View routing: `internal/tui/app_view.go` switch case
- Command to access: `internal/tui/commands_{category}.go`

**New Provider:**
- Provider client: `internal/provider/{name}/client.go`
- Register in: `cmd/m31a/main.go` (add registration block)
- Config: `internal/config/types.go` (add credential config)
- Config file: `~/.m31a/config.toml` (add `[provider.{name}]` section)

**New Slash Command:**
- Command handler: `internal/tui/commands_{category}.go`
- Register in: `internal/tui/commands.go:DefaultCommands()`
- Route in: `internal/tui/app_update_slash.go`

**New Public Package:**
- Package location: `pkg/{name}/`
- Keep imports minimal: only `internal/types/` and `internal/errors/`
- Add tests: `pkg/{name}/{name}_test.go`

**New Config Field:**
- Config struct: `internal/config/types.go`
- Default value: `internal/config/loader.go:DefaultConfig()`
- Env var override: `internal/config/loader.go` (if applicable)
- Documentation: `AGENTS.md` and `docs/TYPES.md`

## Special Directories

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates for LLM instructions
- Generated: No (manually maintained)
- Committed: Yes
- Format: Markdown files loaded via `//go:embed prompts/*.md`

**`.planning/`:**
- Purpose: GSD planning artifacts (phases, plans, requirements)
- Generated: Yes (by GSD commands)
- Committed: Yes (version-controlled planning history)

**`adrenaline/`:**
- Purpose: Product planning, roadmap, reference specification
- Generated: No (manually maintained)
- Committed: Yes

**`docs/`:**
- Purpose: Project documentation (architecture, interfaces, types)
- Generated: No (manually maintained)
- Committed: Yes

**`scripts/`:**
- Purpose: Build and utility scripts
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-06-07*
