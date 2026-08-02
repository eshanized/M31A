<!-- refreshed: 2026-08-03 -->
# Architecture

**Analysis Date:** 2026-08-03

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                    CLI Entry Point                           │
│                  cmd/m31a/main.go                            │
│         (flag parsing, config load, provider init)           │
└────────┬────────────────────────────────────────┬────────────┘
         │                                        │
         ▼                                        ▼
┌────────────────────────┐          ┌──────────────────────────┐
│   TUI (Bubble Tea)     │          │   Headless Mode           │
│   internal/ui/tui/     │          │   --prompt / --goal       │
│   Elm architecture     │          │   No TUI, direct output   │
└────────┬───────────────┘          └────────┬─────────────────┘
         │                                   │
         ▼                                   ▼
┌─────────────────────────────────────────────────────────────┐
│              Workflow Engine (7 phases)                      │
│         internal/engine/workflow/engine.go                   │
│   Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship  │
└────────┬───────────────────┬────────────────────┬───────────┘
         │                   │                    │
         ▼                   ▼                    ▼
┌────────────────┐  ┌────────────────┐  ┌─────────────────────┐
│  Tools Layer   │  │ Provider Layer │  │ Session Manager      │
│  18 built-in   │  │ OpenRouter     │  │ internal/engine/     │
│  Dispatcher    │  │ Zen            │  │   session/           │
│  Permissions   │  │ NVIDIA         │  │ Persistence + CRUD   │
│  Rate limiting │  │ Registry       │  └─────────────────────┘
└────────┬───────┘  └────────┬───────┘
         │                   │
         ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   Integrations Layer                         │
│  git/ keychain/ ledger/ log/ metrics/ provider/ codeintel/   │
│  context/ autodream/ arbitrage/ shell/ skills/               │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                   Core Types & Config                        │
│  internal/core/types/  internal/core/config/                 │
│  internal/core/errors/ internal/infrastructure/              │
└─────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| CLI Entry | Flag parsing, config load, provider registration, TUI launch | `cmd/m31a/main.go` |
| AppState | Top-level Bubble Tea model, all state mutations via Update() | `internal/ui/tui/app_state.go` |
| REPL | Chat interface, message history, streaming display | `internal/ui/tui/repl.go` |
| Workflow Engine | Seven-phase orchestration, LLM streaming, tool dispatch | `internal/engine/workflow/engine.go` |
| State Machine | Phase transition validation and history tracking | `internal/engine/workflow/state_machine.go` |
| Dispatcher | Tool registration, permission enforcement, rate limiting, concurrency control | `internal/tools/dispatcher.go` |
| Provider Registry | LLM provider management, active provider selection | `internal/integrations/provider/registry.go` |
| Session Manager | Session persistence, checkpoint save/restore, task storage | `internal/engine/session/manager.go` |
| Config | TOML config loading, validation, merging, hot-reload | `internal/core/config/loader.go` |
| Types | Shared vocabulary: Message, ToolCall, WorkflowPhase, Task, etc. | `internal/core/types/types.go` |
| Errors | Sentinel errors, structured error types, user-friendly messages | `internal/core/errors/errors.go` |

## Pattern Overview

**Overall:** Elm Architecture (Model-Update-View) via Bubble Tea

**Key Characteristics:**
- Single-threaded state mutation: all state changes flow through `Update()` only
- Goroutines communicate via `tea.Cmd` channels, never shared mutable state
- Seven-phase workflow with validated state machine transitions
- Permission-gated tool execution with risk-based rate limiting
- Provider-agnostic LLM integration via `LLMProvider` interface

## Layers

**Core (`internal/core/`):**
- Purpose: Shared types, configuration, error definitions, infrastructure utilities
- Location: `internal/core/`
- Contains: `types/` (shared vocabulary), `config/` (TOML loading/validation), `errors/` (sentinels and structured errors)
- Depends on: Nothing (leaf layer)
- Used by: All other layers

**Engine (`internal/engine/`):**
- Purpose: Business logic — workflow phases, session management, token estimation, rollback, bisect, task running
- Location: `internal/engine/`
- Contains: `workflow/` (7-phase engine), `session/` (persistence), `tokens/` (estimation), `bisect/`, `rollback/`, `taskrunner/`, `compaction/`, `decision/`, `narrative/`
- Depends on: `core/`, `integrations/`, `tools/`
- Used by: `ui/tui/`, `cmd/m31a/`

**Tools (`internal/tools/`):**
- Purpose: 18 built-in tools exposed to the LLM, with permission enforcement and rate limiting
- Location: `internal/tools/`
- Contains: `dispatcher.go` (orchestration), `fileops/`, `exec/`, `search/`, `ai/`, `git/`, `network/`, `codeanalysis/`, `todo/`, `subagent/`
- Depends on: `core/`, `integrations/metrics/`
- Used by: `engine/workflow/`

