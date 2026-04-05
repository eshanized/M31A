# ARCHITECTURE.md — System Architecture

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent

## Architectural Pattern

M31A follows a **layered architecture with dependency injection**, built on the **Elm Architecture** (Model-View-Update) via Bubble Tea:

```
┌───────────────────────────────────────────────────┐
│                    TUI Layer                       │
│   internal/tui/ — 50+ files (REPL, models, views) │
│   Bubble Tea Msg -> Update() -> Model -> View()    │
├───────────────────────────────────────────────────┤
│                  Workflow Layer                    │
│   internal/workflow/ — Discuss→Plan→Execute→Verify │
│   →Ship (6-phase GSD workflow engine)             │
├───────────────────────────────────────────────────┤
│                  Tool Layer                        │
│   internal/tools/ — Dispatcher, 20+ tools, perms   │
│   internal/tools/subagent/ — Parallel subagents    │
├───────────────────────────────────────────────────┤
│               Provider / AI Layer                  │
│   internal/provider/ — Registry, interfaces, SSE   │
│   openrouter/, zen/ — API client implementations   │
├───────────────────────────────────────────────────┤
│                  Domain Packages                   │
│   pkg/{arbitrage,autodream,bisect,keychain,       │
│        ledger,rollback,session,taskrunner}         │
├───────────────────────────────────────────────────┤
│               Foundation / Config                  │
│   internal/{config,errors,fileutil,git,log,types}  │
└───────────────────────────────────────────────────┘
```

## Data Flow

### Startup Flow (`cmd/m31a/main.go:41`)
1. CLI flags parsed → config loaded from `~/.m31a/config.toml`
2. Logger initialized → provider registry built
3. API keys resolved via keychain → providers registered
4. Session manager, git client, ledger, rollback, autodream created
5. Tool dispatcher initialized with permission config
6. Subagent manager created with git worktree support
7. Bubble Tea TUI program launched with `tea.NewProgram`

### Message Processing Flow (Chat → LLM → Response)
```
User types message
    ↓
ReplModel receives tea.Msg
    ↓
app_update.go: HandleSubmit -> ExecuteWorkflow (async Cmd)
    ↓
workflow/engine.go: Engine processes through phases
    ↓
provider ChatCompletionStream -> SSE streaming
    ↓
Stream events parsed -> segments (text, thinking, tool_calls)
    ↓
Tool dispatcher routes tool calls -> results collected
    ↓
Streaming results sent back to LLM for next iteration
    ↓
Complete -> save to session
```

### Tool Execution Flow
```
LLM responds with tool_calls
    ↓
ReplModel parses tool calls -> dispatcher.Dispatch(ctx, toolCall)
    ↓
PermissionGate checks rules (allow/deny/ask)
    ↓
Tool handler executes (e.g., Bash, Read, Write, Edit)
    ↓
Result formatted -> sent back as tool_result message
    ↓
LLM continues with result context
```

### Subagent Flow
```
Agent tool invoked
    ↓
subagent/manager.go: CreateSubagent()
    ↓
git worktree add (isolated working directory)
    ↓
New dispatcher created (child = true, no background agents)
    ↓
Subagent runs independently with own Bubble Tea program
    ↓
On completion: worktree cleaned up, results merged
```

## Key Design Decisions

### GSD Workflow Engine (`internal/workflow/engine.go`)
Six-phase state machine: **Idle → Initialize → Discuss → Plan → Execute → Verify → Ship**
- Phase transitions validated via `internal/workflow/engine_messages.go`
- Each phase has dedicated handler files (`discuss.go`, `plan.go`, `execute.go`, `verify.go`, `ship.go`)
- Embedded prompt templates (`workflow/prompts/*.md`) loaded via `//go:embed`
- Self-healing on tool failures (max 2 attempts)
- Configurable per-phase models via `[agents]` config

