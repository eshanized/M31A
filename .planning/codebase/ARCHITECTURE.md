<!-- refreshed: 2026-07-16 -->
# Architecture

**Analysis Date:** 2026-07-16

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                    ENTRY POINT                                       │
│  cmd/m31a/main.go — flag parsing, config load, provider registration, TUI launch    │
└─────────────────────────────────────────┬───────────────────────────────────────────┘
                                          │
                                          ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                      TUI LAYER                                       │
│  internal/tui/ — Bubble Tea (Elm architecture)                                       │
│  ┌──────────────────┬──────────────────┬──────────────────┬────────────────────────┐ │
│  │  Screen Router   │  Component Tree  │  Message Loop    │  Narrative Emitter     │ │
│  │  (app_routing.go)│ (components/)    │ (app_update.go)  │ (narrative_emitter.go) │ │
│  └────────┬─────────┴────────┬─────────┴────────┬─────────┴────────────┬─────────┘ │
└───────────┼───────────────────┼──────────────────┼──────────────────────┼───────────┘
            │                   │                  │                      │
            ▼                   ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              WORKFLOW ENGINE                                         │
│  internal/workflow/engine.go — Seven-phase state machine                             │
│  ┌──────────┬──────────┬────────┬──────────┬────────┬──────────┬────────┐           │
│  │Initialize│ Discuss  │ Plan   │ Execute  │ Verify │ Runtime  │ Ship   │           │
│  └────┬─────┴────┬─────┴────┬───┴────┬────┴────┬────┴────┬────┴────┬────┘           │
│       │         │         │        │         │         │        │                   │
│       └─────────┴─────────┴────────┴─────────┴─────────┴────────┴────────┘           │
│                                    │                                                   │
│  ┌────────────────────────────────┼──────────────────────────────────────────────┐    │
│  │  PhaseCoordinator              │  StateMachine (state_machine.go)               │  │
│  │  - PrePhaseSetup               │  - Validates transitions                       │  │
│  │  - PostPhaseExecution          │  - Plan↔Discuss oscillation guard (max 3)      │  │
│  │  - CoordinateTransition        │  - History tracking                            │  │
│  └────────────────────────────────┴──────────────────────────────────────────────┘    │
└────────────────────────────────────┬─────────────────────────────────────────────────┘
                                     │
          ┌──────────────────────────┼──────────────────────────┐
          ▼                          ▼                          ▼
