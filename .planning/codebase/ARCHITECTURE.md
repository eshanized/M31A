# ARCHITECTURE.md — System Architecture

**Analysis Date:** 2026-07-10

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           M31A TUI Application                               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  Bubble Tea (Elm Architecture) — Single-threaded event loop            │  │
│  │  `internal/tui/app.go` — AppState implements tea.Model                 │  │
│  │  Init() → Update(msg) → View()                                         │  │
│  └──────────────────────────────┬────────────────────────────────────────┘  │
│                                 │ tea.Cmd / tea.Msg                          │
│                                 ▼                                            │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    Workflow Engine — 7 Phases                          │  │
│  │  `internal/workflow/engine.go` — Engine orchestrates phases           │  │
│  │  Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship      │  │
│  │  StateMachine (`state_machine.go`) validates phase transitions        │  │
│  │  PhaseCoordinator handles pre/post-phase hooks & metrics              │  │
│  └───────────────┬───────────────────────────┬───────────────────────────┘  │
│                  │                           │                              │
│                  ▼                           ▼                              │
│  ┌─────────────────────────┐   ┌─────────────────────────────────────┐    │
│  │    Provider Layer       │   │           Tool Dispatcher           │    │
│  │  `internal/provider/`   │   │     `internal/tools/dispatcher.go`  │    │
│  │  Registry → 3 Providers │   │  18 built-in tools (defaults.go)    │    │
│  │  - OpenRouter           │   │  Permissions, rate limiting,        │    │
│  │  - Zen                  │   │  concurrency, output bounding       │    │
│  │  - NVIDIA               │   │  Subagent spawning (Agent tool)     │    │
│  └─────────────────────────┘   └─────────────────────────────────────┘    │
│                  │                           │                              │
│                  └───────────────┬────────────┘                              │
│                                  ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    Shared Types — `internal/types/`                    │  │
│  │  WorkflowPhase, ModelInfo, Message, Tool, Task, IntentResult, etc.  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                  │                                          │
│                                  ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │              Public Packages — `pkg/` (no internal deps)               │  │
│  │  keychain, ledger, metrics, retry, rollback, session, taskrunner,     │  │
│  │  bisect, compaction, autodream, arbitrage, coordinator, narrative,    │  │
│  │  history, skills, rollback                                             │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Entry Point

**`cmd/m31a/main.go`** — Application bootstrap:
- Flag parsing (`--prompt`, `--goal`, `--model`, `--version`, `--help`)
- Config loading (`~/.m31a/config.toml` via `internal/config`)
- Logger initialization (`internal/log`)
- Keychain initialization (`pkg/keychain`) — OS keyring for API keys
- Provider registry registration (OpenRouter, Zen, NVIDIA) via `internal/tui/provider_registration.go`
- TUI construction (`internal/tui.NewApp`) with all dependencies:
  - Session manager (`pkg/session`)
  - Tools dispatcher (`internal/tools.DefaultDispatcher`)
  - Git client (`internal/git`)
  - Ledger (`pkg/ledger`)
  - Rollback (`pkg/rollback`)
  - AutoDream (`pkg/autodream`)
  - Subagent manager (`internal/tools/subagent`)
- Signal handling: SIGTERM/SIGINT → `tea.QuitMsg` via channel (preserves Bubble Tea single-threaded contract)
- Hard fallback: force exit after 5s, writes `.force-exit` sentinel

## Layer Architecture

### Layer 1: TUI (Bubble Tea / Elm Architecture)
**Location:** `internal/tui/`

| Component | File | Responsibility |
|-----------|------|----------------|
| AppState (Model) | `app.go`, `app_state.go` | Root model implementing `tea.Model`; holds all sub-models, workflow engine, dispatcher, session |
| Update | `app_update.go`, `app_update_phase.go`, `app_update_commands.go` | Single state mutation point; handles `tea.Msg` from workflow, tools, subagents, user input |
| View | `app_view.go` | Renders current screen; 45+ screen types in `tuitypes.Screen` |
| REPL | `repl.go`, `repl_model.go`, `repl_view.go` | Chat interface with streaming, markdown rendering, slash commands |
| Navigation | `app_nav.go` | Screen stack management (push/pop/replace) |
| Components | `components/` | Reusable UI: sidebar, model selector, discuss/plan/execute/verify/ship screens |
| Commands | `commands/` | Slash command implementations (`/model`, `/provider`, `/session`, etc.) |
| Types | `tuitypes/` | All `tea.Msg` types, Screen enum, shared TUI types |

