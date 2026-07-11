# Codebase Structure

**Analysis Date:** 2026-07-11

## Directory Layout

```
/home/snigdha/Desktop/Helix/M31A/
├── cmd/
│   └── m31a/
│       └── main.go                 # Entry point: flags, config, providers, TUI, signals
├── internal/                       # Private application code (not importable by external modules)
│   ├── codeintel/                  # Codebase intelligence (AST indexing, symbol extraction)
│   │   └── indexer.go
│   ├── config/                     # Configuration loading, merging, validation (TOML)
│   │   ├── loader.go               # LoadConfig, WatchConfig (hot-reload)
│   │   ├── types.go                # Config struct definitions (548 lines)
│   │   ├── merge.go                # Config merging (file + env + defaults)
│   │   ├── project_context.go      # Project context detection
│   │   └── instructions.go         # AGENTS.md discovery
│   ├── context/                    # Dynamic context sources for prompt building
│   │   └── registry.go             # DateTime, Environment, Git sources
│   ├── decision/                   # Decision logging (v1.5)
│   │   └── logger.go
│   ├── errors/                     # Custom error types, sentinel errors
│   │   └── errors.go
│   ├── fileutil/                   # File utilities
│   │   └── fileutil.go
│   ├── git/                        # Git wrapper (init, commit, diff, worktree)
│   │   └── git.go
│   ├── log/                        # Slog logger initialization
│   │   └── logger.go
│   ├── logging/                    # Structured logging helpers
│   │   └── logging.go
│   ├── provider/                   # LLM provider abstraction + 3 implementations
│   │   ├── registry.go             # Provider registry, active provider management
│   │   ├── interface.go            # LLMProvider interface, ChatRequest, ToolDefinition
│   │   ├── base_client.go          # Shared HTTP, caching, SSE parsing, health checks
│   │   ├── capabilities.go         # Model capability heuristics (reasoning, tools, vision)
│   │   ├── reasoning.go            # Reasoning config per model
│   │   ├── cache.go                # Model cache with TTL + stale fallback
│   │   ├── fallback.go             # Auto-fallback between providers
│   │   ├── openrouter/
│   │   │   └── client.go           # OpenRouter API client
│   │   ├── zen/
│   │   │   └── client.go           # Zen API client
│   │   └── nvidia/
│   │       └── client.go           # NVIDIA NIM API client
│   ├── shell/                      # Shell command execution helpers
│   │   └── shell.go
│   ├── testutil/                   # Test utilities
│   │   └── testutil.go
│   ├── tokens/                     # Token estimation (tiktoken-go wrapper)
│   │   └── estimator.go
│   ├── tools/                      # Tool system (18 built-in tools + dispatcher)
│   │   ├── dispatcher.go           # Core dispatcher: permissions, rate limit, concurrency
│   │   ├── defaults.go             # DefaultDispatcher() — registers all 18 tools
│   │   ├── interface.go            # Tool interface, PermissionRequest/Response
│   │   ├── permissions.go          # Permission rule evaluation, persistent permissions
│   │   ├── output_store.go         # Tool output bounding (prevents context exhaustion)
│   │   ├── concurrency.go          # Concurrency semaphore
│   │   ├── tooldefs.go             # Tool definition helpers
│   │   ├── constants.go            # Rate limit constants, channel buffers
│   │   ├── bash.go                 # Bash execution (sandboxed, timeout, obfuscation detect)
│   │   ├── fileread.go             # File read with line ranges
│   │   ├── filewrite.go            # File write with backup
│   │   ├── edit.go                 # String replacement editing (fuzzy matching)
│   │   ├── glob.go                 # Glob pattern file listing
│   │   ├── grep.go                 # Ripgrep wrapper
│   │   ├── todowrite.go            # Todo list management (TODO.md sync)
│   │   ├── todoread.go             # Todo list reading
│   │   ├── webfetch.go             # HTTP fetch with redirect handling
│   │   ├── websearch.go            # Web search (DuckDuckGo HTML scrape)
│   │   ├── codemap.go              # Codebase map generation
│   │   ├── codecomplexity.go       # Cyclomatic complexity analysis
│   │   ├── agent.go                # Subagent spawning tool
│   │   ├── filedelete.go           # File deletion with backup
│   │   ├── filemove.go             # File move/rename
│   │   ├── filelist.go             # Directory listing
│   │   ├── httpcheck.go            # HTTP endpoint health check
│   │   ├── devserver.go            # Dev server management
│   │   ├── question.go             # Ask user question tool
│   │   ├── memory.go               # Cross-session memory (MEMORY.md)
│   │   ├── performance.go          # Performance tracking
│   │   ├── strings.go              # String utilities
│   │   ├── pathhelpers.go          # Path utilities
│   │   ├── toolcall.go             # ToolCall parsing helpers
│   │   ├── dns_cache.go            # DNS caching
│   │   ├── prockill.go             # Process killing (cross-platform)
│   │   ├── backup.go               # File backup utilities
│   │   ├── persistent_permissions.go
│   │   ├── permissions_test.go
│   │   └── subagent/               # Subagent infrastructure
│   │       ├── manager.go          # Subagent lifecycle, worktrees
│   │       ├── profile.go          # Subagent profiles (explore, general, security, etc.)
│   │       ├── loop.go             # Subagent execution loop
│   │       ├── loop_parse.go       # LLM response parsing for subagents
│   │       ├── worktree.go         # Git worktree management
│   │       └── events.go           # Subagent event types
│   ├── tui/                        # Bubble Tea TUI (Elm architecture)
│   │   ├── app.go                  # AppState init, shutdown, workflow engine init
│   │   ├── app_state.go            # AppState struct (250+ fields, all screens/models)
│   │   ├── app_update.go           # Update() — single dispatch for 60+ message types
│   │   ├── app_handlers.go         # Handler routing, screen navigation
│   │   ├── app_handlers_config.go  # Config screen handlers
│   │   ├── app_handlers_workflow.go # Workflow phase handlers
│   │   ├── app_handlers_tick.go    # Tick/health handlers
│   │   ├── app_handlers_provider.go # Provider/model handlers
│   │   ├── app_handlers_misc.go    # Misc handlers (toast, error, etc.)
│   │   ├── app_routing.go          # Screen routing logic
│   │   ├── app_nav.go              # Navigation stack
│   │   ├── app_input.go            # Input handling
│   │   ├── app_helpers.go          # Helper methods
│   │   ├── app_agent.go            # Agent mode handlers
│   │   ├── app_channel.go          # Channel/message handling
│   │   ├── app_session.go          # Session management
│   │   ├── commands.go             # Command registry re-exports
│   │   ├── constants.go            # UI constants (intervals, capacities)
│   │   ├── repl_model.go           # REPL screen model (chat interface)
│   │   ├── repl_commands.go        # REPL slash commands
│   │   ├── repl_clipboard.go       # Clipboard integration
│   │   ├── repl_thinking.go        # Thinking block rendering
│   │   ├── repl_search.go          # Message search
│   │   ├── repl_scrollbar.go       # Scrollbar rendering
│   │   ├── repl_mouse.go           # Mouse handling in REPL
│   │   ├── repl_quickactions.go    # Quick action buttons
│   │   ├── sidebar_model.go        # Sidebar (files, tasks, git, todos)
│   │   ├── plan_model.go           # Plan screen
│   │   ├── execute_model.go        # Execute screen
│   │   ├── verify_model.go         # Verify screen
│   │   ├── runtime_model.go        # Runtime screen
│   │   ├── ship_model.go           # Ship screen
│   │   ├── settings_model.go       # Settings screen (tabs)
│   │   ├── settings_tabs.go        # Settings tab definitions
│   │   ├── config_model.go         # Config editor model
│   │   ├── config_model_general.go
│   │   ├── config_model_tui.go
│   │   ├── config_model_provider.go
│   │   ├── config_model_advanced.go
│   │   ├── config_model_sections.go
│   │   ├── modelselector_view.go   # Model picker
│   │   ├── phase_model_picker.go   # Dual-model picker (planning vs coding)
│   │   ├── discuss_model.go        # Discuss Q&A screen
│   │   ├── diff_model.go           # Diff viewer
│   │   ├── ledger_model.go         # Session ledger
│   │   ├── rollback_model.go       # Rollback screen
│   │   ├── bisect_model.go         # Git bisect screen
│   │   ├── dashboard_model.go      # Dashboard
│   │   ├── sessiondetail_model.go  # Session detail view
│   │   ├── fileexplorer_model.go   # File explorer
│   │   ├── tooldetail_model.go     # Tool call detail
│   │   ├── ghostpicker_model.go    # Ghost writer picker
│   │   ├── ghostoutput_model.go    # Ghost writer output
│   │   ├── metrics_model.go        # Metrics visualization
│   │   ├── resume_model.go         # Session resume screen
│   │   ├── goalinput_model.go      # Goal input screen
│   │   ├── firstrun_model.go       # First-run wizard
│   │   ├── help_model.go           # Help screen
│   │   ├── chat_history_model.go   # Chat history browser
│   │   ├── command_palette.go      # Command palette (ctrl+p)
│   │   ├── command_palette_screen.go
│   │   ├── home_model.go           # Home/landing screen
│   │   ├── confirm_quit_model.go   # Quit confirmation
│   │   ├── mention.go              # @-mention file resolution
│   │   ├── mention_view.go
│   │   ├── narrative_handler.go    # Narrative engine integration
│   │   ├── streaming.go            # Streaming response handling
│   │   ├── truncate.go             # Text truncation utilities
│   │   ├── diff_view.go            # Diff rendering
│   │   ├── filewatcher.go          # fsnotify file watcher
│   │   ├── handler_navigation.go   # Navigation handlers
│   │   ├── handler_stream.go       # Streaming handlers
│   │   ├── handler_tool.go         # Tool call handlers
│   │   ├── handler_config.go       # Config handlers
│   │   ├── handler_sidebar.go      # Sidebar handlers
│   │   ├── handler_runtime.go      # Runtime handlers
│   │   ├── commands/               # Slash command implementations
│   │   │   ├── registry.go         # CommandRegistry, CommandInfo, CommandResult
│   │   │   ├── parser.go           # Slash command parsing
│   │   │   ├── builtin.go          # Built-in commands (/help, /model, /theme, etc.)
│   │   │   ├── workflow.go         # Workflow commands (/plan, /execute, etc.)
│   │   │   ├── session.go          # Session commands (/resume, /session)
│   │   │   ├── tools.go            # Tool commands
│   │   │   ├── config.go           # Config commands
│   │   │   ├── provider.go         # Provider commands
│   │   │   └── debug.go            # Debug commands
│   │   ├── layout/                 # Layout engine (responsive, stack-based)
│   │   │   ├── page.go             # Page layout
│   │   │   ├── stack.go            # Stack layout
│   │   │   ├── responsive.go       # Breakpoint-based responsive layout
│   │   │   ├── constraints.go      # Constraint solver
│   │   │   ├── box.go              # Box model
│   │   │   └── minscreen.go        # Minimum screen size handling
│   │   ├── streaming/              # Agent streaming loop
│   │   │   ├── agent_loop.go       # Autonomous agent loop
│   │   │   └── streaming.go
│   │   ├── a11y/                   # Accessibility
│   │   │   ├── announce.go         # Screen reader announcements
│   │   │   └── terminal.go         # Terminal capability detection
│   │   ├── components/             # Reusable UI components
│   │   │   ├── permission_modal.go
│   │   │   ├── question_model.go
│   │   │   └── ...
│   │   ├── theme/                  # Theme system (Lipgloss)
│   │   │   ├── manager.go
│   │   │   ├── theme.go
│   │   │   └── styles.go
│   │   └── tuitypes/               # TUI-specific message types
│   │       └── tuitypes.go
│   ├── types/                      # Shared type vocabulary (core domain types)
│   │   ├── types.go                # Message, ToolCall, Task, ModelInfo, WorkflowPhase, RiskLevel, etc.
│   │   ├── plan.go                 # Plan parsing, validation
│   │   ├── toolcall.go             # ToolCall helpers
│   │   ├── git.go                  # Git types
│   │   └── constants.go            # Constants (MaxLLMResponseBytes, DefaultContextLength, etc.)
│   ├── workflow/                   # Workflow engine (7 phases)
│   │   ├── engine.go               # Engine core (1520 lines) — phase dispatch, LLM streaming, tool calls
│   │   ├── state_machine.go        # Phase transition validation, history, cycle guard
│   │   ├── phase_coordinator.go    # Pre/post-phase hooks, checkpoints, metrics
│   │   ├── context_builder.go      # System prompt + dynamic context composition
│   │   ├── prompt_builder.go       # Prompt template loading (4-level priority)
│   │   ├── prompt_templates.go     # Embedded prompt templates
│   │   ├── prompts/                # Embedded prompt files (go:embed)
│   │   │   └── loader.go
│   │   ├── cost_tracker.go         # Cost accumulation, budget enforcement
│   │   ├── cache.go                # Workflow cache (project, tool defs)
│   │   ├── workflow_cache.go
│   │   ├── initialize.go           # Phase 1: project detection, git init, planning dir
│   │   ├── discuss.go              # Phase 2: clarifying questions, quality check
│   │   ├── plan.go                 # Phase 3: plan generation, chunking, check, gates
│   │   ├── execute.go              # Phase 4: task execution, self-heal, quality gates
│   │   ├── verify.go               # Phase 5: verification, reporting
│   │   ├── runtime.go              # Phase 6: runtime checks, dev server
│   │   ├── ship.go                 # Phase 7: ship preflight, changelog, commit
│   │   ├── research.go             # Pre-plan research
│   │   ├── intent.go               # Intent classification (LLM-based routing)
│   │   ├── classify.go             # Prompt classification
│   │   ├── retry.go                # Retry logic for phases
│   │   ├── diff_summary.go         # Diff summarization
│   │   ├── agent_switch.go         # Agent switching mid-workflow
│   │   ├── coverage_gates.go       # Coverage gate evaluation
│   │   ├── verify_report.go        # Verification report generation
│   │   ├── ship_preflight.go       # Pre-ship checklist
│   │   ├── ship_lock.go            # Ship lock file (cross-platform)
│   │   ├── workflow.go             # Workflow types (PhaseResult, DiscussState, etc.)
│   │   ├── workflow_cache.go
│   │   ├── engine_messages.go      # MsgEmitter, workflow message types
│   │   ├── engine_parse.go         # Plan parsing from LLM output
│   │   ├── engine_verify.go        # Verification helpers
│   │   ├── prompt_templates.go
│   │   └── thinking_indicator.go
│   └── wiring/                     # Dependency wiring helpers
│       └── wiring.go
├── pkg/                            # Public packages (stable APIs, no internal/ imports)
│   ├── arbitrage/                  # Model cost optimization (recommend cheaper models)
│   │   ├── arbitrage.go
│   │   └── doc.go
│   ├── autodream/                  # Session consolidation (auto-summarization)
│   │   ├── autodream.go
│   │   └── doc.go
│   ├── bisect/                     # Git bisect automation
│   │   ├── bisect.go
│   │   ├── exec.go
│   │   └── doc.go
│   ├── compaction/                 # Proactive session compaction
│   │   ├── compaction.go
│   │   ├── serialize.go
│   │   └── template.go
│   ├── coordinator/                # Subagent coordination
│   │   └── coordinator.go
│   ├── history/                    # Frecent (frequency + recency) history
│   │   └── history.go
│   ├── keychain/                   # OS keychain integration
│   │   ├── keychain.go             # Interface
│   │   ├── keychain_darwin.go      # macOS Keychain
│   │   ├── keychain_linux.go       # Secret Service / D-Bus
│   │   └── keychain_windows.go     # Windows Credential Manager
│   ├── ledger/                     # Session ledger (LEDGER.md)
│   │   ├── ledger.go
│   │   └── doc.go
│   ├── metrics/                    # Observability collector
│   │   ├── collector.go
│   │   └── types.go
│   ├── narrative/                  # Narrative engine (message classification, grouping)
│   │   ├── engine.go
│   │   ├── bridge.go
│   │   ├── classifier.go
│   │   ├── grouper.go
│   │   ├── timing.go
│   │   ├── templates.go
│   │   └── types.go
│   ├── retry/                      # Retry policies with error classification
│   │   └── policy.go
│   ├── rollback/                   # Git-based rollback
│   │   └── rollback.go
│   ├── session/                    # Session persistence (STATE.md, PLAN.md, TASKS.md)
│   │   ├── manager.go
│   │   ├── session.go
│   │   ├── session_info.go
│   │   ├── planning.go
│   │   ├── checkpoint.go
│   │   └── doc.go
│   ├── skills/                     # Skill discovery for composable slash commands
│   │   ├── loader.go
│   │   ├── discovery.go
│   │   └── skill.go
│   └── taskrunner/                 # Parallel task execution
│       └── runner.go
├── docs/                           # User documentation
│   ├── ARCHITECTURE.md
│   ├── CONFIG.md
│   ├── INTERFACES.md
│   ├── KEYBINDINGS.md
│   ├── QUICKSTART.md
│   ├── SCREENS.md
│   ├── TOOLS.md
│   ├── TROUBLESHOOTING.md
│   └── WORKFLOW.md
├── scripts/                        # Build/release scripts
│   ├── validate-release.sh
│   └── verify_v1.sh
├── .github/                        # GitHub Actions, issue templates
├── .planning/                      # GSD planning artifacts
│   └── codebase/                   # These analysis documents
├── e2e_test.go                     # End-to-end tests (binary execution)
├── go.mod                          # Go module (go 1.25.0)
├── go.sum
├── Makefile                        # Build, test, lint, cross-compile targets
├── m31a.json                       # Project metadata
├── README.md
├── AGENTS.md                       # Agent instructions (this project's CLAUDE.md)
├── CHANGELOG.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── LICENSE
├── .golangci.yml                   # golangci-lint config
├── .goreleaser.yaml                # Release configuration
├── install.sh                      # Installer script
└── layout.test                     # Layout engine tests
```

