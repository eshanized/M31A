<!-- refreshed: 2026-08-04 -->
# Architecture

**Analysis Date:** 2026-08-04

## System Overview

```text
┌──────────────────────────────────────────────────────────────────┐
│                       CLI Entry Point                            │
│                  cmd/m31a/main.go                                │
│  flag parsing → config load → provider registration → TUI launch  │
└──────────┬───────────────────────────┬───────────────────────────┘
           │ (headless --prompt/--goal)│ (interactive TUI)
           ▼                           ▼
┌─────────────────────┐   ┌───────────────────────────────────────┐
│   Headless Mode      │   │        Bubble Tea TUI (Elm)          │
│  runHeadless()       │   │  internal/ui/tui/                    │
│  runHeadlessWorkflow │   │  AppState → Update() → View()        │
└─────────┬───────────┘   └──────────┬────────────────────────────┘
          │                          │
          ▼                          ▼
┌──────────────────────────────────────────────────────────────────┐
│                    Workflow Engine                                │
│               internal/engine/workflow/                           │
│  7-phase pipeline: Initialize → Discuss → Plan → Execute         │
│                     → Verify → Runtime → Ship                    │
│  StateMachine + WorkflowState + PhaseCoordinator                 │
└──────────┬──────────────────────────┬────────────────────────────┘
           │                          │
           ▼                          ▼
┌──────────────────────┐  ┌───────────────────────────────────────┐
│   Tools Dispatcher    │  │       LLM Provider Layer              │
│  internal/tools/      │  │  internal/integrations/provider/      │
│  18 built-in tools    │  │  OpenRouter / Zen / Nvidia            │
│  permissions + rate   │  │  Registry + Fallback + SSE parsing    │
│  limiting + semaphore │  │  ChatCompletionStream()               │
└──────────┬───────────┘  └──────────┬────────────────────────────┘
           │                         │
           ▼                         ▼
┌──────────────────────────────────────────────────────────────────┐
│                   Infrastructure & Integrations                   │
│  internal/integrations/    internal/infrastructure/              │
│  git / keychain / ledger / fileutil / retry /                    │
│  context / metrics / narrative / compaction / skills /            │
│  codeintel / autodream / shell / history / arbitrage              │
└──────────────────────────────────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────────────────────────────────┐
│                  Shared Type Vocabulary                           │
│               internal/core/types/                                │
│  WorkflowPhase / Task / Message / ModelInfo / Tool / ToolDef      │
│  RiskLevel / IntentType / ComplexityLevel / ProjectState          │
└──────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| CLI entry | Flag parsing, config load, provider registration, TUI/headless launch | `cmd/m31a/main.go` |
| Config | TOML config loading, validation, merge, hot-reload | `internal/core/config/loader.go`, `internal/core/config/types.go` |
| Types | Shared vocabulary: phases, tasks, messages, models, tools | `internal/core/types/types.go` |
| Errors | Sentinel errors, typed error wrappers, user-facing messages | `internal/core/errors/errors.go` |
| Workflow Engine | Orchestrates 7-phase pipeline, LLM streaming, tool dispatch | `internal/engine/workflow/engine.go` |
| State Machine | Thread-safe phase transitions with oscillation guard | `internal/engine/workflow/state_machine.go` |
| Phase Coordinator | Pre-phase setup, post-phase metrics, transition side effects | `internal/engine/workflow/phase_coordinator.go` |
| Initialize | Project detection, git init, planning dir, deep analysis | `internal/engine/workflow/initialize.go` |
| Discuss | Clarifying questions via LLM, quality checking, retry | `internal/engine/workflow/discuss.go` |
| Plan | Task generation from LLM, refinement, plan markdown | `internal/engine/workflow/plan.go` |
| Execute | Task scheduling (topological sort), tool dispatch, self-heal | `internal/engine/workflow/execute.go` |
| Verify | Acceptance checking, self-heal loops, bisect on failure | `internal/engine/workflow/verify.go` |
| Runtime | Dev server start, HTTP smoke tests | `internal/engine/workflow/runtime.go` |
| Ship | Final commit, ledger entry, session archive | `internal/engine/workflow/ship.go` |
| Task Runner | Dependency-aware task scheduling with parallelism control | `internal/engine/taskrunner/runner.go` |
| Bisect | Git bisect for failure isolation | `internal/engine/bisect/bisect.go` |
| Rollback | Git-based session rollback | `internal/engine/rollback/rollback.go` |
| Session | Session persistence, checkpoint resume, planning state | `internal/engine/session/manager.go`, `internal/engine/session/session.go` |
| Compaction | Automatic session compaction when context fills | `internal/engine/compaction/compaction.go` |
| Decision Log | Non-blocking decision logging with ring buffer overflow | `internal/engine/decision/logger.go` |
| Narrative | Event classification, grouping, template rendering | `internal/engine/narrative/engine.go` |
| Token Estimator | Token counting via tiktoken | `internal/engine/tokens/` |
| Tools Dispatcher | Tool registration, permissions, rate limiting, concurrency control | `internal/tools/dispatcher.go` |
| Tool Definitions | Schema generation from registered tools | `internal/tools/tooldefs.go` |
| File Operations | FileRead, FileWrite, Edit, FileList, FileDelete, FileMove | `internal/tools/fileops/` |
| Execution Tools | Bash, DevServer | `internal/tools/exec/` |
| Search Tools | WebFetch, WebSearch, Glob, Grep | `internal/tools/search/` |
| AI Tools | AskUserQuestion, Agent (subagent spawning) | `internal/tools/ai/`, `internal/tools/subagent/` |
| Code Analysis | CodeMap, CodeComplexity | `internal/tools/codeanalysis/` |
| Git Tools | Git operations from tool context | `internal/tools/git/` |
| Network Tools | HTTPCheck | `internal/tools/network/` |
| Todo Tools | TodoWrite, TodoRead | `internal/tools/todo/` |
| Subagent Manager | Parallel subagent lifecycle, worktree isolation, budget enforcement | `internal/tools/subagent/manager.go` |
| Subagent Loop | Per-subagent agentic conversation loop | `internal/tools/subagent/loop.go` |
| Provider Registry | LLM provider registration, active selection, fallback | `internal/integrations/provider/registry.go` |
| LLM Providers | OpenRouter, Zen, Nvidia — streaming chat completion | `internal/integrations/provider/openrouter/`, `zen/`, `nvidia/` |
| SSE Parser | Server-sent event parsing for streaming responses | `internal/integrations/provider/sse.go` |
| Git Client | Git operations for workflow phases | `internal/integrations/git/git.go` |
| Keychain | OS keychain for API key storage | `internal/integrations/keychain/keychain.go` |
| Ledger | Session history persistence in markdown | `internal/integrations/ledger/ledger.go` |
| Context Registry | Dynamic context source management and change detection | `internal/integrations/context/registry.go` |
| Metrics | Tool/LLM/phase metrics collection and JSON persistence | `internal/integrations/metrics/collector.go` |
| Code Intelligence | Source code indexing, AST parsing, relevance scoring | `internal/integrations/codeintel/codeintel.go` |
| AutoDream | Context consolidation / compression | `internal/integrations/autodream/autodream.go` |
| Skills | Composable slash commands from markdown files | `internal/integrations/skills/` |
| Shell | Platform-specific shell execution | `internal/integrations/shell/` |
| History | Frecency-based prompt history | `internal/integrations/history/history.go` |
| Arbitrage | Model selection optimization | `internal/integrations/arbitrage/arbitrage.go` |
| Retry | Configurable retry policies | `internal/infrastructure/retry/policy.go` |
| File Util | Atomic writes, file locking | `internal/infrastructure/fileutil/atomic.go` |
| TUI App | Bubble Tea top-level model, Init/Update/View | `internal/ui/tui/app.go`, `internal/ui/tui/app_state.go` |
| TUI Update | Central message dispatch to handler files | `internal/ui/tui/app_update.go` |
| TUI REPL | Chat interface model | `internal/ui/tui/repl_model.go` |
| TUI Screens | Per-phase screen models (Plan, Execute, Verify, Ship, etc.) | `internal/ui/tui/plan_model.go`, `execute_model.go`, etc. |
| TUI Components | Reusable UI primitives (badge, card, progress, etc.) | `internal/ui/tui/components/` |
| TUI Layout | Header/footer chrome, responsive constraints | `internal/ui/tui/layout/` |
| TUI Streaming | LLM streaming to TUI message bridge | `internal/ui/tui/streaming/` |
| TUI Theme | Dark theme management | `internal/ui/tui/theme/` |
| TUI Commands | Slash command definitions | `internal/ui/tui/commands/` |

## Pattern Overview

**Overall:** Elm Architecture (Model-Update-View) for TUI + 7-Phase Workflow Engine

**Key Characteristics:**
- Bubble Tea enforces single-threaded state mutation via `Update()` — all goroutine communication uses `tea.Cmd` / `tea.Msg` channels
- Workflow engine uses a finite state machine with validated transitions and oscillation guards
- Provider layer is polymorphic via `LLMProvider` interface — three concrete implementations (OpenRouter, Zen, Nvidia)
- Tools are registered dynamically into a `Dispatcher` with permission policies, rate limiting, and concurrency semaphores
- Session state is persisted to disk (JSON) and can be resumed via checkpoint mechanism
- Subagent architecture uses git worktree isolation with independent dispatchers per child agent

## Layers

**Core Layer (`internal/core/`):**
- Purpose: Shared vocabulary and configuration — the foundation of all other layers
- Location: `internal/core/`
- Contains: Type definitions, config loading, error sentinels, file utilities
- Depends on: Nothing (leaf layer)
- Used by: Everything else

**Engine Layer (`internal/engine/`):**
- Purpose: Business logic — workflow orchestration, task execution, session management
- Location: `internal/engine/`
- Contains: Workflow engine (7 phases), task runner, bisect, rollback, compaction, decision log, narrative, tokens
- Depends on: `internal/core/`, `internal/integrations/`, `internal/tools/`
- Used by: `internal/ui/tui/` and `cmd/m31a/`

**Integrations Layer (`internal/integrations/`):**
- Purpose: External system adapters — git, LLM providers, keychain, metrics, etc.
- Location: `internal/integrations/`
- Contains: Provider implementations, git client, keychain, ledger, context, code intelligence, skills, shell
- Depends on: `internal/core/`
- Used by: `internal/engine/`, `internal/ui/tui/`, `cmd/m31a/`

**Tools Layer (`internal/tools/`):**
- Purpose: Tool execution framework — dispatcher, permissions, rate limiting, and 18+ built-in tools
- Location: `internal/tools/`
- Contains: Dispatcher, tool interfaces, file ops, exec, search, AI, git, subagent, todo
- Depends on: `internal/core/`, `internal/integrations/metrics/`
- Used by: `internal/engine/`, `internal/ui/tui/`

**Infrastructure Layer (`internal/infrastructure/`):**
- Purpose: Low-level utilities shared across the codebase
- Location: `internal/infrastructure/`
- Contains: Atomic file writes, file locking, retry policies
- Depends on: Nothing (leaf layer)
- Used by: `internal/engine/`, `internal/integrations/`

**UI Layer (`internal/ui/tui/`):**
- Purpose: Terminal user interface — Bubble Tea models, views, screens, components
- Location: `internal/ui/tui/`
- Contains: AppState (top model), REPL, per-screen models, components, layout, streaming, theme, commands
- Depends on: `internal/core/`, `internal/engine/`, `internal/integrations/`, `internal/tools/`
- Used by: `cmd/m31a/` (only consumer)

## Data Flow

### Primary Interactive Flow

1. User types in REPL → `internal/ui/tui/repl_model.go` captures input
2. `AppState.Update()` dispatches to handler → `internal/ui/tui/app_update.go`
3. REPL streams message to LLM via `provider.ChatCompletionStream()` → `internal/integrations/provider/`
4. Streaming chunks arrive as `StreamChunkMsg` → rendered in REPL view
5. If LLM requests tool calls → `tools.Dispatcher.CallTool()` → `internal/tools/dispatcher.go`
6. Tool permission check → permission modal in TUI → user approves/denies
7. Tool executes (e.g., `Bash`, `FileWrite`, `Edit`) → result returned to LLM
8. Loop continues until LLM produces final text response

### Workflow Execution Flow

1. User submits goal via `GoalInputModel` or `--goal` flag → `internal/ui/tui/goalinput_model.go`
2. Intent classification: `workflow.Engine.classifyIntent()` → `internal/engine/workflow/classify.go`
3. Workflow mode selection based on intent + complexity → `internal/core/types/types.go`
4. Phase execution: `workflow.Engine.RunPhase()` → `internal/engine/workflow/engine.go`
5. Each phase runs its own logic (initialize, discuss, plan, execute, verify, runtime, ship)
6. Phase transitions validated by `StateMachine.Transition()` → `internal/engine/workflow/state_machine.go`
7. TUI updates via `MsgEmitter.Emit()` → `internal/ui/tui/app_channel.go`
8. Final phase (Ship) commits changes and writes ledger entry

### Tool Execution Flow

1. LLM generates tool call in response → parsed from SSE stream
2. `Dispatcher.CallTool()` invoked → `internal/tools/dispatcher.go`
3. Permission check: risk level assessment + policy evaluation → `internal/tools/permissions.go`
4. If dangerous: permission request sent to TUI via channel → user approves
5. Rate limiting: token bucket check → `ToolRateLimitBurst` / `ToolRateLimitPerSec`
6. Concurrency: semaphore acquisition → `MaxConcurrentTools` limit
7. Tool executes with bounded output → `exec.OutputStore` limits lines/bytes
8. Result returned to LLM conversation context

### Subagent Flow

1. LLM calls Agent tool → `internal/tools/subagent/manager.go`
2. Manager spawns child goroutine with isolated worktree → `internal/tools/subagent/worktree.go`
3. Child gets own `Dispatcher`, provider reference, conversation loop → `internal/tools/subagent/loop.go`
4. Child runs agentic loop: LLM → tool dispatch → LLM until done/budget-exhausted
5. Events emitted to parent via `eventCh` → `internal/ui/tui/subagent_bridge.go`
6. Parent TUI renders subagent status in sidebar

**State Management:**
- TUI state: `AppState` struct — single-threaded via Bubble Tea contract, no mutex needed
- Workflow state: `WorkflowState` struct with `sync.RWMutex` guards for goroutine-safe reads
- Session state: Persisted to `<workDir>/.m31a/sessions/<id>/` as JSON files
- Provider state: `Registry` with `sync.RWMutex` for concurrent provider access
- Tool state: `Dispatcher` with `sync.RWMutex` for concurrent tool registration/calls
- Config state: `Config` struct loaded from TOML, hot-reloadable via file watcher

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Polymorphic access to LLM backends — all providers implement the same interface
- Examples: `internal/integrations/provider/interface.go`, `internal/integrations/provider/openrouter/`, `zen/`, `nvidia/`
- Pattern: Strategy pattern with runtime provider switching

**Tool Interface:**
- Purpose: Extensible tool system — any tool implementing `types.Tool` can be registered
- Examples: `internal/tools/fileops/`, `internal/tools/exec/`, `internal/tools/search/`
- Pattern: Plugin architecture with schema self-description via `SchemaProvider`

**WorkflowPhase State Machine:**
- Purpose: Validated phase transitions with history tracking
- Examples: `internal/engine/workflow/state_machine.go`
- Pattern: Finite state machine with transition map and oscillation guard

**MsgEmitter:**
- Purpose: Bridge between workflow goroutine and TUI main thread
- Examples: `internal/ui/tui/app_channel.go` (channelEmitter implementation)
- Pattern: Channel-based message passing (Bubble Tea contract)

**ContextBuilder:**
- Purpose: Assemble system prompts from multiple dynamic sources
- Examples: `internal/engine/workflow/context_builder.go`, `internal/integrations/context/registry.go`
- Pattern: Builder pattern with change detection and caching

**Dispatcher:**
- Purpose: Central tool execution hub with permissions, rate limiting, and concurrency control
- Examples: `internal/tools/dispatcher.go`
- Pattern: Mediator pattern — all tool calls go through the dispatcher

## Entry Points

**CLI Entry (`cmd/m31a/main.go`):**
- Location: `cmd/m31a/main.go`
- Triggers: Binary execution
- Responsibilities: Flag parsing, config load, provider registration, TUI or headless mode launch

**TUI App Init:**
- Location: `internal/ui/tui/app.go:Init()`
- Triggers: Bubble Tea program start
- Responsibilities: Session setup, screen routing, file watcher, config watcher, health ticker

**TUI App Update:**
- Location: `internal/ui/tui/app_update.go:Update()`
- Triggers: Any `tea.Msg` (keyboard, mouse, timer, streaming, workflow)
- Responsibilities: Central message dispatch to handler functions

**Workflow Engine RunPhase:**
- Location: `internal/engine/workflow/engine.go:RunPhase()`
- Triggers: TUI phase transition or headless mode
- Responsibilities: Dispatch to phase-specific run function

**Tools Dispatcher CallTool:**
- Location: `internal/tools/dispatcher.go:CallTool()`
- Triggers: LLM tool call from conversation
- Responsibilities: Permission check, rate limiting, tool execution, output bounding

## Architectural Constraints

- **Threading:** Bubble Tea is strictly single-threaded — all state mutations go through `Update()`. Goroutines communicate via `tea.Cmd` / `tea.Msg` channels. Workflow engine uses `MsgEmitter` to bridge goroutine output to TUI.
- **Global state:** No module-level singletons. `slog.SetDefault()` is the only global mutation in `main.go`. All other state is held in structs passed via dependency injection.
- **Circular imports:** `internal/core/types` is the shared vocabulary that breaks cycles. The `tools` package re-exports sub-package constructors via `tools_reexport.go` to avoid import cycles between `tools/ai` and `tools/subagent`.
- **CGO_ENABLED=0:** Hard constraint — binary must be static. No C dependencies allowed. If any dependency requires CGO, the build breaks.
- **`pkg/` must NOT import `internal/`:** Enforced by Go module system. Currently `pkg/` is empty.
- **Provider models are dynamic:** Never hardcode model names — always fetch from provider APIs or use config defaults.

## Anti-Patterns

### Mutating AppState from Goroutines

**What happens:** Goroutine directly modifies `AppState` fields (e.g., `m.workflowPhase = ...`)
**Why it's wrong:** Breaks Bubble Tea's single-threaded contract, causes data races and session corruption
**Do this instead:** Send a `tea.Msg` via the emitter channel → `Update()` handles the mutation. See `internal/ui/tui/app_channel.go`

### Hardcoding Model Names

**What happens:** Code references specific model IDs like `"anthropic/claude-3-opus"`
**Why it's wrong:** Model lists are dynamic — models are added/removed by providers constantly
**Do this instead:** Use `registry.ActiveProvider().FetchModels()` or `cfg.Model.Default`. See `cmd/m31a/main.go:344-380`

### Importing `internal/` from `pkg/`

**What happens:** Package in `pkg/` imports from `internal/`
**Why it's wrong:** Violates Go module dependency boundary — `internal/` is private to the module
**Do this instead:** Extract shared types into `internal/core/types/` or move code to `internal/`

### Direct Tool Execution Without Dispatcher

**What happens:** Code calls tool functions directly (e.g., `fileops.NewFileRead(workDir).Execute(...)`)
**Why it's wrong:** Bypasses permission checks, rate limiting, output bounding, and metrics collection
**Do this instead:** Always go through `dispatcher.CallTool()` → `internal/tools/dispatcher.go`

### Creating Providers Without Registry

**What happens:** Code instantiates a provider and uses it directly
**Why it's wrong:** Skips health checks, fallback logic, model caching, and capability detection
**Do this instead:** Register via `provider.Registry.Register()` and access via `registry.ActiveProvider()`

## Error Handling

**Strategy:** Sentinel errors + typed error wrappers with user-friendly messages

**Patterns:**
- Sentinel errors defined in `internal/core/errors/errors.go` (e.g., `ErrProviderUnreachable`, `ErrRateLimited`)
- Typed wrappers: `ToolError`, `ProviderError`, `ConfigError` — each with `Unwrap()` for `errors.Is`/`errors.As`
- User-facing messages via `errors.UserMessage()` — pattern-matches error strings for actionable feedback
- Workflow phases return `(*PhaseResult, error)` — result carries success flag, error carries details
- Tool execution returns `ToolError` with tool name and operation context
- All errors wrapped with `fmt.Errorf("%w", err)` for chain preservation

## Cross-Cutting Concerns

**Logging:** `log/slog` with structured logging. Logger initialized in `main.go` with version context. Per-component loggers via `slog.Default()`.

**Validation:** Config validation in `internal/core/config/config_validate.go`. Tool input validation via JSON schema self-description. Permission rule validation in `internal/tools/permissions.go`.

**Authentication:** API keys stored in OS keychain via `internal/integrations/keychain/`. Keys resolved at startup via `cfg.ResolveAPIKeys(kc)`. Never written to disk in plaintext. `.env` files gitignored except `.env.example`.

**Metrics:** Session-level metrics collection via `internal/integrations/metrics/collector.go`. Tracks tool calls, LLM usage, phase durations, heal triggers. Persisted as JSON per session.

**Compaction:** Automatic context window management via `internal/engine/compaction/compaction.go`. Proactive compaction before phase transitions. Summary-based compression of old messages.

**Narrative:** Event classification and grouping via `internal/engine/narrative/`. Transforms raw engine events into user-friendly narrative messages.

---

*Architecture analysis: 2026-08-04*