┌─────────────────────┐   ┌─────────────────────┐   ┌─────────────────────┐
│   PROVIDER LAYER    │   │   TOOLS LAYER       │   │  SESSION/STATE      │
│  internal/provider/ │   │  internal/tools/    │   │  pkg/session/       │
│  ┌────────────────┐ │   │  ┌────────────────┐ │   │  ┌────────────────┐ │
│  │ Registry       │ │   │  │ Dispatcher     │ │   │  │ Manager        │ │
│  │ (registry.go)  │ │   │  │ (dispatcher.go)│ │   │  │ (manager.go)   │ │
│  └────────────────┘ │   │  └───────┬────────┘ │   │  └───────┬────────┘ │
│  ┌────────────────┐ │   │  ┌───────┴────────┐ │   │  ┌───────┴────────┐ │
│  │ LLMProvider    │ │   │  │ 18 Built-in    │ │   │  │ Project-local  │ │
│  │ (interface.go) │ │   │  │ Tools          │ │   │  │ .m31a/session  │ │
│  └────────────────┘ │   │  └────────────────┘ │   │  │ JSON files     │ │
│  ┌────────────────┐ │   │  ┌────────────────┐ │   │  │ Checkpoint/    │ │
│  │ OpenRouter     │ │   │  │ Permissions    │ │   │  │ Rollback       │ │
│  │ Zen            │ │   │  │ Rate limiting  │ │   │  └────────────────┘ │
│  │ Nvidia         │ │   │  │ Concurrency    │ │   └─────────────────────┘
│  │ (subdirs/)     │ │   │  │ Output bounds  │ │
│  └────────────────┘ │   │  └────────────────┘ │
└─────────────────────┘   └─────────────────────┘
          │                          │                          │
          └──────────────────────────┼──────────────────────────┘
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              SHARED TYPES (pkg/types/)                               │
│  Canonical definitions: Message, ToolCall, ModelInfo, WorkflowPhase, Task, etc.     │
│  internal/types/ provides type aliases for backward compatibility.                   │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | CLI flags, config loading, provider registration, TUI construction, signal handling, headless modes | `cmd/m31a/main.go` |
| **TUI App** | Bubble Tea model (`AppState`), screen routing, message loop, narrative emitter, sidebar, REPL | `internal/tui/app.go`, `internal/tui/app_*.go` |
| **Workflow Engine** | Seven-phase orchestration, LLM streaming, context building, compaction, cost tracking, checkpointing | `internal/workflow/engine.go` |
| **State Machine** | Phase transition validation, history, oscillation guard | `internal/workflow/state_machine.go` |
| **Phase Coordinator** | Pre/post phase hooks, transition side effects, metrics | `internal/workflow/phase_coordinator.go` |
| **Provider Registry** | Register/activate providers, health checks, fallback ordering | `internal/provider/registry.go` |
| **LLM Providers** | OpenRouter, Zen, Nvidia — dynamic model discovery, streaming, cost estimation | `internal/provider/*/client.go` |
| **Tools Dispatcher** | Tool registry, permission gating, rate limiting (token bucket), concurrency semaphore, output bounding | `internal/tools/dispatcher.go` |
| **Built-in Tools** | 18 tools: Bash, FileRead/Write/Edit/List/Delete/Move, Glob, Grep, WebFetch, WebSearch, Git, CodeMap, CodeComplexity, DevServer, HTTPCheck, TodoWrite/Read, AskUserQuestion, Agent | `internal/tools/*.go`, `internal/tools/defaults.go` |
| **Session Manager** | Project-local session persistence (`.m31a/session.json`, `messages.json`), checkpoints, recent models | `pkg/session/manager.go` |
| **Keychain** | OS-native secret storage (D-Bus Secret Service, macOS Keychain, Windows Credential Manager) | `pkg/keychain/keychain.go` + platform files |
| **Config** | 6-layer TOML cascade (defaults → global → project → env → flags → hot-reload) | `internal/config/loader.go`, `internal/config/types.go` |
| **Decision Log** | Structured decision receipts with categories (intent, plan, tool, heal, etc.) | `internal/decision/logger.go` |
| **Narrative System** | Message classification (narrative/grouped/hidden/expanded), templated rendering | `pkg/narrative/` |
| **Code Intelligence** | 4-language parser (Go, TS, Python, Rust), import graph, relevance scoring | `internal/codeintel/` |
| **Compaction** | Automatic session summarization when context fills | `pkg/compaction/` |
| **Metrics** | Tool calls, LLM usage, phase durations, heal events per session | `pkg/metrics/` |
| **Rollback** | Git-based snapshot/restore for task-level undo | `pkg/rollback/` |
| **Task Runner** | Parallel task execution with dependency resolution | `pkg/taskrunner/` |

## Pattern Overview

**Overall:** Layered architecture with strict dependency direction — `cmd/` → `internal/` → `pkg/`. The `pkg/` directory contains public packages that external projects may import; `internal/` packages are private by Go module system enforcement.

**Key Characteristics:**
- **Single-threaded TUI**: Bubble Tea's Elm architecture — all state mutations in `Update()`, never from goroutines. Cross-goroutine communication via `tea.Cmd` / channels.
- **Seven-phase workflow**: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship. Configurable skipping via `WorkflowMode` (auto/full/fast/direct).
- **Dynamic provider/model discovery**: Models fetched from provider APIs at runtime, never hardcoded. Capability detection via pattern matching (F-011, F-012).
- **Permission-gated tools**: Every tool execution passes through `Dispatcher.ensurePermission()` with rule engine, batch approval, and per-risk-level rate limiting.
- **Structured error handling**: Sentinel errors in `internal/errors/errors.go` with user-friendly messages; `fmt.Errorf("%w", err)` wrapping throughout.
- **Checkpoint/resume**: Workflow state persisted to disk per phase; supports crash recovery and session resumption.

## Layers

### Entry Point Layer
- **Purpose**: Application bootstrap, configuration, dependency wiring
- **Location**: `cmd/m31a/`
- **Contains**: Flag parsing, `.env` loading, logger init, keychain setup, provider registration, TUI construction, signal handling
- **Depends on**: `internal/config`, `internal/log`, `internal/provider`, `internal/tui`, `internal/tools`, `internal/workflow`, `pkg/session`, `pkg/keychain`, `pkg/rollback`, `pkg/ledger`, `pkg/autodream`
- **Used by**: Binary execution only