**Key invariant:** All state mutations occur in `Update()`. Goroutines communicate via `tea.Cmd` → `tea.Msg` channels. Never mutate `AppState` from a goroutine.

### Layer 2: Workflow Engine
**Location:** `internal/workflow/engine.go` (1,494 lines)

**Core types:**
- `Engine` — Orchestrates 7 phases, holds provider, dispatcher, token estimator, session manager, config
- `WorkflowState` — Mutable session state (plan, messages, intent, decisions, checkpoints)
- `StateMachine` — `internal/workflow/state_machine.go` — Validates phase transitions
- `PhaseCoordinator` — `phase_coordinator.go` — Pre/post-phase hooks, metrics, checkpoints

**Seven Phases (WorkflowPhase enum in `internal/types/types.go`):**
```
PhaseIdle → PhaseInitialize → PhaseDiscuss → PhasePlan → PhaseExecute → PhaseVerify → PhaseRuntime → PhaseShip → PhaseIdle
```

**Phase transitions** validated by `StateMachine.validTransitions` map. Plan↔Discuss oscillation guarded (max 3 cycles).

**Phase execution flow** (`RunPhase`):
1. Budget guardrail check (`costTracker.TotalCost()` vs `BudgetLimitUSD`)
2. `PhaseCoordinator.PrePhaseSetup` — compaction check, checkpoint save
3. Phase-specific `run<Phase>` method (e.g., `runExecute`, `runPlan`)
4. `PhaseCoordinator.PostPhaseExecution` — metrics, duration recording
5. Return `PhaseResult` (tasks, messages, error, cost, usage, tool calls)

**Streaming LLM calls:**
- `streamLLMWithTools` / `streamLLM` / `streamLLMStreaming` — unified request prep, preflight context check, retry with exponential backoff
- `consumeStreamWithTools` — parses native tool_call chunks + text deltas
- `preflightContextCheck` — estimates tokens, triggers auto-compaction or truncation at 80%/95% context window

**Key subsystems (v1.5+):**
- Decision logging: `internal/decision/logger.go` → `Engine.LogDecision()`
- Self-heal: `Engine.HealTask()` — re-runs failed task with failure context
- Checkpoint resume: `SaveCheckpointData` / `LoadCheckpointData` → `pkg/session.Checkpoint`
- Auto-compaction: `pkg/compaction.Compactor` — LLM-based session summarization
- Cost tracking: `CostTracker` with per-phase budget enforcement

### Layer 3a: Provider Layer
**Location:** `internal/provider/`

| File | Purpose |
|------|---------|
| `interface.go` | `LLMProvider` interface: `Name()`, `APIKey()`, `FetchModels()`, `CachedModels()`, `ChatCompletionStream()`, `EstimateCost()`, `HealthCheck()`, `GetModel()` |
| `registry.go` | `Registry` — thread-safe provider registry with active provider tracking, `TrySetActive`/`RollbackActive` for atomic health-check-driven failover |
| `common.go` | Shared HTTP client, SSE parsing, retry logic, request building |
| `capabilities.go` | Dynamic model capability detection (tools, reasoning, vision, context length) from API metadata + config overrides |
| `reasoning.go` | Reasoning token handling, thinking budget management |
| `openrouter/`, `zen/`, `nvidia/` | Provider implementations — each wraps `BaseClient`, implements `LLMProvider` |

**Model discovery:** Dynamic via `FetchModels()` — never hardcoded. Capabilities merged from API + `config.toml` `ModelCapabilities.KnownCapabilities`.

### Layer 3b: Tool Dispatcher
**Location:** `internal/tools/dispatcher.go` (480 lines)

