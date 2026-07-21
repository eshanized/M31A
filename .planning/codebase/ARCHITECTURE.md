# Architecture

**Analysis Date:** Tue Jul 21 2026

## Overview

M31A is a terminal-based AI coding assistant built with Go and Bubble Tea (Elm architecture). It provides a 7-phase workflow engine that orchestrates LLM providers, 18 built-in tools, and session management through a single-threaded TUI event loop. The system emphasizes crash resilience, cost observability, and extensibility through provider/tool/plugin interfaces.

## Entry Points

- `cmd/m31a/main.go` — CLI entry, flag parsing, config load, provider registration, TUI construction, headless modes (`--prompt`, `--goal`)

## Architectural Pattern

**Pattern**: Bubble Tea (Elm Architecture) — TUI is single-threaded, all state mutations go through `Update(msg tea.Msg)`. Never mutate `AppState` from goroutines. Use `tea.Cmd` / `tea.Msg` for async.

**Workflow Engine**: 7 phases (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship) in `internal/workflow/engine.go`

**Provider Layer**: 3 providers (OpenRouter, Zen, NVIDIA) in `internal/provider/`, dynamic model discovery via `FetchModels(ctx)`

**Tools**: 18 built-in tools in `internal/tools/`, registered in `defaults.go`. Dispatcher in `dispatcher.go` handles permissions, rate limiting (token bucket), concurrency (semaphore), output bounding.

**Dependency Rule**: `pkg/` must NOT import `internal/` (enforced by Go modules)

**Shared Types**: `internal/types/types.go` — shared vocabulary across all layers

## Key Components

### TUI (Bubble Tea)

**Location**: `internal/tui/`

- `app.go` — `AppState` implements `tea.Model` with `Init()`, `Update(msg)`, `View()`
- `app_state.go` — 269-field struct holding all app state (screens, workflow, providers, tools, sessions, sub-agents, narrative engine)
- **Screen routing**: 30+ screens in `internal/tui/screens/` (Home, REPL, Discuss, Plan, Execute, Verify, Runtime, Ship, Settings, etc.)
- **State mutations ONLY through Update()** — goroutines send `tea.Msg` via channels (`emitterCh`, `fileWatcher.Events`, `permListenerCmd`, etc.)
- **Narrative engine**: `internal/narrative/engine.go` intercepts workflow messages, classifies them (narrative/grouped/hidden/expanded), renders via templates

### Workflow Engine

**Location**: `internal/workflow/`

| File | Responsibility |
|------|----------------|
| `engine.go` | Core engine: `RunPhase()`, `streamLLM()`, `streamLLMWithTools()`, preflight context check, proactive compaction, checkpointing, decision logging |
| `state_machine.go` | Validates phase transitions (7 phases + guards against Plan↔Discuss oscillation) |
| `phase_coordinator.go` | Delegates pre-phase setup, post-phase metrics, transition side effects |
| `context_builder.go` | Builds LLM context from project state, code intel, dynamic sources, research output |
| `prompt_builder.go` | Loads embedded prompts with 4-level override chain (config → project → global → embedded) |
| `initialize.go` | Project detection, git init, planning dir, deep analysis, preflight |
| `discuss.go` | Clarifying questions via LLM streaming |
| `plan.go` | Task generation with research, chunked planning, plan checker, coverage gates |
| `execute.go` | Dependency-ordered execution via `taskrunner`, parallel groups, self-heal (max 2 attempts) |
| `verify.go` | Build/test/lint verification with quality gates |
| `runtime.go` | Dev server management, runtime verification |
| `ship.go` | Changelog generation, commit, push, PR creation |
| `cost_tracker.go` | Per-model cost accumulation, budget limit enforcement |
| `compaction/` | Auto-compaction with tiktoken calibration |

**Phase Flow** (from `engine.go:819-891`):
```go
phases := []types.WorkflowPhase{
    types.PhaseInitialize,
    types.PhaseDiscuss,
    types.PhasePlan,
    types.PhaseExecute,
    types.PhaseVerify,
    types.PhaseRuntime,
    types.PhaseShip,
}
for _, phase := range phases {
    result, err := engine.RunPhase(ctx, phase, goal)
    engine.Transition(ctx, phase, nextPhase)
}
```

### Provider Layer

**Location**: `internal/provider/`

**Interface** (`interface.go:15-23`):
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

