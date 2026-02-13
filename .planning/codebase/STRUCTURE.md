# Codebase Structure

**Analysis Date:** 2026-06-11

## Directory Layout

```
M31A/
├── cmd/                          # Binary entry points
│   ├── m31a/                     # Main binary: init, provider setup, TUI launch
│   │   ├── main.go               # Entry point: run(), CLI flags, dependency wiring
│   │   └── usage.go              # CLI help/usage formatting
│   └── firstrunpreview/          # Dev tool: first-run screen preview
│       └── main.go
├── internal/                     # Private application code
│   ├── config/                   # Configuration loading and validation
│   │   ├── loader.go             # Multi-layer TOML loading, env vars, validation
│   │   ├── loader_test.go
│   │   └── types.go              # Config struct definitions
│   ├── errors/                   # Sentinel errors
│   │   ├── errors.go             # All sentinel errors + UserMessage()
│   │   └── errors_test.go
│   ├── fileutil/                 # File utilities
│   │   └── atomic.go             # AtomicWrite (temp + rename)
│   ├── git/                      # Git operations wrapper
│   │   ├── git.go                # Git struct with all git operations
│   │   └── git_test.go
│   ├── log/                      # Structured logging
│   │   ├── log.go                # slog with file rotation
│   │   └── log_test.go
│   ├── provider/                 # LLM provider abstraction
│   │   ├── interface.go          # LLMProvider interface, ChatRequest
│   │   ├── registry.go           # Thread-safe provider registry
│   │   ├── cache.go              # Model cache with TTL and singleflight
│   │   ├── cache_refresh_test.go
│   │   ├── cache_test.go
│   │   ├── common.go             # Shared HTTP helpers, error sanitization
│   │   ├── common_test.go
│   │   ├── capabilities.go       # Model capability detection
│   │   ├── fallback.go           # Auto-fallback logic
│   │   ├── resilience_test.go
│   │   ├── reasoning.go          # Reasoning/thinking parameter handling
│   │   ├── reasoning_test.go
│   │   ├── sse.go                # SSE stream parser
│   │   ├── sse_test.go
│   │   ├── openrouter/           # OpenRouter client implementation
│   │   └── zen/                  # Zen client implementation
│   ├── tokens/                   # Token estimation
│   │   ├── estimator.go          # tiktoken-go + rune fallback + EMA
│   │   ├── estimator_test.go
│   │   └── context_warning_test.go
│   ├── tools/                    # Tool implementations + dispatcher
│   │   ├── interface.go          # PermissionRequest, PermissionResponse
│   │   ├── dispatcher.go         # Tool registry, execution, permission routing
│   │   ├── dispatcher_test.go
│   │   ├── permissions.go        # Permission gate, rule matching
│   │   ├── permissions_test.go
│   │   ├── permission_timeout_test.go
│   │   ├── bash.go               # Bash tool (PTY on Linux/macOS)
│   │   ├── bash_unix.go
│   │   ├── bash_windows.go
│   │   ├── bash_test.go
│   │   ├── bash_security_test.go
│   │   ├── bash_kill_test.go
│   │   ├── fileread.go           # FileRead tool
│   │   ├── fileread_test.go
│   │   ├── filewrite.go          # FileWrite tool (atomic writes)
│   │   ├── filewrite_test.go
│   │   ├── glob.go               # Glob tool (doublestar)
│   │   ├── glob_test.go
│   │   ├── grep.go               # Grep tool (rg or pure-Go fallback)
│   │   ├── grep_test.go
│   │   ├── grep_truncation_test.go
│   │   ├── webfetch.go           # WebFetch tool (SSRF protection)
│   │   ├── webfetch_test.go
│   │   ├── webfetch_security_test.go
│   │   ├── edit.go               # Edit tool (search/replace)
│   │   ├── question.go           # AskUserQuestion tool
│   │   ├── todo.go               # TodoWrite tool
│   │   ├── filelist.go           # FileList tool
│   │   ├── filedelete.go         # FileDelete tool
│   │   ├── filemove.go           # FileMove tool
│   │   ├── constants.go          # Tool constants
│   │   ├── defaults.go           # Default dispatcher setup
│   │   ├── tools_test.go
│   │   └── toolinput_test.go
│   ├── tui/                      # Bubble Tea TUI application
│   │   ├── app.go                # Init(), Shutdown(), RunPhaseCmd()
│   │   ├── app_state.go          # AppState struct, NewApp()
│   │   ├── app_update.go         # Update() message routing
│   │   ├── app_update_commands.go # Slash command handling in Update()
│   │   ├── app_update_phase.go   # Phase transition handling
│   │   ├── app_view.go           # View() dispatch to active screen
│   │   ├── app_channel.go        # Channel-based message emitter
│   │   ├── types.go              # Screen enum, message types
│   │   ├── constants.go          # TUI constants
│   │   ├── commands.go           # CommandRegistry, DefaultCommands()
│   │   ├── commands_core.go      # /help, /clear, /status, etc.
│   │   ├── commands_config.go    # /settings, /config, /theme
│   │   ├── commands_git.go       # /diff, /rollback, /bisect
│   │   ├── commands_session.go   # /sessions, /export, /fork
│   │   ├── commands_workflow.go  # /new, /phase, /plan, /execute
│   │   ├── commands_ai.go        # /compress, /optimize, /model
│   │   ├── commands_config_diskusage_unix.go
│   │   ├── commands_config_diskusage_windows.go
│   │   ├── repl_model.go         # REPL screen model
│   │   ├── repl_view.go          # REPL screen view
│   │   ├── repl_state.go         # REPL state management
│   │   ├── repl_keys.go          # REPL key bindings
│   │   ├── repl_stream.go        # REPL streaming renderer
│   │   ├── repl_thinking.go      # REPL thinking block rendering
│   │   ├── repl_commands.go      # REPL slash command handling
│   │   ├── repl_quickactions.go  # REPL quick action buttons
│   │   ├── repl_welcome.go       # REPL welcome message
│   │   ├── repl_clipboard.go     # REPL clipboard integration
│   │   ├── repl.go               # REPL helpers
│   │   ├── plan_model.go         # Plan screen model
│   │   ├── plan_view.go          # Plan screen view
│   │   ├── plan_refine.go        # Plan refinement logic
│   │   ├── execute_model.go      # Execute screen model
│   │   ├── execute_view.go       # Execute screen view
│   │   ├── verify.go             # Verify screen
│   │   ├── ship_model.go         # Ship screen model
│   │   ├── ship_view.go          # Ship screen view
│   │   ├── settings_model.go     # Settings screen model
│   │   ├── settings_view.go      # Settings screen view
│   │   ├── settings_tabs.go      # Settings tab navigation
│   │   ├── settings_edit.go      # Settings field editing
│   │   ├── resume_model.go       # Resume screen model
│   │   ├── resume_view.go        # Resume screen view
│   │   ├── modelselector.go      # Model selector screen
│   │   ├── modelselector_list.go # Model selector list
│   │   ├── modelselector_view.go # Model selector view
│   │   ├── firstrun_model.go     # First-run wizard model
│   │   ├── firstrun_view.go      # First-run wizard view
│   │   ├── goalinput.go          # Goal input screen
│   │   ├── discuss.go            # Discuss Q&A screen
│   │   ├── diff_model.go         # Diff viewer model
│   │   ├── diff_view.go          # Diff viewer view
│   │   ├── ledger.go             # Ledger screen
│   │   ├── rollback.go           # Rollback screen
│   │   ├── metrics.go            # Metrics screen
│   │   ├── config_model.go       # Config viewer model
│   │   ├── bisect_model.go       # Bisect screen model
│   │   ├── dashboard_model.go    # Dashboard screen model
│   │   ├── sessiondetail_model.go # Session detail model
│   │   ├── fileexplorer_model.go # File explorer model
│   │   ├── notification_model.go # Notification center model
│   │   ├── tooldetail_model.go   # Tool detail model
│   │   ├── themepicker_model.go  # Theme picker model
│   │   ├── phasemodelpicker.go   # Phase model picker
│   │   ├── phasemodelpicker_view.go
│   │   ├── cmdpalette.go         # Command palette
│   │   ├── sidebar.go            # Sidebar component
│   │   ├── statusbar.go          # Status bar component
│   │   ├── header.go             # Header component
│   │   ├── health.go             # Health check ticker
│   │   ├── help.go               # Help overlay
│   │   ├── toast.go              # Toast notifications
│   │   ├── transition.go         # Screen transitions
│   │   ├── streaming.go          # Streaming helpers
│   │   ├── history.go            # Command history
│   │   ├── mention.go            # Mention/autocomplete
│   │   ├── mention_view.go
│   │   ├── truncate.go           # Text truncation helpers
│   │   ├── helpers.go            # TUI utility functions
│   │   ├── keybindings.go        # Global key bindings
│   │   ├── keybindings_screens.go # Per-screen key bindings
│   │   ├── provider_registration.go # Provider registration helper
│   │   ├── providerbadge.go      # Provider badge component
│   │   ├── cache_refresh.go      # Cache refresh logic
│   │   ├── components/           # Reusable TUI components
│   │   │   ├── badge.go          # Badge component
│   │   │   ├── bash_renderer.go  # Bash output renderer
│   │   │   ├── breadcrumb.go     # Breadcrumb navigation
│   │   │   ├── card.go           # Card component
│   │   │   ├── codeblock.go      # Code block renderer
│   │   │   ├── confirm.go        # Confirmation dialog
│   │   │   ├── datatable.go      # Data table
│   │   │   ├── divider.go        # Divider line
│   │   │   ├── dropdown.go       # Dropdown menu
│   │   │   ├── file_renderers.go # File content renderers
│   │   │   ├── filetree.go       # File tree component
│   │   │   ├── filterchips.go    # Filter chips
│   │   │   ├── logo.go           # Logo component
│   │   │   ├── message.go        # Message bubble
│   │   │   ├── message_test.go
│   │   │   ├── metriccard.go     # Metric card
│   │   │   ├── notification_list.go # Notification list
│   │   │   ├── permission.go     # Permission modal
│   │   │   ├── permission_test.go
│   │   │   ├── progress.go       # Progress bar
│   │   │   ├── question.go       # Question modal
│   │   │   ├── search.go         # Search input
│   │   │   ├── sparkline.go      # Sparkline chart
│   │   │   ├── sparkline_test.go
│   │   │   ├── special_renderers.go # Special content renderers
│   │   │   ├── spinner.go        # Spinner animation
│   │   │   ├── splitpane.go      # Split pane layout
│   │   │   ├── starfield.go      # Starfield animation
│   │   │   ├── starfield_test.go
│   │   │   ├── statrow.go        # Stat row
│   │   │   ├── tabbar.go         # Tab bar
│   │   │   ├── taskgraph.go      # Task dependency graph
│   │   │   ├── thinking.go       # Thinking block
│   │   │   ├── thinking_test.go
│   │   │   ├── timeline.go       # Timeline
│   │   │   ├── toolcard.go       # Tool card
│   │   │   ├── toolcard_test.go
│   │   │   ├── toolrenderers.go  # Tool output renderers
│   │   │   ├── truncate.go       # Truncation
│   │   │   └── workflow_phasebar.go # Workflow phase bar
│   │   ├── layout/               # Layout utilities
│   │   │   ├── minscreen.go      # Minimum screen size
│   │   │   ├── page.go           # Page layout
│   │   │   ├── page_test.go
│   │   │   └── responsive.go     # Responsive breakpoints
│   │   └── theme/                # Theme system
│   │       ├── theme.go          # Theme struct, Mode enum
│   │       ├── theme_test.go
│   │       ├── colors.go         # Color palette definitions
│   │       ├── borders.go        # Border styles
│   │       ├── registry.go       # Theme registry
│   │       ├── shadow.go         # Shadow rendering
│   │       ├── tabs.go           # Tab styles
│   │       └── unicode.go        # Unicode box-drawing chars
│   └── workflow/                 # Six-phase workflow engine
│       ├── engine.go             # Engine struct, RunPhase(), Transition()
│       ├── engine_messages.go    # MsgEmitter, PhaseResult, message types
│       ├── engine_parse.go       # Response parsing
│       ├── engine_parse_test.go
│       ├── engine_verify.go      # Verification logic
│       ├── engine_test.go
│       ├── initialize.go         # Initialize phase
│       ├── initialize_test.go
│       ├── discuss.go            # Discuss phase
│       ├── discuss_test.go
│       ├── plan.go               # Plan phase
│       ├── plan_test.go
│       ├── plan_parser.go        # Plan response parser
│       ├── execute.go            # Execute phase
│       ├── execute_test.go
│       ├── verify.go             # Verify phase
│       ├── verify_test.go
│       ├── ship.go               # Ship phase
│       ├── ship_test.go
│       ├── integration_test.go   # Full workflow integration test
│       ├── workflow_test.go
│       ├── phase_transition_test.go
│       ├── self_heal_visibility_test.go
│       ├── streaming_progress_test.go
│       ├── intermediate_progress_test.go
│       ├── thinking_indicator_test.go
│       ├── parse_tool_calls_test.go
│       └── prompts/              # Embedded prompt templates
│           ├── base.md
│           ├── tool-use.md
│           ├── plan-format.md
│           ├── execute-task.md
│           ├── discuss-questions.md
│           ├── self-heal.md
│           └── demonstration-format.md
├── pkg/                          # Public packages (reusable)
│   ├── autodream/                # Context consolidation
│   │   ├── autodream.go          # Consolidator struct
│   │   ├── autodream_test.go
│   │   └── doc.go
│   ├── ledger/                   # Cross-session learning
│   │   ├── ledger.go             # Ledger struct, entries, stats
│   │   ├── ledger_test.go
│   │   └── doc.go
│   ├── rollback/                 # Git commit rollback
│   │   ├── rollback.go           # Rollback struct, chain, reset
│   │   ├── rollback_test.go
│   │   └── doc.go
│   ├── arbitrage/                # Cost-aware model selection
│   │   ├── arbitrage.go          # Scorer, Recommend(), CompareModels()
│   │   ├── arbitrage_test.go
│   │   └── doc.go
│   ├── bisect/                   # Git bisect wrapper
│   │   ├── bisect.go             # Bisect struct, Run()
│   │   ├── bisect_test.go
│   │   ├── exec.go               # exec-based git fallback
│   │   └── doc.go
│   ├── taskrunner/               # Task scheduling
│   │   ├── runner.go             # Runner struct, Schedule(), ExecuteGroup()
│   │   ├── runner_test.go
│   │   └── doc.go
│   ├── session/                  # Session management
│   │   ├── session.go            # Session struct, NewSession()
│   │   ├── session_test.go
│   │   ├── session_resumedat_test.go
│   │   ├── manager.go            # Manager struct, CRUD operations
│   │   ├── manager_test.go
│   │   ├── checkpoint.go         # Checkpoint save/load
│   │   ├── checkpoint_test.go
│   │   ├── planning.go           # Planning file read/write
│   │   ├── planning_test.go
│   │   ├── session_info.go       # SessionInfo struct
│   │   └── doc.go
│   └── keychain/                 # OS keychain integration
│       ├── keychain.go           # Keychain interface
│       ├── keychain_linux.go     # Linux secret-service
│       ├── keychain_darwin.go    # macOS Keychain
│       ├── keychain_windows.go   # Windows Credential Manager
│       ├── keychain_test.go
│       └── errors.go
├── docs/                         # Documentation
│   ├── ARCHITECTURE.md           # Package dependency graph, data flow
│   ├── INTERFACES.md             # Interface/type definitions mirror
│   ├── TYPES.md                  # Constants, errors, enums reference
│   └── ROADMAP.md                # Implementation roadmap
├── adrenaline/                   # Planning documents
│   ├── ROADMAP.md                # Master roadmap
│   ├── REFERENCE.md              # Feature reference
│   └── idea.md                   # Original idea
├── .github/                      # GitHub Actions
├── images/                       # Documentation images
├── scripts/                      # Build/release scripts
├── .planning/                    # GSD planning directory
│   └── codebase/                 # Codebase analysis documents
├── go.mod                        # Go module definition
├── go.sum                        # Dependency checksums
├── Makefile                      # Build targets
├── .golangci.yml                 # Linter configuration
├── .goreleaser.yaml              # Release configuration
├── AGENTS.md                     # Agent instructions
├── README.md                     # Project README
├── CONTRIBUTING.md               # Contribution guidelines
├── CHANGELOG.md                  # Version changelog
├── LICENSE                       # MIT license
├── install.sh                    # Install script
├── opencode.json                 # OpenCode configuration
└── .gitignore                    # Git ignore rules
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Binary entry point and CLI handling
- Contains: `main.go` (entry point, dependency wiring), `usage.go` (CLI help formatting)
- Key files: `cmd/m31a/main.go`

**`internal/config/`:**
- Purpose: Multi-layer configuration loading and validation
- Contains: TOML parsing, env var overrides, project config discovery, ${VAR} substitution
- Key files: `internal/config/loader.go`, `internal/config/types.go`

**`internal/errors/`:**
- Purpose: Sentinel error definitions and user-friendly messages
- Contains: All sentinel errors (ErrProviderUnreachable, ErrRateLimited, etc.), UserMessage() function
- Key files: `internal/errors/errors.go`

**`internal/fileutil/`:**
- Purpose: File system utilities
- Contains: AtomicWrite (crash-safe temp+rename writes)
- Key files: `internal/fileutil/atomic.go`

**`internal/git/`:**
- Purpose: Git operations wrapper
- Contains: All git commands (add, commit, diff, log, status, branch, etc.)
- Key files: `internal/git/git.go`

**`internal/log/`:**
- Purpose: Structured logging with file rotation
- Contains: slog setup, daily rotation, 7-day retention
- Key files: `internal/log/log.go`

**`internal/provider/`:**
- Purpose: LLM provider abstraction and implementations
- Contains: LLMProvider interface, Registry, ModelCache, SSE parser, fallback logic, OpenRouter/Zen clients
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/cache.go`

