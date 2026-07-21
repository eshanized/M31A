# Codebase Structure

**Analysis Date:** Tue Jul 21 2026

## Directory Layout

```
M31A/
├── cmd/
│   └── m31a/
│       ├── main.go          # CLI entry, config, providers, TUI, headless modes
│       ├── usage.go         # Help text generation
│       └── .gitignore
├── internal/
│   ├── autodream/           # Auto-compaction of session history
│   ├── bisect/              # Git bisect automation for regression finding
│   ├── codeintel/           # Codebase intelligence (symbol index, summaries)
│   ├── compaction/          # LLM-based session summarization
│   ├── config/              # TOML config with 4-level prompt override chain
│   ├── context/             # Dynamic context sources (git, env, datetime)
│   ├── coordinator/         # Per-session concurrency control
│   ├── decision/            # Decision logging (ring buffer audit trail)
│   ├── errors/              # Sentinel errors + wrapped error types
│   ├── fileutil/            # Atomic write, file locking (cross-platform)
│   ├── git/                 # Git operations wrapper
│   ├── history/             # Frecent (frequency+recency) command history
│   ├── keychain/            # OS keychain integration (macOS/Win/Linux)
│   ├── ledger/              # LEDGER.md append-only session records
│   ├── logging/             # Structured audit logging
│   ├── metrics/             # Session metrics collection (tools, LLM, phases)
│   ├── narrative/           # Event classification & narrative rendering
│   ├── provider/            # LLM provider abstraction + 3 implementations
│   │   ├── mock/            # Test mocks
│   │   ├── openrouter/      # OpenRouter API client
│   │   ├── zen/             # Zen (OpenCode) API client
│   │   └── nvidia/          # NVIDIA NIM API client
│   ├── rollback/            # Git-based rollback to checkpoints
│   ├── retry/               # Retry policies with backoff
│   ├── session/             # Session management (project-local .m31a/)
│   ├── shell/               # Shell detection & command execution
│   ├── skills/              # Slash-command skill discovery & loading
│   ├── taskrunner/          # Dependency-ordered task execution with parallelism
│   ├── tokens/              # Token estimation with EMA calibration
│   ├── tools/               # 18 built-in tools + dispatcher
│   │   ├── ai/              # AskUserQuestion tool
│   │   ├── exec/            # Bash, DevServer tools
│   │   ├── fileops/         # FileRead, FileWrite, Edit, FileList, FileDelete, FileMove
│   │   ├── search/          # Glob, Grep, WebSearch, WebFetch
│   │   └── subagent/        # Parallel sub-agent manager with Git worktrees
│   ├── tui/                 # Bubble Tea TUI application
│   │   ├── components/      # Reusable UI components (modals, tool cards)
│   │   ├── commands/        # Command registry & key bindings
│   │   ├── layout/          # Layout calculations
│   │   ├── screens/         # 30+ screen models (one per workflow phase + utils)
│   │   ├── streaming/       # SSE streaming rendering
│   │   ├── theme/           # Lipgloss theme system (dark only)
│   │   └── tuitypes/        # TUI-specific message types
│   ├── types/               # Shared type vocabulary (NO internal imports)
│   ├── workflow/            # 7-phase workflow engine
│   │   ├── prompts/         # Embedded prompt templates (go:embed)
│   │   └── templates/       # Embedded project templates (Next.js, etc.)
│   └── wiring/              # Dependency injection wiring
├── pkg/
│   └── errors/              # Public error types (no internal imports)
├── docs/                    # User documentation (Markdown)
├── .github/workflows/       # CI pipeline
├── scripts/                 # Release/validation scripts
├── go.mod / go.sum          # Go 1.25, CGO_ENABLED=0
├── Makefile                 # build, test, lint, check, cross, cover
└── .golangci.yml            # golangci-lint config (govet, staticcheck, errcheck, ineffassign, unused)
```

## Key Locations