## Module Organization

### cmd/
- **Purpose**: Application entry points
- **Packages**: `cmd/m31a` (main binary)
- **Key file**: `cmd/m31a/main.go` — 477 lines, orchestrates entire startup

### internal/ (Private Application Code)
**Rule**: `internal/` packages CANNOT be imported by external modules. `pkg/` MUST NOT import `internal/`.

| Package | Responsibility | Key Files |
|---------|---------------|-----------|
| `codeintel` | Codebase indexing for planning context | `indexer.go` |
| `config` | TOML config load/merge/validate/watch | `loader.go`, `types.go` (548 lines) |
| `context` | Dynamic context sources (datetime, env, git) | `registry.go` |
| `decision` | Decision logging (structured) | `logger.go` |
| `errors` | Sentinel errors, wrapping helpers | `errors.go` |
| `fileutil` | File utilities | `fileutil.go` |
| `git` | Git operations wrapper | `git.go` |
| `log` | Slog logger setup | `logger.go` |
| `logging` | Structured logging helpers | `logging.go` |
| `provider` | LLM provider abstraction + 3 implementations | `registry.go`, `interface.go`, `base_client.go`, `openrouter/`, `zen/`, `nvidia/` |
| `shell` | Shell execution helpers | `shell.go` |
| `testutil` | Test utilities | `testutil.go` |
| `tokens` | Token estimation (tiktoken) | `estimator.go` |
| `tools` | Tool system (dispatcher + 18 tools) | `dispatcher.go`, `defaults.go`, `bash.go`, `edit.go`, `agent.go`, `subagent/` |
| `tui` | Bubble Tea TUI (20+ screen models) | `app.go`, `app_state.go`, `app_update.go`, `*_model.go` |
| `types` | Core domain types (shared vocabulary) | `types.go` (350 lines), `plan.go`, `constants.go` |
| `workflow` | 7-phase workflow engine | `engine.go` (1520 lines), `state_machine.go`, `phase_coordinator.go`, `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go` |
| `wiring` | Dependency wiring helpers | `wiring.go` |

