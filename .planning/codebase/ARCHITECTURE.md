<!-- refreshed: 2026-07-23 -->
# Architecture

**Analysis Date:** 2026-07-23

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                        TUI Layer (Bubble Tea)                │
├──────────────────┬──────────────────┬───────────────────────┤
│   App (Model)    │   Components     │   Screens              │
│  `internal/ui/   │  `internal/ui/   │  `internal/ui/         │
│   tui/app.go`    │   tui/components/│   tui/screens/         │
└────────┬─────────┴────────┬─────────┴───────────┬────────────┘
         │                  │                     │
         ▼                  ▼                     ▼
┌─────────────────────────────────────────────────────────────┐
│                    Workflow Engine                           │
│         `internal/engine/workflow/engine.go`                │
│  Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship
└──────────────────┬──────────────────┬───────────────────────┘
                   │                  │
         ┌─────────▼─────┐    ┌────────▼────────┐
         │  Tools Layer  │    │ Provider Layer  │
         │ `internal/    │    │ `internal/       │
         │  tools/`      │    │  integrations/   │
         │  dispatcher.go│    │  provider/`      │
         └───────────────┘    └─────────────────┘
                   │                  │
                   ▼                  ▼
┌─────────────────────────────────────────────────────────────┐
│                    Core Infrastructure                       │
├─────────────────────┬─────────────────────┬──────────────────┤
│  Session Manager    │  Config System      │  Type Vocabulary │
│  `internal/engine/  │  `internal/core/    │  `internal/       │
│  session/`          │  config/`           │  core/types/`     │
└─────────────────────┴─────────────────────┴──────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point / TUI App** | Flag parsing, config load, provider registration, TUI construction, signal handling | `cmd/m31a/main.go` |
| **TUI Model (App)** | Bubble Tea Model; holds all UI state, routes messages to handlers | `internal/ui/tui/app.go` |
| **TUI Update** | Single state mutation point (Elm architecture); handles all `tea.Msg` | `internal/ui/tui/app_update.go` |
| **Workflow Engine** | Orchestrates 7-phase workflow; phase dispatch, LLM streaming, checkpointing | `internal/engine/workflow/engine.go` |
| **State Machine** | Validates phase transitions; maintains history; prevents Plan↔Discuss oscillation | `internal/engine/workflow/state_machine.go` |
| **Phase Coordinator** | Pre-phase setup, post-phase metrics, transition side effects | `internal/engine/workflow/phase_coordinator.go` |
| **Provider Registry** | Manages 3 LLM providers (OpenRouter, Zen, Nvidia); dynamic model discovery | `internal/integrations/provider/registry.go` |
| **Tools Dispatcher** | Registers 18+ tools; permission gating, rate limiting, concurrency control | `internal/tools/dispatcher.go` |
| **Session Manager** | Project-local session persistence (`.m31a/session.json`, `messages.json`) | `internal/engine/session/manager.go` |
| **Config Loader** | TOML config from `~/.m31a/config.toml`; env var overrides; dotenv support | `internal/core/config/loader.go` |
| **Core Types** | Shared vocabulary: WorkflowPhase, Message, Tool, ModelInfo, RiskLevel, etc. | `internal/core/types/types.go` |

## Pattern Overview

**Overall:** Clean Architecture with strict layer separation — `internal/` packages may import `pkg/`, but `pkg/` must NOT import `internal/`. Enforced by Go module structure.

**Key Characteristics:**
- **Elm Architecture (Bubble Tea):** Single state object (`App` in `app.go`), all mutations via `Update(msg tea.Msg) (tea.Model, tea.Cmd)`. Never mutate state from goroutines — use `tea.Cmd` / `tea.Msg`.
- **Seven-Phase Workflow:** Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship. State machine validates transitions (`state_machine.go`).
- **Provider Abstraction:** `provider.LLMProvider` interface; models discovered dynamically via `FetchModels()`; never hardcode model names.
- **Tool Dispatcher:** Central registry with permission rules, token-bucket rate limiting (per-risk-level), and concurrency semaphore.
- **Session Persistence:** Project-local (`.m31a/`) + global config (`~/.m31a/`). Checkpoint/resume via workflow engine.
- **Configuration:** TOML with env var overrides; feature flags control phase behavior (Fast/Direct/Full modes).

