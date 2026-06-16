<!-- refreshed: 2026-06-16 -->
# Architecture

**Analysis Date:** 2026-06-16

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                          Entry Point                                │
│  `cmd/m31a/main.go` — CLI flags, config loading, provider wiring    │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         TUI Layer (Bubble Tea)                      │
│  `internal/tui/` — 29 screens, page layout, screen transitions      │
│                                                                     │
│  AppState (root) → Screen enum routing → Sub-model Update/View      │
│  MsgEmitter pattern decouples workflow engine from Bubble Tea       │
│  Sub-packages: components/ layout/ streaming/ commands/ tuitypes/   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Workflow Engine (6 phases)                      │
│  `internal/workflow/engine.go` — core orchestrator                  │
│                                                                     │
│  Initialize → Discuss → Plan → Execute → Verify → Ship              │
│                                                                     │
│  Prompt templates embedded via embed.FS (11 markdown files)         │
│  30+ message types for TUI communication                            │
│  Plan parser extracts tasks from LLM markdown output                │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
┌───────────────────┐ ┌────────────────┐ ┌────────────────────────────┐
│  Provider Layer   │ │  Tool Layer    │ │  Packages (domain logic)   │
│  OpenRouter, Zen  │ │  Bash, Read,   │ │  session, ledger, rollback │
│  Fallback, SSE    │ │  Write, Glob,  │ │  bisect, taskrunner,       │
│  Health checks    │ │  Grep, Web     │ │  keychain, autodream,      │
│                   │ │  Permissions   │ │  arbitrage, history        │
└────────┬──────────┘ └───────┬────────┘ └────────────┬───────────────┘
         │                    │                       │
         ▼                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Infrastructure Layer                            │
│  git/ config/ errors/ tokens/ codeintel/ fileutil/ log/             │
│  TOML config, slog logging, atomic I/O, token estimation            │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | CLI parsing, config loading, dependency wiring, signal handling | `cmd/m31a/main.go` |
| **AppState** | Top-level Bubble Tea model; all state mutations; screen routing | `internal/tui/app_state.go` |
| **Update Loop** | Single dispatch point for all tea.Msg; no goroutine state mutations | `internal/tui/app_update.go` |
| **View Renderer** | Composes header+content+footer via unified PageChrome layout | `internal/tui/app_view.go` |
| **REPL Model** | Chat viewport, message history, @-mentions, slash commands | `internal/tui/repl.go` |
| **Streaming** | Goroutine-owned channels for SSE chunk delivery to TUI | `internal/tui/streaming/streaming.go` |
| **Agent Loop** | Autonomous multi-iteration tool-calling loop with LLM | `internal/tui/streaming/agent_loop.go` |
| **Workflow Engine** | Six-phase orchestration, LLM streaming, plan parsing, tool dispatch | `internal/workflow/engine.go` |
| **Plan Parser** | Markdown → Task extraction with retry/refinement logic | `internal/workflow/plan_parser.go` |
| **LLMProvider Interface** | Abstraction for LLM API communication | `internal/provider/interface.go` |
| **BaseClient** | Shared HTTP transport, model cache, health checks, cost estimation | `internal/provider/base_client.go` |
| **Registry** | Thread-safe multi-provider management with active provider tracking | `internal/provider/registry.go` |<!-- refreshed: 2026-06-16 -->
# Architecture

**Analysis Date:** 2026-06-16

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                          Entry Point                                │
│  `cmd/m31a/main.go` — CLI flags, config loading, provider wiring    │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         TUI Layer (Bubble Tea)                      │
│  `internal/tui/` — 29 screens, page layout, screen transitions      │
│                                                                     │
│  AppState (root) → Screen enum routing → Sub-model Update/View      │
│  MsgEmitter pattern decouples workflow engine from Bubble Tea       │
│  Sub-packages: components/ layout/ streaming/ commands/ tuitypes/   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Workflow Engine (6 phases)                      │
│  `internal/workflow/engine.go` — core orchestrator                  │
│                                                                     │
│  Initialize → Discuss → Plan → Execute → Verify → Ship              │
│                                                                     │
│  Prompt templates embedded via embed.FS (11 markdown files)         │
│  30+ message types for TUI communication                            │
│  Plan parser extracts tasks from LLM markdown output                │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
┌───────────────────┐ ┌────────────────┐ ┌────────────────────────────┐
│  Provider Layer   │ │  Tool Layer    │ │  Packages (domain logic)   │
│  OpenRouter, Zen  │ │  Bash, Read,   │ │  session, ledger, rollback │
│  Fallback, SSE    │ │  Write, Glob,  │ │  bisect, taskrunner,       │
│  Health checks    │ │  Grep, Web     │ │  keychain, autodream,      │
│                   │ │  Permissions   │ │  arbitrage, history        │
└────────┬──────────┘ └───────┬────────┘ └────────────┬───────────────┘
         │                    │                       │
         ▼                    ▼                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Infrastructure Layer                            │
