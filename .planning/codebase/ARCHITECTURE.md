<!-- refreshed: 2026-07-11 -->
# Architecture

**Analysis Date:** 2026-07-11

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              M31A Application                                │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌──────────────────┐    ┌────────────────────────────┐  │
│  │  cmd/m31a   │───▶│   internal/tui   │───▶│  internal/workflow/engine  │  │
│  │  (entry)    │    │  (Bubble Tea)    │    │  (7-phase workflow)        │  │
│  └─────────────┘    └────────┬─────────┘    └───────────┬────────────────┘  │
│                               │                          │                    │
│                    ┌──────────┼──────────┐               │                    │
│                    ▼          ▼          ▼               ▼                    │
│              ┌──────────┐ ┌───────┐ ┌────────┐    ┌──────────┐              │
│              │internal/ │ │internal│ │internal│    │ internal/│              │
│              │ provider │ │ tools  │ │ types  │    │ codeintel│              │
│              └──────────┘ └───────┘ └────────┘    └──────────┘              │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                              pkg/ (public APIs)                       │   │
│  │  session  ledger  rollback  bisect  taskrunner  narrative  keychain  │   │
│  │  retry    compaction  metrics   coordinator skills    arbitrage       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | Flag parsing, config load, provider registration, TUI construction, signal handling | `cmd/m31a/main.go` |
| **TUI AppState** | Bubble Tea model; single-threaded state machine; screen routing; workflow orchestration | `internal/tui/app.go`, `internal/tui/app_state.go` |
| **Workflow Engine** | Seven-phase orchestration (Initialize→Discuss→Plan→Execute→Verify→Runtime→Ship); LLM streaming; tool dispatch; checkpointing | `internal/workflow/engine.go` |
| **Provider Registry** | Manages 3 LLM providers (OpenRouter, Zen, NVIDIA); dynamic model discovery; health checks; fallback | `internal/provider/registry.go` |
| **Tools Dispatcher** | 18 built-in tools; permission system (rules + agent profiles); rate limiting; concurrency control; output bounding | `internal/tools/dispatcher.go` |
| **Shared Types** | Core type vocabulary: `Message`, `ToolCall`, `Task`, `WorkflowPhase`, `ModelInfo`, `RiskLevel` | `internal/types/types.go` |
| **Code Intelligence** | Codebase indexing for planning context (AST parsing, symbol extraction) | `internal/codeintel/` |

## Pattern Overview

**Overall:** Elm Architecture (Bubble Tea) + Pipeline Workflow Engine

**Key Characteristics:**
- **Single-threaded state mutations**: All state changes go through `AppState.Update()` — no goroutine mutates `AppState` directly. Background work communicates via `tea.Cmd` / `tea.Msg` channels.
- **Message-passing for async**: Workflow engine emits `tea.Msg` via `MsgEmitter` channel; TUI drains via `drainEmitterCmd()` / `drainAdaptiveCmd()`.
- **Phase state machine**: `StateMachine` enforces valid phase transitions (Initialize→Discuss→Plan→Execute→Verify→Runtime→Ship) with cycle guard (`maxDiscussPlanCycles = 3`).
- **Provider abstraction**: `LLMProvider` interface with 3 implementations; models fetched dynamically at runtime (no hardcoded model names).
- **Tool dispatcher pattern**: Central `Dispatcher` manages tool registry, permission evaluation, rate limiting (token bucket), concurrency semaphore, and output store.
- **Dependency rule**: `pkg/` packages MUST NOT import `internal/` — enforced by Go module structure.
- **Checkpoint/resume**: `session.Manager` persists `STATE.md`, `PLAN.md`, `TASKS.md`, checkpoints; engine restores via `LoadCheckpointData()`.

## Layers

### TUI Layer (`internal/tui/`)
- **Purpose**: Terminal UI, user interaction, screen routing, streaming render
- **Location**: `internal/tui/`
- **Contains**: `AppState` (main model), 25+ screen models (REPL, Sidebar, Plan, Execute, Verify, Runtime, Ship, Settings, etc.), command system, key bindings, narrative engine integration
- **Depends on**: `internal/workflow`, `internal/provider`, `internal/tools`, `internal/types`, `internal/config`, `pkg/*`
- **Used by**: `cmd/m31a/main.go` (entry point)

