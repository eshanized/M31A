# Codebase Structure

**Analysis Date:** 2026-07-16

## Directory Layout

```
M31A/
├── cmd/
│   └── m31a/                          # Entry point package
│       ├── main.go                    # Application bootstrap (598 lines)
│       └── usage.go                   # CLI help text, slash command listing
│
├── internal/                          # Private packages (not importable externally)
│   ├── codeintel/                     # Code intelligence: 4-language parser, import graph, relevance
│   │   ├── indexer.go
│   │   ├── parser.go
│   │   ├── relevance.go
│   │   └── *_test.go
│   ├── config/                        # Configuration system (6-layer TOML cascade)
│   │   ├── loader.go                  # Load + merge + validate (25K lines with tests)
│   │   ├── merge.go                   # Layer merging logic
│   │   ├── types.go                   # Config structs (571 lines)
│   │   ├── validate.go
│   │   ├── project_context.go
│   │   └── *_test.go
│   ├── context/                       # Dynamic context system
│   │   ├── registry.go
│   │   ├── sources.go
│   │   └── estimation.go
│   ├── decision/                      # Decision logging and receipts
│   │   ├── logger.go
│   │   └── types.go
│   ├── errors/                        # Sentinel errors with user-friendly messages
│   │   └── errors.go
│   ├── fileutil/                      # Atomic file writes
│   │   └── atomic.go
│   ├── git/                           # Git operations wrapper
│   │   └── git.go
│   ├── log/                           # Structured logging with daily rotation
│   │   └── logger.go
│   ├── provider/                      # LLM provider abstraction layer
│   │   ├── interface.go               # LLMProvider interface (8 methods)
│   │   ├── base_client.go             # Shared HTTP, caching, SSE, retry
│   │   ├── cache.go                   # Thread-safe model cache (TTL + stale + singleflight)
│   │   ├── capabilities.go            # Heuristic capability detection from model ID
│   │   ├── fallback.go                # Parallel health checks, priority switching
│   │   ├── registry.go                # Thread-safe provider registry
│   │   ├── sse.go                     # SSE parser with watchdog
│   │   ├── common.go                  # Shared request/response helpers
│   │   ├── reasoning.go               # Reasoning token handling
│   │   ├── model_metadata.go          # Context length, pricing, architecture
│   │   ├── openrouter/                # OpenRouter client (300+ models)
│   │   │   └── client.go
│   │   ├── zen/                       # Zen/OpenCode provider
│   │   │   └── client.go
│   │   ├── nvidia/                    # Nvidia NIM gateway
│   │   │   └── client.go
│   │   └── mock/                      # Test mocks
│   │       └── mock.go
│   ├── tokens/                        # Token estimation (tiktoken + EMA calibration)
│   │   └── estimator.go
│   ├── tools/                         # 18 built-in tools + dispatcher + permissions
│   │   ├── dispatcher.go              # Execution pipeline: concurrency, rate limit, perms (14K lines)
│   │   ├── defaults.go                # Registers all 18 tools
│   │   ├── permissions.go             # Rule-based permission evaluation
│   │   ├── interface.go               # Tool interface definition
│   │   ├── constants.go               # Risk levels, timeouts, limits
│   │   ├── agent.go                   # Agent tool (spawn subagents)
│   │   ├── bash*.go                   # Shell execution with sandbox
│   │   ├── edit.go                    # 7-strategy cascading replacement
│   │   ├── file*.go                   # FileRead/Write/List/Delete/Move
│   │   ├── glob.go / grep.go          # Pattern search
│   │   ├── webfetch*.go / websearch.go
│   │   ├── codemap.go / codecomplexity.go
│   │   ├── git.go
│   │   ├── devserver.go
│   │   ├── httpcheck.go
│   │   ├── todo*.go
│   │   ├── question.go                # AskUserQuestion tool
│   │   ├── memory.go                  # Memory tool
│   │   ├── output_store.go            # Output bounding
│   │   ├── dns_cache.go
│   │   ├── concurrency.go
│   │   ├── persistent_permissions.go
│   │   ├── metrics.go
│   │   ├── prockill_*.go
│   │   ├── strings.go
│   │   ├── tooldefs.go
│   │   ├── subagent/                  # Parallel subagent manager
│   │   │   ├── manager.go
│   │   │   ├── worktree.go
│   │   │   └── *_test.go
│   │   └── *_test.go                  # Extensive test coverage
│   ├── tui/                           # Bubble Tea TUI (33 screens)
│   │   ├── app.go                     # AppState, Init, Shutdown (775 lines)
│   │   ├── app_update.go              # Single dispatch point for all messages
│   │   ├── app_view.go                # Root view rendering
│   │   ├── app_routing.go             # Screen routing logic
│   │   ├── router.go                  # Screen stack management
│   │   ├── types.go                   # Re-exports from tuitypes/
│   │   ├── screen.go                  # Screen type constants
│   │   ├── commands/                  # 60+ slash command implementations
│   │   │   ├── registry.go
│   │   │   ├── workflow.go
│   │   │   ├── provider.go
│   │   │   ├── session.go
│   │   │   └── ...
│   │   ├── components/                # Reusable TUI components
│   │   │   ├── sidebar/               # Sidebar with todo, files, decisions
│   │   │   ├── repl/                  # REPL input/output rendering
│   │   │   ├── plan/ / execute/ / verify/ / ship/  # Phase screens
│   │   │   ├── permission/            # Permission modal
│   │   │   ├── modelselector/         # Model picker with pricing
│   │   │   ├── help/                  # Help screen
│   │   │   ├── notifications/         # Toast + history
│   │   │   ├── ledger/ / rollback/ / bisect/ / metrics/
│   │   │   └── ...
│   │   ├── layout/                    # Responsive layout engine
│   │   │   ├── chrome.go
│   │   │   ├── breakpoints.go
│   │   │   └── sidebar.go
│   │   ├── streaming/                 # Streaming response rendering
│   │   │   ├── renderer.go
│   │   │   └── thinking.go
│   │   ├── theme/                     # Lipgloss theming (dark only)
│   │   │   ├── theme.go
│   │   │   └── palettes.go
│   │   ├── tuitypes/                  # TUI-specific type definitions
│   │   │   ├── types.go
│   │   │   └── interfaces.go
│   │   ├── narrative*.go              # Narrative message classification
│   │   ├── keybindings*.go
│   │   ├── repl*.go                   # REPL state, streaming, commands
│   │   ├── sidebar*.go                # Sidebar model/view
│   │   ├── plan*.go / execute*.go / verify*.go / ship*.go / runtime*.go
│   │   ├── settings*.go               # Settings screen (26K lines)
│   │   ├── resume*.go                 # Session resume screen
│   │   ├── rollback*.go               # Rollback screen
│   │   ├── ledger*.go                 # Ledger screen
│   │   ├── metrics*.go                # Metrics screen
│   │   ├── phasemodelpicker*.go       # Per-phase model picker
│   │   ├── phase_transition*.go       # Phase transition animations
│   │   ├── subagents*.go              # Subagent management screen
│   │   ├── transition*.go             # Screen transitions
│   │   ├── toast.go                   # Toast notifications
│   │   ├── help_model.go / help_view.go
│   │   ├── home_model.go / home_view.go
│   │   ├── modelselector*.go
│   │   ├── sessiondetail*.go
│   │   ├── tooldetail*.go
│   │   └── tui_harness_test.go
│   ├── types/                         # Type aliases to pkg/types (backward compat)
│   │   ├── types.go
│   │   ├── providers.go
│   │   ├── toolcall.go
│   │   ├── plan.go
│   │   ├── git.go
│   │   ├── constants.go
│   │   └── *_test.go
│   └── workflow/                      # Seven-phase workflow engine
│       ├── engine.go                  # Core engine (54K lines)
│       ├── engine_*.go                # Phase runners, messages, parsing, wiring
│       ├── state_machine.go           # Phase transition validation
│       ├── phase_coordinator.go       # Pre/post phase hooks
│       ├── prompt_builder.go          # Embedded/project/global prompt loading
│       ├── context_builder.go         # System prompt composition
│       ├── workflow_cache.go          # Caching for project, tools, prompts
│       ├── cost_tracker.go
│       ├── retry.go                   # Retry logic with exponential backoff
│       ├── initialize.go / discuss.go / plan.go / execute.go / verify.go / runtime.go / ship.go
│       ├── intent.go / classify.go    # LLM-based intent classification
│       ├── execute_heal.go / execute_quality.go / execute_preflight.go
│       ├── plan_check.go / plan_chunk.go / plan_parser.go
│       ├── research.go
│       ├── diff_summary.go
│       ├── coverage_gates.go
│       ├── agent_switch.go
│       ├── templates/                 # Embedded website templates
│       ├── prompts/                   # Embedded prompt templates (base.md, execute-task.md, etc.)
│       └── *_test.go
│
├── pkg/                               # Public packages (importable by external projects)
│   ├── arbitrage/                     # Model cost optimizer with task classification
│   │   ├── arbitrage.go
│   │   └── *_test.go
│   ├── autodream/                     # Context consolidation with reentrancy guard
│   │   ├── autodream.go
│   │   └── *_test.go
│   ├── bisect/                        # Git-bisect wrapper for model comparison
│   │   ├── bisect.go
│   │   ├── exec.go
│   │   └── *_test.go
│   ├── compaction/                    # Session compaction utilities
│   │   ├── compaction.go
│   │   ├── serialize.go
│   │   ├── template.go
│   │   └── *_test.go
│   ├── coordinator/                   # Drain management for workflow→TUI
│   │   └── coordinator.go
│   ├── errors/                        # Public error types
│   │   └── errors.go
│   ├── history/                       # Frecent prompt history with scoring
│   │   ├── history.go
│   │   └── *_test.go
│   ├── keychain/                      # OS keychain abstraction
│   │   ├── keychain.go                # Interface + caching wrapper
│   │   ├── keychain_linux.go          # D-Bus Secret Service + pass CLI fallback
│   │   ├── keychain_darwin.go         # macOS security CLI
│   │   ├── keychain_windows.go        # Windows Credential Manager
│   │   └── *_test.go
│   ├── ledger/                        # Cross-session learning store (markdown-backed)
│   │   ├── ledger.go
│   │   └── *_test.go
│   ├── metrics/                       # Metrics collection and reporting
│   │   ├── collector.go
│   │   ├── types.go
│   │   └── *_test.go
│   ├── retry/                         # Retry logic with exponential backoff
│   │   ├── policy.go
│   │   └── *_test.go
│   ├── rollback/                      # Commit-chain manager (soft/hard/safe reset)
│   │   ├── rollback.go
│   │   └── *_test.go
│   ├── session/                       # Session lifecycle, persistence, checkpointing
│   │   ├── manager.go                 # Project-local session storage (661 lines)
│   │   ├── session.go
│   │   ├── checkpoint.go
│   │   ├── planning.go
│   │   ├── doc.go
│   │   └── *_test.go
│   ├── skills/                        # Skill discovery and management
│   │   ├── skill.go
│   │   ├── loader.go
│   │   ├── discovery.go
│   │   └── *_test.go
│   ├── taskrunner/                    # Kahn's algorithm for topological sort, bounded parallelism
│   │   ├── runner.go
│   │   └── *_test.go
│   └── types/                         # Canonical type definitions (368 lines)
│       ├── types.go
│       ├── toolcall.go
│       ├── plan.go
│       ├── fileutil.go
│       ├── git.go
│       └── constants.go
│
├── docs/                              # User-facing documentation
│   ├── ARCHITECTURE.md
│   ├── CONFIG.md
│   ├── PROVIDERS.md
│   ├── TOOLS.md
│   ├── WORKFLOW.md
│   ├── INTERFACES.md
│   ├── TYPES.md
│   ├── SCREENS.md
│   ├── KEYBINDINGS.md
│   ├── SLASH_COMMANDS.md
│   ├── QUICKSTART.md
│   ├── GETTING_STARTED.md
│   ├── ONBOARDING.md
│   ├── TROUBLESHOOTING.md
│   ├── DEVELOPMENT_GUIDE.md
│   ├── SECURITY_MODEL.md
│   ├── GIT_INTEGRATION.md
│   ├── LOGGING.md
│   ├── METRICS.md
│   ├── CODE_INTELLIGENCE.md
│   ├── CODE_COMPLEXITY_TOOL.md
│   ├── COMMAND_PALETTE.md
│   ├── PROMPT_SYSTEM.md
│   ├── PROMPT_HISTORY.md
│   ├── SUBAGENTS.md
│   ├── SIX_PHASE_WORKFLOW.md
│   ├── SESSION_MANAGEMENT.md
│   ├── TERMINAL_UI.md
│   ├── ACCESSIBILITY.md
│   ├── DESIGN_TOKENS.md
│   ├── KNOWLEDGE_SYSTEM.md
│   ├── DECISION_LOGGING.md
│   ├── MOUSE_SUPPORT.md
│   └── FILE_UTILITIES.md
│
├── M31A.wiki/                         # Internal wiki (architecture decisions, etc.)
│   ├── Architecture.md
│   ├── Provider-System.md
│   ├── Prompt-System.md
│   ├── Tools-System.md
│   ├── Terminal-UI.md
│   ├── Task-Runner.md
│   ├── Subagents.md
│   ├── Slash-Commands.md
│   ├── Six-Phase-Workflow.md
│   ├── Session-Management.md
│   ├── Security-Model.md
│   ├── NVIDIA-NIM-Provider.md
│   ├── Metrics.md
│   ├── Logging.md
│   ├── Layout-Engine.md
│   ├── Knowledge-System.md
│   ├── Keybindings.md
│   ├── Git-Integration.md
│   ├── Decision-Logging.md
│   ├── Context-Registry.md
│   ├── Code-Intelligence.md
│   ├── CodeComplexity-Tool.md
│   ├── Chat-History.md
│   ├── Accessibility.md
│   ├── Design-Tokens.md
│   ├── Mouse-Support.md
│   ├── Home.md
│   └── Getting-Started.md
│
├── scripts/
│   ├── validate-release.sh            # Release validation harness
│   └── verify_v1.sh                   # Acceptance test suite
│
├── .github/
│   ├── workflows/ci.yml               # CI pipeline (build, test, lint)
│   ├── dependabot.yml
│   ├── ISSUE_TEMPLATE/
│   └── PULL_REQUEST_TEMPLATE.md
│
├── .env.example                       # Example environment variables
├── .env.test                          # Test environment (gitignored in practice)
├── .golangci.yml                      # golangci-lint config
├── .goreleaser.yaml                   # Cross-compile + release config
├── CHANGELOG.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── LICENSE
├── README.md
├── SECURITY.md
├── AGENTS.md                          # Agent instructions (this file)
├── go.mod / go.sum
├── Makefile                           # Build, test, lint, cross-compile targets
├── m31a.json                          # Project metadata
├── install.sh                         # One-liner installer
├── e2e_test.go                        # End-to-end tests (requires API keys)
└── *.test                             # Test binaries (gitignored)
```

