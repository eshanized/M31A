# ARCHITECTURE.md — M31A System Architecture

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Pattern

**Elm Architecture (TEA)** via Bubble Tea — unidirectional data flow with immutable state updates.

```
Model → Update(msg) → Model → View
         ↑                  |
         └── Cmd(msg) ──────┘
```

All state mutations happen exclusively in `Update()`. Goroutines send messages via `tea.Cmd` / `tea.Msg` channels. No shared mutable state.

## Layer Architecture

```
┌─────────────────────────────────────────┐
│  cmd/m31a/main.go                       │  Entry point, flag parsing, wiring
├─────────────────────────────────────────┤
│  internal/ui/tui/                       │  Presentation layer (Bubble Tea)
│  └─ app.go → repl_model.go → view     │  180+ files, screens, models
├─────────────────────────────────────────┤
│  internal/engine/                       │  Business logic
│  └─ workflow/ (100 files)              │  Phase orchestration, execution
│  └─ session/                           │  Session persistence
│  └─ bisect/, rollback/                 │  Error recovery
│  └─ coordinator/                       │  Task coordination
├─────────────────────────────────────────┤
│  internal/tools/                        │  Tool system (18+ tools)
│  └─ dispatcher.go                      │  Permission, rate limiting, concurrency
│  └─ fileops/, exec/, search/, git/    │  Tool implementations
├─────────────────────────────────────────┤
│  internal/integrations/                 │  External services
│  └─ provider/                          │  LLM providers (3)
│  └─ keychain/, git/, ledger/           │  OS integrations
├─────────────────────────────────────────┤
│  internal/core/                         │  Shared types, config, errors
│  └─ types/, config/, errors/           │  Foundation layer
└─────────────────────────────────────────┘
```

## Data Flow

### Tool Execution Flow
```
LLM response → parse tool calls → dispatcher.Dispatch()
  → permission check (rules, batch approval, policy)
  → rate limiter (token bucket)
  → concurrency semaphore
  → tool.Execute()
  → output bounding (OutputStore)
  → response back to LLM
```

### Workflow Phase Flow
```
Phase: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship
  ↓
engine.RunPhase(ctx, phase, goal)
  → phase-specific handler (initialize.go, discuss.go, plan.go, execute.go, etc.)
  → LLM streaming with tool execution loop
  → coverage gates verification
  → transition to next phase
```

### Session Persistence
```
Session Manager (session/manager.go)
  ├─ Project-local: <workDir>/.m31a/sessions/
  ├─ Global: ~/.m31a/sessions/
  ├─ Checkpoints (session/checkpoint.go)
  └─ Planning state (session/planning.go)
```

## Key Abstractions

### Types (`internal/core/types/`)
- `types.Tool` — interface for all tools
- `types.ToolDefinition` — tool schema for LLM
- `types.Message` — chat message (role + content)
- `types.ModelInfo` — model metadata
- `types.StreamIterator` — streaming LLM responses
- `types.WorkflowPhase` — phase enum (Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship)
- `types.RiskLevel` — tool risk classification (safe, low, medium, high, dangerous, destructive)

### Provider Interface (`internal/integrations/provider/interface.go`)
```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx) ([]ModelInfo, error)
    ChatCompletionStream(ctx, ChatRequest) (*StreamIterator, error)
    EstimateCost(modelID, Usage) float64
    HealthCheck(ctx) HealthStatus
    GetModel(id) (*ModelInfo, error)
}
```

### Dispatcher (`internal/tools/dispatcher.go`)
- Central tool execution manager
- Handles permissions, rate limiting, concurrency control
- Batch approval system for repeated tool calls
- Persistent permission rules (saved to disk)

## Entry Points

| Entry | Path | Purpose |
|-------|------|---------|
| Main binary | `cmd/m31a/main.go` | CLI parsing, provider setup, TUI launch |
| Headless mode | `cmd/m31a/main.go:runHeadless()` | Single prompt → LLM response |
| Workflow mode | `cmd/m31a/main.go:runHeadlessWorkflow()` | Full 7-phase workflow without TUI |

## State Management

- **TUI State**: `internal/ui/tui/app.go` — `AppState` struct, updated only via `Update()`
- **Session State**: `internal/engine/session/` — persisted to disk, resumable
- **Workflow State**: `internal/engine/workflow/engine.go` — phase state machine
- **Permission State**: `internal/tools/dispatcher.go` — in-memory + persistent rules