**Dispatcher responsibilities:**
- Tool registry (`map[string]types.Tool`) — 18 built-in tools registered in `defaults.go`
- Permission system: rules from `config.PermissionsConfig`, persistent per-project approvals
- Rate limiting: token bucket (general + dangerous-tool specific)
- Concurrency: semaphore (`MaxConcurrentTools`)
- Output bounding: `OutputStore` — truncates tool output to `maxLines`/`maxBytes`
- Question/Permission channels: goroutine-safe request/response via `sync.Map` per-request routing
- Batch approvals: user can approve all future calls for tool+risk combo (task-scoped)
- Metrics collection: `pkg/metrics.Collector` integration
- Subagent support: `Agent` tool spawns child agents with isolated worktrees + dispatchers

**Built-in tools (from `defaults.go`):**
`Bash`, `FileRead`, `FileWrite`, `Edit`, `TodoWrite`, `TodoRead`, `WebFetch`, `WebSearch`, `Glob`, `Grep`, `FileList`, `FileDelete`, `FileMove`, `CodeMap`, `CodeComplexity`, `DevServer`, `HTTPCheck`, `Agent`

### Layer 4: Shared Types
**Location:** `internal/types/types.go` (350 lines)

**Core type vocabulary (used across all layers):**
- `WorkflowPhase`, `WorkflowMode`, `ComplexityLevel`, `IntentType`, `IntentResult`
- `RiskLevel` (safe/medium/dangerous/destructive), `TaskStatus`
- `ModelInfo`, `Pricing`, `Capabilities`, `ArchInfo`
- `Message`, `MessageSegment`, `ToolCall`, `ToolInput`, `ToolResult`, `ToolError`
- `Task`, `ProjectState`, `Session`, `StreamChunk`, `StreamIterator`
- `HealthStatus`, `DiffSummary`, `FileDiff`
- Constants: `MaxLLMResponseBytes`, `DefaultContextLength`, `MaxHealAttempts`, `MaxToolsPerCall`

### Layer 5: Public Packages (`pkg/`)
**Constraint:** `pkg/` MUST NOT import `internal/` (enforced by Go module structure).

| Package | Purpose |
|---------|---------|
| `pkg/keychain` | OS keyring integration (macOS Keychain, Windows Credential Manager, Linux Secret Service/D-Bus) |
| `pkg/ledger` | Session record persistence (LEDGER.md) |
| `pkg/metrics` | Tool/LLM call metrics collection |
| `pkg/retry` | Exponential backoff with error classification |
| `pkg/rollback` | Git-based rollback to session start hash |
| `pkg/session` | Session persistence, checkpoints, project state |
| `pkg/taskrunner` | Parallel task execution with dependencies |
| `pkg/bisect` | Git bisect automation for regression finding |
| `pkg/compaction` | LLM-based session summarization |
| `pkg/autodream` | Proactive session compression |
| `pkg/arbitrage` | Model cost optimization recommendations |
| `pkg/coordinator` | Multi-agent coordination |
| `pkg/narrative` | Structured narrative event stream for TUI |
| `pkg/history` | Command history with frecency |
| `pkg/skills` | Skill discovery and loading |
| `pkg/rollback` | Rollback operations |

## Data Flow

### Primary Request Path (User Goal → Workflow → LLM/Tools → TUI)

```
1. User submits goal in TUI (REPL or /workflow)
       │
       ▼
2. AppState.RunPhaseCmd(PhaseInitialize) → tea.Cmd
       │
       ▼
3. Goroutine: Engine.RunPhase(ctx, PhaseInitialize, goal)
       │  ├─ Budget check
       │  ├─ PhaseCoordinator.PrePhaseSetup (compaction, checkpoint)
       │  └─ runInitialize(): intent classification via LLM
       │
       ▼
4. Engine emits PhaseResultMsg via MsgEmitter channel
       │
       ▼
5. AppState.Update(PhaseResultMsg) — single-threaded state mutation
       │  ├─ Updates workflowPhase, tasks, messages
       │  ├─ Triggers next phase via RunPhaseCmd(PhaseDiscuss)
       │  └─ Returns tea.Batch(drainEmitterCmd, nextPhaseCmd)
       │
       ▼
6. Repeat for Discuss → Plan → Execute → Verify → Runtime → Ship
       │
       ├─ Discuss: LLM generates clarifying questions → TUI renders → User answers
       ├─ Plan: LLM generates plan.md → TUI renders for approval/refinement
       ├─ Execute: LLM + tools (streamLLMWithTools) → Tool calls via Dispatcher
       │            └─ Dispatcher: permission check → rate limit → concurrency → execute
       ├─ Verify: Run verification commands, self-heal on failure
       ├─ Runtime: Dev server health checks, smoke tests
       └─ Ship: Git commit, PR creation, ledger entry
```