## Layers

**TUI Layer:**
- **Purpose:** Interactive terminal UI, user input handling, screen rendering
- **Location:** `internal/ui/tui/`
- **Contains:** App model, screens (home, discuss, plan, execute, verify, ship, settings), components (sidebar, REPL, command palette), handlers
- **Depends on:** Workflow Engine, Provider Registry, Tools Dispatcher, Session Manager, Config
- **Used by:** Entry point (`main.go`)

**Workflow Engine Layer:**
- **Purpose:** Orchestrates multi-phase AI-assisted development workflow
- **Location:** `internal/engine/workflow/`
- **Contains:** Engine, StateMachine, PhaseCoordinator, ContextBuilder, PromptBuilder, CostTracker, Compactor, CodeIntel indexer
- **Depends on:** Provider (LLMProvider), Tools (Dispatcher), Session Manager, Git, Ledger, Config, Types
- **Used by:** TUI (via App), Headless mode (`main.go`)

**Provider Layer:**
- **Purpose:** Abstract LLM provider communication; dynamic model catalog
- **Location:** `internal/integrations/provider/`
- **Contains:** Registry, OpenRouter/Zen/Nvidia clients, capabilities detection, model metadata, health checks
- **Depends on:** Core Types (ChatRequest, ModelInfo, StreamIterator)
- **Used by:** Workflow Engine, TUI (model selector)

**Tools Layer:**
- **Purpose:** Sandboxed tool execution with permissions, rate limits, output bounding
- **Location:** `internal/tools/`
- **Contains:** Dispatcher, 18+ tool implementations (fileops, exec, search, ai, codeanalysis, git, todo, network), subagent manager
- **Depends on:** Core Types (Tool, ToolInput, ToolResult), Config (PermissionsConfig, ToolsConfig)
- **Used by:** Workflow Engine (Execute phase), TUI (Agent tool), Subagents

**Core Infrastructure Layer:**
- **Purpose:** Shared primitives, configuration, session management, error types
- **Location:** `internal/core/`, `internal/engine/session/`, `internal/engine/tokens/`, `internal/engine/taskrunner/`, `internal/engine/bisect/`, `internal/engine/rollback/`, `internal/engine/compaction/`, `internal/engine/narrative/`, `internal/engine/decision/`, `internal/engine/coordinator/`
- **Contains:** Types vocabulary, TOML config loader, session persistence, token estimator, task runner, git bisect, rollback, compaction, narrative engine, decision logger, run coordinator
- **Depends on:** Minimal (stdlib, `github.com/BurntSushi/toml`)
- **Used by:** All upper layers

**Integrations Layer:**
- **Purpose:** External system adapters
- **Location:** `internal/integrations/`
- **Contains:** Git client, Ledger (session log), Keychain (OS credential store), CodeIntel (codebase indexer), Context sources, Metrics collector, Logging, Autodream, Arbitrage, Shell, Skills
- **Depends on:** Core Types, Config
- **Used by:** Workflow Engine, TUI

**Package Layer (`pkg/`):**
- **Purpose:** Public reusable packages with zero internal dependencies
- **Location:** `pkg/errors/`
- **Contains:** Error wrapping utilities
- **Constraint:** Must NOT import `internal/...` (enforced by module structure)

## Data Flow

### Primary Request Path (TUI → Workflow → LLM → Tools → Response)

