# Codebase Structure

**Analysis Date:** 2026-06-04

## Directory Layout

```
M31A/
├── cmd/m31a/              # Binary entry point — no logic, just wiring
│   └── main.go            # Config, providers, TUI launch
├── internal/              # Private application packages
│   ├── config/            # TOML config parsing + multi-layer merge
│   │   ├── loader.go      # Load(), DefaultConfig(), validation, env overrides
│   │   └── types.go       # Config, ProviderConfig, ModelConfig, UIConfig, etc.
│   ├── errors/            # Sentinel errors — zero internal imports
│   │   └── errors.go      # Err* vars + UserMessage() helper
│   ├── git/               # Git operations wrapper
│   │   └── git.go         # Init, Commit, Log, Diff, HeadHash, Branch, Bisect
│   ├── log/               # Structured slog logger
│   │   └── log.go         # NewLogger(), file rotation, daily rotation
│   ├── provider/          # LLM provider abstraction layer
│   │   ├── interface.go   # LLMProvider interface, ChatRequest, ToolDefinition
│   │   ├── registry.go    # Registry (thread-safe provider map)
│   │   ├── sse.go         # SSEParser (line-by-line SSE event parsing)
│   │   ├── cache.go       # ModelCache (TTL-based model catalog cache)
│   │   ├── fallback.go    # Auto-fallback on 429/503
│   │   ├── reasoning.go   # Per-model reasoning config (DeepSeek, OpenAI, Anthropic, Qwen)
│   │   ├── openrouter/    # OpenRouter API client
│   │   │   └── client.go  # FetchModels, ChatCompletionStream, HealthCheck
│   │   └── zen/           # OpenCode Zen API client
│   │       └── client.go  # Same interface, OpenAI-compatible endpoints
│   ├── tokens/            # Token estimation for context tracking
│   │   └── estimator.go   # tiktoken-go + rune fallback, EMA calibration
│   ├── tools/             # Tool implementations + dispatcher
│   │   ├── interface.go   # PermissionRequest, PermissionResponse, PermissionGate
│   │   ├── dispatcher.go  # Dispatcher: registry, permission gate, execution
│   │   ├── defaults.go    # DefaultDispatcher: registers all V1 tools
│   │   ├── permissions.go # checkPermission, glob matching, agent profiles
│   │   ├── bash.go        # Bash tool (PTY on Unix, pipe on Windows)
│   │   ├── bash_unix.go   # PTY allocation for Linux/macOS
│   │   ├── bash_windows.go# Plain pipe fallback for Windows
│   │   ├── fileread.go    # FileRead: encoding detection, size limit
│   │   ├── filewrite.go   # FileWrite: atomic write, backup, mkdir
│   │   ├── edit.go        # Edit: unified diff / search-replace
│   │   ├── glob.go        # Glob: recursive pattern matching via doublestar
│   │   ├── grep.go        # Grep: ripgrep or pure-Go fallback
│   │   ├── webfetch.go    # WebFetch: HTTP GET with SSRF protection
│   │   ├── todo.go        # TodoWrite: persistent task tracking
│   │   └── question.go    # AskUserQuestion: blocking question modal
│   ├── tui/               # Bubble Tea TUI application
│   │   ├── app.go         # AppState struct, NewApp(), Init(), RunPhaseCmd()
│   │   ├── app_update.go  # Update() — main message dispatch (1238 lines)
│   │   ├── app_view.go    # View() — screen routing + rendering
│   │   ├── app_workflow.go# initWorkflowEngine(), persistWorkflowState()
│   │   ├── types.go       # Screen enum, message types (PhaseResultMsg, etc.)
│   │   ├── repl.go        # ReplModel: main chat screen (~861 lines)
│   │   ├── repl_view.go   # REPL View() rendering
│   │   ├── repl_stream.go # Stream chunk handling, thinking blocks
│   │   ├── repl_thinking.go# Thinking block expand/collapse
│   │   ├── repl_commands.go# REPL slash command dispatch
│   │   ├── repl_quickactions.go# Quick action key bindings
│   │   ├── streaming.go   # StartStreamCmd(), StreamMsg/StreamDoneMsg
│   │   ├── commands.go    # CommandRegistry, CommandHandler, CommandContext
│   │   ├── commands_core.go# /help, /status, /clear, /quit
│   │   ├── commands_ai.go # /models, /compress, /optimize
│   │   ├── commands_session.go# /resume, /fork, /prev, /next
│   │   ├── commands_git.go# /git, /rollback
│   │   ├── commands_config.go# /settings, /theme, /config
│   │   ├── commands_workflow.go# /workflow, /plan, /execute
│   │   ├── settings.go    # SettingsModel: 6-tab inline config editor
│   │   ├── modelselector.go# ModelSelector: fuzzy search, provider filter
│   │   ├── modelselector_list.go# List filtering logic
│   │   ├── modelselector_view.go# Model selector rendering
│   │   ├── resume.go      # ResumeModel: session list browser
│   │   ├── firstrun.go    # FirstRunModel: API key setup wizard
│   │   ├── plan.go        # PlanModel: task list display + cost panel
│   │   ├── execute.go     # ExecuteModel: task progress tracker
│   │   ├── verify.go      # VerifyModel: pass/fail checklist
│   │   ├── ship.go        # ShipModel: summary banner
│   │   ├── diff.go        # DiffModel: git diff overlay
│   │   ├── health.go      # HealthCheckTicker(), adaptive interval
│   │   ├── header.go      # Header rendering with cache
│   │   ├── statusbar.go   # Status bar rendering
│   │   ├── sidebar.go     # SidebarModel: git status panel
│   │   ├── cmdpalette.go  # CommandPaletteModel: ctrl+p fuzzy search
│   │   ├── keybindings.go # KeyRegistry, leader key system
│   │   ├── keybindings_screens.go# Default key bindings per screen
│   │   ├── history.go     # FrecentHistory for command history
│   │   ├── providerbadge.go# Provider badge rendering ([OR]/[ZEN])
│   │   ├── spinner.go     # Spinner animation
│   │   ├── truncate.go    # Text truncation utilities
│   │   ├── cache_refresh.go# CacheRefreshTicker for model cache
│   │   ├── components/    # Reusable TUI components
│   │   │   ├── message.go # MessageRenderer: Glamour markdown, user/assistant bubbles
│   │   │   ├── toolcard.go# ToolCard: tool execution display with status badges
│   │   │   ├── thinking.go# ThinkingBlock: collapsible thinking display
│   │   │   ├── permission.go# PermissionModal: tool permission gate UI
│   │   │   ├── question.go# QuestionModal: AskUserQuestion UI
│   │   │   ├── progress.go# ProgressBar component
│   │   │   ├── badge.go   # Badge rendering utilities
│   │   │   ├── sparkline.go# Sparkline visualization
│   │   │   ├── metriccard.go# Metric card display
│   │   │   ├── statrow.go # Stat row layout
│   │   │   ├── filterchips.go# Filter chip components
│   │   │   ├── bash_renderer.go# Bash tool output renderer
│   │   │   ├── file_renderers.go# FileRead/FileWrite output renderers
│   │   │   ├── special_renderers.go# WebFetch/TodoWrite renderers
│   │   │   └── toolrenderers.go# Renderer registry per tool name
│   │   └── theme/         # Color palette and styles
│   │       ├── theme.go   # Theme struct, Manager, dark/light palettes
│   │       └── colors.go  # Color constants for both palettes
│   ├── workflow/          # Six-phase workflow engine
│   │   ├── engine.go      # Engine struct, RunPhase(), Transition(), parseToolCalls()
│   │   ├── initialize.go  # runInitialize: project detection, git init, PROJECT.md
│   │   ├── discuss.go     # runDiscuss: LLM questions, progressive streaming
│   │   ├── plan.go        # runPlan: task list generation, JSON validation, retries
│   │   ├── execute.go     # runExecute: task runner, tool dispatch, self-heal
│   │   ├── verify.go      # runVerify: acceptance checks, bisect on failure
│   │   ├── ship.go        # runShip: final commit, ledger update, archive
│   │   └── prompts/       # Embedded prompt templates
│   │       ├── base.md
│   │       ├── tool-use.md
│   │       ├── plan-format.md
│   │       ├── execute-task.md
│   │       ├── discuss-questions.md
│   │       ├── self-heal.md
│   │       └── verify-checklist.md
├── pkg/                   # Public reusable packages
│   ├── session/           # Session lifecycle and file persistence
│   │   ├── manager.go     # NewSession, SaveSession, LoadSession, ListSessions
│   │   ├── planning.go    # SaveProject, SaveTasks, SaveState (Markdown I/O)
│   │   ├── checkpoint.go  # SaveCheckpoint, LoadCheckpoints (max 2)
│   │   └── session_info.go# SessionInfo for listing
│   ├── keychain/          # OS-native secure key storage
│   │   ├── keychain.go    # Keychain interface + New() factory
│   │   ├── keychain_linux.go# D-Bus Secret Service + pass fallback
│   │   ├── keychain_darwin.go# /usr/bin/security CLI
│   │   └── keychain_windows.go# Stub (ErrNotImplemented)
│   ├── taskrunner/        # Task scheduling and execution
│   │   └── runner.go      # Topological sort (Kahn's), ExecuteGroup, Summary
│   ├── arbitrage/         # Model cost optimization
│   │   └── arbitrage.go   # Scorer, CostEstimator, ArbitrageRecommender
│   ├── bisect/            # Git bisect wrapper
│   │   └── bisect.go      # Run(good, bad, checkFn), auto-reset
│   ├── ledger/            # Cross-session learning ledger
│   │   └── ledger.go      # Append, Query, Stats, context injection
│   ├── rollback/          # Commit rollback chain
│   │   └── rollback.go    # Chain, Preview, SoftReset, HardReset, BackupBranch
│   └── autodream/         # Context consolidation
│       └── autodream.go   # Consolidate, Pause, Resume, Stats
├── docs/                  # Project documentation
│   ├── ARCHITECTURE.md    # Package dependency graph, data flow
│   ├── INTERFACES.md      # All Go interface/type definitions (LLM mirror)
│   ├── TYPES.md           # Constants, env vars, sentinel errors, enums
│   ├── CONFIG.md          # Config reference
│   └── SLASH_COMMANDS.md  # All slash commands
├── .planning/             # GSD planning directory
│   ├── codebase/          # Codebase analysis documents
│   ├── phases/            # Phase plans
│   ├── PROJECT.md         # Project overview
│   ├── REQUIREMENTS.md    # Requirements
│   ├── ROADMAP.md         # Implementation roadmap
│   └── STATE.md           # Current state
├── adrenaline/            # Design documents and roadmap
│   └── ROADMAP.md         # Detailed implementation roadmap
├── go.mod                 # Module: github.com/eshanized/M31A, Go 1.24
├── go.sum                 # Dependency checksums
├── Makefile               # Build, test, lint, cross-compile, release targets
├── .golangci.yml          # Linter configuration
├── .goreleaser.yaml       # Release automation
├── .gitignore             # Git ignore rules
├── LICENSE                # MIT license
├── AGENTS.md              # Agent instructions (architecture rules, package layout)
├── README.md              # Project README
├── CONTRIBUTING.md        # Contributing guide
└── CHANGELOG.md           # Version changelog
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Binary entry point — zero logic beyond wiring
- Contains: `main.go` (config load, provider init, TUI launch), `usage.go` (help text)
- Key files: `cmd/m31a/main.go`

**`internal/config/`:**
- Purpose: Multi-layer configuration (global TOML → env vars → project m31a.toml)
- Contains: Config struct definitions, TOML parsing, validation, variable substitution
- Key files: `internal/config/loader.go`, `internal/config/types.go`

**`internal/errors/`:**
- Purpose: Sentinel errors shared across all packages
- Contains: `Err*` vars, `UserMessage()` helper for user-friendly error strings
- Key files: `internal/errors/errors.go`

**`internal/types/`:**
- Purpose: Core domain types — the leaf package with zero internal imports
- Contains: `Message`, `Task`, `ToolCall`, `WorkflowPhase`, `ModelInfo`, `Session`, `StreamChunk`, constants
- Key files: `internal/types/types.go`, `internal/types/constants.go`

**`internal/provider/`:**
- Purpose: LLM API abstraction with streaming, caching, and auto-fallback
- Contains: `LLMProvider` interface, `Registry`, SSE parser, model cache, reasoning config, fallback logic
- Sub-packages: `openrouter/` (OpenRouter client), `zen/` (Zen client)
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/sse.go`, `internal/provider/cache.go`