## Directory Purposes

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| `cmd/m31a/` | Application entry point, binary construction | `main.go`, `usage.go` |
| `internal/codeintel/` | Multi-language code analysis for context relevance | `indexer.go`, `parser.go`, `relevance.go` |
| `internal/config/` | Configuration loading, validation, hot-reload | `loader.go`, `types.go`, `merge.go` |
| `internal/context/` | Dynamic context registry and token estimation | `registry.go`, `sources.go` |
| `internal/decision/` | Structured decision logging during workflow | `logger.go`, `types.go` |
| `internal/errors/` | Sentinel error definitions with user messages | `errors.go` |
| `internal/fileutil/` | Atomic write utilities | `atomic.go` |
| `internal/git/` | Git operations (commit, diff, branch, stash) | `git.go` |
| `internal/log/` | Structured logging with rotation | `logger.go` |
| `internal/provider/` | LLM provider abstraction + 3 implementations | `interface.go`, `registry.go`, `openrouter/`, `zen/`, `nvidia/` |
| `internal/tokens/` | Token counting with tiktoken + EMA calibration | `estimator.go` |
| `internal/tools/` | Tool implementations, dispatcher, permissions | `dispatcher.go`, `defaults.go`, `*.go` |
| `internal/tui/` | Bubble Tea TUI — screens, components, rendering | `app.go`, `app_update.go`, `commands/`, `components/` |
| `internal/types/` | Type aliases to `pkg/types` for backward compat | `types.go`, `providers.go` |
| `internal/workflow/` | Seven-phase orchestration engine | `engine.go`, `state_machine.go`, phase files |
| `pkg/arbitrage/` | Model cost optimization | `arbitrage.go` |
| `pkg/autodream/` | Context consolidation | `autodream.go` |
| `pkg/bisect/` | Git-bisect wrapper | `bisect.go`, `exec.go` |
| `pkg/compaction/` | Session summarization | `compaction.go`, `template.go` |
| `pkg/coordinator/` | Workflow→TUI drain coordination | `coordinator.go` |
| `pkg/errors/` | Public error types | `errors.go` |
| `pkg/history/` | Frecent prompt history | `history.go` |
| `pkg/keychain/` | OS keychain (Linux/macOS/Windows) | `keychain.go`, `keychain_*.go` |
| `pkg/ledger/` | Cross-session learning (LEDGER.md) | `ledger.go` |
| `pkg/metrics/` | Session metrics collection | `collector.go`, `types.go` |
| `pkg/retry/` | Exponential backoff retry | `policy.go` |
| `pkg/rollback/` | Git-based undo/redo | `rollback.go` |
| `pkg/session/` | Session persistence, checkpoints | `manager.go`, `session.go` |
| `pkg/skills/` | Slash command skill system | `skill.go`, `discovery.go` |
| `pkg/taskrunner/` | Parallel task execution with DAG | `runner.go` |
| `pkg/types/` | **Canonical type definitions** — single source of truth | `types.go` |

