<!-- refreshed: 2026-07-13 -->
# Architecture

**Analysis Date:** 2026-07-13

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLI Entry Point                                │
│                     `cmd/m31a/main.go` — flag parsing, config load,         │
│                     provider registration, TUI construction                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                TUI Layer                                    │
│                     `internal/tui/` — Bubble Tea (Elm architecture)         │
│  ┌──────────┬───────────┬────────────┬────────────┬───────────┬───────────┐ │
│  │ App      │  REPL     │  Sidebar   │  Workflow  │  Modals   │  Screens  │ │
│  │ `app.go` │ `repl.go` │ `sidebar_` │ `execute_` │ `perm_`   │ `screen.go│ │
│  │          │           │  model.go` │  model.go` │  model.go`│           │ │
│  └──────────┴───────────┴────────────┴────────────┴───────────┴───────────┘ │
├─────────────────────────────────────────────────────────────────────────────┤
│                            Workflow Engine                                  │
│            `internal/workflow/` — 7-phase state machine                     │
│   Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship           │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐  ┌───────────────────┐  ┌──────────────────────────┐  │
│  │   Tool System    │  │  Provider System   │  │   Context System         │  │
│  │ `internal/tools/`│  │ `internal/provider/`│  │ `internal/context/`     │  │
│  │ 18 built-in tools│  │ OpenRouter/Zen/Nvidia│ │ Dynamic context sources │  │
│  └──────────────────┘  └───────────────────┘  └──────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────────────┤
│                              PKG Layer                                       │
│  ┌──────────┬───────────┬────────────┬────────────┬───────────┬───────────┐ │
│  │ session  │ taskrunner│  rollback  │  bisect    │ compaction│  metrics  │ │
│  │ keychain │  ledger   │  autodream │  retry     │ narrative │  skills   │ │
│  └──────────┴───────────┴────────────┴────────────┴───────────┴───────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| CLI Entry | Flag parsing, config load, provider registration, signal handling | `cmd/m31a/main.go` |
| AppState | Top-level Bubble Tea model; all state mutations through Update() | `internal/tui/app_state.go` |
| AppState.Update | Single dispatch point for all messages; thin routing to handlers | `internal/tui/app_update.go` |
| ReplModel | Chat REPL: text input, message rendering, streaming | `internal/tui/repl.go` |
| SidebarModel | Session browser, file tree, workflow phase indicators | `internal/tui/sidebar_model.go` |
| Engine | Orchestrates 7 workflow phases, LLM streaming, tool dispatch | `internal/workflow/engine.go` |
| StateMachine | Validates phase transitions, prevents oscillation | `internal/workflow/state_machine.go` |
| Dispatcher | Tool registration, permission gating, rate limiting, concurrency control | `internal/tools/dispatcher.go` |
| Registry | Provider registration, active provider tracking | `internal/provider/registry.go` |
| Session Manager | Session CRUD, checkpoint save/restore, task persistence | `pkg/session/manager.go` |
| ContextBuilder | Builds LLM context for each phase with token budgeting | `internal/workflow/context_builder.go` |
| PromptBuilder | Loads prompt templates with 4-level priority chain | `internal/workflow/prompt_builder.go` |
| PhaseCoordinator | Pre/post-phase lifecycle, metrics, transition side effects | `internal/workflow/phase_coordinator.go` |

## Pattern Overview

**Overall:** Elm Architecture (Model-Update-View) with a 7-phase workflow state machine.

**Key Characteristics:**
- Strict single-threaded state mutation via Bubble Tea's `Update()` — no goroutine-shared mutable state
- Workflow phases execute as `tea.Cmd` coroutines that emit `tea.Msg` back to the TUI
- Tool execution is gated through a permission system with batch approval and rate limiting
- Provider abstraction enables multiple LLM backends without code changes
- `pkg/` layer must never import `internal/` — enforced by Go module boundary

## Layers

**CLI (`cmd/m31a/`):**
- Purpose: Application entry point, flag parsing, signal handling
- Location: `cmd/m31a/main.go`
- Contains: `main()`, `run()`, `runHeadless()`, signal handler, terminal restore
- Depends on: `internal/config`, `internal/provider`, `internal/tui`, `pkg/keychain`, `pkg/session`
- Used by: OS process

**TUI (`internal/tui/`):**
- Purpose: Terminal UI rendering, user interaction, screen routing
- Location: `internal/tui/`
- Contains: `AppState` (top model), 30+ screen models, `Router`, streaming infrastructure
- Depends on: `internal/workflow`, `internal/tools`, `internal/provider`, `pkg/session`
- Used by: CLI entry point

