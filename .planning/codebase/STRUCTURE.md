# Codebase Structure

**Analysis Date:** 2026-06-11

## Directory Layout

```
M31A/
├── cmd/
│   ├── m31a/                    # Binary entry point
│   │   ├── main.go              # CLI init, provider setup, TUI launch, signal handling
│   │   └── usage.go             # CLI help/usage printing
│   └── firstrunpreview/         # First-run preview utility
├── internal/                    # Private packages (Go convention)
│   ├── config/                  # TOML config parsing, env vars, keychain resolution
│   │   ├── loader.go            # Load(), Save(), DefaultConfig()
│   │   ├── types.go             # Config struct, all sub-configs, PermissionRule
│   │   └── loader_test.go
│   ├── errors/                  # Sentinel errors — zero internal imports
│   │   └── errors.go            # Err* vars + UserMessage() helper
│   ├── fileutil/                # Atomic file write utilities
│   │   ├── atomic.go            # WriteAtomic(), MkdirAllAtomic()
│   │   └── atomic_test.go
│   ├── git/                     # Git operations wrapper
│   │   ├── git.go               # New(), Status(), Diff(), Commit(), Log(), HeadHash()
│   │   └── git_test.go
│   ├── log/                     # Structured slog logger with daily rotation
│   │   └── (logger setup)
│   ├── provider/                # LLMProvider interface + implementations
│   │   ├── interface.go         # LLMProvider, ChatRequest, ToolDefinition
│   │   ├── registry.go          # Registry: Register, SetActive, Get, List
│   │   ├── openrouter/          # OpenRouter API client
│   │   │   ├── client.go        # OpenRouter struct, ChatCompletionStream, FetchModels
│   │   │   └── client_test.go
│   │   └── zen/                 # Zen gateway client
│   │       ├── client.go        # Zen struct, ChatCompletionStream, FetchModels
│   │       └── client_test.go
│   ├── tokens/                  # Token estimation for LLM cost tracking
│   │   └── (tiktoken-go wrapper + EMA calibration)
│   ├── tools/                   # Tool implementations + dispatcher
│   │   ├── interface.go         # PermissionRequest, PermissionResponse, Dispatcher methods
│   │   ├── dispatcher.go        # Tool registry, permission gate, rate limiter, Execute()
│   │   ├── defaults.go          # DefaultDispatcher() — registers all 12 tools
│   │   ├── permissions.go       # Permission rule evaluation, ask/skip/allow
│   │   ├── bash.go              # Bash tool (PTY on Linux/macOS)
│   │   ├── bash_unix.go         # Unix-specific PTY handling
│   │   ├── bash_windows.go      # Windows cmd.exe fallback
│   │   ├── fileread.go          # FileRead tool (5MB limit, binary detection)
│   │   ├── filewrite.go         # FileWrite tool (atomic write, backup)
│   │   ├── edit.go              # Edit tool (string replacement)
│   │   ├── glob.go              # Glob tool (doublestar ** patterns)
│   │   ├── grep.go              # Grep tool (ripgrep-style regex)
│   │   ├── webfetch.go          # WebFetch tool (HTTP fetch, SSRF protection)
│   │   ├── question.go          # AskUserQuestion tool (interactive prompt)
│   │   ├── todo.go              # TodoWrite tool (task list persistence)
│   │   ├── filelist.go          # FileList tool (directory listing)
│   │   ├── filedelete.go        # FileDelete tool (backup before delete)
│   │   ├── filemove.go          # FileMove tool (atomic rename)
│   │   └── constants.go         # Tool-specific constants
│   ├── tui/                     # Bubble Tea app — all screens and components
│   │   ├── app.go               # AppState.Init(), Shutdown(), RunPhaseCmd()
│   │   ├── app_state.go         # AppState struct (all state fields), NewApp()
│   │   ├── app_update.go        # Update() — central message dispatch (1800+ lines)
│   │   ├── app_view.go          # View() — screen rendering, chrome composition
│   │   ├── app_update_commands.go # Command routing (/help, /clear, etc.)
│   │   ├── app_update_phase.go  # Workflow phase result handling
│   │   ├── app_channel.go       # channelEmitter for workflow → TUI bridge
│   │   ├── types.go             # Screen enum, AppMsg, PhaseResultMsg, all tea.Msg types
│   │   ├── constants.go         # WidthUltraCompact=40, WidthCompact=60, WidthFull=80
│   │   ├── repl.go              # REPL keyboard handling, enter/stream routing
│   │   ├── repl_model.go        # ReplModel struct, messages, viewport, textarea
│   │   ├── repl_view.go         # REPL rendering (messages + input area)
│   │   ├── repl_stream.go       # LLM streaming: startChatStream, handleStreamMsg
│   │   ├── repl_thinking.go     # Thinking block display/toggle
│   │   ├── repl_keys.go         # REPL key bindings
│   │   ├── repl_commands.go     # Slash command handling (/model, /settings, etc.)
│   │   ├── repl_quickactions.go # Quick action bar
│   │   ├── repl_welcome.go      # Welcome screen when no messages
│   │   ├── repl_clipboard.go    # Copy to clipboard
│   │   ├── plan_model.go        # Plan review screen
│   │   ├── plan_view.go         # Plan rendering
│   │   ├── plan_refine.go       # Plan refinement feedback
│   │   ├── execute_model.go     # Task execution progress screen
│   │   ├── execute_view.go      # Execution progress rendering
│   │   ├── verify.go            # Verification results screen
│   │   ├── ship_model.go        # Ship summary screen
│   │   ├── ship_view.go         # Ship rendering
│   │   ├── discuss.go           # Discuss Q&A screen
│   │   ├── settings_model.go    # Settings editor (6 tabs)
│   │   ├── settings_view.go     # Settings rendering
│   │   ├── settings_edit.go     # Settings editing logic
│   │   ├── settings_tabs.go     # Settings tab navigation
│   │   ├── modelselector.go     # Model/provider picker
│   │   ├── modelselector_list.go # Model list rendering
│   │   ├── modelselector_view.go # Model selector rendering
│   │   ├── phasemodelpicker.go  # Dual-model picker (planning vs coding)
│   │   ├── phasemodelpicker_view.go # Phase model picker rendering
│   │   ├── firstrun_model.go    # First-run setup wizard
│   │   ├── firstrun_view.go     # First-run rendering
│   │   ├── goalinput.go         # Full-screen goal entry
│   │   ├── resume_model.go      # Session browser
│   │   ├── resume_view.go       # Session browser rendering
│   │   ├── dashboard_model.go   # Workflow pipeline overview
│   │   ├── bisect_model.go      # Git bisect interactive
│   │   ├── config_model.go      # Full config viewer
│   │   ├── diff_model.go        # Diff viewer
│   │   ├── diff_view.go         # Diff rendering
│   │   ├── ledger.go            # Learning ledger browser
│   │   ├── rollback.go          # Commit chain browser
│   │   ├── metrics.go           # Session analytics
│   │   ├── help.go              # Keybinding help overlay
│   │   ├── sidebar.go           # Git status sidebar
│   │   ├── header.go            # Header bar (brand, breadcrumb, model)
│   │   ├── statusbar.go         # Status bar (cwd, branch, cost)
│   │   ├── toast.go             # Toast notifications
│   │   ├── notification_model.go # Notification history
│   │   ├── sessiondetail_model.go # Session detail preview
│   │   ├── fileexplorer_model.go # File tree browser
│   │   ├── tooldetail_model.go  # Expandable tool output
│   │   ├── themepicker_model.go # Theme browser/preview
│   │   ├── mention.go           # @mention file context injection
│   │   ├── mention_view.go      # @mention popup rendering
│   │   ├── cmdpalette.go        # Command palette (ctrl+p)
│   │   ├── commands.go          # Command registry
│   │   ├── commands_core.go     # Core commands (new, clear, quit)
│   │   ├── commands_ai.go       # AI commands (compress, retry)
│   │   ├── commands_git.go      # Git commands (commit, diff, log)
│   │   ├── commands_session.go  # Session commands (resume, fork, export)
│   │   ├── commands_config.go   # Config commands (set, get, reset)
│   │   ├── commands_workflow.go # Workflow commands (plan, execute, verify, ship)
│   │   ├── keybindings.go       # Key registry, leader key system
│   │   ├── keybindings_screens.go # Per-screen key bindings
│   │   ├── provider_registration.go # RegisterProvider helper
│   │   ├── providerbadge.go     # Provider badge rendering
│   │   ├── streaming.go         # Streaming helpers
│   │   ├── cache_refresh.go     # Model cache refresh
│   │   ├── health.go            # Health check ticker
│   │   ├── transition.go        # Screen transition animations
│   │   ├── truncate.go          # Text truncation helpers
│   │   ├── helpers.go           # General UI helpers
│   │   ├── history.go           # Frecent history (fuzzy recent)
│   │   ├── metrics.go           # Metrics collection
│   │   ├── components/          # Reusable TUI components (40 files)
│   │   │   ├── badge.go         # Badge component
│   │   │   ├── bash_renderer.go # Bash output renderer
│   │   │   ├── breadcrumb.go    # Breadcrumb navigation
│   │   │   ├── card.go          # Card component
│   │   │   ├── codeblock.go     # Code block renderer
│   │   │   ├── confirm.go       # Confirmation dialog
│   │   │   ├── datatable.go     # Data table component
│   │   │   ├── divider.go       # Divider component
│   │   │   ├── dropdown.go      # Dropdown component
│   │   │   ├── file_renderers.go # File content renderers
│   │   │   ├── filetree.go      # File tree component
│   │   │   ├── filterchips.go   # Filter chips
│   │   │   ├── logo.go          # Logo animation
│   │   │   ├── message.go       # Message bubble renderer
│   │   │   ├── metriccard.go    # Metric card component
│   │   │   ├── notification_list.go # Notification list
│   │   │   ├── permission.go    # Permission modal
│   │   │   ├── progress.go      # Progress bar
│   │   │   ├── question.go      # Interactive question modal
│   │   │   ├── search.go        # Search component
│   │   │   ├── sparkline.go     # Sparkline chart
│   │   │   ├── special_renderers.go # Special content renderers
│   │   │   ├── spinner.go       # Spinner component
│   │   │   ├── splitpane.go     # Split pane layout
│   │   │   ├── starfield.go     # Starfield background
│   │   │   ├── statrow.go       # Stats row
│   │   │   ├── tabbar.go        # Tab bar component
│   │   │   ├── taskgraph.go     # Task dependency graph
│   │   │   ├── thinking.go      # Thinking block component
│   │   │   ├── timeline.go      # Timeline component
│   │   │   ├── toolcard.go      # Tool call card
│   │   │   ├── toolrenderers.go # Tool output renderers
│   │   │   ├── truncate.go      # Truncation helpers
│   │   │   └── workflow_phasebar.go # Workflow phase bar
│   │   ├── layout/              # Responsive layout system
│   │   │   ├── page.go          # PageChrome, RenderPage, BuildHeader, BuildFooter
│   │   │   ├── responsive.go    # Breakpoints, Detect(), ShowSidebar(), etc.
│   │   │   ├── minscreen.go     # TooNarrow rendering
│   │   │   └── page_test.go, responsive_test.go
│   │   └── theme/               # Color palette and theme definitions
│   │       ├── theme.go         # Theme struct, colors
│   │       ├── registry.go      # Theme registry
│   │       ├── colors.go        # Color definitions
│   │       ├── borders.go       # Border styles
│   │       ├── shadow.go        # Shadow styles
│   │       ├── tabs.go          # Tab styles
│   │       └── unicode.go       # Unicode decorations
│   ├── types/                   # Shared core types — leaf package (zero internal imports)
│   │   ├── types.go             # Message, Task, ToolCall, ModelInfo, Tool interface, etc.
│   │   ├── constants.go         # ModelCacheTTL, MaxFileSize, BashTimeout, etc.
│   │   ├── plan.go              # Plan-related types
│   │   ├── git.go               # Git-related types
│   │   └── types_test.go
│   └── workflow/                # Six-phase workflow engine
│       ├── engine.go            # Engine struct, NewEngine(), RunPhase(), Transition()
│       ├── engine_messages.go   # MsgEmitter interface, PhaseResult, all event message types
│       ├── engine_parse.go      # LLM response parsing
│       ├── engine_verify.go     # Task verification logic
│       ├── initialize.go        # Phase 1: session creation, project type detection
│       ├── discuss.go           # Phase 2: clarifying questions generation
│       ├── plan.go              # Phase 3: task list generation with dependencies
│       ├── plan_parser.go       # TASKS.md parser
│       ├── execute.go           # Phase 4: task execution with tool dispatch
│       ├── verify.go            # Phase 5: acceptance checks, self-heal loop
│       ├── ship.go              # Phase 6: final commit, archive, ledger update
│       └── prompts/             # Embedded prompt templates (go:embed)
│           ├── base.md          # System prompt base
│           ├── tool-use.md      # Tool usage instructions
│           ├── plan-format.md   # Plan output format
│           ├── execute-task.md  # Task execution instructions
│           ├── discuss-questions.md # Discussion format
│           ├── self-heal.md     # Self-heal instructions
│           └── demonstration-format.md # Demo output format
├── pkg/                         # Public packages (can be imported by external code)
│   ├── arbitrage/               # Model cost optimization
│   │   ├── arbitrage.go         # Recommend(), Scorer, complexity scoring
│   │   └── arbitrage_test.go
│   ├── autodream/               # Context consolidation
│   │   ├── autodream.go         # Consolidator, Consolidate(), threshold checks
│   │   └── autodream_test.go
│   ├── bisect/                  # Git bisect wrapper
│   │   └── (bisect logic)
│   ├── keychain/                # OS-specific keychain
│   │   └── (linux/darwin/windows implementations)
│   ├── ledger/                  # Cross-session learning ledger
│   │   ├── ledger.go            # Ledger, Load(), Save(), Append()
│   │   └── ledger_test.go
│   ├── rollback/                # Commit chain browser
│   │   ├── rollback.go          # Rollback, History(), Revert()
│   │   └── rollback_test.go
│   ├── session/                 # Session lifecycle and file persistence
│   │   ├── manager.go           # Manager, NewManager(), Create(), Load(), ListSessions()
│   │   ├── session.go           # Session struct, SessionID generation
│   │   ├── checkpoint.go        # Checkpoint save/load
│   │   ├── planning.go          # Planning file read/write (PROJECT.md, TASKS.md, STATE.md)
│   │   ├── session_info.go      # Session metadata
│   │   └── *_test.go
│   └── taskrunner/              # DAG-based task scheduler
│       ├── runner.go            # Runner, Schedule() (Kahn's algo), ExecuteGroup()
│       └── runner_test.go
├── docs/                        # Project documentation
│   ├── ARCHITECTURE.md          # Package dependency graph, data flow diagrams
│   ├── INTERFACES.md            # Go interface/type definitions mirror
│   └── TYPES.md                 # Constants, errors, enums reference
├── scripts/                     # Build/install scripts
├── images/                      # README images
├── dist/                        # Cross-compiled binaries (generated)
├── adrenaline/                  # (ancillary directory)
├── .planning/                   # GSD planning artifacts
│   └── codebase/                # Codebase analysis documents
├── go.mod                       # Go module definition
├── go.sum                       # Dependency checksums
├── Makefile                     # Build, test, lint, cross-compile targets
├── AGENTS.md                    # Agent instructions and architecture rules
├── CONTRIBUTING.md              # Contribution guidelines
├── CHANGELOG.md                 # Version history
├── README.md                    # Project overview
├── LICENSE                      # License file
├── install.sh                   # Installation script
└── opencode.json                # OpenCode configuration
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Binary entry point; all initialization logic
- Contains: `main.go` (CLI flags, config, providers, session, TUI launch), `usage.go` (help text)
- Key files: `cmd/m31a/main.go`

**`internal/config/`:**
- Purpose: Configuration parsing and management
- Contains: TOML loader, Config struct with nested sub-configs, default values
- Key files: `internal/config/loader.go`, `internal/config/types.go`

**`internal/errors/`:**
- Purpose: Sentinel error definitions for the entire codebase
- Contains: 20+ `var Err*` sentinels, `UserMessage()` helper for user-facing strings
- Key files: `internal/errors/errors.go`

**`internal/types/`:**
- Purpose: Shared core types; leaf package with zero internal imports
- Contains: Message, Task, ToolCall, ModelInfo, Tool interface, WorkflowPhase, TaskStatus, constants
- Key files: `internal/types/types.go`, `internal/types/constants.go`

**`internal/provider/`:**
- Purpose: LLM API abstraction and provider management
- Contains: LLMProvider interface, Registry, OpenRouter client, Zen client
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`