## Key File Locations

**Entry Points:**
- Main: `cmd/m31a/main.go:run()` (line 243)
- Headless prompt: `cmd/m31a/main.go:runHeadless()` (line 183)
- Headless workflow: `cmd/m31a/main.go:runHeadlessWorkflow()` (line 57)

**Configuration:**
- Config struct: `internal/config/types.go:Config` (line 9)
- Loader: `internal/config/loader.go:Load()` (line ~100)
- Hot-reload: `internal/config/loader.go:WatchConfig()` (line ~400)

**Workflow Engine:**
- Engine: `internal/workflow/engine.go:Engine` (line 99)
- RunPhase: `internal/workflow/engine.go:RunPhase()` (line 820)
- StateMachine: `internal/workflow/state_machine.go:StateMachine` (line 14)
- PhaseCoordinator: `internal/workflow/phase_coordinator.go:PhaseCoordinator` (line 14)

**Provider Layer:**
- Interface: `internal/provider/interface.go:LLMProvider` (line 15)
- Registry: `internal/provider/registry.go:Registry` (line 11)
- OpenRouter: `internal/provider/openrouter/client.go:Client` (line 20)
- Zen: `internal/provider/zen/client.go:Client`
- Nvidia: `internal/provider/nvidia/client.go:Client`

