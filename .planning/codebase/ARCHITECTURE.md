<!-- refreshed: 2026-06-14 -->
# Architecture

**Analysis Date:** 2026-06-14

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         CLI Entry Point                             │
│                   cmd/m31a/main.go                                  │
│         flag parsing, signal handling, wiring, TUI launch           │
└──────────────────────┬──────────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     TUI Application Layer                           │
│             internal/tui/  (Bubble Tea Elm Architecture)           │
│                                                                     │
│  ┌──────────────┐ ┌────────────┐ ┌────────────┐ ┌───────────────┐  │
│  │  AppState     │ │ ReplModel  │ │ Sidebar    │ │ Sub-Models    │  │
│  │ (root model)  │ │ (chat)     │ │ (sessions) │ │ plan/verify/  │  │
│  │               │ │            │ │            │ │ ship/discuss  │  │
│  └──────┬───────┘ └────────────┘ └────────────┘ └───────────────┘  │
│         │                                                           │
│  ┌──────┴──────────────────────────────────────────────────────┐    │
│  │         Commands System (commands_*.go)                      │    │
│  │   slash commands, keybindings, command palette               │    │
│  └─────────────────────────────────────────────────────────────┘    │
│         │                                                           │
│  ┌──────┴──────────────────────────────────────────────────────┐    │
│  │         Sub-components (components/)                         │    │
│  │   permission modal, question modal, thinking, toolcard       │    │
│  └─────────────────────────────────────────────────────────────┘    │
└──────────────────────┬──────────────────────────────────────────────┘
                       │
          ┌────────────┼────────────────┐
          │            │                │
          ▼            ▼                ▼
┌──────────────┐ ┌───────────┐ ┌─────────────────────┐
│   Workflow   │ │  Tools    │ │   Provider Layer     │
│   Engine     │ │  Dispatch │ │   (LLM Clients)      │
│              │ │           │ │                      │
│ Initialize   │ │ Bash      │ │ openrouter/client.go │
│ Discuss      │ │ FileR/W   │ │ zen/client.go        │
│ Plan         │ │ Edit      │ │ registry.go          │
│ Execute      │ │ Glob      │ │ interface.go         │
│ Verify       │ │ Grep      │ │ cache.go             │
│ Ship         │ │ WebFetch  │ │ fallback.go          │
│              │ │ Agent     │ │ sse.go               │
└──────┬───────┘ └─────┬─────┘ └─────────────────────┘
       │               │
       │               │
       ▼               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Supporting Packages (pkg/)                       │
│                                                                     │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ ┌──────────┐ ┌──────────┐  │
│  │ session  │ │ taskrunner│ │ rollback │ │ keychain │ │ ledger   │  │
│  │ (CRUD)   │ │ (DAG)    │ │ (git)   │ │ (OS)    │ │ (audit)  │  │
│  └──────────┘ └──────────┘ └─────────┘ └──────────┘ └──────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────────────┐           │
│  │ bisect   │ │ autodream│ │ arbitrage                │           │
│  │ (git)    │ │(context) │ │ (cost optimization)      │           │
│  └──────────┘ └──────────┘ └──────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Internal Utilities (internal/)                   │
│                                                                     │
│  config/  errors/  fileutil/  git/  log/  tokens/  types/          │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **Entry Point** | CLI flag parsing, logger init, config load, wiring all dependencies, signal handling, TUI launch | `cmd/m31a/main.go` |
| **AppState** | Root Bubble Tea model; owns all sub-models, screen routing, message dispatch | `internal/tui/app_state.go` |
| **AppState.Update** | Single message dispatch point; all state mutations flow through here | `internal/tui/app_update.go` |
| **ReplModel** | Chat interface: message history, streaming LLM responses, user input | `internal/tui/repl.go` |
| **Workflow Engine** | Orchestrates the 6-phase workflow: Initialize, Discuss, Plan, Execute, Verify, Ship | `internal/workflow/engine.go` |
| **Dispatcher** | Tool registry, permission enforcement, rate limiting, tool execution | `internal/tools/dispatcher.go` |
| **Provider Registry** | Multi-provider management, active provider selection, health checks | `internal/provider/registry.go` |
| **LLMProvider Interface** | Abstraction for LLM backends (OpenRouter, Zen) | `internal/provider/interface.go` |
| **Session Manager** | CRUD for sessions, checkpoints, tasks, project state on disk | `pkg/session/manager.go` |
| **Task Runner** | Topological sort + concurrent execution of task DAG | `pkg/taskrunner/runner.go` |
| **Subagent Manager** | Parallel child agents with isolated git worktrees | `internal/tools/subagent/manager.go` |
| **Config Loader** | TOML config loading with hot-reload via fsnotify | `internal/config/loader.go` |
| **Git Wrapper** | Git operations: add, commit, log, bisect, diff, worktree | `internal/git/git.go` |
| **Ledger** | Persistent session audit trail in markdown format | `pkg/ledger/ledger.go` |
| **Rollback** | Safe commit rollback with stash support | `pkg/rollback/rollback.go` |
| **AutoDream** | Context consolidation (message summarization to save tokens) | `pkg/autodream/autodream.go` |
| **Arbitrage** | Cost-based model recommendation for task complexity | `pkg/arbitrage/arbitrage.go` |
| **Bisect** | Automated git bisect for regression detection | `pkg/bisect/bisect.go` |
| **Keychain** | OS-native secure storage for API keys | `pkg/keychain/keychain.go` |