### Workflow Engine Layer (`internal/workflow/`)
- **Purpose**: Seven-phase execution orchestration, LLM interaction, tool dispatch, state persistence
- **Location**: `internal/workflow/`
- **Contains**: `Engine` (core), `StateMachine`, `PhaseCoordinator`, `ContextBuilder`, `PromptBuilder`, phase implementations (`initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go`), caching, cost tracking
- **Depends on**: `internal/provider`, `internal/tools`, `internal/types`, `internal/config`, `internal/codeintel`, `pkg/session`, `pkg/metrics`, `pkg/ledger`, `pkg/rollback`, `pkg/compaction`, `pkg/retry`
- **Used by**: `internal/tui/app.go` (via `workflowEngineInterface`)

### Provider Layer (`internal/provider/`)
- **Purpose**: LLM API abstraction, model discovery, streaming, health checks, capability detection
- **Location**: `internal/provider/`
- **Contains**: `Registry`, `BaseClient` (shared HTTP, caching, SSE parsing), `openrouter/client.go`, `zen/client.go`, `nvidia/client.go`, `capabilities.go` (model capability heuristics), `reasoning.go` (reasoning config per model)
- **Depends on**: `internal/types`, `internal/errors`
- **Used by**: `internal/workflow/engine.go`, `cmd/m31a/main.go`

### Tools Layer (`internal/tools/`)
- **Purpose**: Tool definitions, execution dispatcher, permission system, rate limiting, concurrency control
- **Location**: `internal/tools/`
- **Contains**: `Dispatcher` (core), 18 tool implementations (`bash.go`, `fileread.go`, `filewrite.go`, `edit.go`, `glob.go`, `grep.go`, `todowrite.go`, `webfetch.go`, `websearch.go`, `codemap.go`, `codecomplexity.go`, `agent.go`, `filedelete.go`, `filemove.go`, `filelist.go`, `httpcheck.go`, `devserver.go`, `question.go`), `permissions.go`, `output_store.go`, `concurrency.go`, `defaults.go`
- **Depends on**: `internal/types`, `internal/config`, `pkg/metrics`
- **Used by**: `internal/workflow/engine.go`, `internal/tui/app.go`

### Shared Types (`internal/types/`)
- **Purpose**: Type vocabulary shared across all internal packages
- **Location**: `internal/types/`
- **Contains**: `types.go` (core types), `plan.go`, `toolcall.go`, `git.go`, `constants.go`
- **Depends on**: (stdlib only)
- **Used by**: All internal packages, pkg packages

### Public Packages (`pkg/`)
- **Purpose**: Reusable libraries with stable APIs; no dependency on `internal/`
- **Location**: `pkg/`
- **Contains**: 
  - `session/` — session persistence, checkpoints, project state
  - `ledger/` — session record ledger (LEDGER.md)
  - `rollback/` — git-based rollback
  - `bisect/` — git bisect automation
  - `taskrunner/` — parallel task execution
  - `narrative/` — narrative engine for UI message classification/grouping
  - `keychain/` — OS keychain integration (macOS/Windows/Linux)
  - `retry/` — retry policies with classification
  - `compaction/` — session auto-compaction
  - `metrics/` — observability collector
  - `coordinator/` — subagent coordination
  - `skills/` — skill discovery for slash commands
  - `arbitrage/` — model cost optimization
  - `autodream/` — session consolidation
  - `history/` — frecent history

## Data Flow

### Primary Request Path (User Goal → Workflow Execution)

1. **Entry** (`cmd/m31a/main.go:116`)
   - Parse flags (`--prompt`, `--goal`, `--model`)
   - Load config (`config.Load`)
   - Initialize keychain, resolve API keys
   - Register providers (`tui.RegisterProvider`)

2. **TUI Initialization** (`internal/tui/app.go:297`)
   - `NewApp()` constructs `AppState` with all dependencies
   - `tea.NewProgram(app)` starts Bubble Tea event loop

3. **User Input** (`internal/tui/app_update.go:30`)
   - `AppState.Update()` handles `tea.KeyMsg`, `tea.MouseMsg`, slash commands
   - Goal submission → `GoalSubmittedMsg` → `handleGoalSubmittedMsg()`

4. **Workflow Start** (`internal/tui/app.go:315`)
   - `RunPhaseCmd(PhaseInitialize)` creates `tea.Cmd` running `Engine.RunPhase()` in goroutine
   - Engine emits `PhaseResultMsg` via `MsgEmitter` channel