│  git/ config/ errors/ tokens/ codeintel/ fileutil/ log/             │
│  TOML config, slog logging, atomic I/O, token estimation            │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | CLI parsing, config loading, dependency wiring, signal handling | `cmd/m31a/main.go` |
| **AppState** | Top-level Bubble Tea model; all state mutations; screen routing | `internal/tui/app_state.go` |
| **Update Loop** | Single dispatch point for all `tea.Msg`; no goroutine state mutations | `internal/tui/app_update.go` |
| **View Renderer** | Composes header + content + footer via unified PageChrome layout | `internal/tui/app_view.go` |
| **REPL Model** | Chat viewport, message history, @-mentions, slash commands | `internal/tui/repl.go` |
| **Streaming** | Goroutine-owned channels for SSE chunk delivery to TUI | `internal/tui/streaming/streaming.go` |
| **Agent Loop** | Autonomous multi-iteration tool-calling loop with LLM | `internal/tui/streaming/agent_loop.go` |
| **Workflow Engine** | Six-phase orchestration, LLM streaming, plan parsing, tool dispatch | `internal/workflow/engine.go` |
| **Plan Parser** | Markdown → Task extraction with retry/refinement logic | `internal/workflow/plan_parser.go` |
| **LLMProvider Interface** | Abstraction for LLM API communication | `internal/provider/interface.go` |
| **BaseClient** | Shared HTTP transport, model cache, health checks, cost estimation | `internal/provider/base_client.go` |
| **Registry** | Thread-safe multi-provider management with active provider tracking | `internal/provider/registry.go` |
| **Fallback** | Parallel health checks, automatic failover on 429/503 | `internal/provider/fallback.go` |
| **SSE Parser** | Streaming response parsing with watchdog timeout | `internal/provider/sse.go` |
| **Dispatcher** | Tool registration, permission gating, rate limiting, execution | `internal/tools/dispatcher.go` |
| **Permissions** | Per-request approval channels with configurable timeout | `internal/tools/permissions.go` |
| **Session Manager** | Session CRUD, checkpoint/restore, planning file I/O | `pkg/session/manager.go` |
| **Task Runner** | Kahn's algorithm topological sort, bounded parallelism | `pkg/taskrunner/runner.go` |
| **Git Client** | Shell-based git operations, satisfies `types.GitClient` interface | `internal/git/git.go` |
| **Ledger** | Cross-session learning records (markdown tables) | `pkg/ledger/ledger.go` |
| **Rollback** | Git commit chain management, soft/hard/safe reset | `pkg/rollback/rollback.go` |
| **AutoDream** | Context window consolidation (message compression) | `pkg/autodream/autodream.go` |
| **Keychain** | OS-native secure API key storage (Linux/macOS/Windows) | `pkg/keychain/keychain.go` |
| **Arbitrage** | Model cost optimization, task complexity scoring | `pkg/arbitrage/arbitrage.go` |
| **CodeIntel** | Project indexing, import graph, relevance scoring | `internal/codeintel/codeintel.go` |
| **Tokens** | tiktoken-go + rune fallback, EMA calibration | `internal/tokens/estimator.go` |
| **Config** | TOML config loading, hot-reload, env expansion | `internal/config/loader.go` |
| **Errors** | Sentinel errors + `UserMessage()` mapping | `internal/errors/errors.go` |
| **Atomic I/O** | Crash-safe file writes (temp + rename) | `internal/fileutil/atomic.go` |
| **Logging** | Structured slog logging, daily rotation, 7-day retention | `internal/log/log.go` |