**`internal/tokens/`:**
- Purpose: Token estimation and context usage
- Contains: tiktoken-go integration, rune fallback, EMA calibration, context warning
- Key files: `internal/tokens/estimator.go`

**`internal/tools/`:**
- Purpose: Tool implementations and execution dispatch
- Contains: Dispatcher, PermissionGate, Bash, FileRead, FileWrite, Glob, Grep, WebFetch, Edit, Question, TodoWrite
- Key files: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`

**`internal/tui/`:**
- Purpose: Bubble Tea TUI application
- Contains: 26+ screen models, 40+ components, theme system, command registry, layout utilities
- Key files: `internal/tui/app.go`, `internal/tui/app_state.go`, `internal/tui/app_update.go`, `internal/tui/app_view.go`

**`internal/tui/components/`:**
- Purpose: Reusable TUI components
- Contains: badge, card, codeblock, confirm, datatable, filetree, message, permission, progress, question, search, sparkline, spinner, splitpane, tabbar, taskgraph, thinking, timeline, toolcard
- Key files: `internal/tui/components/permission.go`, `internal/tui/components/toolcard.go`

**`internal/tui/layout/`:**
- Purpose: Layout utilities
- Contains: MinimumScreen, Page layout, responsive breakpoints
- Key files: `internal/tui/layout/page.go`, `internal/tui/layout/responsive.go`

**`internal/tui/theme/`:**
- Purpose: Theme system
- Contains: Theme struct, color palettes, border styles, shadow rendering, Unicode helpers
- Key files: `internal/tui/theme/theme.go`, `internal/tui/theme/colors.go`

**`internal/workflow/`:**
- Purpose: Six-phase workflow engine
- Contains: Engine struct, phase implementations (Initialize, Discuss, Plan, Execute, Verify, Ship), prompt templates
- Key files: `internal/workflow/engine.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates
- Contains: base.md, tool-use.md, plan-format.md, execute-task.md, discuss-questions.md, self-heal.md, demonstration-format.md
- Key files: `internal/workflow/prompts/base.md`

