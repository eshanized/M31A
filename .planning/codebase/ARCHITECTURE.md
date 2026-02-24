<!-- refreshed: 2026-06-11 -->
# Architecture

**Analysis Date:** 2026-06-11

## System Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│                        TUI Layer (Bubble Tea)                    │
│   internal/tui/  —  AppState, screens, components, layout       │
│   AppState.Update() ← tea.Msg ← tea.Cmd                        │
├──────────────┬───────────────┬───────────────┬──────────────────┤
│  REPL Screen │  Plan Screen  │ Execute Screen│  20+ more screens│
│  internal/tui│  internal/tui │  internal/tui │                  │
│  /repl*.go   │  /plan_*.go   │  /execute_*.go│                  │
└──────┬───────┴───────┬───────┴───────┬───────┴──────────────────┘
       │               │               │
       ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Workflow Engine                               │
│   internal/workflow/  —  Engine, 6-phase pipeline               │
│   Initialize → Discuss → Plan → Execute → Verify → Ship         │
├──────────────┬──────────────────────────────────────────────────┤
│              │                                                  │
│   ┌──────────▼──────────┐    ┌────────────────────────────┐    │
│   │  Task Runner         │    │  Token Estimator           │    │
│   │  pkg/taskrunner/     │    │  internal/tokens/          │    │
│   │  DAG → topological   │    │  tiktoken-go + EMA         │    │
│   └─────────────────────┘    └────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Provider Layer                                │
│   internal/provider/  —  LLMProvider interface + Registry       │
│   ┌──────────────────┐    ┌──────────────────┐                  │
│   │ OpenRouter Client │    │ Zen Client        │                  │
│   │ /openrouter/      │    │ /zen/             │                  │
│   └──────────────────┘    └──────────────────┘                  │
│   ChatCompletionStream() → StreamIterator → StreamChunk          │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Tool Layer                                    │
│   internal/tools/  —  Dispatcher + 12 registered tools          │
│   Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch,       │
│   TodoWrite, AskUserQuestion, FileList, FileDelete, FileMove    │
│   Permission gate → risk assessment → user prompt                │
└─────────────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Infrastructure                                │
│   internal/config/  —  TOML config + keychain resolution        │
│   internal/errors/  —  sentinel errors                          │
│   internal/git/     —  git operations wrapper                   │
│   internal/log/     —  slog logger with rotation                │
│   pkg/session/      —  session lifecycle + file persistence     │
│   pkg/keychain/     —  OS keychain (linux/darwin/windows)       │
│   pkg/autodream/    —  context consolidation                    │
│   pkg/ledger/       —  cross-session learning                   │
│   pkg/rollback/     —  commit chain browser                     │
│   pkg/arbitrage/    —  model cost optimization                  │
└─────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **AppState** | Top-level Bubble Tea model; owns all state; single `Update()` dispatch | `internal/tui/app_state.go` |
| **AppState.Update** | Central message router; delegates to sub-models and handlers | `internal/tui/app_update.go` |
| **AppState.View** | Renders full terminal frame: header + content + footer via PageLayout | `internal/tui/app_view.go` |
| **ReplModel** | Chat REPL screen; message input, streaming display, slash commands | `internal/tui/repl.go`, `internal/tui/repl_model.go` |
| **Engine** | Six-phase workflow orchestrator; LLM calls, tool dispatch, self-heal | `internal/workflow/engine.go` |
| **Dispatcher** | Tool registry + permission gate + rate limiter; executes tool calls | `internal/tools/dispatcher.go` |
| **Registry** | Provider registry; tracks active provider, thread-safe swap | `internal/provider/registry.go` |
| **Session Manager** | Session lifecycle, file persistence, checkpoints, planning files | `pkg/session/manager.go` |
| **TaskRunner** | DAG-based task scheduling (Kahn's algorithm), sequential group execution | `pkg/taskrunner/runner.go` |
| **Token Estimator** | Token counting via tiktoken-go with EMA calibration fallback | `internal/tokens/` |
| **Git Client** | Git operations wrapper (status, diff, commit, log) | `internal/git/git.go` |
| **Config Loader** | TOML config parsing, env var overrides, keychain resolution | `internal/config/loader.go` |
| **PageLayout** | Unified header/content/footer chrome with responsive breakpoints | `internal/tui/layout/page.go` |

## Pattern Overview

**Overall:** Elm Architecture (Model-View-Update) via Bubble Tea

**Key Characteristics:**
- Single-threaded state mutations through `AppState.Update()` only
- Goroutines communicate via `tea.Cmd` / `tea.Msg` — never mutate state directly
- Provider layer is interface-based (`LLMProvider`) with OpenRouter and Zen implementations
- Workflow engine runs in goroutines, emits events via `MsgEmitter` → channel → `tea.Cmd`
- Tool execution is synchronous within the main loop (V1 sequential dispatch)
- All planning state lives in Markdown files under `~/.m31a/sessions/<id>/planning/`
- Context pruning: each workflow phase discards prior conversation; reads from files only

## Layers

**TUI Layer:**
- Purpose: User-facing terminal interface; all rendering and user input
- Location: `internal/tui/`
- Contains: 25+ screen models, 40+ reusable components, theme system, responsive layout
- Depends on: `internal/workflow`, `internal/provider`, `internal/tools`, `internal/config`, `internal/types`
- Used by: `cmd/m31a/main.go` (entry point creates AppState, launches Bubble Tea)

**Workflow Layer:**
- Purpose: Six-phase task execution pipeline; orchestrates LLM calls and tool dispatch
- Location: `internal/workflow/`
- Contains: Engine struct, 6 phase implementations, prompt templates (embedded), plan parser
- Depends on: `internal/provider`, `internal/tools`, `internal/types`, `internal/config`, `pkg/session`, `pkg/taskrunner`
- Used by: `internal/tui/` (AppState initializes and runs phases)

**Provider Layer:**
- Purpose: LLM API abstraction; streaming chat completion, model catalog, health checks
- Location: `internal/provider/`
- Contains: `LLMProvider` interface, `Registry`, OpenRouter client, Zen client
- Depends on: `internal/types`, `internal/errors`
- Used by: `internal/workflow/`, `internal/tui/`

**Tool Layer:**
- Purpose: Filesystem and shell tool execution with permission gating
- Location: `internal/tools/`
- Contains: 12 tools (Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch, TodoWrite, AskUserQuestion, FileList, FileDelete, FileMove), Dispatcher, permissions
- Depends on: `internal/types`, `internal/errors`, `internal/config` (known violation CR-09)
- Used by: `internal/workflow/` (Engine calls Dispatcher.Execute)

**Types Layer:**
- Purpose: Shared core types; leaf package with zero internal imports
- Location: `internal/types/`
- Contains: Message, Task, ToolCall, ModelInfo, WorkflowPhase, TaskStatus, constants
- Depends on: nothing (stdlib only)
- Used by: everything

**Infrastructure Layer:**
- Purpose: Cross-cutting concerns; config, logging, git, sessions, keychain
- Location: `internal/config/`, `internal/errors/`, `internal/git/`, `internal/log/`, `pkg/session/`, `pkg/keychain/`
- Depends on: `internal/types` (config/errors); `pkg/keychain` depends on OS-specific packages
- Used by: all layers

**Optional Packages (pkg/):**
- Purpose: Self-contained domain logic; no internal package imports
- Location: `pkg/autodream/`, `pkg/ledger/`, `pkg/rollback/`, `pkg/arbitrage/`, `pkg/taskrunner/`, `pkg/bisect/`
- Depends on: `internal/types`, `internal/errors` only
- Used by: `internal/tui/`, `internal/workflow/`

## Data Flow

### Primary Request Path (User sends a message)

1. User types in REPL textarea, presses Enter (`internal/tui/repl.go:handleEnterKey`)
2. `ReplModel` emits `SlashCommandMsg{Command: input}` via `tea.Cmd`
3. `AppState.Update` receives message, routes to `handleSlashCommand` (`internal/tui/app_update.go`)
4. For regular messages: REPL calls `startChatStream()` which spawns a goroutine
5. Goroutine calls `LLMProvider.ChatCompletionStream()` (`internal/provider/interface.go`)
6. SSE stream is parsed line-by-line into `StreamChunk` events (`internal/provider/openrouter/client.go`)
7. Each chunk is wrapped in `tea.Cmd` that returns `StreamMsg` → forwarded to `AppState.Update`
8. `ReplModel.handleStreamMsg()` appends delta to message content, triggers re-render
9. `StreamDoneMsg` finalizes the message and records token usage

### Workflow Execution Path (Goal → Ship)

1. User submits goal via `/goal` → `GoalSubmittedMsg` (`internal/tui/app_update.go`)
2. `AppState.RunPhaseCmd(PhaseInitialize)` spawns goroutine (`internal/tui/app.go:188`)
3. `Engine.RunPhase()` dispatches to `runInitialize()` → `runDiscuss()` → `runPlan()` → `runExecute()` → `runVerify()` → `runShip()` (`internal/workflow/engine.go:240-255`)
4. Each phase reads/writes planning files via `SessionManager` (`pkg/session/planning.go`)
5. Plan phase: LLM generates task list → parsed by `plan_parser.go` → saved to `TASKS.md`
6. Execute phase: `TaskRunner.Schedule()` builds DAG → `TaskRunner.ExecuteGroup()` runs tasks sequentially (`pkg/taskrunner/runner.go`)
7. Each task: LLM generates tool calls → `Dispatcher.Execute()` runs tools → self-heal on failure
8. Verify phase: checks task outputs, runs acceptance criteria, self-heal loop
9. Ship phase: creates commits, archives session, updates ledger

### REPL Streaming Path (LLM response → display)

1. `streamChat()` goroutine calls `provider.ChatCompletionStream()` (`internal/tui/repl_stream.go`)
2. Iterator reads SSE lines, yields `StreamChunk{Type: "content"|"thinking"|"done"}`
3. Each chunk wrapped in `func() tea.Msg { return StreamMsg{...} }` (`internal/tui/repl_stream.go`)
4. `AppState.Update()` receives `StreamMsg`, delegates to `ReplModel.handleStreamMsg()` (`internal/tui/repl.go:58`)
5. `handleStreamMsg` appends delta to message content, calls `renderMessages()`
6. `View()` re-renders with updated content in viewport

**State Management:**
- All state lives in `AppState` struct (`internal/tui/app_state.go`)
- State mutations only in `Update()` — goroutines emit `tea.Cmd` returning `tea.Msg`
- Workflow engine state (tasks, phase) persisted to disk via `SessionManager`
- Provider/model state tracked in `AppState.activeProvider`, `AppState.activeModel`
- Config state loaded once at startup, hot-reloaded via `/settings`

## Key Abstractions

**LLMProvider:**
- Purpose: Abstract LLM API behind a common interface
- Examples: `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`
- Pattern: Interface with 7 methods; Registry manages active provider; streaming via `StreamIterator`

**Tool:**
- Purpose: Abstract filesystem/shell operations with risk levels
- Examples: `internal/tools/bash.go`, `internal/tools/fileread.go`, `internal/tools/grep.go`
- Pattern: `types.Tool` interface with `Name()`, `Description()`, `RiskLevel()`, `Execute()`; optional `SchemaProvider` for JSON Schema

**Workflow Phase:**
- Purpose: Named execution stage with strict transition ordering
- Examples: `internal/workflow/initialize.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`
- Pattern: Engine dispatches via switch on `WorkflowPhase`; each phase is a method returning `*PhaseResult`

**Screen:**
- Purpose: Full-screen TUI view routed by `AppState.screen`
- Examples: `internal/tui/repl.go`, `internal/tui/plan_model.go`, `internal/tui/execute_model.go`
- Pattern: Enum `Screen` (25 values) in `internal/tui/types.go`; `View()` dispatches to `renderActiveScreen()`

**MsgEmitter:**
- Purpose: Decouple workflow engine events from Bubble Tea framework
- Examples: `internal/workflow/engine_messages.go`, `internal/tui/app.go:309-311`
- Pattern: `channelEmitter` wraps `chan tea.Msg`; engine emits typed messages; `drainEmitterCmd()` bridges to `Update()`

## Entry Points

**`cmd/m31a/main.go`:**
- Location: `cmd/m31a/main.go`
- Triggers: User runs `m31a` binary
- Responsibilities: Parse CLI flags, init logger, load config, resolve API keys via keychain, create provider registry, create session manager, create tools dispatcher, create git client, create TUI app, launch Bubble Tea program, handle SIGTERM/SIGINT

**`AppState.Init()`:**
- Location: `internal/tui/app.go:22`
- Triggers: Bubble Tea calls after `NewProgram().Run()`
- Responsibilities: Session cleanup, startup routing (first-run wizard vs REPL), start health ticker, start permission/question listeners, async provider enrichment, resume or create session

**`AppState.Update()`:**
- Location: `internal/tui/app_update.go:23`
- Triggers: Every `tea.Msg` from Bubble Tea runtime
- Responsibilities: Central dispatch for window resize, keyboard, screen routing, streaming, workflow results, permissions, questions, toast expiry, health checks, model selection

**`Engine.RunPhase()`:**
- Location: `internal/workflow/engine.go:222`
- Triggers: `AppState.RunPhaseCmd()` spawns goroutine
- Responsibilities: Budget guardrail check, dispatch to phase-specific method, accumulate cost, return `PhaseResult`

## Architectural Constraints

- **Threading:** Bubble Tea single-threaded. All state mutations go through `Update()` only. Goroutines emit `tea.Cmd` functions returning `tea.Msg` values.
- **Global state:** `slog.SetDefault()` called once at startup (`cmd/m31a/main.go:65`). No other module-level singletons.
- **Circular imports:** None permitted. `internal/types/` is the leaf package. `internal/errors/` has zero internal imports.
- **No CGO:** Build requires `CGO_ENABLED=0` for static binary. PTY for Bash tool uses `creack/pty` (pure Go).
- **No external LLM connections:** Only OpenRouter and Zen gateway APIs. No direct Anthropic/OpenAI.
- **No telemetry:** Zero external calls except to configured LLM providers.
- **Context pruning:** Each workflow phase discards prior conversation. State read from `planning/` files only.

## Anti-Patterns

### Mutating AppState from Goroutines

**What happens:** Goroutine directly writes to `AppState` fields
**Why it's wrong:** Breaks Bubble Tea's single-threaded contract; causes data races, rendering glitches
**Do this instead:** Goroutine returns `tea.Cmd` that returns `tea.Msg`; `Update()` handles the message (`internal/tui/app.go:188-217`)

### Importing internal/config from internal/tools

**What happens:** `internal/tools/dispatcher.go`, `permissions.go`, `defaults.go` import `internal/config`
**Why it's wrong:** Dependency rule violation — `internal/tools/` may only import `internal/types/` and `internal/errors/`
**Do this instead:** Move `PermissionRule` type to `internal/types/types.go`; update all import paths (deferred to Phase 26+; documented as CR-09)

### Hardcoding Model IDs

**What happens:** Model list is static or hardcoded in UI code
**Why it's wrong:** Models change frequently; new models added by providers daily
**Do this instead:** Models discovered dynamically from provider APIs via `FetchModels()` and cached with TTL (`internal/provider/interface.go:12`)

### Blocking Update() with Long Operations

**What happens:** `Update()` calls synchronous LLM API or tool execution
**Why it's wrong:** Freezes entire TUI; no spinner, no cancel, no streaming
**Do this instead:** Spawn goroutine via `tea.Cmd`; stream results via `tea.Msg`; cancel via `context.CancelFunc`

## Error Handling

**Strategy:** Sentinel errors in `internal/errors/errors.go` with `errors.Is()` matching; user-friendly messages via `UserMessage()` function

**Patterns:**
- Sentinel errors: `var ErrProviderUnreachable = errors.New("provider unreachable")` — use `errors.Is(err, ErrProviderUnreachable)`
- Error wrapping: `fmt.Errorf("load tasks: %w", err)` — preserves sentinel chain
- User-facing: `errors.UserMessage(err)` returns actionable strings like "Rate limited — retry in a moment" (`internal/errors/errors.go:49-126`)
- Tool errors: `toolResultError` wrapper distinguishes rule-level errors from Go errors (`internal/tools/dispatcher.go:255-259`)
- Provider errors: HTTP status → normalized sentinel (401→ErrInvalidKey, 429→ErrRateLimited, 503→ErrProviderUnreachable)

## Cross-Cutting Concerns

**Logging:** Structured slog with daily rotation, 7-day retention. `internal/log/` creates logger at startup; set as `slog.SetDefault()`. JSON format by default, configurable via `M31A_LOG_FORMAT`.

**Validation:** Permission rules evaluated in `Dispatcher.ensurePermission()` (`internal/tools/dispatcher.go:265`). Tool input validated against parameter count limits (max 1000 params). File sizes capped at 5MB. Context window protection via token estimation.

**Authentication:** API keys resolved in order: env var → OS keychain → config file. Never plaintext storage preferred. `pkg/keychain/` wraps OS-specific secret service (Linux: dbus secret-service, macOS: Keychain, Windows: Credential Manager).

**Responsive Layout:** Terminal width detected via `tea.WindowSizeMsg`. Breakpoints: UltraNarrow (<40), Compact (40-59), Standard (60-79), Full (80+). `internal/tui/layout/responsive.go` defines breakpoints and `ShowSidebar()`/`ShowFooterHints()`/`ShowFooterCost()` helpers.

---

*Architecture analysis: 2026-06-11*