| Area | Path | Description |
|------|------|-------------|
| **Entry Point** | `cmd/m31a/main.go` | CLI flags, config load, keychain, provider registration, TUI construction, headless modes (`--prompt`, `--goal`) |
| **TUI App** | `internal/tui/app.go` | `AppState` (tea.Model), Init/Update/View, screen routing, workflow engine init |
| **TUI Screens** | `internal/tui/screens/` | 30+ screen models: `home/`, `repl/`, `discuss/`, `plan/`, `execute/`, `verify/`, `runtime/`, `ship/`, `settings/`, `resume/`, `dashboard/`, `bisect/`, `file_explorer/`, `model_selector/`, `phase_model_picker/`, `plan_refine/`, `chat_history/`, `ghost_picker/`, `ghost_output/`, `command_palette/`, `confirm_quit/`, `diff/`, `help/`, `ledger/`, `metrics/`, `notification/`, `rollback/`, `session_detail/`, `tool_detail/`, `goal_input/`, `first_run/`, `mention/`, `sidebar/` |
| **Workflow Engine** | `internal/workflow/engine.go` | Core `Engine` with `RunPhase()`, state machine, context builder, prompt builder |
| **Workflow Phases** | `internal/workflow/{initialize,discuss,plan,execute,verify,runtime,ship}.go` | Phase implementations |
| **State Machine** | `internal/workflow/state_machine.go` | Validated phase transitions, Plan↔Discuss oscillation guard |
| **Phase Coordinator** | `internal/workflow/phase_coordinator.go` | Pre/post-phase hooks, transition side effects |
| **Providers** | `internal/provider/` | `interface.go` (LLMProvider), `registry.go`, `base_client.go`, `openrouter/`, `zen/`, `nvidia/` |
| **Tools** | `internal/tools/` | `interface.go` (Tool), `dispatcher.go` (permissions, rate limit, concurrency), `defaults.go` (18 tool registration) |
| **Tool Categories** | `internal/tools/{fileops,exec,search,ai,subagent}/` | Tool implementations |
| **Session Manager** | `internal/session/manager.go` | Project-local sessions in `<workDir>/.m31a/`, checkpoints, plans, tasks |
| **Types** | `internal/types/types.go` | Shared vocabulary: WorkflowPhase, Message, Tool, Task, ModelInfo, IntentResult, etc. |
| **Config** | `internal/config/` | TOML with 4-level prompt override chain, hot-reload via fsnotify |
| **Error Types** | `internal/errors/errors.go` | Sentinel errors + ProviderError, ToolError, ConfigError with UserMessage() |
| **Metrics** | `internal/metrics/collector.go` | Tool calls, LLM usage, phase durations, heals → METRICS.json |

## Module Boundaries

```
cmd/m31a/           → internal/tui/, internal/workflow/, internal/provider/, internal/tools/, internal/session/, internal/types, internal/config, internal/keychain, internal/git, internal/ledger, internal/rollback, internal/tokens, internal/autodream
internal/tui/       → internal/workflow, internal/provider, internal/tools, internal/session, internal/types, internal/config, internal/git, internal/ledger, internal/rollback, internal/autodream, internal/decision, internal/narrative, internal/metrics, internal/arbitrage, internal/subagent, internal/keychain, internal/history
internal/workflow/  → internal/provider, internal/tools, internal/session, internal/types, internal/config, internal/git, internal/ledger, internal/decision, internal/tokens, internal/codeintel, internal/compaction, internal/context, internal/metrics, internal/retry, internal/taskrunner
internal/provider/  → internal/types, internal/errors
internal/tools/     → internal/types, internal/config, internal/errors, internal/metrics, internal/tools/fileops, internal/tools/exec, internal/tools/search, internal/tools/ai, internal/tools/subagent
internal/session/   → internal/types, internal/coordinator, internal/errors
internal/types/     → (no internal imports - leaf package)
pkg/errors/         → (no internal imports - public API)
```

**Dependency Rule**: `pkg/` must NOT import `internal/` (enforced by Go module structure). `internal/types` is the shared vocabulary — all internal packages may import it, but it imports nothing from `internal/`.

## Naming Conventions