**Registry** (`registry.go`) — thread-safe provider registration, active provider management with `TrySetActive()` for atomic failover, `RollbackActive()` for health-check failures.

**BaseClient** (`base_client.go`) — shared HTTP transport (connection pooling), dual clients (streaming no-timeout, catalog with timeout), model cache with stale fallback, SSE parser, health check with latency thresholds.

**Implementations**:
- `openrouter/client.go` — OpenRouter API, model catalog with pricing/architecture
- `zen/client.go` — Zen (OpenCode) API
- `nvidia/client.go` — NVIDIA NIM API

**Dynamic Model Discovery**: `FetchModels(ctx)` called at startup and on model picker refresh — models never hardcoded.

### Tools System

**Location**: `internal/tools/`

**Tool Interface** (`types.go:241-246`):
```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}
```

**SchemaProvider** (optional) — provides JSON Schema for LLM tool definitions.

**18 Built-in Tools** (registered in `defaults.go:70-145`):

| Category | Tools |
|----------|-------|
| File Ops | `FileRead`, `FileWrite`, `Edit`, `FileList`, `FileDelete`, `FileMove` |
| Exec | `Bash` (with blocked/obfuscation patterns), `DevServer` |
| Search | `Glob`, `Grep`, `WebSearch`, `WebFetch` (DNS cache) |
| AI | `AskUserQuestion` |
| Todo | `TodoWrite`, `TodoRead` |
| Code Intel | `CodeMap`, `CodeComplexity`, `HTTPCheck`, `Git` |
| Subagent | `Agent` (spawns parallel child agents with own worktrees) |

**Dispatcher** (`dispatcher.go`) — central execution hub:
- **Permissions**: Rule-based with TTL, per-agent profiles, batch approval
- **Rate Limiting**: Token bucket (configurable burst/sec), stricter for dangerous tools
- **Concurrency**: Semaphore (default 4, configurable)
- **Output Bounding**: `OutputStore` limits tool output to prevent context exhaustion
- **Metrics**: Records call count, success/fail, duration per tool

### Session Management

**Location**: `internal/session/manager.go`

- Project-local storage in `<workDir>/.m31a/` (flat files: `session.json`, `messages.json`, `checkpoint.json`, `plan.md`, `tasks.json`, `MEMORY.md`)
- Global config in `~/.m31a/`
- Checkpointing at phase boundaries for crash recovery
- Coordinator for per-session concurrency control

### Key Shared Types (`internal/types/types.go`)

| Type | Purpose |
|------|---------|
| `WorkflowPhase` | 7 phases + `PhaseIdle` |
| `WorkflowMode` | `auto`, `full`, `fast`, `direct` (phase skipping) |
| `IntentType` / `IntentResult` | LLM-based prompt classification for routing |
| `Message` / `MessageSegment` | Conversation with segments (text, thinking, tool_call, compaction) |
| `ToolCall` / `ToolResult` | Structured tool invocation & result |
| `Task` | Plan task with deps, files, acceptance criteria, heal tracking |
| `ModelInfo` | Provider, pricing, context length, capabilities |
| `ProjectState` | Goal, project type, framework, discuss answers |
| `Session` | Metadata for resume/history |

## Data Flow

### Primary Request Path (TUI → Workflow → LLM → Tools → TUI)

```
User Input (REPL)
    │
    ▼
AppState.Update() ───▶ RunPhaseCmd(phase) ───▶ tea.Cmd runs engine.RunPhase() in goroutine
    │                                                    │
    │                              ┌─────────────────────┼─────────────────────┐
    │                              ▼                     ▼                     ▼
    │                        buildPlanContext        streamLLM()           executeTaskWithTools()
    │                              │                     │                     │
    │                              ▼                     ▼                     ▼
    │                        ContextBuilder          prepareStreamRequest  Dispatcher.Execute()
    │                              │                     │                     │
    │                              ▼                     ▼                     ▼
    │                        LLM Request            SSE Stream            Tool Execution
    │                              │                     │                     │
    │                              ▼                     ▼                     ▼
    │                        Parse Response        consumeStream()      ToolResult + Metrics
    │                              │                     │                     │
    └──────────────────────────────┼─────────────────────┼─────────────────────┘
                                   ▼                     ▼
                            PhaseResultMsg          TaskStartMsg,
                            (via emitterCh)         ToolStartMsg,
                                                       TaskUpdateMsg
                                   │                     │
                                   └─────────────────────┘
                                             │
                                             ▼
                                    AppState.Update()
                                    (drains emitterCh via
                                     drainAdaptiveCmd())
                                             │
                                             ▼
                                       REPL View Update
```

