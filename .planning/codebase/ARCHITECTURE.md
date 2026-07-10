# M31A System Architecture

## High-Level Architecture

M31A is organized into three concentric layers, each with a clear role boundary:

```
┌──────────────────────────────────────────┐
│  cmd/m31a/  — Binary entry point          │
│  (flag parsing, wiring, tea.Program)      │
├──────────────────────────────────────────┤
│  internal/  — Private application logic  │
│  ┌────────┐ ┌──────────┐ ┌────────────┐  │
│  │  tui/  │ │workflow/ │ │ provider/  │  │
│  │33 scrns│ │ 7 phases │ │3 providers │  │
│  └────────┘ └──────────┘ └────────────┘  │
│  ┌────────┐ ┌──────────┐ ┌────────────┐  │
│  │ tools/ │ │ types/   │ │codeintel/  │  │
│  │18 tools│ │shared    │ │4-lang index│  │
│  └────────┘ └──────────┘ └────────────┘  │
├──────────────────────────────────────────┤
│  pkg/  — Importable utility packages     │
│  session · ledger · rollback · keychain  │
│  autodream · taskrunner · metrics · ...  │
└──────────────────────────────────────────┘
```

**Dependency rule:** `internal/` may import `pkg/`. `pkg/` must NOT import `internal/`. This is enforced by the Go module system. `cmd/m31a` imports both.

---

## Bubble Tea Elm Architecture