**`internal/tui/`:**
- Purpose: Bubble Tea terminal UI — all screens, components, theme
- Contains: `AppState` (Bubble Tea model), screen implementations, reusable components, theme system
- Sub-packages: `components/` (UI primitives), `theme/` (color palettes)
- Key files: `internal/tui/app.go`, `internal/tui/repl.go`, `internal/tui/app_update.go`

**`internal/tools/`:**
- Purpose: Tool implementations and permission-gated dispatcher
- Contains: `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `TodoWrite`, `AskUserQuestion`
- Key files: `internal/tools/dispatcher.go`, `internal/tools/bash.go`, `internal/tools/defaults.go`

**`internal/workflow/`:**
- Purpose: Six-phase workflow engine with context pruning
- Contains: `Engine`, phase implementations, embedded prompt templates
- Sub-directories: `prompts/` (7 embedded markdown files)
- Key files: `internal/workflow/engine.go`, `internal/workflow/execute.go`

**`internal/git/`:**
- Purpose: Git operations wrapper for session management
- Contains: Init, Add, Commit, Log, Diff, HeadHash, Branch operations
- Key files: `internal/git/git.go`

**`internal/tokens/`:**
- Purpose: Token estimation and context usage tracking
- Contains: tiktoken-go for GPT/Claude, rune-based fallback, EMA calibration
- Key files: `internal/tokens/estimator.go`

**`internal/log/`:**
- Purpose: Structured logging to file only
- Contains: slog logger with daily rotation, 7-day retention
- Key files: `internal/log/log.go`

**`pkg/session/`:**
- Purpose: Session lifecycle — ID generation, file persistence, planning file I/O
- Contains: `Manager`, checkpoint system, PROJECT.md/TASKS.md/STATE.md reader/writer
- Key files: `pkg/session/manager.go`, `pkg/session/planning.go`, `pkg/session/checkpoint.go`

**`pkg/keychain/`:**
- Purpose: OS-native secure key storage
- Contains: Platform-specific implementations (Linux/macOS/Windows)
- Key files: `pkg/keychain/keychain.go`, `pkg/keychain/keychain_linux.go`

**`pkg/taskrunner/`:**
- Purpose: Task scheduling with dependency resolution
- Contains: Topological sort (Kahn's algorithm), sequential group execution
- Key files: `pkg/taskrunner/runner.go`

**`pkg/arbitrage/`:**
- Purpose: Model cost optimization engine
- Contains: Complexity scoring, cost estimation, arbitrage recommendations
- Key files: `pkg/arbitrage/arbitrage.go`

**`pkg/bisect/`:**
- Purpose: Git bisect wrapper for regression detection
- Contains: Automated bisect between session start and HEAD
- Key files: `pkg/bisect/bisect.go`

**`pkg/ledger/`:**
- Purpose: Cross-session learning ledger
- Contains: Append-only markdown file, aggregate stats, context injection
- Key files: `pkg/ledger/ledger.go`

**`pkg/rollback/`:**
- Purpose: Commit rollback with backup branches
- Contains: Chain view, soft/hard reset, diff preview
- Key files: `pkg/rollback/rollback.go`

**`pkg/autodream/`:**
- Purpose: Context consolidation when conversation grows large
- Contains: Summarize oldest 50% of messages, pause/resume/compress
- Key files: `pkg/autodream/autodream.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point — config, providers, TUI launch
- `internal/tui/app.go:124`: `NewApp()` — TUI application construction
- `internal/tui/app.go:498`: `Init()` — Bubble Tea initialization (health ticker, permission listener)
- `internal/workflow/engine.go:250`: `Engine.RunPhase()` — workflow phase execution

