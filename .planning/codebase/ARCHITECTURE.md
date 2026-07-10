<!-- refreshed: 2026-07-10 -->
# Architecture

**Analysis Date:** 2026-07-10

## System Overview

```text
┌───────────────────────────────────────────────────────────────────────┐
│                          CLI Entry Point                              │
│                   `cmd/m31a/main.go`                                  │
│   flag parsing, config load, provider registration, TUI launch        │
├───────────┬───────────────────────────────────┬───────────────────────┤
│  Provider │          TUI (Bubble Tea)          │   Session Manager     │
│ Registry  │         `internal/tui/`            │   `pkg/session/`      │
│`internal/ │  AppState (single-threaded model)  │   checkpoint, resume  │
│ provider/ │  screens, components, commands     │   persistence         │
│ registry  │  Update() is the ONLY mutator      │                       │
└─────┬─────┴──────────────┬────────────────────┴──────────┬────────────┘
      │                    │                              │
      ▼                    ▼                              ▼
┌───────────────────────────────────────────────────────────────────────┐
│                     Workflow Engine                                    │
│               `internal/workflow/engine.go`                           │
│                                                                       │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌───────────────────┐   │
│   │Initialize│→ │ Discuss  │→ │   Plan   │→ │     Execute       │   │
│   └──────────┘  └──────────┘  └──────────┘  └───────┬───────────┘   │
│                                                       │              │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐           │              │
│   │  Ship    │← │ Runtime  │← │  Verify  │←──────────┘              │
│   └──────────┘  └──────────┘  └──────────┘                          │
│                                                                       │
│   StateMachine ─ ContextBuilder ─ CostTracker ─ PhaseCoordinator     │
│   PromptBuilder ─ DecisionLogger ─ Compactor ─ CodeIntel Indexer     │
├───────────────────────────────────────────────────────────────────────┤
│                     Tools Dispatcher                                   │
│                `internal/tools/dispatcher.go`                         │
│                                                                       │
│   18 built-in tools: Bash, FileRead, FileWrite, Edit, Grep, Glob,   │
│   FileList, FileDelete, FileMove, CodeMap, CodeComplexity,           │
│   WebFetch, WebSearch, DevServer, HTTPCheck, AskUserQuestion,        │
│   TodoWrite, TodoRead, Agent                                         │
│                                                                       │
│   ┌──────────┐  ┌──────────┐  ┌────────────────────────────────┐    │
│   │Permissions│ │Rate Limits│  │  Subagent Manager (parallel)   │    │
│   └──────────┘  └──────────┘  │  `internal/tools/subagent/`    │    │
│                                └────────────────────────────────┘    │
└───────────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌────────────────┐  ┌────────────────┐  ┌────────────────────────────┐
│   LLM Provider │  │   Git Client   │  │   Support Packages (pkg/)  │
│   Layer         │  │   `internal/   │  │                            │
│   `internal/    │  │    git/`       │  │  session    taskrunner     │
│    provider/`   │  │                │  │  rollback   ledger         │
│                 │  │   Shell        │  │  keychain   narrative      │
│  ┌───────────┐  │  │   `internal/   │  │  compaction metrics       │
│  │OpenRouter │  │  │    shell/`     │  │  arbitrage  coordinator   │
│  │Zen        │  │  │                │  │  history    retry         │
│  │NVIDIA NIM │  │  │   CodeIntel    │  │  bisect     autodream     │
│  └───────────┘  │  │   `internal/   │  │  skills                  │
│                 │  │    codeintel/` │  │                            │
└────────────────┘  └────────────────┘  └────────────────────────────┘
         │                                       │
         ▼                                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Config & Types                                     │