**Tools:**
- Dispatcher: `internal/tools/dispatcher.go:Dispatcher` (line 20)
- Defaults: `internal/tools/defaults.go:DefaultDispatcher()` (line 11)
- Tool Interface: `pkg/types/types.go:Tool` (line 241)
- Permission Rules: `internal/tools/permissions.go`

**TUI:**
- AppState: `internal/tui/app.go:AppState` (defined in `tuitypes/types.go`)
- Update: `internal/tui/app_update.go:Update()`
- View: `internal/tui/app_view.go:View()`
- Screens: `internal/tui/tuitypes/types.go:Screen` constants
- Commands: `internal/tui/commands/registry.go:CommandRegistry`

**Session:**
- Manager: `pkg/session/manager.go:Manager` (line 24)
- NewSession: `pkg/session/manager.go:NewSession()` (line 149)
- Checkpoints: `pkg/session/checkpoint.go`

**Types (Canonical):**
- All core types: `pkg/types/types.go`
- WorkflowPhase: line 18-29
- Tool/ToolCall/ToolResult: line 164-202
- Message: line 170-179
- ModelInfo: line 140-151
- Task: line 256-268

## Naming Conventions

**Files:**
- Go files: `snake_case.go` (e.g., `engine.go`, `state_machine.go`, `file_read.go`)
- Test files: `*_test.go` (co-located)
- Platform-specific: `*_linux.go`, `*_darwin.go`, `*_windows.go` (build tags)
- Embed directives: `//go:embed templates/...` at package level