### TUI Layer
- **Purpose**: Terminal user interface, screen management, user input handling
- **Location**: `internal/tui/`
- **Contains**: 
  - `AppState` — root Bubble Tea model
  - Screen router (`app_routing.go`, `router.go`)
  - Components (`components/`) — sidebar, REPL, modals, plan/execute/verify/ship screens
  - Commands (`commands/`) — slash command implementations
  - Narrative emitter — classifies and renders workflow messages
  - Layout engine (`layout/`) — responsive breakpoints (ultra-compact/compact/full)
  - Theme system (`theme/`) — single dark theme with customization
- **Depends on**: `internal/config`, `internal/types`, `internal/workflow`, `pkg/session`, `pkg/metrics`, `pkg/arbitrage`
- **Used by**: Entry point

### Workflow Engine Layer
- **Purpose**: Orchestrate the seven-phase coding workflow with LLM interactions
- **Location**: `internal/workflow/`
- **Contains**:
  - `Engine` — core orchestrator, phase runners, LLM streaming, context building
  - `StateMachine` — phase transition validation
  - `PhaseCoordinator` — pre/post phase hooks, metrics, checkpoints
  - `PromptBuilder` — loads embedded/project/global prompt templates
  - `ContextBuilder` — composes system prompts with dynamic context (git, env, files)
  - Phase implementations: `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go`
  - Self-healing: `execute_heal.go`, `self_heal_visibility_test.go`
  - Intent classification: `intent.go`, `classify.go`
- **Depends on**: `internal/provider`, `internal/tools`, `internal/tokens`, `internal/errors`, `internal/decision`, `internal/context`, `internal/codeintel`, `pkg/compaction`, `pkg/ledger`, `pkg/metrics`, `pkg/retry`, `pkg/session`
- **Used by**: TUI layer (via `RunPhaseCmd`), headless mode

### Provider Layer
- **Purpose**: Abstract LLM provider interactions with unified interface
- **Location**: `internal/provider/`
- **Contains**:
  - `LLMProvider` interface (`interface.go`) — `FetchModels`, `ChatCompletionStream`, `EstimateCost`, `HealthCheck`, `GetModel`
  - `Registry` — thread-safe provider registration/activation with fallback support
  - `BaseClient` — shared HTTP client, model caching, SSE parsing, retry logic
  - Provider implementations: `openrouter/`, `zen/`, `nvidia/`
  - Capability detection: `capabilities.go` — reasoning/tool/vision/chat flags from model ID patterns
  - Model metadata: `model_metadata.go` — context length, pricing, architecture
- **Depends on**: `internal/types`, `internal/errors`
- **Used by**: Workflow engine, TUI (model picker), headless mode

### Tools Layer
- **Purpose**: Execute LLM-requested actions with safety controls
- **Location**: `internal/tools/`
- **Contains**:
  - `Dispatcher` — central execution hub with permission gating, rate limiting (token bucket), concurrency semaphore, output store
  - `Tool` interface (`interface.go`) — `Name()`, `Description()`, `RiskLevel()`, `Execute()`
  - 18 built-in tools registered in `defaults.go`
  - Permission system: rule-based with TTL, agent profiles, batch approval
  - Subagent support: `subagent/` — spawns child agents with isolated dispatchers and git worktrees
- **Depends on**: `internal/config`, `internal/errors`, `internal/types`, `pkg/metrics`
- **Used by**: Workflow engine (via `Dispatcher`), TUI (permission/question listeners)

### Session/State Layer
- **Purpose**: Persist and restore session state, workflow checkpoints, cross-session learning
- **Location**: `pkg/session/`, `pkg/keychain/`, `pkg/ledger/`, `pkg/rollback/`
- **Contains**:
  - `Manager` — project-local session storage (`.m31a/session.json`, `messages.json`), checkpoint save/load
  - `Keychain` — OS-native secret storage for API keys (never written to disk)
  - `Ledger` — cross-session learning records (`LEDGER.md`)
  - `Rollback` — git-based task snapshots for undo
- **Depends on**: `pkg/types`, `pkg/coordinator` (concurrency control), `internal/errors`
- **Used by**: Entry point, workflow engine, TUI