### pkg/ (Public Packages)
**Rule**: Stable APIs, no `internal/` imports. Can be imported by external projects.

| Package | Purpose | Key Files |
|---------|---------|-----------|
| `arbitrage` | Model cost optimization | `arbitrage.go` |
| `autodream` | Session auto-consolidation | `autodream.go` |
| `bisect` | Git bisect automation | `bisect.go`, `exec.go` |
| `compaction` | Session compaction | `compaction.go`, `serialize.go`, `template.go` |
| `coordinator` | Subagent coordination | `coordinator.go` |
| `history` | Frecent history | `history.go` |
| `keychain` | OS keychain (cross-platform) | `keychain.go`, `keychain_darwin.go`, `keychain_linux.go`, `keychain_windows.go` |
| `ledger` | Session ledger | `ledger.go` |
| `metrics` | Observability collector | `collector.go`, `types.go` |
| `narrative` | Narrative engine (UI message processing) | `engine.go`, `bridge.go`, `classifier.go`, `grouper.go` |
| `retry` | Retry policies | `policy.go` |
| `rollback` | Git rollback | `rollback.go` |
| `session` | Session persistence | `manager.go`, `session.go`, `planning.go`, `checkpoint.go` |
| `skills` | Slash command skills | `loader.go`, `discovery.go`, `skill.go` |
| `taskrunner` | Parallel task execution | `runner.go` |