**`internal/tools/`:**
- Purpose: Tool implementations and execution dispatch
- Contains: 12 tools, Dispatcher (registry + permission gate + rate limiter), permission rules
- Key files: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`, `internal/tools/bash.go`

**`internal/workflow/`:**
- Purpose: Six-phase workflow engine
- Contains: Engine struct, 6 phase implementations, prompt templates (embedded), plan parser
- Key files: `internal/workflow/engine.go`, `internal/workflow/execute.go`, `internal/workflow/prompts/`

**`internal/tui/`:**
- Purpose: Terminal UI — all screens, components, and user interaction
- Contains: AppState (Bubble Tea model), 25+ screen models, 40+ components, theme system, responsive layout
- Key files: `internal/tui/app_state.go`, `internal/tui/app_update.go`, `internal/tui/app_view.go`, `internal/tui/repl.go`

**`internal/tui/components/`:**
- Purpose: Reusable TUI components shared across screens
- Contains: 40 component files (badge, card, message, permission modal, toolcard, thinking, etc.)
- Key files: `internal/tui/components/permission.go`, `internal/tui/components/message.go`, `internal/tui/components/toolcard.go`

**`internal/tui/layout/`:**
- Purpose: Responsive terminal layout system
- Contains: PageChrome, breakpoints, header/footer builders, narrow-screen guards
- Key files: `internal/tui/layout/page.go`, `internal/tui/layout/responsive.go`

**`internal/tui/theme/`:**
- Purpose: Color palette and visual theme definitions
- Contains: Theme struct, dark/light/auto modes, border styles, unicode decorations
- Key files: `internal/tui/theme/theme.go`, `internal/tui/theme/registry.go`

**`internal/git/`:**
- Purpose: Git operations wrapper
- Contains: Status, Diff, Commit, Log, HeadHash operations
- Key files: `internal/git/git.go`

**`internal/log/`:**
- Purpose: Structured logging with file rotation
- Contains: slog logger creation, daily rotation, 7-day retention
- Key files: `internal/log/` (logger setup)

**`internal/fileutil/`:**
- Purpose: Atomic file write utilities
- Contains: WriteAtomic (write to temp, rename), MkdirAllAtomic
- Key files: `internal/fileutil/atomic.go`

**`internal/tokens/`:**
- Purpose: Token estimation for LLM cost tracking
- Contains: tiktoken-go wrapper, EMA calibration for accuracy
- Key files: `internal/tokens/` (estimator)

**`pkg/session/`:**
- Purpose: Session lifecycle and file-based state persistence
- Contains: Manager (create/load/list/cleanup), checkpoints, planning file I/O
- Key files: `pkg/session/manager.go`, `pkg/session/planning.go`, `pkg/session/checkpoint.go`

**`pkg/taskrunner/`:**
- Purpose: DAG-based task scheduling with topological sort
- Contains: Runner, Schedule() (Kahn's algorithm), ExecuteGroup(), lifecycle callbacks
- Key files: `pkg/taskrunner/runner.go`

**`pkg/arbitrage/`:**
- Purpose: Model cost optimization via complexity scoring
- Contains: Recommend(), Scorer, complexity analysis, cost comparison
- Key files: `pkg/arbitrage/arbitrage.go`

**`pkg/autodream/`:**
- Purpose: Context consolidation when messages grow large
- Contains: Consolidator, threshold checks, message summarization
- Key files: `pkg/autodream/autodream.go`

**`pkg/ledger/`:**
- Purpose: Cross-session learning ledger (decisions, patterns)
- Contains: Ledger, Load/Save/Append to LEDGER.md
- Key files: `pkg/ledger/ledger.go`

**`pkg/rollback/`:**
- Purpose: Commit chain browser for undo
- Contains: Rollback, History(), Revert() via git
- Key files: `pkg/rollback/rollback.go`

**`pkg/bisect/`:**
- Purpose: Git bisect wrapper
- Contains: Bisect logic for finding offending commits
- Key files: `pkg/bisect/`

**`pkg/keychain/`:**
- Purpose: OS-specific secret storage for API keys
- Contains: Linux (dbus secret-service), macOS (Keychain), Windows (Credential Manager)
- Key files: `pkg/keychain/`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point — CLI flags, init, TUI launch
- `internal/tui/app.go:22`: `AppState.Init()` — Bubble Tea startup routing
- `internal/tui/app_update.go:23`: `AppState.Update()` — central message dispatch

**Configuration:**
- `~/.m31a/config.toml`: User configuration (runtime, not in repo)
- `internal/config/types.go`: Config struct definition (source of truth for config schema)
- `internal/config/loader.go`: Config loading/saving logic
- `internal/types/constants.go`: All compile-time constants

**Core Logic:**
- `internal/workflow/engine.go`: Workflow engine — RunPhase(), Transition(), streamLLM()
- `internal/workflow/execute.go`: Task execution — runExecute(), executeTaskWithTools()
- `internal/tools/dispatcher.go`: Tool dispatcher — Execute(), ensurePermission()
- `internal/provider/interface.go`: LLMProvider interface definition
- `internal/provider/openrouter/client.go`: OpenRouter streaming implementation

**Testing:**
- `*_test.go` files co-located with source files throughout `internal/` and `pkg/`
- `Makefile` targets: `test`, `test-fast`, `test-verbose`, `bench`

## Naming Conventions

**Files:**
- Screen models: `<screen>_model.go` (e.g., `plan_model.go`, `execute_model.go`)
- Screen views: `<screen>_view.go` (e.g., `plan_view.go`, `execute_view.go`)
- REPL variants: `repl_<aspect>.go` (e.g., `repl_stream.go`, `repl_thinking.go`)
- Commands: `commands_<category>.go` (e.g., `commands_git.go`, `commands_ai.go`)
- Tests: `<name>_test.go` (standard Go convention)
- Platform-specific: `<name>_unix.go`, `<name>_windows.go` (e.g., `bash_unix.go`)

**Directories:**
- `internal/` for private packages, `pkg/` for public packages
- `components/` for reusable TUI widgets
- `prompts/` for embedded LLM prompt templates

**Types:**
- Interfaces: noun form (`LLMProvider`, `Tool`, `MsgEmitter`)
- Structs: noun form (`Engine`, `Dispatcher`, `Registry`)
- Messages: `<Name>Msg` (e.g., `StreamMsg`, `PhaseResultMsg`, `PermissionRequestMsg`)
- Constants: `PascalCase` (e.g., `MaxFileSize`, `BashTimeout`)

## Where to Add New Code

**New Tool:**
- Implementation: `internal/tools/<toolname>.go`
- Register in: `internal/tools/defaults.go` (add to `DefaultDispatcher()`)
- Test: `internal/tools/<toolname>_test.go`
- Risk level: implement `types.Tool` interface with appropriate `RiskLevel()`

**New Workflow Phase:**
- Implementation: `internal/workflow/<phase>.go`
- Add case to: `internal/workflow/engine.go:RunPhase()` switch
- Prompt template: `internal/workflow/prompts/<phase>.md`
- Add phase constant: `internal/types/types.go` (WorkflowPhase enum)
- Update transitions: `internal/workflow/engine.go:validPhaseTransitions`

**New TUI Screen:**
- Model: `internal/tui/<screen>_model.go`
- View: `internal/tui/<screen>_view.go`
- Add screen constant: `internal/tui/types.go` (Screen enum)
- Add rendering case: `internal/tui/app_view.go:renderActiveScreen()`
- Add update case: `internal/tui/app_update.go` (message routing)

**New TUI Component:**
- Implementation: `internal/tui/components/<name>.go`
- Test: `internal/tui/components/<name>_test.go`
- Reuse across screens; import from `internal/tui/components/`

**New Provider:**
- Implementation: `internal/provider/<name>/client.go`
- Implement: `LLMProvider` interface from `internal/provider/interface.go`
- Register: `cmd/m31a/main.go` (add registration block)

**New pkg/ Package:**
- Implementation: `pkg/<name>/`
- Keep zero internal imports (may use `internal/types/` and `internal/errors/`)
- Each package gets a `doc.go` for godoc

## Special Directories

**`internal/workflow/prompts/`:**
- Purpose: Embedded LLM prompt templates loaded via `//go:embed`
- Contains: `base.md`, `tool-use.md`, `plan-format.md`, `execute-task.md`, `discuss-questions.md`, `self-heal.md`, `demonstration-format.md`
- Generated: No (manually authored Markdown)
- Committed: Yes

**`dist/`:**
- Purpose: Cross-compiled binaries for linux/darwin/windows × amd64/arm64
- Contains: `m31a-<os>-<arch>` binaries
- Generated: Yes (by `make cross` or goreleaser)
- Committed: No (gitignored)

**`~/.m31a/`:**
- Purpose: Runtime data directory (not in repo)
- Contains: `config.toml`, `m31a.log`, `LEDGER.md`, `sessions/`
- Generated: Yes (at runtime)
- Committed: No (user-specific)

**`.planning/`:**
- Purpose: GSD planning artifacts and codebase analysis
- Contains: `codebase/` (ARCHITECTURE.md, STRUCTURE.md, STACK.md, etc.)
- Generated: Yes (by GSD tools)
- Committed: Yes

---

*Structure analysis: 2026-06-11*
