# Codebase Structure

**Analysis Date:** 2026-07-23

## Directory Layout

```
M31A/
├── cmd/
│   └── m31a/                    # Entry point
│       ├── main.go              # Flag parsing, config, providers, TUI launch, headless modes
│       ├── main_test.go         # Binary tests
│       └── usage.go             # Usage text for --help
├── internal/
│   ├── core/                    # Core primitives (no internal deps)
│   │   ├── config/              # TOML config loader, validation, types
│   │   │   ├── loader.go        # Load() from file + env vars
│   │   │   ├── types.go         # Config struct (nested: Provider, Model, UI, Permissions, Features, Tools, Agents, Git, Verify, Compaction, etc.)
│   │   │   ├── merge.go         # Config merging (file + env + defaults)
│   │   │   ├── config_validate.go
│   │   │   ├── instructions.go  # AGENTS.md discovery
│   │   │   └── *_test.go
│   │   ├── errors/              # Sentinel errors
│   │   │   └── errors.go        # ErrPhaseTransition, ErrSessionNotFound, ErrPermissionDenied, etc.
│   │   └── types/               # Shared type vocabulary (CANONICAL)
│   │       ├── types.go         # WorkflowPhase, Message, Tool, ModelInfo, Task, Session, RiskLevel, Intent, etc.
│   │       ├── constants.go     # MaxHealAttempts, token limits, timeouts
│   │       ├── toolcall.go      # ToolCall, ToolInput, ToolResult
│   │       ├── plan.go          # Plan, PlanTask types
│   │       ├── git.go           # Git types
│   │       └── fileutil.go
│   ├── engine/                  # Workflow execution layer
│   │   ├── workflow/            # 7-phase engine (core)
│   │   │   ├── engine.go        # Engine struct, RunPhase, Transition, checkpointing
│   │   │   ├── state_machine.go # Phase transition validation
│   │   │   ├── phase_coordinator.go
│   │   │   ├── context_builder.go
│   │   │   ├── prompt_builder.go
│   │   │   ├── prompt_templates.go
│   │   │   ├── cost_tracker.go
│   │   │   ├── workflow_cache.go
│   │   │   ├── initialize.go    # Phase: project analysis
│   │   │   ├── discuss.go       # Phase: Q&A clarification
│   │   │   ├── plan.go          # Phase: task planning
│   │   │   ├── execute.go       # Phase: task execution
│   │   │   ├── verify.go        # Phase: verification
│   │   │   ├── runtime.go       # Phase: runtime checks
│   │   │   ├── ship.go          # Phase: commit/PR/changelog
│   │   │   ├── research.go      # Deep research sub-phase
│   │   │   ├── compaction.go    # Auto-compaction integration
│   │   │   ├── intent.go        # LLM-based intent classification
│   │   │   ├── classify.go      # Intent classification logic
│   │   │   ├── agent_switch.go
│   │   │   ├── retry.go
│   │   │   ├── diff_summary.go
│   │   │   └── *_test.go        # Extensive test coverage
│   │   ├── session/             # Session persistence
│   │   │   ├── manager.go       # SessionManager (project-local + global)
│   │   │   ├── session.go       # Session struct, Load/Save
│   │   │   ├── checkpoint.go    # Workflow checkpoint save/restore
│   │   │   ├── planning.go      # Planning dir management
│   │   │   ├── coordinator.go   # Cross-session run coordinator
│   │   │   ├── fileutil.go
│   │   │   ├── session_info.go
│   │   │   └── *_test.go
│   │   ├── taskrunner/          # Parallel task execution
│   │   │   ├── runner.go
│   │   │   └── runner_test.go
│   │   ├── bisect/              # Git bisect automation
│   │   │   ├── bisect.go
│   │   │   └── *_test.go
│   │   ├── rollback/            # Git rollback
│   │   │   └── rollback.go
│   │   ├── compaction/          # Session auto-compaction
│   │   │   ├── compaction.go
│   │   │   ├── serialize.go
│   │   │   ├── template.go
│   │   │   └── *_test.go
│   │   ├── narrative/           # Narrative event stream
│   │   │   ├── engine.go
│   │   │   ├── classifier.go
│   │   │   ├── grouper.go
│   │   │   ├── templates.go
│   │   │   ├── types.go
│   │   │   ├── event.go
│   │   │   ├── bridge.go
│   │   │   └── *_test.go
│   │   ├── decision/            # Decision logging
│   │   │   ├── logger.go
│   │   │   ├── receipt.go
│   │   │   ├── query.go
│   │   │   ├── redact.go
│   │   │   └── *_test.go
│   │   ├── tokens/              # Token estimation
│   │   │   ├── estimator.go
│   │   │   └── *_test.go
│   │   └── coordinator/         # Run coordination
│   │       └── coordinator.go
│   ├── integrations/            # External system adapters
│   │   ├── provider/            # LLM providers (3 implementations)
│   │   │   ├── interface.go     # LLMProvider interface
│   │   │   ├── registry.go      # ProviderRegistry
│   │   │   ├── common.go        # Shared HTTP, streaming, SSE
│   │   │   ├── capabilities.go  # Model capability detection
│   │   │   ├── model_metadata.go
│   │   │   ├── cache.go
│   │   │   ├── fallback.go
│   │   │   ├── reasoning.go
│   │   │   ├── openrouter/      # OpenRouter client
│   │   │   ├── zen/             # Zen client
│   │   │   ├── nvidia/          # Nvidia client
│   │   │   ├── mock/            # Test mocks
│   │   │   └── *_test.go
│   │   ├── git/                 # Git operations
│   │   │   └── git.go
│   │   ├── ledger/              # Session ledger (LEDGER.md)
│   │   │   └── ledger.go
│   │   ├── keychain/            # OS keychain (macOS/Windows/Linux)
│   │   │   └── keychain.go
│   │   ├── codeintel/           # Codebase indexing
│   │   │   └── indexer.go
│   │   ├── context/             # Dynamic context sources
│   │   │   └── registry.go
│   │   ├── metrics/             # Session metrics collection
│   │   │   └── collector.go
│   │   ├── log/                 # Structured logging
│   │   │   └── logger.go
│   │   ├── autodream/           # AutoDream integration
│   │   │   └── autodream.go
│   │   ├── arbitrage/           # Model arbitrage
│   │   │   └── arbitrage.go
│   │   ├── shell/               # Shell utilities
│   │   │   └── shell.go
│   │   ├── skills/              # Skill system
│   │   │   └── skills.go
│   │   ├── history/             # Command history
│   │   │   └── history.go
│   │   └── logging/             # Logging setup
│   │       └── logging.go
│   ├── infrastructure/          # Cross-cutting utilities
│   │   ├── retry/               # Retry policies
│   │   │   └── retry.go
│   │   └── fileutil/            # File utilities
│   │       └── fileutil.go
│   ├── tools/                   # Tool system (18+ tools)
│   │   ├── dispatcher.go        # Central registry, permissions, rate limits, concurrency
│   │   ├── defaults.go          # DefaultDispatcher() — registers all tools
│   │   ├── toolcall.go          # ToolCall execution wrapper
│   │   ├── interface.go         # Tool interface, PermissionRequest/Response
│   │   ├── constants.go         # Rate limit constants
│   │   ├── permissions.go       # Permission rule evaluation
│   │   ├── persistent_permissions.go
│   │   ├── subagent/            # Parallel subagent system
│   │   │   ├── manager.go
│   │   │   ├── loop.go
│   │   │   ├── worktree.go
│   │   │   ├── profile.go
│   │   │   ├── events.go
│   │   │   └── *_test.go
│   │   ├── fileops/             # File operations
│   │   │   ├── fileread.go
│   │   │   ├── filewrite.go
│   │   │   ├── edit.go
│   │   │   ├── filelist.go
│   │   │   ├── filedelete.go
│   │   │   ├── filemove.go
│   │   │   ├── pathhelpers.go
│   │   │   ├── helpers.go
│   │   │   ├── constants.go
│   │   │   ├── backup.go
│   │   │   └── *_test.go
│   │   ├── exec/                # Command execution
│   │   │   ├── bash.go
│   │   │   ├── bash_unix.go
│   │   │   ├── bash_windows.go
│   │   │   ├── prockill_unix.go
│   │   │   ├── prockill_windows.go
│   │   │   ├── bash_sandbox_linux.go
│   │   │   ├── bash_sandbox_darwin.go
│   │   │   ├── bash_sandbox_windows.go
│   │   │   ├── bash_sandbox_other.go
│   │   │   ├── devserver.go
│   │   │   ├── concurrency.go
│   │   │   ├── output_store.go
│   │   │   └── *_test.go
│   │   ├── search/              # Search tools
│   │   │   ├── glob.go
│   │   │   ├── grep.go
│   │   │   ├── webfetch.go
│   │   │   ├── webfetch_html.go
│   │   │   ├── websearch.go
│   │   │   ├── dns_cache.go
│   │   │   ├── ip_filter.go
│   │   │   ├── metrics.go
│   │   │   └── *_test.go
│   │   ├── ai/                  # AI-powered tools
│   │   │   ├── agent.go         # Agent tool (spawns subagent)
│   │   │   ├── question.go      # AskUser tool
│   │   │   └── memory.go
│   │   ├── codeanalysis/        # Code analysis tools
│   │   │   ├── codemap.go
│   │   │   ├── codecomplexity.go
│   │   │   └── *_test.go
│   │   ├── git/                 # Git tool
│   │   │   └── git.go
│   │   ├── network/             # Network tools
│   │   │   └── httpcheck.go
│   │   ├── todo/                # Todo tools
│   │   │   ├── todo.go
│   │   │   ├── todowrite.go
│   │   │   ├── todoread.go
│   │   │   └── *_test.go
│   │   └── *_test.go            # Dispatcher, tools integration tests
│   ├── ui/                      # Terminal UI (Bubble Tea)
│   │   └── tui/
│   │       ├── app.go           # App model (root state)
│   │       ├── app_state.go     # AppState struct
│   │       ├── app_update.go    # Update() — main message router
│   │       ├── app_update_commands.go
│   │       ├── app_update_phase.go
│   │       ├── app_update_workflow.go
│   │       ├── app_update_extra_test.go
│   │       ├── app_handlers.go
│   │       ├── app_handlers_workflow.go
│   │       ├── app_handlers_config.go
│   │       ├── app_handlers_provider.go
│   │       ├── app_handlers_tick.go
│   │       ├── app_handlers_misc.go
│   │       ├── app_routing.go
│   │       ├── app_nav.go
│   │       ├── app_screens.go
│   │       ├── app_session.go
│   │       ├── app_helpers.go
│   │       ├── app_agent.go
│   │       ├── app_channel.go
│   │       ├── router.go
│   │       ├── types.go         # TUI-specific types
│   │       ├── constants.go
│   │       ├── commands.go      # Command registry
│   │       ├── keybindings.go
│   │       ├── streaming.go
│   │       ├── narrative.go
│   │       ├── toast.go
│   │       ├── transition_*.go  # Phase transitions UI
│   │       ├── provider_registration.go
│   │       ├── screens/         # Full-screen views
│   │       │   ├── home.go
│   │       │   ├── discuss.go
│   │       │   ├── plan.go
│   │       │   ├── execute.go
│   │       │   ├── verify.go
│   │       │   ├── ship.go
│   │       │   ├── settings.go
│   │       │   ├── firstrun.go
│   │       │   ├── modelselector.go
│   │       │   ├── help.go
│   │       │   ├── dashboard.go
│   │       │   ├── ledger.go
│   │       │   ├── decision.go
│   │       │   ├── rollback.go
│   │       │   ├── sessiondetail.go
│   │       │   └── runtime.go
│   │       ├── components/      # Reusable UI components
│   │       │   ├── sidebar.go
│   │       │   ├── sidebar_render.go
│   │       │   ├── repl.go
│   │       │   ├── repl_view.go
│   │       │   ├── repl_model.go
│   │       │   ├── repl_stream.go
│   │       │   ├── repl_commands.go
│   │       │   ├── repl_mouse.go
│   │       │   ├── repl_search.go
│   │       │   ├── repl_clipboard.go
│   │       │   ├── repl_quickactions.go
│   │       │   ├── repl_thinking.go
│   │       │   ├── repl_state.go
│   │       │   ├── repl_footer.go
│   │       │   ├── repl_welcome.go
│   │       │   ├── commandpalette.go
│   │       │   ├── commandpalette_model.go
│   │       │   ├── cmdpalette.go
│   │       │   ├── fileexplorer.go
│   │       │   ├── diff.go
│   │       │   ├── diff_view.go
│   │       │   ├── ghostpicker.go
│   │       │   ├── ghostoutput.go
│   │       │   ├── mention.go
│   │       │   ├── mention_view.go
│   │       │   ├── metrics.go
│   │       │   ├── notification.go
│   │       │   ├── tooldetail.go
│   │       │   ├── chathistory.go
│   │       │   ├── plan_refine.go
│   │       │   ├── phasemodelpicker.go
│   │       │   ├── phase_transition.go
│   │       │   ├── phase_transition_model.go
│   │       │   ├── resume.go
│   │       │   ├── resume_view.go
│   │       │   ├── settings.go
│   │       │   ├── settings_model.go
│   │       │   ├── settings_tabs.go
│   │       │   ├── settings_view.go
│   │       │   ├── subagents.go
│   │       │   ├── subagents_model.go
│   │       │   ├── verify.go
│   │       │   ├── ship.go
│   │       │   ├── runtime.go
│   │       │   ├── ledger.go
│   │       │   ├── decision.go
│   │       │   ├── discuss.go
│   │       │   ├── plan.go
│   │       │   ├── execute.go
│   │       │   ├── firstrun.go
│   │       │   ├── firstrun_wizard.go
│   │       │   ├── firstrun_view.go
│   │       │   ├── goalinput.go
│   │       │   ├── home.go
│   │       │   ├── home_view.go
│   │       │   ├── confirmquit.go
│   │       │   ├── config_model.go
│   │       │   ├── config_model_*.go
│   │       │   └── ...
│   │       ├── layout/          # Layout utilities
│   │       │   └── layout.go
│   │       ├── theme/           # Theme system (dark only)
│   │       │   └── theme.go
│   │       ├── streaming/       # Streaming renderers
│   │       │   └── ...
│   │       ├── tuitypes/        # TUI type aliases
│   │       │   └── tuitypes.go
│   │       ├── a11y/            # Accessibility
│   │       │   └── a11y.go
│   │       ├── commands/        # Slash command definitions
│   │       │   ├── commands.go
│   │       │   ├── workflow.go
│   │       │   ├── session.go
│   │       │   ├── config.go
│   │       │   ├── provider.go
│   │       │   ├── tools.go
│   │       │   └── ...
│   │       └── *_test.go        # TUI tests (harness, input, navigation)
│   └── tests/                   # Internal test utilities
│       └── testutil/
│           └── testutil.go
├── pkg/                         # Public packages (zero internal deps)
│   └── errors/
│       └── errors.go            # Error wrapping utilities
├── .planning/                   # GSD planning artifacts (gitignored)
├── .m31a/                       # Global config dir (created at runtime)
├── docs/                        # Documentation
├── scripts/                     # Build/release scripts
├── tests/                       # E2E tests
│   └── e2e_test.go              # Binary-level tests (requires API keys)
├── go.mod                       # Go 1.25.0, CGO_ENABLED=0
├── go.sum
├── Makefile                     # build, test, lint, cross, check
├── .golangci.yml                # golangci-lint config (govet, staticcheck, errcheck, ineffassign, unused)
├── .goreleaser.yaml             # Cross-compile release config
├── install.sh                   # Installer
├── README.md
├── CHANGELOG.md
├── AGENTS.md                    # Agent instructions (this project's CLAUDE.md equivalent)
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── TESTING.md
├── LICENSE
└── .env.example
```