```text
1. User input in REPL (TUI)                                    `internal/ui/tui/repl.go`
   │
   ▼
2. App.Update() routes to workflow handler                      `internal/ui/tui/app_update_workflow.go`
   │
   ▼
3. Engine.RunPhase(ctx, phase, goal)                            `internal/engine/workflow/engine.go`
   │
   ▼
4. PhaseCoordinator.PrePhase() — checkpoint, metrics           `internal/engine/workflow/phase_coordinator.go`
   │
   ▼
5. ContextBuilder.Build() → assembles system prompt + context  `internal/engine/workflow/context_builder.go`
   │
   ▼
6. Provider.ChatCompletionStream() → StreamIterator            `internal/integrations/provider/*.go`
   │
   ▼
7. Stream chunks → TUI via StreamChunkMsg → REPL renders       `internal/ui/tui/repl_stream.go`
   │
   ▼
8. Tool calls parsed from LLM response → Dispatcher.Execute()  `internal/tools/dispatcher.go`
   │
   ▼
9. Tool.Execute() → ToolResult → back to LLM                   `internal/tools/*/*.go`
   │
   ▼
10. Phase completes → PhaseCoordinator.PostPhase() → Transition `internal/engine/workflow/phase_coordinator.go`
```

### Headless Workflow Path (`--goal` flag)

```text
1. main.go → runHeadlessWorkflow()                              `cmd/m31a/main.go`
   │
   ▼
2. Creates Engine with all dependencies                         `internal/engine/workflow/engine.go`
   │
   ▼
3. Iterates phases sequentially (Initialize → Ship)            `internal/engine/workflow/engine.go`
   │
   ▼
4. Each phase: RunPhase() → Transition()                       `internal/engine/workflow/engine.go`
```

### Session Resume Path

```text
1. main.go → cfg.Features.ResumeOnStartup → sessionMgr.ListSessions()
   │
   ▼
2. App.SetResumeSessionID(sessionID)                           `internal/ui/tui/app.go`
   │
   ▼
3. On TUI start → Session.Load() → restores Messages, Plan     `internal/engine/session/session.go`
   │
   ▼
4. Engine restores from checkpoint if available                `internal/engine/workflow/engine.go`
```

## Key Abstractions

**WorkflowPhase (`internal/core/types/types.go`):**
- Enum: `PhaseIdle`, `PhaseInitialize`, `PhaseDiscuss`, `PhasePlan`, `PhaseExecute`, `PhaseVerify`, `PhaseRuntime`, `PhaseShip`
- Drives StateMachine transitions and TUI screen routing

**LLMProvider (`internal/integrations/provider/interface.go`):**
```go
interface {
  Name() string
  APIKey() string
  FetchModels(ctx) ([]ModelInfo, error)
  CachedModels() []ModelInfo
  ChatCompletionStream(ctx, ChatRequest) (*StreamIterator, error)
  EstimateCost(modelID, Usage) float64
  HealthCheck(ctx) HealthStatus
  GetModel(id) (*ModelInfo, error)
}
```
- Three implementations: `openrouter.Client`, `zen.Client`, `nvidia.Client`
- Models discovered at runtime — never hardcode model IDs

**Tool (`internal/core/types/types.go`):**
```go
interface {
  Name() string
  Description() string
  RiskLevel() RiskLevel
  Execute(ctx, ToolInput) (ToolResult, error)
}
```
- Optional `SchemaProvider` for JSON Schema parameter definitions
- Registered in `tools.DefaultDispatcher()` (`internal/tools/defaults.go`)

**Session (`internal/engine/session/session.go`):**
- Project-local persistence in `<workDir>/.m31a/`
- Files: `session.json` (metadata), `messages.json` (conversation), `checkpoint.json` (workflow state)

**Config (`internal/core/config/types.go`):**
- Single `Config` struct with nested configs for Provider, Model, UI, Permissions, Features, Tools, Agents, Git, Verify, Compaction, etc.
- Loaded from TOML + env vars (`config.Load()` in `internal/core/config/loader.go`)

## Entry Points

**Primary (TUI):**
- `cmd/m31a/main.go` → `run()` → `tea.NewProgram(app).Run()`
- Flags: `--version`, `--help`, `--prompt` (headless single prompt), `--goal` (headless full workflow), `--model`

**Headless Single Prompt:**
- `runHeadless()` → `provider.ChatCompletionStream()` → print response

**Headless Full Workflow:**
- `runHeadlessWorkflow()` → creates Engine → iterates all 7 phases sequentially