### Shared Types Layer
- **Purpose**: Canonical type definitions shared across all layers
- **Location**: `pkg/types/`
- **Contains**: `Message`, `ToolCall`, `ToolResult`, `ModelInfo`, `WorkflowPhase`, `Task`, `Session`, `Usage`, `Pricing`, `HealthStatus`, `ChatRequest`, `ToolDefinition`, `StreamChunk`, `StreamIterator`, and constants
- **Depends on**: Standard library only
- **Used by**: All layers (via `internal/types/` aliases)

## Data Flow

### Primary Request Path (TUI → Workflow → Provider → Tools → TUI)

```
1. User submits goal in TUI (REPL or /workflow)
   └─> cmd/m31a/main.go → tui.NewApp() → AppState.Init()

2. TUI creates workflow Engine for session
   └─> tui/app.go:initWorkflowEngine() → workflow.NewEngine()

3. User triggers phase (e.g., Plan) via slash command or workflow start
   └─> tui/app.go:RunPhaseCmd() → tea.Cmd runs Engine.RunPhase() in goroutine

4. Engine.RunPhase() for Plan phase:
   a. PrePhaseSetup (PhaseCoordinator) — checkpoint, compaction check, budget guard
   b. Build context: ContextBuilder → system prompt + dynamic context (git, files, project)
   c. Stream LLM: Engine.prepareStreamRequest() → provider.ChatCompletionStream()
   d. Consume stream: Engine.consumeStreamWithTools() → tool calls dispatched
   e. Tool execution: Dispatcher.Execute() → permission check → tool.Execute() → result
   f. Results fed back to LLM until completion
   g. PostPhaseExecution — metrics, checkpoint save, transition

5. Phase result emitted via MsgEmitter → TUI Update() handles PhaseResultMsg
   └─> Updates sidebar, REPL, plan screen

6. Transition to next phase: Engine.Transition() → StateMachine validates → PhaseCoordinator side effects
```

### Headless Workflow Path (`--goal` flag)

```
1. main.go:runHeadlessWorkflow() creates Engine directly (no TUI)
2. Runs all 7 phases sequentially via Engine.RunPhase()
3. No MsgEmitter — results printed to stderr
4. Exits with code 0/1
```

### Provider Model Discovery

```
1. Registry.Register() called for each configured provider (main.go)
2. On first model fetch: Provider.FetchModels() → HTTP GET /models
3. Response parsed → []types.ModelInfo cached in BaseClient.Cache (TTL: 5 min default)
4. Subsequent calls return cached models until expiry
5. Capability detection: provider.ParseModelCapabilities(modelID) → CapFlags
```

### Tool Execution with Permissions

```
1. LLM emits tool_call chunk → Engine.finalizeToolCalls() → []ToolCall
2. For each call: Dispatcher.Execute(ctx, call)
3. Concurrency semaphore (MaxConcurrentTools, default 4)
4. Rate limiter: token bucket (ToolRateLimitPerSec, burst)
5. Dangerous tools: stricter rate limiter (DangerousRateLimitPerSec)
4. Permission check: Dispatcher.ensurePermission()
   - Rule engine: match tool/resource/pattern → action (allow/deny/ask)
   - Agent default: per-agent profile default action
   - Batch approval: user can approve all future same-tool+risk calls
   - Fallback: dangerous tools always ask
5. If allowed: tool.Execute() → ToolResult
6. Output bounded: OutputStore bounds to max lines/bytes
7. Metrics recorded: Collector.RecordToolCall()
```

### Session Persistence

```
1. Session created: Manager.NewSession() → .m31a/session.json + messages.json
2. On each phase completion: Manager.UpdateWorkflowState() → session.json
3. On REPL message: Manager.SaveMessages() → messages.json (debounced)
4. On checkpoint: Engine.SaveCheckpointData() → session.Checkpoint + Manager.SaveCheckpoint()
5. On resume: Manager.LoadSession() → restores messages, workflow phase, goal
6. On shutdown: AppState.saveSessionOnShutdown() → full persist
```

### State Management (Bubble Tea Contract)