## Key File Locations

### Entry Points
- **Main binary**: `cmd/m31a/main.go`
- **TUI App creation**: `internal/tui/app.go:297` (`NewApp`)
- **TUI Update loop**: `internal/tui/app_update.go:19` (`AppState.Update`)

### Configuration
- **Config struct**: `internal/config/types.go` (548 lines, 23 config sections)
- **Config loading**: `internal/config/loader.go` (`Load`, `LoadDotEnv`, `WatchConfig`)
- **Default config**: `internal/config/loader.go` (`DefaultConfig`)

### Workflow Engine
- **Engine core**: `internal/workflow/engine.go` (1520 lines)
- **Phase implementations**: `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go`
- **State machine**: `internal/workflow/state_machine.go`
- **Phase coordination**: `internal/workflow/phase_coordinator.go`
- **Context building**: `internal/workflow/context_builder.go`
- **Prompt templates**: `internal/workflow/prompts/` (embedded via `go:embed`)

### Provider Layer
- **Registry**: `internal/provider/registry.go`
- **Interface**: `internal/provider/interface.go`
- **Base client**: `internal/provider/base_client.go` (HTTP, caching, SSE)
- **OpenRouter**: `internal/provider/openrouter/client.go`
- **Zen**: `internal/provider/zen/client.go`
- **NVIDIA**: `internal/provider/nvidia/client.go`
- **Capabilities**: `internal/provider/capabilities.go` (heuristics for reasoning/tools/vision)