**Workflow (`internal/workflow/`):**
- Purpose: 7-phase orchestration of LLM interactions and tool execution
- Location: `internal/workflow/`
- Contains: `Engine`, `StateMachine`, `PhaseCoordinator`, `ContextBuilder`, `PromptBuilder`
- Depends on: `internal/provider`, `internal/tools`, `internal/codeintel`, `pkg/session`, `pkg/taskrunner`
- Used by: TUI (via `AppState.workflowEngine`)

**Tools (`internal/tools/`):**
- Purpose: 18 built-in tools for file operations, code analysis, bash execution
- Location: `internal/tools/`
- Contains: Tool implementations, `Dispatcher`, permission system, rate limiting
- Depends on: `internal/types`, `internal/config`
- Used by: Workflow engine, TUI dispatcher

**Provider (`internal/provider/`):**
- Purpose: LLM provider abstraction with 3 backends (OpenRouter, Zen, Nvidia)
- Location: `internal/provider/`
- Contains: `LLMProvider` interface, `Registry`, SSE streaming, model capability detection
- Depends on: `internal/types`, `internal/errors`
- Used by: Workflow engine, TUI

**Types (`internal/types/`):**
- Purpose: Shared type vocabulary across all layers
- Location: `internal/types/`
- Contains: `Message`, `ToolCall`, `Task`, `WorkflowPhase`, `ModelInfo`, `Tool` interface
- Depends on: None (leaf package)
- Used by: Everything

**PKG Layer (`pkg/`):**
- Purpose: Reusable packages that must NOT import `internal/`
- Location: `pkg/`
- Contains: `session`, `taskrunner`, `rollback`, `bisect`, `compaction`, `metrics`, `keychain`, `ledger`, `narrative`, `autodream`, `arbitrage`, `coordinator`, `retry`, `skills`, `history`
- Depends on: `internal/types` only (for type definitions)
- Used by: `internal/` packages

## Data Flow

### Primary Request Path (User sends a message)

1. User types in REPL textarea → `ReplModel.Update()` handles `tea.KeyMsg` (`internal/tui/repl.go:72`)
2. Message sent to LLM provider → `provider.ChatCompletionStream()` (`internal/provider/interface.go:14`)
3. Stream chunks emitted as `StreamMsg` → `handleStreamMsg()` (`internal/tui/app_update.go:93`)
4. Tool calls returned → `Dispatcher.Execute()` with permission gating (`internal/tools/dispatcher.go`)
5. Tool results sent back to LLM → next `ChatCompletionStream()` iteration
6. Final response rendered in REPL viewport (`internal/tui/repl_view.go`)

### Workflow Phase Execution

1. User submits goal → `GoalSubmittedMsg` → `Engine.RunPhase(ctx, PhaseInitialize, goal)` (`internal/workflow/engine.go:676`)
2. Phase runs as `tea.Cmd` → `workflowPhaseCmd()` emits `PhaseResultMsg`
3. `AppState` receives `PhaseResultMsg` → advances `workflowPhaseIndex` (`internal/tui/app_update_phase.go`)
4. Next phase dispatched via `Engine.RunPhase()` → repeat until `PhaseShip`
5. Each phase emits progress via `MsgEmitter` → `channelEmitter` → `tea.Cmd`

### Tool Execution Flow

1. LLM returns `ToolCall` in stream chunk → `tools.HandleToolCalls()` (`internal/tools/toolcall.go`)
2. `Dispatcher.Execute()` checks permissions → `PermissionRequest` sent to TUI channel
3. User approves → `PermissionResponse` → tool execution proceeds
4. Tool result returned → appended to messages → next LLM iteration

**State Management:**
- Bubble Tea guarantees single-threaded access to `AppState` — no mutex needed
- Workflow engine runs in a goroutine but communicates only via `MsgEmitter` channels
- `WorkflowState` uses `sync.RWMutex` for `Messages`, `planMarkdown`, `planVersion` — read by TUI, written by workflow goroutine
- Session state persisted to disk via `session.Manager` at `<workDir>/.m31a/`

## Key Abstractions

**Tool Interface:**
- Purpose: Uniform contract for all 18 built-in tools
- Examples: `internal/tools/bash.go`, `internal/tools/edit.go`, `internal/tools/grep.go`
- Pattern: `Name()`, `Description()`, `RiskLevel()`, `Execute(ctx, ToolInput) (ToolResult, error)`

**LLMProvider Interface:**
- Purpose: Uniform contract for LLM backends
- Examples: `internal/provider/openrouter/`, `internal/provider/zen/`, `internal/provider/nvidia/`
- Pattern: `FetchModels()`, `ChatCompletionStream()`, `HealthCheck()`, `GetModel()`