│  `internal/config/`        `internal/types/`                        │
│  TOML-based config         Shared type vocabulary (Phase, Message,  │
│  Hot-reload via watcher    Task, ToolCall, ModelInfo, etc.)         │
│                                                                       │
│  `internal/errors/`        `internal/tokens/`                       │
│  Sentinel errors           Token estimation with EMA calibration    │
│                                                                       │
│  `internal/context/`       `internal/decision/`                     │
│  Dynamic system context    Decision logging for audit trail         │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| CLI Entry | Flag parsing, provider registration, TUI launch, headless mode | `cmd/m31a/main.go` |
| AppState | Single Bubble Tea model, owns ALL state mutations via `Update()` | `internal/tui/app_state.go` |
| Engine | Orchestrates 7-phase workflow, LLM streaming, tool dispatch | `internal/workflow/engine.go` |
| StateMachine | Validates and tracks phase transitions with history | `internal/workflow/state_machine.go` |
| ContextBuilder | Composes system prompts from base + dynamic context | `internal/workflow/context_builder.go` |
| PromptBuilder | Loads prompt templates with 4-level override chain | `internal/workflow/prompt_builder.go` |
| PhaseCoordinator | Delegates pre/post-phase setup and transition side effects | `internal/workflow/phase_coordinator.go` |
| CostTracker | Tracks LLM cost against configurable budget | `internal/workflow/cost_tracker.go` |
| DecisionLogger | Records decisions for audit trail | `internal/decision/logger.go` |
| Compactor | Auto-compaction of conversation history when context fills | `pkg/compaction/compaction.go` |
| Dispatcher | Tool registration, permission gates, rate limiting, concurrency | `internal/tools/dispatcher.go` |
| SubagentManager | Parallel child agents with independent worktrees | `internal/tools/subagent/manager.go` |
| Registry | Provider management, active provider selection, atomic swap | `internal/provider/registry.go` |
| SessionManager | Session persistence, checkpoint/resume, task persistence | `pkg/session/manager.go` |
| TaskRunner | Task scheduling with dependency resolution and parallel execution | `pkg/taskrunner/runner.go` |
| Git | Git operations for commit, branch, diff, worktree management | `internal/git/git.go` |
| CodeIntel | Codebase indexing, import graph, symbol lookup, relevance scoring | `internal/codeintel/codeintel.go` |
| Tokens | Token estimation with EMA calibration for context management | `internal/tokens/estimator.go` |
| Config | TOML-based configuration with hot-reload support | `internal/config/loader.go` |

## Pattern Overview

**Overall:** Elm Architecture (Bubble Tea) + Seven-Phase Workflow Engine

**Key Characteristics:**
- Strict single-threaded TUI state mutations via Bubble Tea's `Update()` method
- Workflow engine runs in goroutines, communicates back to TUI via `tea.Msg` channels
- Provider layer abstracts LLM APIs behind `LLMProvider` interface (OpenRouter, Zen, NVIDIA NIM)
- Tools are registered in a Dispatcher with permission gates, rate limiting, and concurrency control
- Subagents run in parallel with independent git worktrees
- `pkg/` packages are reusable (no imports from `internal/`), `internal/` contains app-specific logic
- Prompt templates are embedded via `go:embed` with 4-level override chain

## Layers

**CLI Entry (`cmd/m31a/`):**
- Purpose: Application bootstrap, flag parsing, provider registration, TUI creation
- Location: `cmd/m31a/main.go`
- Contains: `main()`, `run()`, `runHeadless()`, provider registration
- Depends on: `internal/config`, `internal/provider`, `internal/tui`, `pkg/keychain`, `pkg/session`
- Used by: User launches the binary

**TUI Layer (`internal/tui/`):**
- Purpose: Terminal UI with Bubble Tea Elm architecture
- Location: `internal/tui/`
- Contains: `AppState` (single model), screen models, handlers, components, layout
- Depends on: `internal/workflow`, `internal/tools`, `internal/provider`, `pkg/session`
- Used by: All user interaction

**Workflow Engine (`internal/workflow/`):**
- Purpose: Seven-phase workflow orchestration (Initialize -> Discuss -> Plan -> Execute -> Verify -> Runtime -> Ship)
- Location: `internal/workflow/`
- Contains: `Engine`, `StateMachine`, `ContextBuilder`, `PromptBuilder`, `CostTracker`, phase implementations
- Depends on: `internal/tools`, `internal/provider`, `internal/types`, `pkg/session`, `pkg/taskrunner`
- Used by: TUI via `WorkflowEngine` interface, runs phases in goroutines

**Tools Layer (`internal/tools/`):**
- Purpose: 18 built-in tool implementations with permission gating and rate limiting
- Location: `internal/tools/`
- Contains: Tool implementations (`Bash`, `FileRead`, `Edit`, etc.), `Dispatcher`, `Agent`
- Depends on: `internal/types`, `internal/config`
- Used by: Workflow engine dispatches tools via LLM tool calls

**Provider Layer (`internal/provider/`):**
- Purpose: LLM API abstraction with provider registry and capability detection
- Location: `internal/provider/`
- Contains: `LLMProvider` interface, `Registry`, provider implementations (`openrouter/`, `zen/`, `nvidia/`)
- Depends on: `internal/types`
- Used by: Workflow engine streams LLM responses

**Shared Types (`internal/types/`):**
- Purpose: Canonical type vocabulary shared across all layers
- Location: `internal/types/`
- Contains: `WorkflowPhase`, `Message`, `Task`, `ToolCall`, `ModelInfo`, `Tool` interface, constants
- Depends on: Nothing (leaf package)
- Used by: Every layer in the system