The TUI is built on [Bubble Tea](https://github.com/charmbracelet/bubbletea), which follows the Elm architecture.

### Model — `AppState`

`internal/tui/app_state.go` — `AppState` is the single top-level model:

- Holds all UI state: screen routing, sub-models (one per screen), workflow phase, provider/model refs, permission modal state, toast queue, etc.
- Has 33+ sub-model fields (one per TUI screen, e.g. `replModel`, `planModel`, `verifyModel`, `settingsModel`).
- Holds a reference to `workflowEngineInterface` (satisfied by `*workflow.Engine`) and coordinates workflow phases.
- **No mutex needed** — Bubble Tea guarantees single-threaded access to the model. The comment at the struct definition explicitly states this.

```go
// AppState is the top-level Bubble Tea model.
// All state mutations go through Update(). The Bubble Tea runtime guarantees
// single-threaded access to the model, so no mutex is needed.
type AppState struct { ... }
```

### Update — `(m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd)`

`internal/tui/app_update.go` — The single dispatch point for all messages.

- Matches on message type via a `switch msg := msg.(type)` statement.
- Each case delegates to a handler in the corresponding `handler_*.go` file, keeping `Update()` as a thin dispatcher.
- Returns a new model and an optional `tea.Cmd` (a function that will run asynchronously).
- **Critical invariant:** Never mutate `AppState` from a goroutine. All state changes must flow through here.

Handler files split by concern:
- `handler_workflow.go` — workflow phase transitions
- `handler_config.go` — config changes
- `handler_modal.go` — permission modals
- `handler_navigation.go` — screen routing
- `handler_sidebar.go` — sidebar refresh
- `handler_stream.go` — LLM streaming chunks
- `handler_tool.go` — tool call results
- `handler_runtime.go` — dev server events

### View — `(m *AppState) View() string`

`internal/tui/app_view.go` — Renders the full terminal frame as a string.

- Uses a unified `PageLayout` system: 1-line header + content viewport + 1-line footer.
- Delegates to per-screen renderers based on `m.screen` (a `Screen` enum).
- Supports visual screen transitions (slide/fade compositing via `RenderTransition`).
- Renders at up to 10fps using a tick-driven render rate limiter.

---

## Workflow Engine Phases

### Overview

`internal/workflow/engine.go` — `Engine` orchestrates 7 sequential phases. Entry via `RunPhase(ctx, phase, goal)`.

```
Idle → Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship → Idle
```

### Phase Dispatch

`RunPhase` is a central switch statement:

```go
switch phase {
case PhaseInitialize: result, err = e.runInitialize(ctx, goal)
case PhaseDiscuss:    result, err = e.runDiscuss(ctx, goal)
case PhasePlan:       result, err = e.runPlan(ctx, goal)
case PhaseExecute:    result, err = e.runExecute(ctx, goal)
case PhaseVerify:     result, err = e.runVerify(ctx, goal)
case PhaseRuntime:    result, err = e.runRuntime(ctx, goal)
case PhaseShip:       result, err = e.runShip(ctx, goal)
}
```

Each phase implementation lives in its own file: `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `engine_verify.go`, `runtime.go`, `ship.go`.

### The 7 Phases

| # | Phase | Source Files | What Happens |
|---|-------|-------------|--------------|
| 1 | **Initialize** | `initialize.go`, `init_deep.go` | Detects project type; builds code intelligence index; optional deep analysis |
| 2 | **Discuss** | `discuss.go`, `discuss_check.go` | LLM asks clarifying questions; quality scoring; completeness checks |
| 3 | **Plan** | `plan.go`, `plan_check.go`, `plan_chunk.go` | Pre-plan research, structured task breakdown, coverage/security gates, chunked generation |
| 4 | **Execute** | `execute.go`, `execute_preflight.go`, `execute_quality.go` | Kahn topological sort, LLM tool calls, loop detection, quality gates |
| 5 | **Verify** | `engine_verify.go`, `verify.go`, `verify_report.go` | Build + test, self-healing (2 retries), structured verification report |
| 6 | **Runtime** | `runtime.go` | Dev server lifecycle, HTTP smoke tests, route discovery |
| 7 | **Ship** | `ship.go`, `ship_preflight.go` | Pre-ship checklist, git commit, changelog generation, ledger entry |

### Phase Skipping (WorkflowMode)

`internal/types/types.go` defines four modes controlling which phases run:

```go
ModeAuto   WorkflowMode = "auto"   // classify intent, choose automatically
ModeFull   WorkflowMode = "full"   // all 7 phases
ModeFast   WorkflowMode = "fast"   // skip Plan: Init→Discuss→Exec→Verify→Ship
ModeDirect WorkflowMode = "direct" // skip Discuss+Plan+Verify: Init→Exec→Ship
```

Mode is resolved from LLM intent classification (`IntentResult`) via `WorkflowModeForIntent()`.

### StateMachine

`internal/workflow/state_machine.go` — `StateMachine` validates phase transitions against a predefined graph:

```go
validTransitions: {
    PhaseIdle:       {PhaseInitialize},
    PhaseInitialize: {PhaseDiscuss, PhaseExecute, PhaseIdle},
    PhaseDiscuss:    {PhasePlan, PhaseExecute, PhaseIdle},
    PhasePlan:       {PhaseExecute, PhasePlan, PhaseDiscuss, PhaseIdle},
    PhaseExecute:    {PhaseVerify, PhaseShip, PhaseIdle},
    PhaseVerify:     {PhaseRuntime, PhaseShip, PhaseExecute, PhaseIdle},
    PhaseRuntime:    {PhaseShip, PhaseExecute, PhaseIdle},
    PhaseShip:       {PhaseIdle},
}
```

A `discussPlanCycles` counter caps Plan↔Discuss oscillation at 3 cycles (BUG-12 prevention).

### Engine Decomposition (v1.7)

Rather than a monolithic engine, responsibilities are split across focused structs:

| Component | File | Role |
|-----------|------|------|
| `Engine` | `engine.go` | Phase dispatch, LLM streaming, subsystem orchestration |
| `WorkflowState` | `engine.go` | Mutable session state (plan content, messages, intent, decisions) |
| `StateMachine` | `state_machine.go` | Phase transition validation and history |
| `WorkflowCache` | `workflow_cache.go` | Cross-phase result caching (project state, code intel) |
| `PhaseCoordinator` | `phase_coordinator.go` | Pre/post phase setup, metrics, transition side-effects |
| `ContextBuilder` | `context_builder.go` | Builds system prompts per phase |
| `CostTracker` | `cost_tracker.go` | Cumulative LLM cost with budget enforcement |
| `PromptBuilder` | `prompt_builder.go` | Loads prompt templates from disk or embedded defaults |

---

## Component Relationships and Data Flow

```
main.go
  │ constructs
  ▼
AppState (tea.Model)
  │ holds reference to
  ├─► workflow.Engine
  │     │ calls
  │     ├─► provider.LLMProvider  (streams LLM responses via SSE)
  │     ├─► tools.Dispatcher      (executes tool calls with permission gating)
  │     ├─► session.Manager       (persists messages, tasks, checkpoints)
  │     └─► pkg/*                 (ledger, metrics, compaction, retry, taskrunner)
  │
  ├─► provider.Registry           (manages 3 providers; auto-fallback)
  ├─► tools.Dispatcher            (permissions, rate limiting, concurrency)
  ├─► git.Git                     (commit, diff, rollback)
  └─► subagent.Manager            (parallel child agents in isolated worktrees)

Workflow → TUI data flow:
  Engine.RunPhase()  (goroutine via tea.Cmd)
    │ emits via MsgEmitter interface
    ▼
  channelEmitter → buffered chan tea.Msg (capacity=512)
    │ drained via tea.Cmd chain
    ▼
  AppState.Update(msg) → mutates model → View() re-renders
```

---

## State Management

### AppState — UI state

The `AppState` struct in `app_state.go` is the single source of truth for all TUI state. Key groupings:

| Group | Key Fields |
|-------|-----------|
| Screen routing | `screen Screen`, `prevScreen`, `screenStack []Screen`, `screenCap int` |
| Workflow | `workflowEngine`, `workflowPhase`, `workflowMode`, `workflowGoal`, `workflowCancel` |
| Provider/model | `registry *provider.Registry`, `activeProvider string`, `activeModel *types.ModelInfo` |
| Sub-models | `replModel`, `planModel`, `verifyModel`, `settingsModel`, ... (33+ screens) |
| Permissions | `permRequest *tools.PermissionRequest`, `permModal *components.PermissionModal` |
| Toasts | `toasts []Toast`, `toastTimers map[int]*time.Timer` |
| Workflow channel | `emitterCh chan tea.Msg` |

### WorkflowState — engine state

`WorkflowState` (embedded in `Engine`) holds mutable engine state:

- `Messages []types.Message` — the active conversation history
- `planMarkdown string`, `planVersion int` — current plan and revision counter
- `intentResult *types.IntentResult` — LLM-classified user intent
- `decisionLog *decision.Logger` — ring-buffer of structured decisions (capacity 256)
- `checkpointData *CheckpointData` — for session resume across crashes
- `transitionMu sync.Mutex` — serializes phase transitions to prevent interleaved checkpoint saves

### tea.Cmd — async work

All asynchronous work (LLM calls, tool execution, file I/O) is modeled as `tea.Cmd`:

```go
// A tea.Cmd is a function that returns a tea.Msg
type Cmd func() Msg
```

Goroutines spawned by `tea.Cmd` communicate back to `Update()` via the returned `tea.Msg`. This keeps model mutation single-threaded.

### tea.Batch — composing commands

Multiple commands are composed with `tea.Batch(cmds...)`. The `Init()` sequence batches: health ticks, permission listeners, sidebar refresh, file watcher, config watcher, and subagent event listener.

---

## Concurrency Model

### Single-threaded TUI contract

Bubble Tea enforces a single-threaded model: only one goroutine ever calls `Update()` or `View()`. Documented in `app_update.go`:

```
// CRITICAL: Never mutate AppState from a goroutine. All mutations go here.
```

### Goroutines via tea.Cmd

Blocking work runs in goroutines spawned by `tea.Cmd`. The goroutine emits results by returning a `tea.Msg`, which Bubble Tea delivers to `Update()` on the main loop.

### Workflow → TUI bridge

`app_channel.go` — The `channelEmitter` implements `workflow.MsgEmitter`:

- Workflow runs in a goroutine (started by `tea.Cmd`).
- The engine calls `e.emit(msg)` which writes to a `chan tea.Msg` (capacity 512).
- Retry logic (3 attempts, 5ms backoff) prevents silent drops; overflow is counted atomically in `globalDropCounter`.
- A draining `tea.Cmd` reads up to `maxDrainPerTick=4` messages per tick and delivers them to `Update()`.

### Signal handling

`main.go` — A dedicated goroutine listens for `SIGTERM`/`SIGINT` and sends `tea.QuitMsg{}` through the program channel (not via `app.Shutdown()` directly), preserving the single-threaded contract. A 5-second hard fallback writes a `.force-exit` sentinel and calls `os.Exit(1)` if the TUI does not quit gracefully.

### Tools concurrency

`internal/tools/dispatcher.go` — Two layers of concurrency control:

- **Semaphore:** `concurrencySem chan struct{}` limits concurrent tool executions to 8.
- **Token bucket rate limiter:**
  - Normal tools: 20 burst / 10 sustained
  - Dangerous/destructive tools: 5 burst / 2 sustained

---

## Key Design Patterns

### Dispatcher Pattern (Tools)

`internal/tools/dispatcher.go` — All 18 tools are registered with a central `Dispatcher`. When the LLM requests a tool call:

1. Dispatcher looks up the tool by name in `tools map[string]types.Tool`.
2. Evaluates permission rules (`rules []config.PermissionRule`) and batch approvals.
3. If prompt required, sends `PermissionRequest` to TUI via `requestCh` channel.
4. Waits for `PermissionResponse` from TUI (with configurable timeout, default 300s).
5. Acquires rate-limit tokens and `concurrencySem` slot.
6. Calls `tool.Execute(ctx, input)`.
7. Returns `ToolResult` to the workflow engine.

### Permission System

`internal/tools/permissions.go` — Each tool declares a `RiskLevel`:

```go
RiskSafe        // file reads, symbol lookups
RiskMedium      // web fetches, searches
RiskDangerous   // file writes, edits
RiskDestructive // delete, bash commands
```

The TUI renders a modal with `y` (allow once) / `a` (allow always, task-scoped) / `n` (deny) / `e` (exit) options. Batch approvals are stored per `"toolName:riskLevel"` key and expire on phase transition.

### Per-Phase Model Assignment

`engine.go` — `modelForPhase()` resolves the model ID in priority order:
1. Interactive per-phase override (`perPhaseModels map[WorkflowPhase]string`) set by TUI
2. `[agents]` section in `config.toml` (per-phase TOML overrides)
3. `cfg.Agents.Default`
4. Engine's active `modelID`

This enables cheap models for Discuss/Plan and powerful models for Execute/Verify/Ship.

### Checkpoint/Resume

After each phase, `SaveCheckpointData()` persists `CheckpointData` (phase, goal, plan version, decisions) to disk via `session.Manager.SaveCheckpoint()`. On launch with `resume_on_startup = true`, `LoadCheckpointData()` restores engine state from the most recent checkpoint, enabling mid-workflow resume after crashes or `Ctrl+C`.

### AutoDream Context Compression

`pkg/autodream` — When the conversation history approaches the context window limit (threshold: 60% of `ContextLength`), the `Consolidator` replaces the middle portion of `Messages` with an LLM-generated summary. Protected messages (initial goal, tool calls, plans, last 5 messages) are never compacted.

### Narrative Engine

`pkg/narrative` + `internal/tui/narrative.go` — Raw workflow events (`ToolCallMsg`, `PhaseStartMsg`, etc.) are transformed into human-readable progress strings using a template-based `narrative.Engine`. Timing-based deduplication prevents repeated identical descriptions within a short window.