### Tool Execution Flow

```
LLM emits tool_call chunk
       │
       ▼
Engine.consumeStreamWithTools() → []ToolCall
       │
       ▼
Engine calls Dispatcher.Execute(toolName, input) for each
       │
       ▼
Dispatcher.Execute():
  1. Acquire concurrency semaphore
  2. Acquire rate limit token (general or dangerous)
  3. Check permissions (config rules + persistent approvals)
     ├─ If allowed: execute tool
     ├─ If needs approval: send PermissionRequestMsg → TUI → user responds
     └─ If denied: return error to LLM
  4. Execute tool (tool.Execute(ctx, input))
  5. Bound output via OutputStore
  6. Release semaphore, record metrics
  7. Return ToolResult
       │
       ▼
Engine streams ToolResult back to LLM as tool message
       │
       ▼
Next LLM iteration continues...
```

### Concurrency Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    Bubble Tea Main Goroutine                     │
│  tea.Program.Run() → AppState.Update(msg) → AppState.View()     │
│  ─────────────────────────────────────────────────────────────  │
│  ALL state mutations happen HERE. Never mutate AppState from    │
│  other goroutines. Use tea.Cmd to schedule work, tea.Msg to     │
│  deliver results back to Update().                              │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
       ┌────────────┐  ┌────────────┐  ┌────────────┐
       │ Workflow   │  │ Tool       │  │ Subagent   │
       │ Engine     │  │ Dispatcher │  │ Manager    │
       │ Goroutine  │  │ Goroutines │  │ Goroutines │
       └────────────┘  └────────────┘  └────────────┘
              │               │               │
              └───────────────┼───────────────┘
                              ▼
                    ┌─────────────────┐
                    │ tea.Msg Channel │
                    │ (emitterCh)     │
                    └────────┬────────┘
                             │
                    drainEmitterCmd() reads
                    and forwards to
                    tea.Program.Send()
                             │
                             ▼
                    Back to Update() loop