**Integrations (`internal/integrations/`):**
- Purpose: External service adapters — LLM providers, git, keychain, ledger, logging, metrics
- Location: `internal/integrations/`
- Contains: `provider/` (3 LLM providers), `git/`, `keychain/`, `ledger/`, `log/`, `metrics/`, `codeintel/`, `context/`, `autodream/`, `arbitrage/`, `shell/`, `skills/`
- Depends on: `core/`
- Used by: `engine/`, `tools/`, `ui/tui/`

**UI (`internal/ui/tui/`):**
- Purpose: Terminal UI — Bubble Tea models, views, handlers, streaming display, themes
- Location: `internal/ui/tui/`
- Contains: `app_state.go` (root model), `repl.go` (chat), `sidebar_model.go`, `commands/`, `components/`, `theme/`, `streaming/`, `layout/`, `tuitypes/`
- Depends on: `core/`, `engine/`, `integrations/`, `tools/`
- Used by: `cmd/m31a/`

**Infrastructure (`internal/infrastructure/`):**
- Purpose: Cross-cutting utilities — file operations, retry logic
- Location: `internal/infrastructure/`
- Contains: `fileutil/`, `retry/`
- Depends on: Nothing
- Used by: `engine/`, `tools/`

## Data Flow

### Primary Request Path (Interactive TUI)

1. User types input in REPL (`internal/ui/tui/repl.go`)
2. `AppState.Update()` routes the message based on screen state (`internal/ui/tui/app_update.go`)
3. Intent classification via LLM (`internal/engine/workflow/intent.go`)
4. Workflow engine runs the appropriate phase (`internal/engine/workflow/engine.go:RunPhase`)
5. Phase calls LLM via provider interface (`internal/integrations/provider/interface.go`)
6. LLM response streams back through `StreamIterator` (`internal/core/types/types.go`)
7. Tool calls dispatched via `Dispatcher.Execute()` (`internal/tools/dispatcher.go`)
8. Tool results return to LLM in next conversation turn
9. Phase completes, emits `PhaseResultMsg` back to TUI via `MsgEmitter`

### Headless Mode Path

1. `cmd/m31a/main.go` parses `--prompt` or `--goal` flag
2. `runHeadless()` sends single prompt to LLM, prints response (`cmd/m31a/main.go:183`)
3. `runHeadlessWorkflow()` runs all 7 phases sequentially (`cmd/m31a/main.go:57`)
4. Each phase runs via `engine.RunPhase()` with timeout
5. Results printed to stderr, exit code returned

### Tool Execution Flow

1. LLM generates tool call in response
2. `consumeStreamWithTools()` accumulates streamed tool_call chunks (`internal/engine/workflow/engine.go:1524`)
3. `Dispatcher.Execute()` receives `ToolCall` (`internal/tools/dispatcher.go:231`)
4. Concurrency semaphore acquired (`MaxConcurrentTools`)
5. Rate limiter token acquired (per-risk bucket)
6. Permission check via `ensurePermission()` (`internal/tools/dispatcher.go:472`)
7. Tool's `Execute()` method called (`internal/tools/interface.go`)
8. Output bounded by `OutputStore` if configured
9. Result returned to workflow engine

**State Management:**
- Bubble Tea `AppState` owns all TUI state — single-threaded via `Update()` (`internal/ui/tui/app_state.go`)
- Workflow engine state in `WorkflowState` struct with `sync.RWMutex` guards (`internal/engine/workflow/engine.go:45`)
- Session state persisted to disk via `session.Manager` (`internal/engine/session/manager.go`)
- Phase transitions validated by `StateMachine` (`internal/engine/workflow/state_machine.go`)
- Goroutines communicate via channels: `emitterCh`, `requestCh`, `responseCh`, `questionReqCh`

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Abstracts LLM API access behind a common interface
- Examples: `internal/integrations/provider/interface.go`, `internal/integrations/provider/openrouter/`, `internal/integrations/provider/zen/`, `internal/integrations/provider/nvidia/`
- Pattern: Interface with `ChatCompletionStream()`, `FetchModels()`, `HealthCheck()`

**Tool Interface:**
- Purpose: Abstracts tool execution behind a common interface
- Examples: `internal/core/types/types.go:Tool`, `internal/tools/fileops/`, `internal/tools/exec/`
- Pattern: Interface with `Name()`, `Execute()`, `RiskLevel()`

**MsgEmitter Interface:**
- Purpose: Bridges workflow goroutines to Bubble Tea update loop
- Examples: `internal/ui/tui/narrative_emitter.go`, `internal/engine/workflow/engine_messages.go`
- Pattern: Channel-based message passing with drop counting

**StateMachine:**
- Purpose: Validates phase transitions, prevents illegal state changes
- Examples: `internal/engine/workflow/state_machine.go`
- Pattern: Validated transition map with history tracking