**Configuration:**
- `internal/config/types.go`: All config struct definitions (`Config`, `ProviderConfig`, `ModelConfig`, etc.)
- `internal/config/loader.go`: `Load()` — multi-layer config loading with validation
- `~/.m31a/config.toml`: Runtime config file (not in repo)
- `m31a.toml`: Project-level config (walked up from cwd, max 3 levels)

**Core Logic:**
- `internal/provider/interface.go`: `LLMProvider` interface definition
- `internal/tools/dispatcher.go`: Tool dispatcher with permission gate
- `internal/workflow/engine.go`: Workflow engine with phase orchestration
- `internal/tui/app_update.go`: Main `Update()` message dispatch (1238 lines)

**Testing:**
- `internal/workflow/engine_test.go`: Workflow engine tests
- `internal/tui/app_test.go`: TUI application tests
- `internal/tools/bash_test.go`: Bash tool tests
- `internal/provider/openrouter/client_test.go`: OpenRouter client tests

## Naming Conventions

**Files:**
- Snake_case for multi-word files: `app_update.go`, `repl_stream.go`, `bash_unix.go`
- Platform suffixes for OS-specific code: `bash_unix.go`, `bash_windows.go`, `keychain_linux.go`
- Test files: `*_test.go` suffix
- Interface files: `interface.go` for package interfaces