```
┌─────────────────────────────────────────────────────────────┐
│                    AppState (model)                          │
│  - All mutable state in struct fields                        │
│  - Never mutated from goroutines                             │
└─────────────────────┬───────────────────────────────────────┘
                      │ Update(msg) — ONLY place state changes
                      ▼
┌─────────────────────────────────────────────────────────────┐
│  Messages from:                                              │
│  - User input (key/mouse) → tea.KeyMsg, tea.MouseMsg        │
│  - Workflow emitter → PhaseResultMsg, TaskStartMsg, etc.    │
│  - Permission/question channels → PermissionRequestMsg      │
│  - Subagent manager → SubagentEventMsg                      │
│  - File watcher → SidebarRefreshMsg                         │
│  - Config watcher → ConfigReloadMsg                         │
│  - Tickers → HealthCheckTickMsg, SidebarRefreshTickMsg      │
└─────────────────────────────────────────────────────────────┘
```

## Key Abstractions

### LLMProvider Interface
- **Purpose**: Uniform access to different LLM APIs
- **Location**: `internal/provider/interface.go`
- **Methods**: `Name()`, `APIKey()`, `FetchModels()`, `CachedModels()`, `ChatCompletionStream()`, `EstimateCost()`, `HealthCheck()`, `GetModel()`
- **Implementations**: `openrouter.Client`, `zen.Client`, `nvidia.Client`
- **Pattern**: Embed `BaseClient` for shared HTTP, caching, SSE, retry logic