```

**Channels used:**
- `Dispatcher.requestCh` / `responseCh` — permission requests (buffered)
- `Dispatcher.questionReqCh` / `questionRespCh` — AskUser questions
- `Engine.msgEmitter.ch` (chan `tea.Msg`) — workflow → TUI events
- `FileWatcher.Events` — filesystem changes → sidebar refresh
- `ConfigWatcher` channel — config.toml changes → hot reload

**Rate limiters / semaphores (Dispatcher):**
- `rateTokens` (chan struct{}) — general tool rate limit (token bucket)
- `dangerousRateTokens` — stricter limit for dangerous/destructive tools
- `concurrencySem` — `MaxConcurrentTools` simultaneous executions

## Key Abstractions

### WorkflowEngine Interface (`internal/tui/tuitypes/types.go`)
```go
type WorkflowEngine interface {
    RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*PhaseResult, error)
    SetModel(modelID string, p provider.LLMProvider)
    SetPhaseModel(phase types.WorkflowPhase, modelID string)
    SetWorkflowMode(mode types.WorkflowMode)
    SetIntentResult(*types.IntentResult)
    SubmitDiscussAnswer(index int, answer string) error
    SetRefinementFeedback(string)
    PlanContent() string
    PlanVersion() int
    HealTask(ctx context.Context, taskID int) (bool, error)
    SaveCheckpointData(goal string)
    LoadCheckpointData(*CheckpointData)
    // ... more
}
```
Implemented by `*workflow.Engine`. Allows TUI to depend on interface, not concrete type.

### LLMProvider Interface (`internal/provider/interface.go`)
```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```
Three implementations: `openrouter.Provider`, `zen.Provider`, `nvidia.Provider`.

### Tool Interface (`internal/types/types.go`)
```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```
Optional: `SchemaProvider` for JSON Schema parameters.

### StateMachine (`internal/workflow/state_machine.go`)
Encapsulates phase transition logic. Validates against `validTransitions` map. Thread-safe with `sync.RWMutex`. Tracks history and Plan↔Discuss cycle count.

## Entry Points

| Entry Point | File | Trigger |
|-------------|------|---------|
| TUI (interactive) | `cmd/m31a/main.go` → `tea.NewProgram(app).Run()` | Default (no flags) |
| Headless prompt | `runHeadless()` | `--prompt "text"` |
| Headless workflow | `runHeadlessWorkflow()` | `--goal "text"` (not fully implemented) |
| Version | `main.go` | `--version` |

## Architectural Constraints

### Threading
- **Bubble Tea is single-threaded.** All `AppState` mutations in `Update()`.
- Goroutines (workflow, tools, subagents, watchers) communicate via channels → `tea.Msg` → `Update()`.
- `sync.Mutex`/`RWMutex` protects Engine internal state (`WorkflowState`, `StateMachine`, model maps).

### Global State
- `slog.Default()` — structured logging (initialized in `main.go`)
- `internal/types` package-level constants (no mutable globals)
- `Dispatcher` rate limiters & semaphores — per-dispatcher instance state
- `Registry` active provider — mutex-protected map

### Circular Imports
- **None.** Go module system enforces: `pkg/` ↛ `internal/`, `internal/tui` → `internal/workflow` → `internal/provider/tools/types`, `cmd/m31a` → all.

### Build Constraints
- `CGO_ENABLED=0` mandatory (static binary) — enforced in Makefile `GOFLAGS`
- Go 1.25.0 (per `go.mod`)
- Cross-compile targets: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64 (via `.goreleaser.yaml`)

## Anti-Patterns

### ❌ Mutating AppState from Goroutine
```go
// WRONG — breaks Bubble Tea contract
go func() {
    m.workflowPhase = PhaseExecute  // Data race!
}()
```
**Do instead:**
```go
// CORRECT — emit message, let Update() mutate
m.emitterCh <- PhaseResultMsg{Phase: PhaseExecute, Success: true}
```

### ❌ Direct Provider Calls from TUI
```go
// WRONG — bypasses workflow engine, no checkpointing
p.ChatCompletionStream(ctx, req)
```
**Do instead:** Use `Engine.RunPhase()` or `Engine.streamLLMWithTools()`.

### ❌ Hardcoding Model Names
```go
// WRONG — models discovered dynamically
model := "anthropic/claude-3.5-sonnet"
```
**Do instead:** Use `provider.FetchModels()` or `registry.ActiveProvider().CachedModels()`.

### ❌ pkg/ Importing internal/
```go
// WRONG — violates module boundary
import "github.com/eshanized/M31A/internal/types"
```
**Do instead:** Keep `pkg/` pure. Shared types live in `internal/types/`.

## Error Handling

**Strategy:** Return errors, never panic. Wrap with `fmt.Errorf("%w", err)`.

**Error types:** `internal/errors/errors.go` defines sentinel errors:
- `ErrPhaseTransition`, `ErrProviderNotFound`, `ErrProviderUnreachable`
- `ErrInvalidProvider`, `ErrContextExceeded`, `ErrToolNotFound`
- `ErrPermissionDenied`, `ErrRateLimited`

**Retry:** `pkg/retry` — exponential backoff with error classification (retryable vs non-retryable). Used in `Engine.retryChatStream()`.

**LLM Errors:** Classified by `retry.ClassifyError()` → `retry.IsRetryable()`.

## Cross-Cutting Concerns

| Concern | Implementation |
|---------|----------------|
| **Logging** | `slog` via `internal/log.NewLogger()` — JSON output to `~/.m31a/logs/`, level from config |
| **Validation** | Tool input validated via JSON Schema (`SchemaProvider`); config validated on load |
| **Authentication** | API keys via OS keychain (`pkg/keychain`); never written to disk in plaintext |
| **Config** | TOML (`config.toml`) — `internal/config/config.go` with `Load()`/`WatchConfig()` |
| **Persistence** | Session JSON in `~/.m31a/sessions/`; checkpoints; project state; LEDGER.md |
| **Health Checks** | Periodic `HealthCheckTickMsg` → provider `HealthCheck()` → TUI status indicator |

---

*Architecture analysis: 2026-07-10*