**Directories:**
- Lowercase with no separators: `internal/tui/`, `pkg/session/`
- Sub-packages for providers: `internal/provider/openrouter/`, `internal/provider/zen/`
- `components/` for reusable TUI primitives
- `prompts/` for embedded template files

**Types:**
- PascalCase for exported types: `AppState`, `ReplModel`, `Dispatcher`, `Engine`
- Interfaces: verb-noun pattern: `LLMProvider`, `Tool`, `Keychain`, `PermissionGate`
- Message types: `*Msg` suffix: `StreamMsg`, `PhaseResultMsg`, `PermissionRequestMsg`
- Error types: `Err*` prefix: `ErrProviderUnreachable`, `ErrRateLimited`

**Functions:**
- PascalCase for exported: `NewApp()`, `RunPhaseCmd()`, `StartStreamCmd()`
- camelCase for unexported: `buildSystemPrompt()`, `parseToolCalls()`, `validateTasks()`
- Factory functions: `New*` prefix: `NewEngine()`, `NewDispatcher()`, `NewEstimator()`

## Where to Add New Code

**New Tool:**
- Implementation: `internal/tools/<toolname>.go`
- Register in: `internal/tools/defaults.go` (`DefaultDispatcher`)
- Tool renderer: `internal/tui/components/toolrenderers.go`
- Tests: `internal/tools/<toolname>_test.go`