5. **Phase Execution** (`internal/workflow/engine.go:654`)
   - `Engine.RunPhase(ctx, phase, goal)` dispatches to `runInitialize`, `runDiscuss`, `runPlan`, `runExecute`, `runVerify`, `runRuntime`, `runShip`
   - Each phase:
     - Builds context via `ContextBuilder` (`buildDiscussContext`, `buildPlanContext`, etc.)
     - Streams LLM via `streamLLM` / `streamLLMWithTools`
     - Parses response, validates, persists state
     - Emits intermediate messages (`TaskStartMsg`, `ToolStartMsg`, `StreamChunkMsg`)

6. **Tool Execution** (`internal/tools/dispatcher.go:212`)
   - `Dispatcher.Execute()` → permission check → rate limit → concurrency semaphore → tool.Execute()
   - Results returned as `ToolResult`; emitter sends `ToolCompleteMsg`

7. **TUI Update** (`internal/tui/app_update.go:158`)
   - `drainAdaptiveCmd()` reads from emitter channel
   - `handlePhaseResultMsg()` processes `PhaseResultMsg`, transitions screen, starts next phase

8. **Phase Transition** (`internal/workflow/phase_coordinator.go:115`)
   - `PhaseCoordinator.CoordinateTransition()` validates via `StateMachine`, saves checkpoint, writes `STATE.md`, emits `PhaseTransitionCompleteMsg`

### Secondary Flows

**Headless Mode** (`cmd/m31a/main.go:55`):
- `--prompt` → `runHeadless()` → single LLM call → print response
- `--goal` → `runHeadlessWorkflow()` → full workflow without TUI (stub)

**Session Resume** (`internal/tui/app.go:405`):
- On startup, `ResumeOnStartup` loads latest session → `loadAndRestoreSession()` → restores messages, workflow phase, model

**Subagent Spawn** (`internal/tools/agent.go`):
- `Agent` tool creates `subagent.Manager` → new worktree + dispatcher → runs independent workflow → emits `SubagentEventMsg`

**Auto-Compaction** (`internal/workflow/engine.go:968`):
- `proactiveCompactCheck()` before phase transitions; `Compactor.Compact()` summarizes history; emits `CompactionCompleteMsg`

**Model Arbitrage** (`internal/tui/app.go:698`):
- `checkAutoArbitrage()` evaluates cheaper models for task complexity; switches via `Engine.SetModel()`

## Key Abstractions

### MsgEmitter (Internal)
- **Purpose**: Decouple workflow engine from TUI
- **Implementation**: `channelEmitter` wraps `chan tea.Msg`; `narrativeEmitter` wraps with narrative classification
- **Location**: `internal/workflow/engine.go:834` (`emit()`), `internal/tui/app.go:483` (`narrativeEmitter`)

### StateMachine (Workflow)
- **Purpose**: Enforce valid phase transitions, track history, prevent Plan↔Discuss oscillation
- **Location**: `internal/workflow/state_machine.go`
- **Transitions**: Idle→Initialize→{Discuss,Execute}→Plan→Execute→Verify→Runtime→Ship→Idle

### ContextBuilder (Workflow)
- **Purpose**: Compose system prompts + dynamic context (project info, file tree, code intel, research, answers)
- **Location**: `internal/workflow/context_builder.go`
- **Used by**: All phase `build*Context()` methods

### Dispatcher (Tools)
- **Purpose**: Central tool registry + execution pipeline (permissions → rate limit → concurrency → execute → output bound)
- **Location**: `internal/tools/dispatcher.go`
- **Key methods**: `Register()`, `Execute()`, `RequestCh()`, `SetTodoWriteCallback()`

### Provider Registry
- **Purpose**: Manage multiple LLM providers, dynamic model lists, health checks, fallback
- **Location**: `internal/provider/registry.go`
- **Key methods**: `Register()`, `SetActive()`, `ActiveProvider()`, `CachedModels()`

## Entry Points