| Element | Convention | Examples |
|---------|------------|----------|
| **Files** | snake_case.go | `engine.go`, `state_machine.go`, `dispatcher.go` |
| **Directories** | lowercase, singular | `provider/`, `tools/`, `workflow/`, `tui/screens/repl/` |
| **Packages** | lowercase, singular | `workflow`, `provider`, `tools`, `session` |
| **Types** | PascalCase | `Engine`, `LLMProvider`, `Dispatcher`, `ModelInfo`, `WorkflowPhase` |
| **Interfaces** | noun or -er suffix | `LLMProvider`, `Tool`, `SchemaProvider`, `MsgEmitter`, `ContextSource` |
| **Constants** | PascalCase with prefix | `PhaseInitialize`, `RiskDangerous`, `MaxHealAttempts`, `DefaultContextLength` |
| **Functions** | PascalCase (exported), camelCase (unexported) | `RunPhase()`, `buildPlanContext()`, `ensurePermission()` |
| **Test Files** | `_test.go` suffix | `engine_test.go`, `dispatcher_test.go`, `engine_race_test.go` |
| **Embedded Files** | `//go:embed` directive | `prompts/`, `templates/` in `workflow/` |

## Where to Add New Code

### New Feature (Workflow Phase)
1. **Phase Logic**: `internal/workflow/<phase>.go` — implement `run<Phase>()`
2. **State Machine**: Add phase to `validTransitions` in `state_machine.go:28-37`
3. **Screen**: `internal/tui/screens/<phase>/` — create `<phase>.go` + `<phase>_view.go`
4. **Router**: Register screen in `app.go:initScreenUpdaters()`
5. **Prompt**: Add template in `internal/workflow/prompts/` (go:embed)

### New Tool
1. **Implementation**: `internal/tools/<category>/<tool>.go` — implement `types.Tool` interface
2. **Schema** (optional): Implement `types.SchemaProvider` for JSON Schema
3. **Registration**: Add to `internal/tools/defaults.go:DefaultDispatcher()`
4. **Permission**: Default risk level in tool's `RiskLevel()`; configurable via config.toml rules

### New Provider
1. **Client**: `internal/provider/<name>/client.go` — embed `BaseClient`, implement `LLMProvider`
2. **Registration**: Add case in `cmd/m31a/main.go:366-382` (RegisterProvider call)
3. **Constants**: Add to `internal/types/constants.go:129-131` (ProviderOpenRouter, ProviderZen, ProviderNvidia)

### New Screen (Non-Phase)
1. **Model**: `internal/tui/screens/<name>/<name>.go` — implement `Screen` interface
2. **View**: `internal/tui/screens/<name>/<name>_view.go` — Lipgloss rendering
3. **Router**: Add to `app.go:initScreenUpdaters()` and `screenUpdaters` map
4. **Navigation**: Add key binding in `app_input_route.go` or command in `repl_commands.go`

### New Config Option
1. **Struct**: Add field to appropriate struct in `internal/config/types.go`
2. **Default**: Set in `internal/config/loader.go:DefaultConfig()`
3. **Validation**: Add check in `internal/config/config_validate.go`
4. **Usage**: Wire through `config.Config` to consumer (workflow, tools, TUI)

## Special Directories

| Directory | Purpose | Generated? | Committed? |
|-----------|---------|------------|------------|
| `.m31a/` (project) | Project-local sessions, checkpoints, plans, tasks, LEDGER.md, METRICS.json | Yes (runtime) | No (gitignored via .gitignore) |
| `~/.m31a/` (global) | Global config, keychain cache, history.json, frecent history | Yes (runtime) | No |
| `internal/workflow/prompts/` | Embedded prompt templates (base.md, execute-task.md, etc.) | No (source) | Yes |
| `internal/workflow/templates/` | Embedded project templates (Next.js, etc.) | No (source) | Yes |
| `.planning/codebase/` | Codebase analysis docs (this file) | Yes (mapper) | Yes |
| `.planning/graphs/` | Knowledge graph exports | Yes (graphify) | Yes |

## Key Files Reference

### Entry & Configuration
| File | Purpose |
|------|---------|
| `cmd/m31a/main.go` | CLI entry, all initialization, headless modes |
| `cmd/m31a/usage.go` | Help text with command registry |
| `internal/config/types.go` | All config structs (571 lines) |
| `internal/config/loader.go` | TOML load, merge, dotenv, validation |
| `internal/config/merge.go` | Config merge logic (layered overrides) |

