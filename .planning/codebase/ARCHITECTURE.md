<!-- refreshed: 2026-08-23 -->
# Architecture

**Analysis Date:** 2026-08-23

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ENTRY POINT                                     │
│                        cmd/m31a/main.go                                      │
└────────────────────────────────────┬────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              TUI LAYER (Bubble Tea)                          │
│  internal/ui/tui/                                                           │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────────┐  │
│  │ AppState (Model) │  │  Screen Router   │  │  Components (Sidebar,    │  │
│  │ - Session Mgmt   │  │  - Home/REPL     │  │  REPL, Plan, Execute,    │  │
│  │ - Workflow Ctrl  │  │  - Workflow      │  │  Verify, Runtime, Ship,  │  │
│  │ - Provider Reg   │  │  - Settings      │  │  Discuss, Rollback, etc.)│  │
│  └────────┬─────────┘  └────────┬─────────┘  └────────────┬─────────────┘  │
└───────────┼─────────────────────┼─────────────────────────┼────────────────┘
            │                     │                         │
            ▼                     ▼                         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         WORKFLOW ENGINE                                      │
│  internal/engine/workflow/engine.go                                         │
│  Seven phases: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship│
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Engine                                                                │   │
│  │ - StateMachine: phase transitions                                    │   │
│  │ - WorkflowCache: cross-phase data                                    │   │
│  │ - PromptBuilder: phase-specific prompts                              │   │
│  │ - ContextBuilder: assembles LLM context                              │   │
│  │ - PhaseCoordinator: pre/post phase hooks                             │   │
│  │ - PhaseHookRegistry: extensibility points                            │   │
│  │ - CostTracker: budget enforcement                                    │   │
│  │ - Compactor: proactive context compaction                            │   │
│  │ - CodeIntel: codebase intelligence                                   │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────┬────────────────────────────────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              ▼                      ▼                      ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│   PROVIDER LAYER    │ │    TOOLS LAYER      │ │   SESSION/STATE     │
│ internal/integrations/ │ internal/tools/    │ │ internal/engine/    │
│ provider/           │ │                     │ │ session/            │
│ ┌─────────────────┐ │ │ ┌─────────────────┐ │ │ ┌─────────────────┐ │
│ │ Registry        │ │ │ │ Dispatcher      │ │ │ │ Manager         │ │
│ │ - OpenRouter    │ │ │ │ - 18 built-in   │ │ │ │ - session.json  │ │
│ │ - Zen           │ │ │ │ - Permission    │ │ │ │ - messages.json │ │
│ │ - Nvidia        │ │ │ │   management    │ │ │ │ - recovery.json │ │
│ │ - Lazy init     │ │ │ │ - Rate limiting │ │ │ │ - checkpoint    │ │
│ └─────────────────┘ │ │ │ - Concurrency   │ │ │ └─────────────────┘ │
└─────────────────────┘ │ └─────────────────┘ │ └─────────────────────┘
                        │                     │
                        ▼                     ▼
              ┌─────────────────────┐ ┌─────────────────────┐
              │   EXTERNAL          │ │   TASK RUNNER       │
              │   INTEGRATIONS      │ │ internal/engine/    │
              │ internal/integrations/ │ taskrunner/       │
              │ git/, keychain/,    │ │ ┌─────────────────┐ │
              │ ledger/, log/,      │ │ │ Runner          │ │
              │ metrics/, etc.      │ │ │ - Kahn's algo   │ │
              └─────────────────────┘ │ │ - Parallel exec │ │
                                      │ │ - Retry logic   │ │
                                      │ └─────────────────┘ │
                                      └─────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | CLI flags, config load, provider registration, TUI construction | `cmd/m31a/main.go` |