**Directories:**
- Lowercase, singular: `provider/`, `tools/`, `workflow/`, `tui/`
- Subpackages: `tui/components/`, `tui/commands/`, `provider/openrouter/`

**Packages:**
- Match directory name: `package workflow`, `package tools`, `package session`
- Public packages under `pkg/`: `package session`, `package keychain`

**Types:**
- PascalCase for exported: `WorkflowPhase`, `ModelInfo`, `Dispatcher`
- camelCase for unexported: `workDir_`, `rateTokens`
- Interfaces: noun or `-er` suffix: `LLMProvider`, `Tool`, `SchemaProvider`, `Keychain`
- Constants: `PascalCase` for exported, `UPPER_SNAKE` for unexported module-level

**Functions/Methods:**
- PascalCase for exported: `RunPhase()`, `NewEngine()`, `FetchModels()`
- camelCase for unexported: `buildToolDefinitions()`, `ensurePermission()`
- Constructor pattern: `NewX()` returns `(*X, error)`
- Boolean getters: `IsPaused()`, `HasProvider()`, `CanConsolidate()`

**Variables:**
- camelCase: `workDir`, `modelID`, `sessionMgr`
- Acronyms: `ID` not `Id`, `URL` not `Url`, `API` not `Api`
- Channel names: `requestCh`, `responseCh`, `done` (for completion signals)
- Mutexes: `mu`, `planMu`, `transitionMu` (suffix `Mu`)