**`pkg/autodream/`:**
- Purpose: Context consolidation
- Contains: Consolidator struct, message compression, pause/resume, protected message handling
- Key files: `pkg/autodream/autodream.go`

**`pkg/ledger/`:**
- Purpose: Cross-session learning
- Contains: Ledger struct, entries, stats, filtering, atomic writes
- Key files: `pkg/ledger/ledger.go`

**`pkg/rollback/`:**
- Purpose: Git commit rollback
- Contains: Rollback struct, chain browsing, soft/hard reset, stash handling
- Key files: `pkg/rollback/rollback.go`

**`pkg/arbitrage/`:**
- Purpose: Cost-aware model selection
- Contains: Scorer, complexity classification, cost comparison, recommendations
- Key files: `pkg/arbitrage/arbitrage.go`

**`pkg/bisect/`:**
- Purpose: Git bisect wrapper
- Contains: Bisect struct, Run() with check function, log parsing
- Key files: `pkg/bisect/bisect.go`

**`pkg/taskrunner/`:**
- Purpose: Task scheduling
- Contains: Runner struct, topological sort (Kahn's algorithm), sequential execution, retry support
- Key files: `pkg/taskrunner/runner.go`

**`pkg/session/`:**
- Purpose: Session management
- Contains: Manager struct, Session struct, CRUD operations, fork, archive, planning file I/O
- Key files: `pkg/session/manager.go`, `pkg/session/session.go`

**`pkg/keychain/`:**
- Purpose: OS keychain integration
- Contains: Keychain interface, platform-specific implementations (Linux, macOS, Windows)
- Key files: `pkg/keychain/keychain.go`, `pkg/keychain/keychain_linux.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point, dependency wiring, signal handling
- `internal/tui/app.go`: TUI Init(), Shutdown(), RunPhaseCmd()
- `internal/workflow/engine.go`: Workflow engine RunPhase(), Transition()

**Configuration:**
- `~/.m31a/config.toml`: User configuration (TOML)
- `internal/config/loader.go`: Config loading, validation, env var overrides
- `internal/config/types.go`: Config struct definitions
- `go.mod`: Go module definition and dependencies

**Core Logic:**
- `internal/workflow/engine.go`: Six-phase workflow orchestration
- `internal/workflow/plan.go`: Plan phase (task generation)
- `internal/workflow/execute.go`: Execute phase (task execution)
- `internal/tools/dispatcher.go`: Tool registration and execution
- `internal/tools/permissions.go`: Permission gate logic
- `internal/provider/interface.go`: LLMProvider interface
- `internal/provider/registry.go`: Provider registry
- `internal/provider/cache.go`: Model cache with TTL

**State Persistence:**
- `pkg/session/manager.go`: Session CRUD operations
- `pkg/session/session.go`: Session struct definition
- `pkg/session/checkpoint.go`: Checkpoint save/load
- `pkg/session/planning.go`: Planning file I/O
- `internal/fileutil/atomic.go`: Crash-safe atomic writes

**Testing:**
- `*_test.go` files co-located with source files
- `internal/workflow/integration_test.go`: Full workflow integration test
- `internal/tui/components/*_test.go`: Component unit tests

## Naming Conventions

**Files:**
- Go source: `snake_case.go` (e.g., `app_state.go`, `repl_model.go`)
- Tests: `*_test.go` co-located with source (e.g., `engine_test.go`)
- Platform-specific: `*_unix.go`, `*_windows.go` (e.g., `bash_unix.go`)
- Models/screens: `<screen>_model.go`, `<screen>_view.go` (e.g., `plan_model.go`, `plan_view.go`)
- Commands: `commands_<category>.go` (e.g., `commands_core.go`, `commands_git.go`)

**Directories:**
- Internal packages: `lowercase` (e.g., `config`, `provider`, `workflow`)
- Public packages: `lowercase` (e.g., `session`, `keychain`, `arbitrage`)
- Components: `lowercase` (e.g., `components`, `theme`, `layout`)
- Tests: Co-located in same directory

**Functions:**
- Exported: `PascalCase` (e.g., `NewEngine`, `RunPhase`, `ChatCompletionStream`)
- Unexported: `camelCase` (e.g., `runInitialize`, `buildSystemPrompt`)
- Methods: `PascalCase` on exported types, `camelCase` on unexported types
- Message types: `PascalCaseMsg` suffix (e.g., `PhaseResultMsg`, `PermissionRequestMsg`)

**Types:**
- Structs: `PascalCase` (e.g., `AppState`, `Engine`, `Registry`)
- Interfaces: `PascalCase` (e.g., `LLMProvider`, `Tool`, `PermissionGate`)
- Enums: `PascalCase` with `Phase`/`Status`/`Risk` prefix (e.g., `WorkflowPhase`, `TaskStatus`, `RiskLevel`)
- Constants: `PascalCase` (e.g., `PhaseInitialize`, `StatusPending`, `RiskDangerous`)

## Where to Add New Code

**New Workflow Phase:**
- Implementation: `internal/workflow/<phase>.go`
- Phase constant: `internal/types/types.go` (add to `WorkflowPhase` enum)
- Phase transition: `internal/workflow/engine.go` (add to `validPhaseTransitions`)
- Screen model: `internal/tui/<phase>_model.go`
- Screen view: `internal/tui/<phase>_view.go`
- Tests: `internal/workflow/<phase>_test.go`

**New Tool:**
- Implementation: `internal/tools/<tool>.go`
- Registration: `internal/tools/defaults.go` (add to `DefaultDispatcher`)
- Risk level: `internal/tools/<tool>.go` (implement `RiskLevel()` method)
- Schema: `internal/tools/<tool>.go` (implement `SchemaProvider` interface)
- Tests: `internal/tools/<tool>_test.go`

**New TUI Screen:**
- Model: `internal/tui/<screen>_model.go`
- View: `internal/tui/<screen>_view.go`
- Screen constant: `internal/tui/types.go` (add to `Screen` enum)
- Route: `internal/tui/app_update.go` (add case to `Update()`)
- Render: `internal/tui/app_view.go` (add case to `View()`)
- Slash command: `internal/tui/commands.go` (register in `DefaultCommands()`)

**New Supporting Package:**
- Location: `pkg/<package>/`
- Entry point: `pkg/<package>/<package>.go`
- Doc: `pkg/<package>/doc.go`
- Tests: `pkg/<package>/<package>_test.go`

**New Config Field:**
- Type: `internal/config/types.go` (add to appropriate config struct)
- Default: `internal/config/loader.go` (add to `DefaultConfig()`)
- Validation: `internal/config/loader.go` (add to `validateConfig()`)
- Env override: `internal/config/loader.go` (add to `Load()`)

## Special Directories

**`.planning/`:**
- Purpose: GSD workflow planning documents
- Contains: Phase plans, codebase analysis, decision records
- Generated: By GSD commands
- Committed: Yes

**`.github/`:**
- Purpose: GitHub Actions workflows
- Contains: CI/CD pipeline definitions
- Generated: Manually
- Committed: Yes

**`adrenaline/`:**
- Purpose: Original project planning and roadmap
- Contains: ROADMAP.md, REFERENCE.md, idea.md
- Generated: Manually
- Committed: Yes

**`docs/`:**
- Purpose: Project documentation
- Contains: ARCHITECTURE.md, INTERFACES.md, TYPES.md
- Generated: Manually
- Committed: Yes

---

*Structure analysis: 2026-06-11*