### Tools
- **Dispatcher**: `internal/tools/dispatcher.go` (480 lines)
- **Default tools**: `internal/tools/defaults.go` (registers 18 tools)
- **Tool interface**: `internal/tools/interface.go`
- **Individual tools**: `bash.go`, `fileread.go`, `filewrite.go`, `edit.go`, `glob.go`, `grep.go`, `todowrite.go`, `webfetch.go`, `websearch.go`, `codemap.go`, `codecomplexity.go`, `agent.go`, `filedelete.go`, `filemove.go`, `filelist.go`, `httpcheck.go`, `devserver.go`, `question.go`

### TUI Screens (Models)
| Screen | Model File | Purpose |
|--------|------------|---------|
| REPL (chat) | `repl_model.go` | Main conversation interface |
| Sidebar | `sidebar_model.go` | Files, tasks, git, todos |
| Plan | `plan_model.go` | Plan view/approval |
| Execute | `execute_model.go` | Task execution monitoring |
| Verify | `verify_model.go` | Verification results |
| Runtime | `runtime_model.go` | Dev server, runtime checks |
| Ship | `ship_model.go` | Pre-ship checklist, changelog |
| Settings | `settings_model.go` + `settings_tabs.go` | Config editor |
| Model Picker | `modelselector_view.go` | Model selection |
| Phase Model Picker | `phase_model_picker.go` | Dual-model (planning vs coding) |
| Discuss | `discuss_model.go` | Q&A collection |
| Diff | `diff_model.go` | Git diff viewer |
| Ledger | `ledger_model.go` | Session ledger |
| Rollback | `rollback_model.go` | Rollback interface |
| Bisect | `bisect_model.go` | Git bisect UI |
| Dashboard | `dashboard_model.go` | Metrics dashboard |
| Session Detail | `sessiondetail_model.go` | Session inspection |
| File Explorer | `fileexplorer_model.go` | File tree |
| Tool Detail | `tooldetail_model.go` | Tool call inspection |
| Ghost Picker | `ghostpicker_model.go` | Ghost writer model select |
| Ghost Output | `ghostoutput_model.go` | Ghost writer output |
| Metrics | `metrics_model.go` | Metrics visualization |
| Resume | `resume_model.go` | Session resume |
| Goal Input | `goalinput_model.go` | Goal entry |
| First Run | `firstrun_model.go` | Onboarding wizard |
| Help | `help_model.go` | Help screen |
| Chat History | `chat_history_model.go` | Message history browser |
| Command Palette | `command_palette.go` | Ctrl+P palette |
| Home | `home_model.go` | Landing screen |
| Confirm Quit | `confirm_quit_model.go` | Quit confirmation |