## Where to Add New Code

| New Code Type | Location | Notes |
|---------------|----------|-------|
| **New tool** | `internal/tools/` + register in `defaults.go` | Implement `Tool` interface, add to `DefaultDispatcher()` |
| **New provider** | `internal/provider/<name>/` | Implement `LLMProvider`, register in `main.go` |
| **New workflow phase** | `internal/workflow/<phase>.go` + add to `RunPhase()` switch | Update `StateMachine.validTransitions` |
| **New TUI screen** | `internal/tui/components/<screen>/` + add to `Screen` enum in `tuitypes/types.go` | Register route in `app_routing.go` |
| **New slash command** | `internal/tui/commands/<category>.go` | Add to `CommandRegistry` in `commands/registry.go` |
| **New config option** | `internal/config/types.go` in appropriate struct | Add TOML tag, update validation in `validate.go` |
| **New session field** | `pkg/types/types.go:Session` + `pkg/session/manager.go` | Update `SaveSession`/`LoadSession` |
| **New metric** | `pkg/metrics/types.go` + `collector.go` | Record via `collector.RecordX()` |
| **New decision category** | `internal/decision/types.go` | Use in `Engine.LogDecision()` |
| **New prompt template** | `internal/workflow/prompts/<name>.md` | Loaded via `PromptBuilder` |
| **Website template** | `internal/workflow/templates/website-<framework>/` | Embedded via `//go:embed` in `engine.go` |
| **Skill (slash command)** | `pkg/skills/` + skill directory | Implement `Skill` interface, register in `loader.go` |

## Special Directories

| Directory | Purpose | Generated | Committed |
|-----------|---------|-----------|-----------|
| `.m31a/` (project) | Session data, backups, planning, checkpoints | Yes (at runtime) | No (gitignored via `Manager.ensureGitIgnore()`) |
| `~/.m31a/` (global) | Config, recent models, keychain refs, LEDGER.md | Yes | No (gitignored) |
| `dist/` | Cross-compiled release binaries | Yes (by `make cross`) | No |
| `coverage.html` | HTML coverage report | Yes (by `make cover`) | No |
| `*.test` | Test binaries | Yes | No (gitignored) |
| `internal/workflow/templates/` | Embedded website templates | No (source) | Yes (go:embed) |
| `internal/workflow/prompts/` | Embedded prompt templates | No (source) | Yes (go:embed) |

---

*Structure analysis: 2026-07-16*