## Pattern Overview

**Overall:** Six-layer architecture with strict dependency direction (upper → lower). `pkg/` cannot import `internal/`.

**Key Characteristics:**
- **Bubble Tea Elm Architecture** — All TUI state lives in `AppState`. `Update()` is the single mutation point. `View()` is pure.
- **MsgEmitter decoupling** — Workflow engine emits events through an `any`-typed channel interface, avoiding direct dependency on Bubble Tea.
- **Screen enum routing** — `AppState.screen` (an `int` enum) determines which sub-model's `Update`/`View` runs. Sub-models are lazily created.
- **Permission-gated tool execution** — Every tool call flows through `Dispatcher.ensurePermission()` which may block on a channel waiting for user approval.
- **Embed.FS prompts** — 11 markdown prompt templates compiled into the binary via `//go:embed prompts/*.md`.
- **Interface-based testability** — `types.GitClient`, `types.Tool`, `provider.LLMProvider`, and `workflowEngineInterface` allow mock injection.
- **Compile-time interface checks** — `var _ Interface = (*Concrete)(nil)` at the top of implementation files.
- **Channel-based goroutine-to-TUI communication** — Goroutines (streaming, agent loop, config watcher) send `tea.Msg` through buffered channels; `Update()` receives them.

## Layers

**TUI Layer:**
- Purpose: User interaction, keyboard/mouse handling, screen rendering, toast notifications
- Location: `internal/tui/`
- Contains: 29 screen models, sub-packages for components, layout, streaming, commands, theme, tuitypes
- Depends on: provider, tools, workflow, pkg/*
- Used by: Only `cmd/m31a/main.go` creates and runs the Bubble Tea program

**Workflow Engine:**
- Purpose: Six-phase orchestration, LLM interaction, plan parsing, task scheduling
- Location: `internal/workflow/`
- Contains: Phase implementations (initialize, discuss, plan, execute, verify, ship), engine core, prompt registry, classify
- Depends on: provider, tools, tokens, codeintel, pkg/session, pkg/taskrunner, pkg/bisect
- Used by: TUI layer via `WorkflowEngine` interface

**Provider Layer:**
- Purpose: LLM API abstraction with fallback, health checks, model catalog caching
- Location: `internal/provider/`
- Contains: LLMProvider interface, BaseClient, Registry, Fallback, SSE parser, OpenRouter client, Zen client
- Depends on: types, errors
- Used by: TUI, workflow engine

**Tool Layer:**
- Purpose: File system/shell operations with permission control
- Location: `internal/tools/`
- Contains: Dispatcher, 14+ tool implementations, permission system, subagent management
- Depends on: types, errors, config
- Used by: Workflow engine, TUI (permission modal), agent loop

**Package Layer:**
- Purpose: Reusable domain logic with no `internal/` imports
- Location: `pkg/`
- Contains: session, taskrunner, ledger, rollback, bisect, autodream, arbitrage, keychain, history
- Depends on: Only `internal/types`, `internal/errors`, `internal/fileutil` (via the `internal/` boundary — note: `pkg/` currently imports `internal/errors` and `internal/fileutil`, which is an architectural boundary violation tracked as tech debt)
- Used by: Workflow engine, TUI

**Infrastructure Layer:**
- Purpose: Cross-cutting concerns shared across all layers
- Location: `internal/{config,errors,git,tokens,codeintel,fileutil,log,types}/`
- Contains: Config loading, sentinel errors, git wrapper, token estimation, code indexing, atomic I/O, logging, core types
- Depends on: Standard library + minimal third-party
- Used by: All other layers

## Data Flow

### Primary Workflow Path

1. User enters goal via GoalInput screen → `GoalSubmittedMsg` (`internal/tui/goalinput_model.go`)
2. PhaseModelPicker → user selects Planning/Coding models → `PhaseModelPickedMsg` (`internal/tui/phasemodelpicker.go`)
3. `AppState.RunPhaseCmd()` creates a goroutine calling `Engine.RunPhase()` (`internal/tui/app.go:258`)
4. Engine runs Initialize → project detection, git init, planning dir (`internal/workflow/initialize.go:14`)
5. Engine emits `PhaseResultMsg` via `MsgEmitter` → channel → `AppState.Update()` (`internal/tui/app_update.go`)
6. Execute: `taskrunner.Runner.Schedule()` topologically sorts tasks → dispatches tools (`internal/workflow/execute.go:20`)
7. Verify: file checks, syntax validation, test execution (`internal/workflow/verify.go:16`)
8. Ship: final commit, ledger entry, session archival (`internal/workflow/ship.go:31`)

### LLM Streaming Path

1. Engine builds prompt from embedded templates (`internal/workflow/engine.go:227`)
2. Engine calls `provider.ChatCompletionStream()` via registry (`internal/provider/interface.go:14`)
3. SSE parser yields `StreamChunk` objects (`internal/provider/sse.go`)
4. Engine forwards via `MsgEmitter.Emit()` as `StreamChunkMsg` (`internal/workflow/engine_messages.go:10`)
5. `channelEmitter` sends to buffered channel → `drainEmitterCmd()` reads → `Update()` (`internal/tui/app.go:388`)
6. REPL streaming display renders tokens in real-time (`internal/tui/repl_stream.go`)

### Agent Loop Path (REPL Chat)

1. User sends message in REPL → `AgentStreamMsg` initiated (`internal/tui/streaming/agent_loop.go`)
2. Agent loop calls `provider.ChatCompletionStream()` with tool definitions
3. LLM response may contain tool calls → agent loop calls `Dispatcher.Execute()` (`internal/tools/dispatcher.go:127`)
4. Dispatcher checks permissions → may emit `PermissionRequestMsg` → TUI shows modal
5. Tool executes → result fed back to LLM → loop continues (max 50 iterations)
6. Agent loop completes → `AgentDoneMsg` → final message added to REPL

### Tool Execution Permission Path

1. LLM response contains tool call JSON (`internal/tui/streaming/agent_loop.go`)
2. Agent loop parses tool call, calls `Dispatcher.Execute()` (`internal/tools/dispatcher.go:127`)
3. Dispatcher rate-limits via token bucket (`internal/tools/dispatcher.go:132`)
4. `ensurePermission()` checks rules, then sends `PermissionRequest` on channel (`internal/tools/permissions.go`)
5. TUI `permListenerCmd()` receives request → `PermissionRequestMsg` → shows modal (`internal/tui/app.go:292`)
6. User approves/denies → `PermissionResponseMsg` → dispatcher continues (`internal/tui/app_update.go`)

**State Management:**
- **Session state**: `pkg/session.Manager` persists to `<workDir>/.m31a/session.json`
- **Messages**: Separately persisted in `messages.json` for large history
- **Workflow state**: Persisted in `session.json` (goal, phase, questions) — survives restart
- **Checkpoints**: `checkpoint.json` (max 2) for undo/rollback support
- **Config**: TOML with hot-reload via `fsnotify` (`internal/config/loader.go:420+`)
- **TUI state**: In-memory `AppState` with screen enum routing — no persistence
- **Model cache**: In-memory with TTL (5 min active, 24h stale fallback) (`internal/provider/cache.go`)

## Key Abstractions

**Tool Interface (`types.Tool`):**
- Purpose: Unified contract for all LLM-callable operations
- Examples: `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `TodoWrite`, `AskUserQuestion`, `Agent`, `FileDelete`, `FileMove`, `FileList`, `CodeMap`
- Pattern: Each tool implements `Name()`, `Description()`, `RiskLevel()`, `Execute(ctx, ToolInput) (ToolResult, error)`. Optional `SchemaProvider` interface for JSON Schema parameter definitions.

**SchemaProvider Interface (`types.SchemaProvider`):**
- Purpose: Allows tools to define JSON Schema for LLM parameter validation
- Pattern: `ParameterSchema() string` returns raw JSON Schema string
- Used by: All tools that accept structured parameters from LLM

**LLMProvider Interface (`provider.LLMProvider`):**
- Purpose: Abstraction for LLM API communication
- Examples: `openrouter.Client`, `zen.Client`
- Pattern: `Name()`, `APIKey()`, `FetchModels()`, `CachedModels()`, `ChatCompletionStream()`, `EstimateCost()`, `HealthCheck()`, `GetModel()`

**WorkflowEngine Interface (`tuitypes.WorkflowEngine`):**
- Purpose: Decouples TUI from concrete workflow engine for testability
- Examples: `workflow.Engine`
- Pattern: `RunPhase()`, `Transition()`, `SetModel()`, `SetPhaseModel()`, `HealTask()`, `SubmitDiscussAnswer()`, `FinalizeDiscuss()`

**MsgEmitter Interface (`workflow.MsgEmitter`):**
- Purpose: Decouples workflow engine from Bubble Tea framework
- Examples: `channelEmitter` (TUI integration), nil emitter (tests)
- Pattern: `Emit(msg any)` — uses `any` to avoid importing `tea` package

**GitClient Interface (`types.GitClient`):**
- Purpose: Abstracts git operations for testability
- Examples: `git.Git` (shell-based wrapper)
- Pattern: `Init()`, `IsRepo()`, `Add()`, `Commit()`, `Log()`, `Diff()`, `CurrentBranch()`, `IsDirty()`, `HasUncommittedChanges()`

**Keychain Interface (`keychain.Keychain`):**
- Purpose: OS-native secure API key storage
- Examples: Linux (D-Bus Secret Service + pass), macOS (`/usr/bin/security`), Windows (Credential Manager)
- Pattern: Platform-specific implementations behind a common interface (`internal/tui/app_state.go:207`)

**Screen Enum (`tuitypes.Screen`):**
- Purpose: Routes messages to the correct sub-model
- Pattern: 29 integer constants (0–28), each mapped to a sub-model via `ensureXxxModel()` and `renderActiveScreen()`

**PhaseResult (`workflow.PhaseResult`):**
- Purpose: Carries the outcome of a completed workflow phase back to TUI
- Pattern: Contains success flag, tasks, messages, usage, cost, tool calls, commits, diff stats, demonstration

## Entry Points

**CLI Entry (`cmd/m31a/main.go`):**
- Location: `cmd/m31a/main.go`
- Triggers: `go run .` or compiled binary
- Responsibilities: Parse flags, load config, create logger, resolve API keys, register providers, create session/tools/git/ledger/rollback/autodream, wire subagents, launch Bubble Tea program

**TUI Init (`internal/tui/app.go:23`):**
- Location: `internal/tui/app.go`
- Triggers: Bubble Tea program starts
- Responsibilities: Start health ticker, permission listener, file watcher, config watcher, route to first screen (FirstRun or REPL)

**TUI Update (`internal/tui/app_update.go:27`):**
- Location: `internal/tui/app_update.go`
- Triggers: Every `tea.Msg` from Bubble Tea runtime
- Responsibilities: Single dispatch point for window resize, keyboard, mouse, streaming, permissions, workflow results, toasts, screen transitions

**Workflow RunPhase (`internal/workflow/engine.go:258`):**
- Location: `internal/workflow/engine.go`
- Triggers: TUI calls `RunPhaseCmd()` from `app.go:237`
- Responsibilities: Budget guardrail, phase dispatch (switch on WorkflowPhase), cost accumulation

**Dispatcher Execute (`internal/tools/dispatcher.go:127`):**
- Location: `internal/tools/dispatcher.go`
- Triggers: Workflow engine or agent loop calls `Execute(ctx, ToolCall)`
- Responsibilities: Rate limit, tool lookup, input parsing, permission gating, tool execution, error handling

## Architectural Constraints

- **Threading:** Single-threaded Bubble Tea update loop. All state mutations happen in `Update()`. Goroutines communicate only via `tea.Msg` channels. Workflow engine runs in a separate goroutine but uses `MsgEmitter` to send events back.
- **Global state:** `sharedTransport` in `internal/provider/base_client.go` (shared HTTP transport via `sync.Once`). `skipDirsCache` in `internal/types/constants.go` (cached skip dirs). `permissionRequestID` atomic counter in `internal/tools/interface.go`.
- **Circular imports:** `internal/tui/tuitypes/` exists specifically to break the circular dependency between `tui` and its sub-packages (`commands`, `streaming`). The `types.go` re-export pattern in `tui` package further smooths this.
- **pkg → internal imports:** `pkg/session/` imports `internal/errors`, `internal/fileutil`, `internal/types`. `pkg/taskrunner/` imports `internal/errors`, `internal/types`. `pkg/bisect/` imports `internal/errors`. These are tracked as architectural boundary violations.
- **Embed.FS constraints:** Prompt templates in `internal/workflow/prompts/` are compiled into the binary. Changes require recompilation.
- **No CGO:** `CGO_ENABLED=0` enforced everywhere. All dependencies must be pure Go.

## Anti-Patterns

### Goroutine State Mutation

**What happens:** Goroutines directly mutating `AppState` fields.
**Why it's wrong:** Violates Bubble Tea's single-threaded contract. Causes data races, session corruption, non-deterministic bugs.
**Do this instead:** Goroutines send `tea.Msg` through channels. `Update()` receives and processes them. See `internal/tui/streaming/streaming.go` lines 1–20 for the ownership model.

### Direct Tool Execution Without Permission

**What happens:** Calling `tool.Execute()` directly, bypassing the Dispatcher.
**Why it's wrong:** Skips rate limiting, permission gating, and audit logging. Security risk.
**Do this instead:** Always use `Dispatcher.Execute(ctx, ToolCall)` which handles permissions, rate limiting, and error wrapping. See `internal/tools/dispatcher.go:127`.

### Importing internal/ from pkg/

**What happens:** Packages in `pkg/` importing from `internal/`.
**Why it's wrong:** Breaks the layer boundary. `pkg/` is meant to be reusable; `internal/` is not.
**Do this instead:** Define interfaces in `internal/types/` and have `pkg/` depend only on those interfaces. Currently `pkg/session`, `pkg/taskrunner`, `pkg/bisect` import `internal/errors` — this is tracked tech debt.

### Hardcoded Model Selection

**What happens:** Using `cfg.Model.Default` directly without checking per-phase overrides.
**Why it's wrong:** Ignores user's dual-model selection (Planning vs Coding) and AgentsConfig overrides.
**Do this instead:** Use `Engine.modelForPhase(phase)` which checks priority: perPhaseModels → AgentsConfig → cfg.Model.Default → engine modelID. See `internal/workflow/engine.go:138`.

## Error Handling

**Strategy:** Sentinel errors in `internal/errors/errors.go` with `UserMessage()` for user-facing strings. Pattern-matching for HTTP status codes and connection errors.

**Patterns:**
- Sentinel error definitions: `var ErrXxx = errors.New("xxx")` in `internal/errors/errors.go`
- Error wrapping: `fmt.Errorf("context: %w", err)` with lowercase messages
- User messages: `errors.UserMessage(err)` maps sentinels to actionable strings (e.g., "Rate limited — retry in a moment")
- Self-heal loop: Max 2 attempts for recoverable task failures in verify phase (`internal/workflow/verify.go`)
- HTTP status detection: Regex patterns `reHTTP401`, `reHTTP429`, `reHTTP503` in `internal/errors/errors.go`
- Tool execution errors: Wrapped with `ErrToolExecution` sentinel for consistent handling
- Phase transition errors: `ErrPhaseTransition` for invalid state changes

## Cross-Cutting Concerns

**Logging:** `log/slog` only. JSON format (default) or text (via `M31A_LOG_FORMAT`). Daily rotation, 7-day retention in `~/.m31a/`. Never `fmt.Println` or `log.Printf` in production code. See `internal/log/log.go`.

**Validation:** Config validation in `internal/config/loader.go` via `validateConfig()`. Tool input validation in each tool's `Execute()` method. Permission rule validation at config load time.

**Authentication:** API key resolution order: env var → OS keychain → config file. Keychain abstraction per platform. Keys stripped from config file when keychain is available. See `pkg/keychain/keychain.go`.

**File I/O:** Atomic writes via `internal/fileutil/atomic.go` (temp file + `os.Rename()`). Size limits enforced: 50MB max for session files, 5MB max for FileRead. Backup system with configurable max per file.

**Configuration:** TOML-based with hot-reload via `fsnotify`. Config walks up 3 parent directories for project-local `m31a.toml`. Global config in `~/.m31a/config.toml`. Environment variable expansion in string values.

---

*Architecture analysis: 2026-06-16*