### Shared Types
- **Core types**: `internal/types/types.go` — `Message`, `ToolCall`, `Task`, `ModelInfo`, `WorkflowPhase`, `RiskLevel`, `IntentResult`, `ProjectState`, `Session`, `StreamChunk`, `HealthStatus`, `DiffSummary`
- **Constants**: `internal/types/constants.go` — `MaxLLMResponseBytes`, `DefaultContextLength`, `MaxPlanRetries`, `MaxHealAttempts`, `DirPermission`, `HealthCheckInterval`, `SidebarRefreshInterval`

## Test File Organization

```
<package>/
├── *_test.go              # Unit tests (co-located)
├── *_extra_test.go        # Additional test coverage
├── *_integration_test.go  # Integration tests (require API keys)
├── *_bench_test.go        # Benchmarks
```

**Key test files:**
- `internal/workflow/engine_test.go` — Workflow phase tests
- `internal/tools/dispatcher_test.go` — Dispatcher behavior
- `internal/provider/*/integration_test.go` — Real API tests (skipped without keys)
- `e2e_test.go` — Binary execution tests
- `pkg/*/test.go` — Public package tests

**Coverage targets** (from Makefile):
- 75% overall
- 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

## Naming Conventions

**Files:**
- `snake_case.go` for all Go files
- `_test.go` suffix for tests
- `_extra_test.go` for supplemental tests
- `_integration_test.go` for integration tests
- `_windows.go` / `_darwin.go` / `_linux.go` / `_unix.go` for OS-specific code