## Directory Purposes

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| `cmd/m31a/` | Application entry point | `main.go`, `usage.go` |
| `internal/core/types/` | **Canonical type definitions** — all layers depend on this | `types.go`, `constants.go`, `toolcall.go` |
| `internal/core/config/` | Configuration system (TOML + env) | `loader.go`, `types.go`, `merge.go` |
| `internal/core/errors/` | Sentinel error values | `errors.go` |
| `internal/engine/workflow/` | **Core workflow engine** — 7 phases | `engine.go`, `state_machine.go`, `phase_coordinator.go`, `*.go` per phase |
| `internal/engine/session/` | Session persistence & resume | `manager.go`, `session.go`, `checkpoint.go` |
| `internal/engine/taskrunner/` | Parallel task execution | `runner.go` |
| `internal/engine/bisect/` | Git bisect automation | `bisect.go` |
| `internal/engine/rollback/` | Git rollback | `rollback.go` |
| `internal/engine/compaction/` | Auto-compaction | `compaction.go`, `serialize.go` |
| `internal/engine/narrative/` | Narrative event stream | `engine.go`, `classifier.go`, `templates.go` |
| `internal/engine/decision/` | Decision logging | `logger.go`, `receipt.go` |
| `internal/engine/tokens/` | Token estimation (EMA) | `estimator.go` |
| `internal/engine/coordinator/` | Run coordination | `coordinator.go` |
| `internal/integrations/provider/` | LLM providers (OpenRouter, Zen, Nvidia) | `interface.go`, `registry.go`, `openrouter/`, `zen/`, `nvidia/` |
| `internal/integrations/git/` | Git operations wrapper | `git.go` |
| `internal/integrations/ledger/` | Session ledger (Markdown) | `ledger.go` |
| `internal/integrations/keychain/` | OS credential store | `keychain.go` |
| `internal/integrations/codeintel/` | Codebase indexer | `indexer.go` |
| `internal/integrations/metrics/` | Metrics collection | `collector.go` |
| `internal/integrations/log/` | Structured logging | `logger.go` |
| `internal/tools/` | **Tool system** — dispatcher + 18+ tools | `dispatcher.go`, `defaults.go`, `fileops/`, `exec/`, `search/`, `ai/`, `codeanalysis/`, `git/`, `network/`, `todo/`, `subagent/` |
| `internal/ui/tui/` | **Bubble Tea TUI** — Elm architecture | `app.go`, `app_update.go`, `app_state.go`, `router.go`, `screens/`, `components/`, `commands/` |
| `pkg/errors/` | Public error utilities | `errors.go` |
| `tests/` | E2E binary tests | `e2e_test.go` |

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go` — Main entry, TUI launch, headless modes

**Configuration:**
- `internal/core/config/types.go` — Complete Config struct
- `internal/core/config/loader.go` — Load from TOML + env
- `~/.m31a/config.toml` — User config (runtime)

**Workflow Engine:**
- `internal/engine/workflow/engine.go` — Engine struct, RunPhase, Transition
- `internal/engine/workflow/state_machine.go` — Phase transition rules
- `internal/engine/workflow/phase_coordinator.go` — Pre/post phase hooks
- `internal/engine/workflow/{initialize,discuss,plan,execute,verify,runtime,ship}.go` — Phase implementations

**Provider Layer:**
- `internal/integrations/provider/interface.go` — LLMProvider interface
- `internal/integrations/provider/registry.go` — ProviderRegistry
- `internal/integrations/provider/openrouter/*.go` — OpenRouter client
- `internal/integrations/provider/zen/*.go` — Zen client
- `internal/integrations/provider/nvidia/*.go` — Nvidia client

**Tools:**
- `internal/tools/dispatcher.go` — Central dispatcher (permissions, rate limits, concurrency)
- `internal/tools/defaults.go` — DefaultDispatcher() registers all tools
- `internal/tools/fileops/` — FileRead, FileWrite, Edit, FileList, FileDelete, FileMove
- `internal/tools/exec/` — Bash, DevServer
- `internal/tools/search/` — Glob, Grep, WebFetch, WebSearch
- `internal/tools/ai/` — Agent (subagent), AskUser
- `internal/tools/codeanalysis/` — CodeMap, CodeComplexity
- `internal/tools/git/` — Git tool
- `internal/tools/todo/` — TodoWrite, TodoRead
- `internal/tools/network/` — HTTPCheck
- `internal/tools/subagent/` — Parallel subagent manager

**TUI:**
- `internal/ui/tui/app.go` — App model (root state)
- `internal/ui/tui/app_update.go` — Update() message router
- `internal/ui/tui/app_state.go` — AppState struct
- `internal/ui/tui/router.go` — Screen routing
- `internal/ui/tui/screens/` — Full-screen views per phase
- `internal/ui/tui/components/` — Reusable components (REPL, Sidebar, CommandPalette, etc.)
- `internal/ui/tui/commands/` — Slash command definitions

**Session:**
- `internal/engine/session/manager.go` — SessionManager
- `internal/engine/session/session.go` — Session struct
- `internal/engine/session/checkpoint.go` — Checkpoint save/restore
- `<workDir>/.m31a/session.json` — Project session metadata
- `<workDir>/.m31a/messages.json` — Conversation history
- `<workDir>/.m31a/checkpoint.json` — Workflow checkpoint
- `~/.m31a/` — Global config, logs, keychain, ledger

## Naming Conventions

**Files:**
- Go files: `snake_case.go` (e.g., `phase_coordinator.go`, `file_write.go`)
- Test files: `*_test.go` suffix
- Platform-specific: `*_unix.go`, `*_windows.go`, `*_linux.go`, `*_darwin.go`

**Directories:**
- `snake_case` (e.g., `internal/engine/workflow/`, `internal/ui/tui/`)

**Packages:**
- Lowercase, single word where possible (e.g., `workflow`, `session`, `provider`, `tools`, `tui`)
- Matches directory name

**Types:**
- PascalCase (e.g., `WorkflowPhase`, `ToolResult`, `SessionManager`)

**Functions/Methods:**
- PascalCase for exported, camelCase for unexported

**Constants:**
- PascalCase (e.g., `PhaseInitialize`, `RiskDangerous`, `MaxHealAttempts`)

**Config TOML keys:**
- snake_case (e.g., `default_mode`, `timeout_seconds`, `max_parallel_tasks`)

## Where to Add New Code

**New Feature (full workflow phase):**
1. Phase logic: `internal/engine/workflow/<phase_name>.go`
2. Register in `engine.go` phase list
3. Add StateMachine transition in `state_machine.go`
4. TUI screen: `internal/ui/tui/screens/<phase_name>.go`
5. Route in `app_routing.go` and `router.go`

**New Tool:**
1. Implement `types.Tool` interface in `internal/tools/<category>/<tool_name>.go`
2. Register in `internal/tools/defaults.go` → `DefaultDispatcher()`
3. Add permission rule defaults in `internal/core/config/types.go` (PermissionsConfig)

**New LLM Provider:**
1. Implement `provider.LLMProvider` in `internal/integrations/provider/<name>/`
2. Register in `cmd/m31a/main.go` → provider registration block
3. Add config section in `internal/core/config/types.go` (ProviderConfig)

**New Config Option:**
1. Add field to appropriate struct in `internal/core/config/types.go`
2. Add default in `internal/core/config/loader.go` or `types.go` (DefaultConfig())
3. Document in `docs/` or `.env.example`

**New TUI Screen:**
1. Create model/view in `internal/ui/tui/screens/<name>.go`
2. Add route in `internal/ui/tui/router.go`
3. Add navigation in `internal/ui/tui/app_nav.go`
4. Add keybindings in `internal/ui/tui/keybindings.go`

**New Session Data:**
1. Add fields to `internal/engine/session/session.go` (Session struct)
2. Update `Load()` / `Save()` in `manager.go`
3. Handle migration in `session.go` if needed

## Special Directories

| Directory | Purpose | Generated | Committed |
|-----------|---------|-----------|-----------|
| `.m31a/` (project) | Project-local sessions, checkpoints, backups | Yes (runtime) | No (gitignored) |
| `~/.m31a/` | Global config, logs, keychain, ledger | Yes (runtime) | No |
| `.planning/` | GSD planning artifacts | Yes (GSD commands) | No (gitignored) |
| `internal/engine/workflow/templates/` | Embedded website templates (go:embed) | No | Yes |
| `internal/engine/workflow/templates/external/` | External template sources | No | Yes |

---

*Structure analysis: 2026-07-23*