**Support Packages (`pkg/`):**
- Purpose: Reusable library code with no `internal/` imports
- Location: `pkg/`
- Contains: `session`, `taskrunner`, `rollback`, `ledger`, `keychain`, `narrative`, `compaction`, `metrics`, `arbitrage`, `coordinator`, `history`, `retry`, `bisect`, `autodream`, `skills`
- Depends on: Only `internal/types` (allowed boundary)
- Used by: Both `internal/` packages and potentially external consumers

## Data Flow

### Primary Request Path (User sends message)

1. User types in REPL, `AppState.Update()` receives `tea.KeyMsg` (`internal/tui/app_input.go`)
2. Input is routed to screen handler via `screenUpdaters` map (`internal/tui/app_routing.go`)
3. For workflow goals, intent is classified via LLM (`internal/workflow/classify.go`)
4. Workflow phases are launched as `tea.Cmd` goroutines (`internal/tui/app.go:RunPhaseCmd()`)
5. `Engine.RunPhase()` dispatches to phase-specific methods (`internal/workflow/engine.go:694-712`)
6. Phase methods call `Engine.streamLLM()` which streams via `provider.ChatCompletionStream()` (`internal/workflow/engine.go:1392-1401`)
7. LLM tool calls are dispatched via `Dispatcher.Dispatch()` (`internal/tools/dispatcher.go`)
8. Tool results are fed back as `types.Message` entries in the conversation
9. Phase completion emits `PhaseResultMsg` back to TUI via `MsgEmitter`
10. `AppState.Update()` processes the result and routes to next phase or idle

### Tool Dispatch Path

1. LLM response contains `ToolCall` entries (`internal/types/types.go:164-168`)
2. `Engine.consumeStreamWithTools()` collects both text and tool calls (`internal/workflow/engine.go:1231-1286`)
3. `Dispatcher.Dispatch()` checks permissions via `PermissionRequest` channel (`internal/tools/dispatcher.go`)
4. TUI shows permission modal if needed (`internal/tui/handler_tool.go`)
5. Tool's `Execute()` is called with rate limiting and concurrency control (`internal/tools/dispatcher.go:400+`)
6. `ToolResult` is returned and added to conversation as tool message

### Session Persistence Path

1. `SessionManager` saves to `<workDir>/.m31a/session.json` (`pkg/session/manager.go`)
2. Messages saved to `<workDir>/.m31a/messages.json`
3. Tasks saved to `<workDir>/.m31a/tasks.json`
4. Checkpoints saved for resume support (`pkg/session/checkpoint.go`)
5. On shutdown, `AppState.saveSessionOnShutdown()` persists all state (`internal/tui/app.go:206-244`)

**State Management:**
- TUI state: Single `AppState` struct, mutated only in `Update()` (Bubble Tea contract)
- Workflow state: `WorkflowState` inside `Engine`, protected by `transitionMu` for phase transitions
- Session state: Persisted to JSON files in `.m31a/` directory
- Provider state: `Registry` with `sync.RWMutex` for thread-safe provider switching
- Tool state: `Dispatcher` with `sync.RWMutex` for concurrent tool access

## Key Abstractions

**WorkflowEngine Interface:**
- Purpose: Decouples TUI from concrete workflow engine for testing
- Examples: `internal/tui/tuitypes/tuitypes.go:107`
- Pattern: Interface alias (`type WorkflowEngine = tuitypes.WorkflowEngine`)

**LLMProvider Interface:**
- Purpose: Abstracts LLM API differences across providers
- Examples: `internal/provider/interface.go:9-18`
- Pattern: Interface with `Name()`, `FetchModels()`, `ChatCompletionStream()`, `GetModel()`

**Tool Interface:**
- Purpose: Uniform interface for all 18 built-in tools
- Examples: `internal/types/types.go:241-246`
- Pattern: Interface with `Name()`, `Description()`, `RiskLevel()`, `Execute()`

**MsgEmitter:**
- Purpose: Decouples workflow engine from Bubble Tea program for message passing
- Examples: `internal/workflow/engine.go:834-838`
- Pattern: Callback function `func(msg any)` that sends to TUI's channel

**Screen Routing Map:**
- Purpose: Eliminates duplicated per-screen switch statements in Update()
- Examples: `internal/tui/app_state.go:257` (`screenUpdaters map[Screen]screenUpdateFunc`)
- Pattern: Map of `Screen` enum to `screenUpdateFunc` closures

## Entry Points