## Architectural Constraints

- **Threading:** Bubble Tea is strictly single-threaded. All state mutations MUST go through `Update()`. Goroutines communicate via `tea.Cmd` / channels, never shared mutable state. Signal handler sends `tea.QuitMsg` via `Program.Send()`.
- **Global State:** Module-level singletons in `internal/engine/workflow/engine.go` (`var globalConfigDir`), `internal/tools/constants.go` (constants only). Session manager uses file lock (`.m31a/session.lock`) for cross-process safety.
- **Circular Imports:** None detected. Layer separation enforced: `pkg/` → nothing; `internal/core/` → `pkg/`; `internal/engine/` → `internal/core/`, `pkg/`; `internal/integrations/` → `internal/core/`, `pkg/`; `internal/tools/` → `internal/core/`, `pkg/`, `internal/integrations/`; `internal/ui/` → all internal layers.
- **CGO:** Hard constraint `CGO_ENABLED=0` — static binary. No C dependencies anywhere.
- **Provider Models:** Dynamic discovery only. Never hardcode model names. Capabilities detected via `FetchModels()` and `GetModel()`.

## Anti-Patterns

### Mutating TUI State from Goroutine

**What happens:** A goroutine directly modifies `App` fields (e.g., `app.messages = ...`).
**Why it's wrong:** Bubble Tea's `Update()` is the single mutation point. Concurrent writes race with the event loop, corrupting state and causing panics or visual glitches.
**Do this instead:** Send `tea.Msg` via `Program.Send()`; handle in `Update()`.

```go
// WRONG
go func() { app.Messages = append(app.Messages, msg) }()

// CORRECT
p.Send(StreamChunkMsg{Chunk: chunk, Source: "discuss"})
```

### Hardcoding Model IDs

**What happens:** Code references `"gpt-4o"` or `"claude-3.5-sonnet"` directly.
**Why it's wrong:** Providers rotate models; users configure custom endpoints. Model list comes from `FetchModels()` at runtime.
**Do this instead:** Use `provider.Registry.ActiveProvider().FetchModels(ctx)` and let user select via ModelSelector screen.

### Direct Provider Calls from TUI

**What happens:** TUI handler calls `provider.ChatCompletionStream()` directly.
**Why it's wrong:** Bypasses workflow engine's context building, checkpointing, cost tracking, compaction, and phase coordination.
**Do this instead:** Route through `Engine.RunPhase()` or `Engine.RunHeadlessPhase()`.

## Error Handling

**Strategy:** Errors as values. Never panic. Wrap with `fmt.Errorf("%w", err)` using `internal/core/errors/errors.go` sentinel errors (`ErrPhaseTransition`, `ErrSessionNotFound`, `ErrPermissionDenied`, etc.).

**Patterns:**
- Provider errors: wrapped with provider name, sanitized for display (`MaxProviderErrorChars = 200`)
- Tool errors: `ToolError{Err, Hint}` — hint guides LLM self-recovery
- Workflow errors: `PhaseResult{Success, Error, Output}` — engine decides retry/transition
- Config errors: fail fast at startup with descriptive messages

## Cross-Cutting Concerns

**Logging:** `log/slog` with structured JSON output to `~/.m31a/logs/`. Initialized early in `main.go` via `integrations/log.NewLogger()`. TUI also has in-app log viewer.

**Validation:** Config validated on load (`config.Validate()`). Permissions rules validated in `Dispatcher` construction (fail fast). Tool input validated by each tool's `Execute()`.

**Authentication:** API keys stored in OS keychain (`internal/integrations/keychain/`). Config holds only keychain references. `.env` files gitignored (except `.env.example`).

**Compaction:** Automatic session compaction when context fills (`internal/engine/compaction/`). Triggered proactively during Execute phase based on tool call count.

**Metrics:** Optional metrics collection (`FeaturesConfig.MetricsEnabled`) → `METRICS.json` in session dir (`internal/integrations/metrics/`).

---

*Architecture analysis: 2026-07-23*