| Entry Point | Location | Triggers | Responsibilities |
|-------------|----------|----------|------------------|
| **Main** | `cmd/m31a/main.go:116` | CLI invocation | Config, providers, TUI, signals |
| **TUI Init** | `internal/tui/app.go:24` | `tea.Program` start | Session cleanup, routing, watchers |
| **Workflow Phase** | `internal/tui/app.go:315` | User goal submit | `RunPhaseCmd()` → `Engine.RunPhase()` |
| **Headless Prompt** | `cmd/m31a/main.go:62` | `--prompt` flag | Single LLM call |
| **Headless Goal** | `cmd/m31a/main.go:57` | `--goal` flag | Full workflow (stub) |

## Architectural Constraints

- **Threading**: Bubble Tea is single-threaded. All `AppState` mutations in `Update()`. Goroutines communicate via `tea.Cmd`/`tea.Msg` channels only. `sync.Mutex` used only in `Engine` for model ID and code intel.
- **Global State**: Module-level singletons: `permissionRequestID` (atomic counter in `internal/tools/interface.go:12`), `globalDropCounter` (in `internal/tui/app.go`), `ChannelCap` constant.
- **Circular Imports**: None detected. Dependency direction: `cmd/` → `internal/tui/` → `internal/workflow/` → `internal/provider`, `internal/tools`, `internal/types`, `pkg/*`. `pkg/` never imports `internal/`.
- **Configuration**: TOML config at `~/.m31a/config.toml` loaded via `internal/config/loader.go`. Hot-reload via `config.WatchConfig()` → `ConfigReloadMsg`.
- **Secrets**: API keys stored in OS keychain via `pkg/keychain/`; never written to disk in plaintext. `.env` files gitignored.

## Anti-Patterns

### ❌ Direct Goroutine Mutation of AppState
```go
// WRONG: Mutating AppState from goroutine
go func() {
    m.workflowPhase = types.PhaseExecute  // Race with Update()
}()
```
**Why it's wrong**: Bubble Tea guarantees single-threaded `Update()`. Concurrent mutation corrupts state.
**Do this instead**: Send `tea.Msg` via channel/emitter; handle in `Update()`.

### ❌ Hardcoding Model Names
```go
// WRONG: Hardcoded model ID
modelID := "anthropic/claude-3.5-sonnet"
```
**Why it's wrong**: Providers return dynamic model lists. Models change, get deprecated, or vary by region.
**Do this instead**: `provider.FetchModels(ctx)` → select from returned `[]ModelInfo`.

### ❌ pkg/ Importing internal/
```go
// WRONG in pkg/session/manager.go
import "github.com/eshanized/M31A/internal/types"
```
**Why it's wrong**: Breaks module boundary. `internal/` is private to the module.
**Do this instead**: Define shared types in `internal/types/` (accessible to all internal) or in `pkg/` itself.

### ❌ Blocking in Update()
```go
// WRONG: Blocking HTTP call in Update()
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    resp, _ := http.Get(...)  // Blocks event loop
}
```
**Why it's wrong**: Freezes TUI, prevents rendering, breaks streaming.
**Do this instead**: Return `tea.Cmd` that runs async and emits result via `tea.Msg`.

## Error Handling

**Strategy**: Explicit error returns, wrapped with `fmt.Errorf("%w", err)`. No panics in production code.

**Patterns:**
- **Provider errors**: Classified via `retry.ClassifyError()` → `retry.IsRetryable()` → exponential backoff in `Engine.retryChatStream()` (`internal/workflow/engine.go:1439`)
- **Tool errors**: `ToolResult.Error` field for LLM-visible errors; `ToolError` struct with `Hint` for self-recovery
- **Workflow errors**: `PhaseResult.Error` + `PhaseResult.Success=false`; TUI displays via toast/error modal
- **Config errors**: Fail fast in `main()` before TUI starts (e.g., invalid permissions config)

## Cross-Cutting Concerns

**Logging**: `log/slog` with structured fields. Logger initialized in `main()` (`internal/log/logger.go`), passed via context or `slog.Default()`.

**Validation**: 
- Config: `config.Load()` validates TOML structure
- Permissions: `tools.DefaultDispatcher()` validates rules at startup (fail-fast)
- Tasks: `validateTasks()` in `plan.go` checks required fields, dependency cycles

**Authentication**: API keys resolved via `config.ResolveAPIKeys(keychain)` → OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service/D-Bus) with file fallback.

**Observability**: `pkg/metrics/collector.go` records tool calls, LLM usage, phase durations, heal events → `METRICS.json` per session.

---

*Architecture analysis: 2026-07-11*