## Pattern Overview

**Overall:** Elm Architecture (Model-Update-View) with a layered internal package structure.

**Key Characteristics:**
- Bubble Tea TUI with strict single-threaded state mutation via `Update()`
- 6-phase workflow state machine with validated transitions
- Multi-provider LLM abstraction with streaming SSE support
- Tool-based agent loop with permission gating and rate limiting
- File-based session persistence (JSON + markdown)
- Subagent isolation via git worktrees

## Layers

**CLI Entry (`cmd/m31a/`):**
- Purpose: Application bootstrap and wiring
- Location: `cmd/m31a/main.go`
- Contains: Flag parsing, config loading, dependency injection, signal handling
- Depends on: All internal/ and pkg/ packages
- Used by: Shell (user invocation)

**TUI Layer (`internal/tui/`):**
- Purpose: User interface and interaction
- Location: `internal/tui/`
- Contains: Bubble Tea models, views, key handling, commands
- Depends on: workflow, tools, provider, config, types, pkg/*
- Used by: Entry point

**Workflow Engine (`internal/workflow/`):**
- Purpose: LLM-driven 6-phase workflow orchestration
- Location: `internal/workflow/`
- Contains: Phase runners (initialize, discuss, plan, execute, verify, ship), prompt templates, plan parser
- Depends on: provider, tools, types, tokens, pkg/session, pkg/bisect
- Used by: TUI layer

**Provider Layer (`internal/provider/`):**
- Purpose: LLM API abstraction with multi-provider support
- Location: `internal/provider/`
- Contains: Interface, registry, SSE parsing, model cache, resilience, OpenRouter/Zen clients
- Depends on: types, errors
- Used by: Workflow engine, TUI layer

**Tools Layer (`internal/tools/`):**
- Purpose: Safe tool execution with permission control
- Location: `internal/tools/`
- Contains: Dispatcher, individual tools (Bash, FileRead, Edit, etc.), permission system, rate limiting
- Depends on: types, config, errors
- Used by: Workflow engine, TUI agent loop

**Public Packages (`pkg/`):**
- Purpose: Reusable, domain-specific functionality
- Location: `pkg/`
- Contains: session, taskrunner, rollback, keychain, ledger, bisect, autodream, arbitrage
- Depends on: internal/types, internal/git, internal/errors
- Used by: Workflow engine, TUI layer

**Internal Utilities (`internal/`):**
- Purpose: Shared infrastructure code
- Location: `internal/config/`, `internal/errors/`, `internal/fileutil/`, `internal/git/`, `internal/log/`, `internal/tokens/`, `internal/types/`
- Contains: Config types, error sentinels, atomic file ops, git wrapper, logging, token estimation, shared type definitions
- Depends on: Minimal or none (leaf packages)
- Used by: All other packages

## Data Flow

### Primary Workflow Execution Path

1. User enters a goal in the REPL (`internal/tui/repl.go:44`)
2. AppState routes to workflow initialization (`internal/tui/app_update_phase.go`)
3. Workflow Engine classifies prompt complexity (`internal/workflow/classify.go:42`)
4. Engine runs phases sequentially with validated transitions (`internal/workflow/engine.go:257`)
5. Each phase builds context messages, calls LLM via streaming (`internal/workflow/engine.go:833`)
6. LLM returns streamed chunks parsed from SSE (`internal/provider/sse.go`)
7. Execute phase dispatches tool calls through Dispatcher (`internal/tools/dispatcher.go:127`)
8. Tools execute with permission gating (`internal/tools/dispatcher.go:271`)
9. Results flow back as messages to TUI for rendering
10. Ship phase creates final commit and records to ledger (`internal/workflow/ship.go`)

### LLM Streaming Path

1. Engine calls `streamLLM()` or `streamLLMWithTools()` (`internal/workflow/engine.go:833`)
2. Provider sends HTTP request with streaming enabled (`internal/provider/interface.go:14`)
3. SSE response is parsed into `StreamIterator` (`internal/provider/sse.go`)
4. Engine iterates chunks via `consumeStream()` or `consumeStreamWithTools()` (`internal/workflow/engine.go:654`)
5. Tool call chunks accumulated by index into `ToolCall` structs (`internal/workflow/engine.go:682`)
6. Content chunks emitted to TUI via `MsgEmitter` callback

### Tool Execution Path

1. LLM returns tool calls in response (`internal/workflow/engine.go:696`)
2. Dispatcher resolves tool by name (`internal/tools/dispatcher.go:140`)
3. Input JSON is parsed and normalized (`internal/tools/dispatcher.go:155`)
4. Permission check runs against rules + interactive prompt (`internal/tools/dispatcher.go:271`)
5. Rate limiter token acquired (`internal/tools/dispatcher.go:132`)
6. Tool executes with context (`internal/tools/dispatcher.go:189`)
7. Result returned with timing and error info

**State Management:**
- Bubble Tea model (`AppState`) is the single source of truth for UI state
- Session state persisted to disk as JSON files in `~/.m31a/sessions/<id>/`
- Workflow phase transitions validated by `validPhaseTransitions` map (`internal/workflow/engine.go:322`)
- Atomic cost tracking via `atomic.Uint64` bit-packing (`internal/workflow/engine.go:90`)
- All goroutines communicate via `tea.Msg` channels — no direct `AppState` mutation from goroutines

## Key Abstractions

**WorkflowPhase (State Machine):**
- Purpose: Represents the current phase of the 6-phase workflow
- Values: `idle`, `initialize`, `discuss`, `plan`, `execute`, `verify`, `ship`
- Examples: `internal/types/types.go:18`
- Pattern: String-typed enum with validated transitions

**LLMProvider (Interface):**
- Purpose: Abstracts LLM backend for multi-provider support
- Methods: `ChatCompletionStream`, `FetchModels`, `HealthCheck`, `GetModel`
- Examples: `internal/provider/interface.go:9`
- Pattern: Strategy pattern with registry

**Tool (Interface):**
- Purpose: Represents a callable tool with risk assessment
- Methods: `Name`, `Description`, `RiskLevel`, `Execute`
- Examples: `internal/types/types.go:143`
- Pattern: Command pattern with permission gating

**MsgEmitter (Callback):**
- Purpose: Decouples workflow engine from TUI rendering
- Signature: `func(msg any)` — emits typed messages to TUI
- Examples: `internal/workflow/engine.go:488`
- Pattern: Observer/callback pattern

**Dispatcher:**
- Purpose: Central tool execution hub with permission enforcement
- Features: Rate limiting, permission rules, agent-specific configs, interactive prompts
- Examples: `internal/tools/dispatcher.go:18`
- Pattern: Mediator with middleware chain

**TaskRunner:**
- Purpose: DAG-based task execution with dependency resolution
- Features: Topological sort (Kahn's algorithm), bounded parallelism, retry support
- Examples: `pkg/taskrunner/runner.go:27`
- Pattern: Pipeline with semaphore-controlled concurrency

**BaseClient:**
- Purpose: Shared HTTP client and model cache for all providers
- Features: Shared transport, model cache with stale-while-revalidate, health thresholds
- Examples: `internal/provider/base_client.go:35`
- Pattern: Template method (providers embed and override)

## Entry Points

**Main Binary (`cmd/m31a/main.go`):**
- Location: `cmd/m31a/main.go:35`
- Triggers: Shell invocation `m31a` or `./m31a`
- Responsibilities: Bootstrap, config, dependency wiring, launch TUI

**First Run Preview (`cmd/firstrunpreview/main.go`):**
- Location: `cmd/firstrunpreview/main.go`
- Triggers: First-run wizard preview
- Responsibilities: Displays initial setup UI

**TUI Init (`internal/tui/app.go:22`):**
- Location: `internal/tui/app.go:22`
- Triggers: Bubble Tea program start
- Responsibilities: Start health ticker, permission listener, route to initial screen

**Agent Loop (`internal/tui/agent_loop.go:79`):**
- Location: `internal/tui/agent_loop.go`
- Triggers: User sends plain text in autonomous mode or `/agent` command
- Responsibilities: LLM → tool → LLM loop with streaming

## Architectural Constraints

- **Threading:** Single-threaded Bubble Tea event loop. All `AppState` mutations happen in `Update()`. Goroutines communicate only via `tea.Msg` channels. This is a hard constraint — violating it causes race conditions.
- **Global state:** Module-level singletons: `sharedTransport` (HTTP), `sharedTransportOnce` (`internal/provider/base_client.go:15`), `skipDirsCache` (`internal/types/constants.go:121`), regex caches (`internal/workflow/plan_parser.go:23`). All protected by `sync.Once` or `sync.Map`.
- **Circular imports:** None detected. The `internal/` → `pkg/` dependency direction is enforced. `pkg/` packages depend on `internal/types` and `internal/errors` only.
- **Session isolation:** Each session gets its own directory under `~/.m31a/sessions/<id>/` with separate planning files, checkpoints, tasks, and messages.
- **Subagent isolation:** Each subagent gets its own git worktree, dispatcher, and session context. Parent cannot access child state directly — communication is via `SubagentEvent` channels.
- **Config hot-reload:** Config is watched via `fsnotify` and reloaded on changes (`internal/config/loader.go`). Permission updates propagate to the dispatcher without restart.

## Anti-Patterns

### Direct AppState Mutation from Goroutines

**What happens:** Goroutines attempting to modify `AppState` fields directly instead of sending `tea.Msg` values.
**Why it's wrong:** Breaks Bubble Tea's single-threaded contract, causing data races and session corruption.
**Do this instead:** Send a `tea.Msg` via `program.Send()` and handle it in `Update()`. See the signal handler pattern at `cmd/m31a/main.go:265` which sends `tea.QuitMsg{}` instead of calling `app.Shutdown()` directly.

### Tool Execution Without Permission Check

**What happens:** Bypassing `Dispatcher.ensurePermission()` to run tools directly.
**Why it's wrong:** Dangerous operations (Bash, FileWrite, FileDelete) could execute without user consent, violating the security model.
**Do this instead:** Always execute tools through `Dispatcher.Execute()` which enforces the permission chain (`internal/tools/dispatcher.go:127`).

### Bypassing Phase Transition Validation

**What happens:** Manually setting `activePhase` without calling `Transition()`.
**Why it's wrong:** `Transition()` saves checkpoints, writes STATE.md, validates allowed transitions, and guards against oscillation loops.
**Do this instead:** Use `Engine.Transition(ctx, from, to)` which validates against `validPhaseTransitions` (`internal/workflow/engine.go:334`).

## Error Handling

**Strategy:** Sentinel errors with user-friendly messages.

**Patterns:**
- **Sentinel errors** in `internal/errors/errors.go` — defined as package-level `var` values (e.g., `ErrRateLimited`, `ErrContextExceeded`). Checked via `errors.Is()`.
- **User message mapping** via `errors.UserMessage()` (`internal/errors/errors.go:49`) — maps sentinels and pattern-matched error strings to actionable user messages.
- **Wrapped errors** with `fmt.Errorf("context: %w", err)` — preserves the error chain for programmatic handling while adding context.
- **`toolResultError`** wrapper (`internal/tools/dispatcher.go:261`) — distinguishes between "permission rule returned a tool error" (logged in `ToolResult.Error`) and "real Go error" (propagated up).
- **Phase errors** return `*PhaseResult` with `Success: false` and `Error` string — the TUI reads this for display without crashing.

## Cross-Cutting Concerns

**Logging:** Structured logging via `log/slog`. Logger initialized in `main.go:69` with version metadata. Used throughout with `slog.Info/Warn/Error/Debug`.

**Validation:** Config validation during load (`internal/config/loader.go`). Task validation after plan parsing (`internal/workflow/plan.go:87`). Phase transition validation (`internal/workflow/engine.go:336`).

**Permission System:** Multi-layer: config rules → agent defaults → risk-level-based → interactive prompt. Hot-reloadable via `Dispatcher.UpdatePermissions()` (`internal/tools/dispatcher.go:98`).

**Rate Limiting:** Token bucket for tool execution (`internal/tools/dispatcher.go:36`). Configurable via `ToolsConfig`. Prevents resource exhaustion from buggy/malicious LLM outputs.

**Token Estimation:** EMA-calibrated estimator (`internal/tokens/estimator.go`). Preflight context check prevents OOM from oversized conversations (`internal/workflow/engine.go:498`).

**Budget Tracking:** Atomic cost accumulation across phases (`internal/workflow/engine.go:298`). Enforced via `BudgetLimitUSD` config (`internal/config/types.go:214`).

---

*Architecture analysis: 2026-06-14*