| **TUI AppState** | Bubble Tea model; holds all UI state, routes screens, manages workflow | `internal/ui/tui/app.go` |
| **Screen Router** | Navigation between Home, REPL, Workflow, Settings, etc. | `internal/ui/tui/router.go` |
| **Workflow Engine** | Orchestrates 7-phase workflow; state machine, caching, prompts | `internal/engine/workflow/engine.go` |
| **StateMachine** | Validates phase transitions (idle→init→discuss→plan→execute→verify→runtime→ship) | `internal/engine/workflow/engine.go` |
| **PromptBuilder** | Loads and renders phase-specific prompt templates (embedded + overrides) | `internal/engine/workflow/engine_prompts.go` |
| **ContextBuilder** | Assembles LLM context: messages, tools, system prompt, code intelligence | `internal/engine/workflow/engine_context.go` |
| **Provider Registry** | Lazy-registers 3 providers; manages API keys via keychain | `internal/integrations/provider/registry.go` |
| **OpenRouter/Zen/Nvidia** | Provider implementations with streaming, model fetch, health check | `internal/integrations/provider/{openrouter,zen,nvidia}/client.go` |
| **Tools Dispatcher** | Registers 18 tools; handles permissions, rate limiting, concurrency | `internal/tools/dispatcher.go` |
| **Session Manager** | Project-local session storage (session.json, messages.json, recovery.json) | `internal/engine/session/manager.go` |
| **Task Runner** | Topological sort (Kahn's), parallel execution with semaphore, retry | `internal/engine/taskrunner/runner.go` |
| **Keychain** | OS-native credential storage (macOS Keychain, Windows Credential Manager, libsecret) | `internal/integrations/keychain/keychain.go` |
| **Git Client** | Repository operations: status, diff, commit, worktree management | `internal/integrations/git/git.go` |
| **Ledger** | Session record persistence (LEDGER.md) | `internal/integrations/ledger/ledger.go` |
| **Rollback** | Git-based rollback to checkpoint | `internal/engine/rollback/rollback.go` |
| **Bisect** | Automated regression bisect via git | `internal/engine/bisect/bisect.go` |
| **Observability** | Crash capture, pprof server on :6060 | `internal/observability/crash.go` |
| **Extensions** | External tool/provider/hook subprocess protocol | `pkg/extensions/` |

## Pattern Overview

**Overall:** Layered architecture with strict dependency direction — `internal/` packages may import each other; `pkg/` **must not** import `internal/`.

**Key Characteristics:**
- **Single-threaded UI**: Bubble Tea (Elm architecture) — all state mutations in `Update()`, never from goroutines. Goroutines send `tea.Msg` via channels.
- **Lazy Provider Initialization**: Providers registered on first LLM call, not at startup.
- **Project-Local Sessions**: Session data in `<workDir>/.m31a/` (not global `~/.m31a/`); enables per-project isolation.
- **Dynamic Model Discovery**: Models fetched from provider APIs at runtime; never hardcoded.
- **Checkpoint/Recovery**: Engine persists recovery state before each phase transition for crash safety.
- **Permission System**: Rule-based (allow/ask/deny) with batch approval, TTL, per-agent profiles.

## Layers

**TUI Layer:**
- Purpose: User interaction, rendering, input handling, workflow visualization
- Location: `internal/ui/tui/`
- Contains: `AppState` (model), screen models (home, repl, plan, execute, verify, runtime, ship, settings, sidebar), components, theme system, streaming infrastructure
- Depends on: workflow engine, provider registry, tools dispatcher, session manager, git, ledger, rollback
- Used by: `cmd/m31a/main.go`

**Workflow Engine Layer:**
- Purpose: Execute 7-phase development workflow with LLM orchestration
- Location: `internal/engine/workflow/`
- Contains: `Engine`, `StateMachine`, `PromptBuilder`, `ContextBuilder`, `PhaseCoordinator`, `PhaseHookRegistry`, `WorkflowCache`, `CostTracker`, `Compactor`, `CodeIntel`
- Depends on: provider interface, tools dispatcher, token estimator, session manager, git, ledger, metrics
- Used by: TUI layer (via `RunPhaseCmd`)

**Provider Layer:**
- Purpose: Abstract LLM providers behind common interface
- Location: `internal/integrations/provider/`
- Contains: `LLMProvider` interface, `Registry`, `OpenRouter`, `Zen`, `Nvidia` clients, capabilities detection, model metadata, SSE streaming
- Depends on: `internal/core/types` (for `ChatRequest`, `ModelInfo`, `ToolDefinition`)
- Used by: Workflow engine, TUI (for model selection)

**Tools Layer:**
- Purpose: Execute LLM-requested actions (file ops, bash, search, git, web, subagents)
- Location: `internal/tools/`
- Contains: `Dispatcher` (permissions, rate limits, concurrency), 18 built-in tools (`FileRead`, `FileWrite`, `FileEdit`, `Glob`, `Grep`, `Bash`, `Task`, `WebFetch`, `WebSearch`, `Git`, `TodoWrite`, `TodoRead`, `Agent`, etc.), subagent manager
- Depends on: `internal/core/types` (for `Tool`, `ToolInput`, `ToolResult`), `internal/integrations/git`, `internal/integrations/metrics`
- Used by: Workflow engine (via dispatcher), TUI (for permission prompts)

**Session/State Layer:**
- Purpose: Persist and recover session state across restarts
- Location: `internal/engine/session/`
- Contains: `Manager` (session.json, messages.json, recovery.json), `Coordinator` (concurrency control)
- Depends on: `internal/core/types`, `internal/engine/coordinator`
- Used by: Workflow engine, TUI

**External Integrations Layer:**
- Purpose: Adapters for external systems
- Location: `internal/integrations/`
- Contains: `git/`, `keychain/`, `ledger/`, `log/`, `metrics/`, `arbitrage/`, `autodream/`, `codeintel/`, `context/`, `history/`, `shell/`, `skills/`
- Depends on: `internal/core/types`, `internal/core/config`
- Used by: Workflow engine, TUI, tools

**Core/Shared Layer:**
- Purpose: Shared vocabulary across all layers
- Location: `internal/core/`
- Contains: `types/` (all domain types), `config/` (multi-layer config loading), `errors/` (error sentinels)
- Depends on: stdlib only
- Used by: All layers

**Extensions Layer:**
- Purpose: External subprocess-based tools/providers/hooks
- Location: `pkg/extensions/`
- Contains: `protocol.go` (JSON-RPC over stdin/stdout), `adapter_tool.go`, `adapter_provider.go`, `adapter_hook.go`, `registry.go`, `subprocess.go`
- Depends on: stdlib only (no `internal/`)
- Used by: Workflow engine (via `PhaseHookRegistry`), Provider registry, Tools dispatcher

## Data Flow

### Primary Request Path (TUI → Workflow Phase)

1. **User Input** → REPL (`internal/ui/tui/repl_model.go`) parses command or sends message
2. **Workflow Trigger** → `AppState.RunPhaseCmd(phase)` creates `tea.Cmd` that runs `Engine.RunPhase()` in goroutine
3. **Phase Execution** → `Engine.RunPhase()` (`internal/engine/workflow/engine.go:330`)
   - Pre-phase setup via `PhaseCoordinator.PrePhaseSetup()`
   - State machine transition
   - Phase-specific logic: `runInitialize`, `runDiscuss`, `runPlan`, `runExecute`, `runVerify`, `runRuntime`, `runShip`
   - Each phase builds prompt via `PromptBuilder` + `ContextBuilder`
   - LLM streaming via provider's `ChatCompletionStream()` → `StreamIterator`
   - Tool calls dispatched via `Dispatcher.Execute()` → tool implementations
   - Results emitted as `tea.Msg` through `MsgEmitter` channel
4. **TUI Update** → `AppState.Update()` receives messages, updates state, re-renders
5. **Post-Phase** → `PhaseCoordinator.PostPhaseExecution()`, recovery cleared on success

### Headless Mode (--prompt / --goal)

1. **CLI Parse** → `main.go` detects `--prompt` or `--goal` flag
2. **Direct Execution** → `runHeadless()` or `runHeadlessWorkflow()` bypasses TUI
3. **Same Engine** → Creates `Engine` with same dependencies, runs phases sequentially
4. **Output** → Results printed to stdout/stderr

### Session Persistence

1. **On Shutdown** → `AppState.Shutdown()` → `saveSessionOnShutdown()` → `SessionManager.SaveSession()`
2. **Periodic** → `SessionManager.UpdateWorkflowState()` after phase transitions
3. **Recovery** → `Engine.persistRecovery()` before each phase; `Manager.LoadRecoveryBytes()` on resume

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Uniform interface for all LLM providers
- Examples: `internal/integrations/provider/openrouter/client.go`, `zen/client.go`, `nvidia/client.go`
- Pattern: `FetchModels()`, `ChatCompletionStream()`, `HealthCheck()`, `GetModel()`, `EstimateCost()`

**Tool Interface:**
- Purpose: Uniform interface for all executable tools
- Examples: `internal/tools/fileops/read.go`, `internal/tools/exec/bash.go`, `internal/tools/subagent/agent.go`
- Pattern: `Name()`, `Description()`, `RiskLevel()`, `Execute(ctx, ToolInput) (ToolResult, error)`

**WorkflowPhase Enum:**
- Purpose: Type-safe phase identifiers for state machine
- Values: `PhaseIdle`, `PhaseInitialize`, `PhaseDiscuss`, `PhasePlan`, `PhaseExecute`, `PhaseVerify`, `PhaseRuntime`, `PhaseShip`
- File: `internal/core/types/types.go:18-29`

**MsgEmitter Pattern:**
- Purpose: Bridge workflow engine (goroutine) → TUI (Bubble Tea main thread)
- Implementation: Channel `chan tea.Msg` set via `Engine.SetMsgEmitter()`
- Messages: `TaskStartMsg`, `TaskUpdateMsg`, `ToolStartMsg`, `ToolCompleteMsg`, `StreamChunkMsg`, `PhaseResultMsg`

## Entry Points

**CLI Binary (`cmd/m31a/main.go`):**
- Location: `cmd/m31a/main.go`
- Triggers: Direct execution, `make dev`, `make build && ./m31a`
- Flags: `--version`, `--help`, `--prompt`, `--goal`, `--model`, `--debug`, `--log-level`
- Responsibilities: Config load, provider registration, TUI construction, signal handling, pprof server

**TUI Program (`internal/ui/tui/app.go:554`):**
- Location: `tui.NewApp()` → `tea.NewProgram(app, tea.WithAltScreen(), tea.WithMouseAllMotion())`
- Triggers: Default (no flags)
- Responsibilities: Event loop, rendering, input routing

## Architectural Constraints

- **Threading:** Bubble Tea is strictly single-threaded. All state mutations in `Update()`. Goroutines communicate via `tea.Cmd`/`tea.Msg` channels. Violating this causes session corruption.
- **Global State:** Module-level singletons: `globalDropCounter` (`internal/ui/tui/app.go`), `slog.Default()` logger. No other shared mutable state.
- **Circular Imports:** None — enforced by Go module system. `pkg/` cannot import `internal/`.
- **Dependency Rule:** `internal/` packages may import each other freely. `pkg/extensions/` is standalone (stdlib only).
- **CGO:** Hard constraint `CGO_ENABLED=0` — binary must be fully static. No C dependencies.

## Anti-Patterns

### Mutating AppState from Goroutines

**What happens:** Goroutine directly modifies `AppState` fields (e.g., `m.workflowPhase = phase`)
**Why it's wrong:** Bubble Tea requires all state changes in `Update()`. Direct mutation causes race conditions, lost updates, and UI desync.
**Do this instead:** Send `tea.Msg` via channel/emitter; handle in `Update()`. Example: `internal/ui/tui/app.go:550-564` `drainEmitterCmd()`.

### Blocking in Update()

**What happens:** `Update()` performs synchronous I/O (HTTP, file ops, LLM calls)
**Why it's wrong:** Blocks the entire UI thread; app freezes, cannot process input or render.
**Do this instead:** Return `tea.Cmd` that runs async work; emit result as `tea.Msg`. Example: `RunPhaseCmd()` returns `tea.Batch(phaseCmd, drainEmitterCmd())`.

### Hardcoding Model Names

**What happens:** Code references specific model IDs (e.g., `"gpt-4"`, `"claude-3"`)
**Why it's wrong:** Models change frequently; providers have different catalogs. Breaks when model retired or new provider added.
**Do this instead:** Use `Provider.FetchModels()` and `Provider.GetModel()`; store selected model in config/session. Example: `main.go:196-204` auto-detection.

### Skipping Permission Checks

**What happens:** Tool executes without `Dispatcher.ensurePermission()`
**Why it's wrong:** Violates user consent model; dangerous operations (rm, git push) run unchecked.
**Do this instead:** All tools registered via `Dispatcher.Register()`; `Execute()` always calls `ensurePermission()`. Example: `dispatcher.go:180-291`.

## Error Handling

**Strategy:** Return errors, never panic (except `RecoverAndCapture` at top level). Wrap with `fmt.Errorf("%w", err)`.

**Patterns:**
- **Sentinel Errors:** `internal/core/errors/errors.go` defines `ErrSessionNotFound`, `ErrSessionCorrupted`, `ErrToolExecution`, `ErrPhaseTransition`, `ErrCircularDependency`, `ErrKeychainUnavailable`, `ErrKeyNotFound`
- **Structured Tool Errors:** `types.ToolError` carries `Err` + `Hint` for LLM self-recovery (`types.go:204-226`)
- **Recovery Pattern:** `Engine.persistRecovery()` before each phase; `Manager.LoadRecoveryBytes()` on resume
- **Graceful Degradation:** Provider fallback chain (config `FallbackPriority`); keychain unavailable → config file fallback
- **Observability:** `observability.RecoverAndCapture()` writes crash report to `~/.m31a/crashes/` and logs to stderr for CI

## Cross-Cutting Concerns

**Logging:** `slog` (structured, leveled). Initialized in `main.go:343-350`. Configurable via `--log-level`, `M31A_LOG_LEVEL`, `--debug`.

**Validation:** Config validation in `config/validate.go` (type/range checks, unknown key warnings). Permission rules validated on load.

**Authentication:** API keys resolved via priority: env var → OS keychain → config file. Never logged. Keychain implementations: macOS (`security`), Windows (`cmdkey`), Linux (`libsecret`).

**Configuration:** Multi-layer TOML + JSON + env vars. Load order: defaults → global TOML → workspace TOML → env vars → project JSON → var substitution → validation. File: `internal/core/config/loader.go:215-343`.

**Metrics:** Optional (`Features.MetricsEnabled`). `metrics.Collector` records tool calls, LLM usage, phase durations. Written to `METRICS.json` in session dir.

---

*Architecture analysis: 2026-08-23*