**New Slash Command:**
- Handler: `internal/tui/commands_<category>.go` (e.g., `commands_ai.go`)
- Register in: `internal/tui/commands.go` (`DefaultCommands()`)
- Tests: `internal/tui/commands_test.go`

**New Workflow Phase:**
- Implementation: `internal/workflow/<phase>.go`
- Add to: `Engine.RunPhase()` switch in `internal/workflow/engine.go`
- Prompt template: `internal/workflow/prompts/<phase>.md`
- Update `validPhaseTransitions` map in `internal/workflow/engine.go`
- TUI screen: `internal/tui/<phase>.go`

**New Provider:**
- Client: `internal/provider/<name>/client.go`
- Register in: `cmd/m31a/main.go` (provider creation block)
- Add to: `internal/provider/fallback.go` (fallback logic)

**New pkg/ Package:**
- Location: `pkg/<name>/`
- Must import only `internal/types/` and/or `internal/errors/` (not other internal packages)

**New TUI Screen:**
- Screen model: `internal/tui/<screen>.go`
- Add to: `Screen` enum in `internal/tui/types.go`
- Add to: `Update()` dispatch in `internal/tui/app_update.go`
- Add to: `View()` dispatch in `internal/tui/app_view.go`

**New TUI Component:**
- Location: `internal/tui/components/<component>.go`
- Must be self-contained (no AppState dependency)
- Theme-aware: accept `theme.Theme` in constructor

## Special Directories

**`.planning/`:**
- Purpose: GSD workflow state — phases, plans, requirements, codebase analysis
- Generated: Yes (by GSD commands)
- Committed: Yes (for cross-session continuity)

**`.planning/codebase/`:**
- Purpose: Codebase analysis documents (ARCHITECTURE.md, STRUCTURE.md, etc.)
- Generated: Yes (by `/gsd-map-codebase`)
- Committed: Yes

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates for workflow phases
- Generated: No (manually authored)
- Committed: Yes
- Embedded via: `//go:embed prompts/*.md` in `engine.go`

**`adrenaline/`:**
- Purpose: Design documents, roadmap, reference materials
- Generated: No
- Committed: Yes

**`docs/`:**
- Purpose: Project documentation (architecture, interfaces, types, config)
- Generated: No (manually maintained)
- Committed: Yes

---

*Structure analysis: 2026-06-04*