**Dispatcher:**
- Purpose: Central tool orchestration — registration, permissions, rate limiting, concurrency
- Examples: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`
- Pattern: Registry + middleware chain (permission → rate limit → execute)

## Entry Points

**TUI Mode:**
- Location: `cmd/m31a/main.go:243` (`run()`)
- Triggers: `m31a` (no flags)
- Responsibilities: Config load, provider init, session creation, TUI launch via `tea.NewProgram().Run()`

**Headless Prompt Mode:**
- Location: `cmd/m31a/main.go:183` (`runHeadless()`)
- Triggers: `m31a --prompt "..."` 
- Responsibilities: Single LLM call, print response, exit

**Headless Workflow Mode:**
- Location: `cmd/m31a/main.go:57` (`runHeadlessWorkflow()`)
- Triggers: `m31a --goal "..."`
- Responsibilities: Run all 7 phases sequentially without TUI

**Tool Registration:**
- Location: `internal/tools/defaults.go:19` (`DefaultDispatcher()`)
- Triggers: Called during engine initialization
- Responsibilities: Register all 18 built-in tools with permissions and rate limits

## Architectural Constraints

- **Threading:** Single-threaded Bubble Tea event loop. All goroutines must communicate via channels (`tea.Cmd`, `tea.Msg`). Never mutate `AppState` from a goroutine.
- **Global state:** `slog.SetDefault(logger)` sets global logger at startup (`cmd/m31a/main.go:285`). `permissionRequestID` is a global atomic counter (`internal/tools/interface.go:12`).
- **Circular imports:** None — enforced by Go module system. `internal/core/` is the leaf; `pkg/` must not import `internal/`.
- **CGO_ENABLED=0:** Hard constraint — all dependencies must work without CGO. Binary must be statically linked.

## Anti-Patterns

### Goroutine State Mutation

**What happens:** Goroutines directly mutating `AppState` fields
**Why it's wrong:** Violates Bubble Tea's single-threaded contract, causes data races
**Do this instead:** Use `tea.Cmd` closures that return `tea.Msg`, processed by `Update()` — see `internal/ui/tui/app.go:352`

### Hardcoded Model Names

**What happens:** Referencing specific model IDs in code
**Why it's wrong:** Model lists are dynamic per provider; hardcoded names break when models change
**Do this instead:** Use `provider.FetchModels()` and `provider.GetModel()` — see `internal/integrations/provider/interface.go:17`

### Direct Anthropic/OpenAI Usage

**What happens:** Importing Anthropic or OpenAI SDKs directly
**Why it's wrong:** M31A routes through provider layer (OpenRouter, Zen, NVIDIA) — direct API calls bypass rate limiting, cost tracking, and fallback
**Do this instead:** Use `provider.LLMProvider` interface — see `internal/integrations/provider/interface.go:15`

### Panic Instead of Error Return

**What happens:** Using `panic()` for error handling
**Why it's wrong:** Panics crash the process without cleanup; deferred functions may not run
**Do this instead:** Return `error` values, wrap with `fmt.Errorf("%w", err)` — see `internal/core/errors/errors.go`

## Error Handling

**Strategy:** Structured errors with sentinel values and user-friendly messages

**Patterns:**
- Sentinel errors in `internal/core/errors/errors.go` (e.g., `ErrProviderUnreachable`, `ErrToolExecution`)
- Structured error types: `ToolError`, `ProviderError`, `ConfigError` — all implement `Unwrap()`
- User-facing messages via `errors.UserMessage()` which maps sentinels to actionable strings
- Tool errors return `ToolError` with `Hint` for LLM self-recovery — see `internal/core/types/types.go:207`
- Phase-level errors propagated via `PhaseResult.Error` string field

## Cross-Cutting Concerns

**Logging:** `log/slog` with structured attributes. Logger initialized in `cmd/m31a/main.go:278` via `internal/integrations/log/log.go`. Log file at `~/.m31a/logs/m31a.log`.

**Validation:** Config validation in `internal/core/config/config_validate.go`. Phase transition validation in `internal/engine/workflow/state_machine.go`. Permission validation in `internal/tools/permissions.go`.

**Authentication:** API keys resolved from OS keychain (`internal/integrations/keychain/`) with file fallback. Keys never written to disk in plaintext. Keychain availability cached to avoid repeated D-Bus failures.

**Metrics:** Session-level metrics collector (`internal/integrations/metrics/`) tracking tool calls, LLM usage, phase durations. Enabled via `config.Features.MetricsEnabled`.

**Compaction:** Automatic context window management (`internal/engine/compaction/`). Proactive compaction at phase transitions. Falls back to progressive truncation when compaction fails.

---

*Architecture analysis: 2026-08-03*