### Tool Interface
- **Purpose**: Uniform tool execution with risk classification
- **Location**: `pkg/types/types.go` (canonical), `internal/tools/interface.go`
- **Methods**: `Name()`, `Description()`, `RiskLevel()`, `Execute(ctx, ToolInput) (ToolResult, error)`
- **Optional**: `SchemaProvider` — `ParameterSchema()` for LLM function calling
- **Examples**: `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `Git`, `WebFetch`, `Agent`

### WorkflowEngine (via MsgEmitter)
- **Purpose**: Decouple workflow execution from TUI rendering
- **Location**: `internal/workflow/engine.go` + `internal/tui/narrative_emitter.go`
- **Pattern**: `Engine.SetMsgEmitter(emitter)` — emitter receives `tea.Msg` via channel, TUI drains in `drainEmitterCmd()`
- **Messages**: `ThinkingStartMsg`, `ThinkingCompleteMsg`, `TaskStartMsg`, `TaskUpdateMsg`, `ToolStartMsg`, `ToolCompleteMsg`, `PhaseResultMsg`, `CompactionCompleteMsg`, `DecisionsSnapshotMsg`

### StateMachine
- **Purpose**: Enforce valid phase transitions with history
- **Location**: `internal/workflow/state_machine.go`
- **Transitions**: Defined in `validTransitions` map; `Transition(from, to)` validates and records
- **Guard**: Plan↔Discuss cycles limited to 3 (`maxDiscussPlanCycles`)

### ContextBuilder
- **Purpose**: Compose system prompts with dynamic context per phase
- **Location**: `internal/workflow/context_builder.go`
- **Sources**: Embedded base prompt, project AGENTS.md, git status, environment, file snapshots, intent classification
- **Caching**: Base prompt built once (`sync.Once`); full prompts cached per "extras signature"

## Entry Points

| Entry Point | Location | Triggers | Responsibilities |
|-------------|----------|----------|------------------|
| **Main binary** | `cmd/m31a/main.go:run()` | `m31a` | Full TUI bootstrap, config, providers, session, signal handling |
| **Headless prompt** | `cmd/m31a/main.go:runHeadless()` | `m31a --prompt "..."` | Single LLM request, streaming response to stdout |
| **Headless workflow** | `cmd/m31a/main.go:runHeadlessWorkflow()` | `m31a --goal "..."` | Full 7-phase workflow without TUI, exits with code |
| **Version** | `cmd/m31a/main.go` | `m31a --version` | Print version/commit/date/Go version |
| **Help** | `cmd/m31a/usage.go` | `m31a --help` | Print usage with slash commands |

## Architectural Constraints

- **Threading**: Bubble Tea is strictly single-threaded. All `AppState` mutations in `Update()`. Goroutines communicate via channels → `tea.Cmd` → `Update()`. The signal handler sends `tea.QuitMsg` via `Program.Send()`, never calls `AppState.Shutdown()` directly.
- **Global state**: Module-level singletons:
  - `internal/tools.DefaultOutputMaxLines/Bytes` (constants)
  - `internal/types.DefaultContextLength`, `MaxLLMResponseBytes` (constants)
  - `pkg/keychain` — `NewCached()` wraps platform keychain with availability caching
  - `internal/workflow.maxDiscussPlanCycles` (constant = 3)
- **Circular imports**: Prevented by Go module system — `pkg/` cannot import `internal/`. `internal/types` aliases `pkg/types` to break cycles.
- **Config hot-reload**: `config.WatchConfig()` runs in goroutine, sends `ConfigReloadMsg` to TUI via channel.
- **API keys**: Never written to disk. Stored in OS keychain via `pkg/keychain/`. Config holds only references.
- **Provider model lists**: Always dynamic — fetched from provider APIs. Never hardcoded.
- **Static binary**: `CGO_ENABLED=0` enforced in Makefile and goreleaser. No C dependencies.

## Anti-Patterns

### Mutating AppState from Goroutine
**What happens:** A goroutine directly modifies `AppState` fields (e.g., `m.workflowPhase = ...`).
**Why it's wrong:** Bubble Tea's `Update()` is the single state mutation point. Concurrent mutation causes data races and UI corruption.
**Do this instead:** Send a `tea.Msg` via `Program.Send()` or emitter channel; handle in `Update()`.

### Direct Provider HTTP Calls from TUI
**What happens:** TUI code makes raw HTTP requests to provider APIs.
**Why it's wrong:** Bypasses provider abstraction, retry logic, caching, cost estimation, health checks.
**Do this instead:** Use `registry.ActiveProvider().ChatCompletionStream()` via `Engine` or `MsgEmitter`.

### Hardcoding Model Names
**What happens:** Code references specific model IDs like `"anthropic/claude-3.5-sonnet"`.
**Why it's wrong:** Models change, new ones added, pricing updates. Capability detection breaks.
**Do this instead:** Use `provider.FetchModels()` and `provider.GetModel(id)`; capability detection via `provider.ParseModelCapabilities()`.

### Skipping Permission Checks
**What happens:** Tool execution bypasses `Dispatcher.ensurePermission()`.
**Why it's wrong:** Dangerous operations (file delete, bash rm -rf) execute without user consent.
**Do this instead:** Always go through `Dispatcher.Execute()` which enforces permissions.

### Mutating Shared Slices in Workflow
**What happens:** `Engine.state.Messages` appended from goroutine while TUI reads.
**Why it's wrong:** Data race — Go race detector will flag; causes corrupted message history.
**Do this instead:** Use `Engine.state.messagesMu` (RWMutex) or emit `StreamChunkMsg` for TUI to append.

## Error Handling

**Strategy:** Sentinel errors with user-friendly messages, wrapped with context.

**Patterns:**
- **Sentinel errors**: `internal/errors/errors.go` — `ErrInvalidProvider`, `ErrProviderNotFound`, `ErrSessionNotFound`, `ErrPhaseTransition`, `ErrContextExceeded`, `ErrToolExecution`, `ErrKeychainUnavailable`, `ErrInvalidKey`
- **Wrapping**: `fmt.Errorf("context: %w", err)` — preserves error chain for `errors.Is/As`
- **User messages**: `errors.UserMessage(err)` extracts actionable message for TUI toasts
- **Logging**: `slog.Error("operation failed", "error", err)` — structured, includes context
- **Recovery**: `preflightContextCheck()` truncates/compacts before LLM call; `ErrContextExceeded` returned only if truncation insufficient

## Cross-Cutting Concerns

| Concern | Approach |
|---------|----------|
| **Logging** | `internal/log.NewLogger()` — JSON lines with daily rotation, level from config, `slog.Default()` set in main |
| **Validation** | Config validation in `config.Validate()`; permission rules validated on load; tool input JSON schema validated on execute |
| **Authentication** | API keys from OS keychain → config resolution → provider registration. No keys in config files. |
| **Observability** | `pkg/metrics/Collector` — per-session JSONL (`METRICS.json`): tool calls, LLM usage, phase durations, heal events |
| **Security** | Bash tool: command allowlist/blocklist, obfuscation detection, sandbox (landlock/seccomp on Linux), timeout enforcement |
| **Resilience** | Provider retry with exponential backoff (max 2); fallback priority chain; stale model cache fallback; compaction before context overflow |

---

*Architecture analysis: 2026-07-16*