**Screenable Interface:**
- Purpose: Uniform contract for TUI screens
- Examples: `internal/tui/repl.go`, `internal/tui/sidebar_model.go`, `internal/tui/plan_model.go`
- Pattern: `Init()`, `Update(msg)`, `View()`, `SetDimensions(w,h)`, `SetTheme(t)`

**WorkflowPhase:**
- Purpose: Type-safe phase identifiers for the 7-phase state machine
- Examples: `internal/types/types.go` (PhaseInitialize through PhaseShip)
- Pattern: String constants with `StateMachine` transition validation

**Engine:**
- Purpose: Orchestrates entire workflow lifecycle
- Examples: `internal/workflow/engine.go`
- Pattern: `RunPhase()` dispatches to `runInitialize()`, `runDiscuss()`, etc.

## Entry Points

**CLI Entry (`cmd/m31a/main.go`):**
- Location: `cmd/m31a/main.go`
- Triggers: `go run ./cmd/m31a` or compiled binary
- Responsibilities: Flag parsing, config load, provider registration, TUI launch, signal handling

**TUI Init (`internal/tui/app.go`):**
- Location: `internal/tui/app.go:24`
- Triggers: `tea.NewProgram(app).Run()`
- Responsibilities: Start health ticker, permission listener, subagent listener, file watcher

**TUI Update (`internal/tui/app_update.go`):**
- Location: `internal/tui/app_update.go:19`
- Triggers: Any `tea.Msg` from Bubble Tea runtime
- Responsibilities: Single dispatch point for all messages — keyboard, mouse, streaming, workflow results

**Workflow Engine RunPhase:**
- Location: `internal/workflow/engine.go:676`
- Triggers: TUI sends goal or advances workflow
- Responsibilities: Budget check, pre-phase setup, phase execution, post-phase metrics

## Architectural Constraints

- **Threading:** Bubble Tea is strictly single-threaded for `AppState`. Workflow engine runs in a goroutine but communicates via `MsgEmitter` channels only. Never mutate `AppState` from a goroutine.
- **Global state:** `slog.SetDefault(logger)` in `main.go` — process-wide. `types.DirPermission` constant. No module-level singletons in application code.
- **Circular imports:** Enforced by Go module system — `pkg/` must NOT import `internal/`. `internal/types` is the shared vocabulary.
- **CGO_ENABLED=0:** Hard constraint — binary must be static. No C dependencies allowed.

## Anti-Patterns

### Mutating AppState from goroutines

**What happens:** Workflow engine goroutine calls `app.setState()` directly
**Why it's wrong:** Bubble Tea's single-threaded contract is violated, causing data races and state corruption
**Do this instead:** Emit `tea.Msg` via `MsgEmitter` channel → `AppState.Update()` handles it (`internal/tui/app_update.go`)

### Hardcoding model names

**What happens:** Code references `"claude-3-opus"` or similar model IDs
**Why it's wrong:** Provider model lists are dynamic — models are discovered from APIs at runtime
**Do this instead:** Use `provider.GetModel(id)` or `provider.CachedModels()` to discover models dynamically (`internal/provider/registry.go`)

### Importing internal/ from pkg/

**What happens:** `pkg/session` imports `internal/tools`
**Why it's wrong:** Violates the module boundary — `pkg/` must be reusable independently
**Do this instead:** `pkg/` packages only import `internal/types` for type definitions (`pkg/session/session.go`)

## Error Handling

**Strategy:** Errors are returned, never panicked. Structured error types with sentinel values for programmatic matching.

**Patterns:**
- Sentinel errors: `internal/errors/errors.go` defines `ErrProviderUnreachable`, `ErrPhaseTransition`, `ErrTaskFailed`, etc.
- Wrapping: `fmt.Errorf("context: %w", err)` — always wrap with context
- ToolError: `internal/types/types.go:207` — structured error with `Err` + `Hint` for LLM self-recovery
- Phase errors: `PhaseResult.Error` string + `error` return from `RunPhase()`

## Cross-Cutting Concerns

**Logging:** `log/slog` with file rotation in `~/.m31a/m31a.log` (`internal/log/log.go`). Structured key-value pairs. No emojis in log messages.

**Validation:** Config validated at load time (`internal/config/loader.go`). Permission rules validated in `Dispatcher` constructor. Phase transitions validated by `StateMachine.Transition()`.

**Authentication:** API keys stored in OS keychain via `pkg/keychain/`. Never written to disk in plaintext. `config.LoadDotEnv()` loads `.env` before any keychain access.

**Metrics:** `pkg/metrics/Collector` captures tool calls, LLM usage, phase durations, heal events. Session-scoped, not persistent.

**Decision Logging:** `internal/decision/` logs intent classifications, model selections, phase transitions. Emitted to TUI via `DecisionsSnapshotMsg`.

---

*Architecture analysis: 2026-07-13*