**Directories:**
- `snake_case` for package directories
- Feature-based grouping under `internal/` (e.g., `internal/tui/streaming/`, `internal/tools/subagent/`)

**Packages:**
- Lowercase, single word preferred (`tools`, `workflow`, `provider`)
- Subpackages for large domains (`tui/layout`, `tui/streaming`, `tools/subagent`)

## Where to Add New Code

### New Feature (Workflow Phase)
1. Phase logic: `internal/workflow/<phase>.go` (follow `initialize.go` pattern)
2. Register in `Engine.RunPhase()` switch (`engine.go:695`)
3. Add `PhaseResultMsg` handling in `internal/tui/app_update.go`
4. Add screen model if UI needed: `internal/tui/<phase>_model.go`
5. Add route in `AppState.initScreenUpdaters()` (`app.go:379`)

### New Tool
1. Implement `types.Tool` interface in `internal/tools/<tool>.go`
2. Register in `DefaultDispatcher()` (`internal/tools/defaults.go:122`)
3. Add permission rule defaults in `internal/tools/permissions.go` if needed
4. Tests: `internal/tools/<tool>_test.go`

### New Provider
1. Implement `provider.LLMProvider` in `internal/provider/<name>/client.go`
2. Embed `provider.BaseClient` for shared HTTP/caching/SSE
3. Register in `cmd/m31a/main.go:243` (`tui.RegisterProvider`)
4. Add config section in `internal/config/types.go` (`ProviderConfig`)

### New TUI Screen
1. Create `internal/tui/<screen>_model.go` with `Model` struct + `Init/Update/View`
2. Add field to `AppState` (`app_state.go`)
3. Initialize in `NewApp()` (`app.go:297`)
4. Add case in `initScreenUpdaters()` (`app.go:379`)
5. Add route in `navigateToScreen()` (`app_routing.go`)

### New Config Option
1. Add field to appropriate struct in `internal/config/types.go`
2. Add TOML tag (`toml:"field_name"`)
3. Handle default in `DefaultConfig()` (`loader.go`)
4. Wire into consumer (e.g., `FeaturesConfig` → `Engine` via `cfg.Features`)

### New Public Package (pkg/)
1. Create `pkg/<name>/` with `doc.go` (package documentation)
2. Implement API in `<name>.go`
3. **Must not import `internal/`** — only stdlib and other `pkg/`
4. Add tests in `pkg/<name>/<name>_test.go`

## Special Directories

| Directory | Purpose | Generated | Committed |
|-----------|---------|-----------|-----------|
| `.m31a/` (project) | Session data, planning files, backups, tool output | Yes | No (gitignored) |
| `~/.m31a/` (global) | Config, history, keychain, global sessions | Yes | No |
| `.planning/` | GSD planning artifacts (phases, specs, reviews) | Yes | Yes |
| `docs/` | User-facing documentation | No | Yes |
| `M31A.wiki/` | Internal wiki (architecture, decisions) | No | Yes |

---

*Structure analysis: 2026-07-11*