### TUI Core
| File | Purpose |
|------|---------|
| `internal/tui/app.go` | AppState (778 lines), Init/Update/View, workflow init, shutdown |
| `internal/tui/app_state.go` | AppState struct (482 lines), screen models, dependencies |
| `internal/tui/app_update.go` | Update() message handling (phase results, permissions, questions) |
| `internal/tui/app_update_phase.go` | Phase transition handling |
| `internal/tui/app_handlers_workflow.go` | Workflow message handlers |
| `internal/tui/repl.go` | REPL screen entry point |
| `internal/tui/screens/repl/repl_model.go` | REPL state, messages, input handling |
| `internal/tui/screens/repl/repl_view.go` | REPL rendering |
| `internal/tui/components/permission_modal.go` | Permission request UI |
| `internal/tui/theme/manager.go` | Dark theme only, Lipgloss styles |

### Workflow Engine
| File | Purpose |
|------|---------|
| `internal/workflow/engine.go` | Core Engine (1609+ lines), RunPhase, streamLLM, compaction, checkpoints |
| `internal/workflow/state_machine.go` | Phase transition validation |
| `internal/workflow/phase_coordinator.go` | Pre/post-phase hooks |
| `internal/workflow/context_builder.go` | Dynamic context assembly |
| `internal/workflow/prompt_builder.go` | 4-level prompt template loading |
| `internal/workflow/initialize.go` | Project detection, git init, PROJECT.md |
| `internal/workflow/discuss.go` | Clarifying questions via LLM |
| `internal/workflow/plan.go` | Task generation, research, checker, gates, chunking |
| `internal/workflow/execute.go` | Dependency-ordered parallel execution with self-heal |
| `internal/workflow/verify.go` | Build/test/lint verification |
| `internal/workflow/runtime.go` | Dev server management |
| `internal/workflow/ship.go` | Changelog, commit, push, PR |

### Provider Layer
| File | Purpose |
|------|---------|
| `internal/provider/interface.go` | LLMProvider interface (7 methods) |
| `internal/provider/registry.go` | Thread-safe provider registry |
| `internal/provider/base_client.go` | Shared HTTP, caching, SSE, health check |
| `internal/provider/openrouter/client.go` | OpenRouter implementation |
| `internal/provider/zen/client.go` | Zen implementation |
| `internal/provider/nvidia/client.go` | NVIDIA NIM implementation |

### Tools System
| File | Purpose |
|------|---------|
| `internal/tools/interface.go` | Tool interface + SchemaProvider |
| `internal/tools/dispatcher.go` | Central executor (523 lines) |
| `internal/tools/defaults.go` | 18 tool registration |
| `internal/tools/fileops/*.go` | FileRead, FileWrite, Edit, FileList, FileDelete, FileMove |
| `internal/tools/exec/*.go` | Bash, DevServer |
| `internal/tools/search/*.go` | Glob, Grep, WebSearch, WebFetch |
| `internal/tools/ai/*.go` | AskUserQuestion |
| `internal/tools/subagent/manager.go` | Parallel sub-agents with Git worktrees |

### Shared Types
| File | Purpose |
|------|---------|
| `internal/types/types.go` | Core types (383 lines): WorkflowPhase, Message, Tool, Task, ModelInfo, IntentResult |
| `internal/types/constants.go` | 158 lines of constants, SkipDirsMap |
| `internal/types/plan.go` | Plan, PlanOutline, PlanIssue types |
| `internal/types/toolcall.go` | ToolCall, ToolInput, ToolResult, ToolError |

### Session & Persistence
| File | Purpose |
|------|---------|
| `internal/session/manager.go` | Session CRUD, checkpoints, project-local storage |
| `internal/session/session_info.go` | Session metadata struct |
| `internal/ledger/ledger.go` | LEDGER.md append-only records |
| `internal/rollback/rollback.go` | Git-based rollback to checkpoints |

### Cross-Cutting
| File | Purpose |
|------|---------|
| `internal/errors/errors.go` | Sentinel errors + UserMessage() |
| `internal/metrics/collector.go` | Session metrics → METRICS.json |
| `internal/narrative/engine.go` | Event classification & templates |
| `internal/decision/logger.go` | Decision receipt ring buffer |
| `internal/context/registry.go` | Dynamic context sources (git, env, datetime) |
| `internal/compaction/compaction.go` | LLM summarization with calibration |
| `internal/tokens/estimator.go` | Token estimation with EMA calibration |

---

*Structure analysis: Tue Jul 21 2026*