### Provider Abstraction (`internal/provider/interface.go`)
```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx) ([]ModelInfo, error)
    CachedModels() []ModelInfo
    ChatCompletionStream(ctx, req) (*StreamIterator, error)
    EstimateCost(modelID, usage) float64
    HealthCheck(ctx) HealthStatus
    GetModel(id) (*ModelInfo, error)
}
```
- Two implementations: `openrouter.Client` and `zen.Client`
- Base client (`internal/provider/base_client.go`) handles HTTP, caching, SSE parsing
- Registry (`internal/provider/registry.go`) manages active provider, auto-fallback, rollback

### TUI Architecture (`internal/tui/`)
- **App model** (`app.go`) — top-level Bubble Tea model coordinating sub-models
- **REPL model** (`repl_model.go`) — main chat interface with streaming, thinking blocks, tool cards
- **Command system** (`commands.go` + specialized command files) — slash commands with autocomplete
- **Component library** (`internal/tui/components/`) — 30+ reusable TUI components
- **Theme engine** (`internal/tui/theme/`) — dark/light/auto themes, color registration, borders, shadows
- **Layout system** (`internal/tui/layout/`) — responsive layout, min screen detection, page management
- **Agent loop** (`agent_loop.go`) — the LLM interaction loop within the TUI
- **Key bindings** (`keybindings.go`) — configurable keyboard shortcuts with leader key

### TUI Models (screens)
| Model | File | Purpose |
|---|---|---|
| ReplModel | `repl_model.go` | Main chat/REPL interface |
| DashboardModel | `dashboard_model.go` | Dashboard view |
| ConfigModel | `config_model.go` | Configuration editor |
| SettingsModel | `settings_model.go` | Settings screen |
| PlanModel | `plan_model.go` | Workflow plan display |
| ExecuteModel | `execute_model.go` | Execution view |
| VerifyModel | `verify.go` | Verification view |
| ShipModel | `ship_model.go` | Ship/release view |
| DiscussModel | `discuss.go` | Discussion/survey view |
| DiffModel | `diff_model.go` | Git diff viewer |
| ResumeModel | `resume_model.go` | Session resume picker |
| SessionDetailModel | `sessiondetail_model.go` | Session details |
| FileExplorerModel | `fileexplorer_model.go` | File explorer |
| FirstRunModel | `firstrun_model.go` | First-run onboarding |
| ModelSelector | `modelselector.go` | Model picker |
| PhaseModelPicker | `phasemodelpicker.go` | Phase model config |
| BisectModel | `bisect_model.go` | Bisect UI |
| RollbackView | `rollback.go` | Rollback UI |
| CacheRefresh | `cache_refresh.go` | Cache status |
| Toast | `toast.go` | Toast notifications |
| NotificationModel | `notification_model.go` | Notification center |
| SubAgentsModel | `subagents_model.go` | Subagent monitor |
| SettingsEdit | `settings_edit.go` | Inline settings editor |

### Tool System Architecture (`internal/tools/`)
- **Dispatcher** — central registry and executor for all tools
- **Permission gate** — rules-based approval system with risk levels (safe/medium/dangerous/destructive)
- **Per-agent profiles** — different permission defaults per agent type
- **File operations** — atomic writes with backup management
- **SSRF protection** — blocks private IPs in WebFetch
- **Bash security** — timeout enforcement, output limits, kill grace period

### Types & Constants (`internal/types/`)
Central type definitions used across all packages:
- `Message`, `ModelInfo`, `ToolCall`, `Usage`, `Segment`
- `WorkflowPhase` enum (idle→initialize→discuss→plan→execute→verify→ship)
- `TaskStatus` enum (pending/running/done/failed/skipped/unrecoverable)
- `RiskLevel` enum (safe/medium/dangerous/destructive)
- Shared constants for timeouts, limits, defaults

### Subagent Manager (`internal/tools/subagent/`)
- Manager (`manager.go`) — creates and manages parallel subagents
- Loop (`loop.go`, `loop_parse.go`) — subagent interaction loop with tool call parsing
- Worktree (`worktree.go`) — git worktree management for isolated workspaces
- Events (`events.go`) — event aggregation from parallel subagents