### Context Building (`context_builder.go`)

1. `ContextRegistry.LoadAll(ctx)` — parallel load from sources (datetime, env, git, code intel)
2. `ContextRegistry.Reconcile()` — detects changes since last snapshot
3. Dynamic context injected into system prompt per phase

### Proactive Compaction (`engine.go:1146-1208`)

Before phase transitions and every N tool calls during Execute:
1. Estimate tokens vs model context window
2. If > threshold (default 60%): trigger `Compactor.Compact()` with LLM
3. Replace old messages with summary + recent messages
4. Emit `CompactionCompleteMsg` to TUI

### Self-Heal Loop (`execute.go:223-350`)

On task verification failure:
1. Increment `task.HealsAttempted`
2. Build failure context (errors, files, criteria)
3. `streamLLMWithTools()` with heal prompt
4. Dispatch heal tool calls
5. Re-verify; if passes → `StatusDone`, else retry (max 2)

## Key Abstractions

| Abstraction | Location | Purpose |
|-------------|----------|---------|
| `LLMProvider` | `internal/provider/interface.go` | Unified interface for 3 providers |
| `Tool` | `internal/types/types.go:241` | Plugin interface for 18 tools |
| `MsgEmitter` | `internal/workflow/engine.go:1003` | Callback for workflow→TUI async messages |
| `StateMachine` | `internal/workflow/state_machine.go` | Validated phase transitions |
| `ContextSource` | `internal/context/source.go` | Pluggable dynamic context (git, env, datetime) |
| `Dispatcher` | `internal/tools/dispatcher.go` | Central tool execution with policy enforcement |
| `TaskRunner` | `internal/taskrunner/runner.go` | Dependency-ordered parallel task execution |
| `Compactor` | `internal/compaction/compaction.go` | LLM-based context compression |
| `NarrativeEngine` | `internal/narrative/engine.go` | Message classification & templated rendering |

## Entry Points

| Entry Point | Location | Triggers |
|-------------|----------|----------|
| **TUI** | `cmd/m31a/main.go:534` | Default (no flags) — `tea.NewProgram(app).Run()` |
| **Headless Prompt** | `main.go:183` | `--prompt "..."` — single LLM call, print response |
| **Headless Workflow** | `main.go:56` | `--goal "..."` — full 7-phase workflow, no TUI |

## Architectural Constraints

- **Threading**: Bubble Tea is single-threaded. All state mutations in `Update()`. Goroutines communicate via `tea.Cmd` channels (`emitterCh`, `fileWatcher.Events`, permission/question listeners). Signal handler sends `tea.QuitMsg` via `p.Send()` not direct `app.Shutdown()`.
- **Global State**: Module-level singletons: `sharedTransport` (`base_client.go:18`), `globalDropCounter` (`app.go`), `skipDirsCache` (`constants.go:137`). Protected by `sync.Once`/`sync.RWMutex`.
- **Circular Imports**: None detected. Dependency direction: `cmd/` → `internal/tui/` → `internal/workflow/` → `internal/provider, tools, session, types` → `internal/types` (leaf).
- **Error Handling**: Sentinel errors in `internal/errors/errors.go` with `UserMessage()` for actionable user feedback. Wrapped errors preserve chain via `fmt.Errorf("%w", err)`. `ProviderError`, `ToolError`, `ConfigError` carry structured context.

## Cross-Cutting Concerns

### Logging
- **Framework**: `log/slog` with structured JSON output
- **Location**: `internal/log/` (logger init with version, file rotation)
- **Pattern**: `logger.Info("msg", "key", val)` — never `fmt.Printf` in library code

### Validation
- **Config**: `internal/config/loader.go` validates on load, `config_validate.go` for cross-field rules
- **Tool Input**: JSON unmarshal + direct-args normalization in `dispatcher.go:275-295`
- **Plan Tasks**: `validateTasks()` in `plan.go:120` checks IDs, deps, files, criteria

### Authentication
- **API Keys**: Stored in OS keychain via `internal/keychain/` (libsecret/Keychain/Credential Manager). Fallback to config file if keychain unavailable.
- **Env Vars**: `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY` — loaded via `.env` in `config.LoadDotEnv()`

---

*Architecture analysis: Tue Jul 21 2026*