**CLI Main:**
- Location: `cmd/m31a/main.go`
- Triggers: User runs `m31a` binary
- Responsibilities: Flag parsing, config load, provider registration, TUI or headless mode

**TUI Init:**
- Location: `internal/tui/app.go:24`
- Triggers: Bubble Tea runtime calls `Init()` after `tea.NewProgram().Run()`
- Responsibilities: Start health ticker, permission listener, file watcher, session resume

**TUI Update:**
- Location: `internal/tui/app_update.go`
- Triggers: Every user input, timer tick, or message from goroutines
- Responsibilities: Route messages to screen handlers, mutate AppState

**Engine RunPhase:**
- Location: `internal/workflow/engine.go:654`
- Triggers: TUI's `RunPhaseCmd` goroutine
- Responsibilities: Execute a workflow phase (Initialize through Ship)

## Architectural Constraints

- **Threading:** Bubble Tea is strictly single-threaded. All state mutations happen in `Update()`. Workflow phases run in goroutines but communicate back via `tea.Msg` channels. Never mutate `AppState` from a goroutine.
- **Global state:** `slog.SetDefault(logger)` is set once at startup (`cmd/m31a/main.go:164`). No other module-level singletons.
- **Circular imports:** Enforced by Go module system. `pkg/` must NOT import `internal/`. `internal/types` is the leaf package.
- **Build constraint:** `CGO_ENABLED=0` is a hard constraint. Binary must be statically linked.
- **Provider models are dynamic:** Never hardcode model IDs. Models are discovered from provider APIs.
- **API keys via keychain:** `pkg/keychain/` stores keys in OS keychain, never plaintext in config files (except fallback).

## Anti-Patterns

### Mutating AppState from goroutines

**What happens:** Direct field assignment on `AppState` from a workflow goroutine.
**Why it's wrong:** Breaks Bubble Tea's single-threaded contract, causes data races and session corruption.
**Do this instead:** Emit a `tea.Msg` via `MsgEmitter` and handle it in `Update()`. See `internal/workflow/engine.go:834-838`.

### Hardcoding model names

**What happens:** Writing `"claude-3-5-sonnet"` or similar in code.
**Why it's wrong:** Provider model lists are dynamic. Models appear/disappear based on API availability.
**Do this instead:** Use `provider.FetchModels()` and `provider.GetModel()` to discover models at runtime.

### Importing internal from pkg

**What happens:** A `pkg/` package imports from `internal/`.
**Why it's wrong:** Violates Go module boundaries. Makes `pkg/` packages unusable outside this module.
**Do this instead:** Pass `internal/` dependencies as interfaces or parameters. `pkg/` may only import `internal/types`.

### Skipping permissions for destructive tools

**What happens:** Registering a tool with `RiskDestructive` but bypassing permission checks.
**Why it's wrong:** Destructive operations (file delete, bash with dangerous commands) need explicit user approval.
**Do this instead:** Let the `Dispatcher` handle permissions via the `PermissionRequest` channel.

## Error Handling

**Strategy:** Return errors, never panic. Wrap with `fmt.Errorf("%w", err)`.

**Patterns:**
- Sentinel errors defined in `internal/errors/errors.go` (e.g., `ErrPhaseTransition`, `ErrContextExceeded`, `ErrProviderNotFound`)
- Errors wrapped with `%w` for Go 1.13+ error chains
- `ToolError` type with `Err` + `Hint` for LLM self-recovery guidance (`internal/types/types.go:207-226`)
- Non-recoverable errors returned as `fmt.Errorf` with sentinel wrapping
- Recoverable errors (LLM failures, permission timeouts) retried via `pkg/retry/`

## Cross-Cutting Concerns

**Logging:** `log/slog` structured logging. Logger initialized in `cmd/m31a/main.go:157-164`. Set as default via `slog.SetDefault()`. Used throughout via `slog.Info/Warn/Error`.

**Validation:** Config validation in `internal/config/loader.go`. Permission rules validated at dispatcher creation time. Tool input validated per-tool via JSON schema.

**Authentication:** API keys resolved via OS keychain (`pkg/keychain/`) with platform-specific implementations (`keychain_darwin.go`, `keychain_linux.go`, `keychain_windows.go`). Fallback to config file. Keys never written to disk in plaintext.

**Metrics:** `pkg/metrics/` collector records tool calls, LLM usage, phase durations, heals. Persists to `METRICS.json` in session directory.

**Narrative:** `pkg/narrative/` engine classifies and groups tool call events for human-readable output. Configurable via `internal/config/types.go:NarrativeConfig`.

---

*Architecture analysis: 2026-07-10*
