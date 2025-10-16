<!-- refreshed: 2026-06-07 -->
# Architecture

**Analysis Date:** 2026-06-07

## System Overview

```text
┌──────────────────────────────────────────────────────────────────┐
│                        cmd/m31a/                                 │
│                   Binary Entry Point Only                         │
│         Config load → Provider registry → TUI launch              │
└──────────────────────────┬───────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│                       internal/tui/                               │
│              Bubble Tea Application (all screens)                 │
│  ┌───────────┬──────────┬──────────┬──────────┬──────────────┐   │
│  │   REPL    │  Plan    │ Execute  │  Verify  │  Ship Screen │   │
│  │  Screen   │  Screen  │  Screen  │  Screen  │  + 9 more    │   │
│  └───────────┴──────────┴──────────┴──────────┴──────────────┘   │
│  ┌──────────────────────┐  ┌────────────────────────────────┐    │
│  │  internal/tui/       │  │  internal/tui/theme/           │    │
│  │  components/         │  │  Color palette, styles         │    │
│  │  Reusable widgets    │  └────────────────────────────────┘    │
│  └──────────────────────┘                                        │
└───────────┬──────────────┬──────────────┬────────────────────────┘
            │              │              │
            ▼              ▼              ▼
┌────────────────┐ ┌──────────────┐ ┌─────────────────────────────┐
│ internal/      │ │ internal/    │ │  internal/workflow/          │
│ provider/      │ │ tools/       │ │  Six-Phase Workflow Engine   │
│ LLM gateway    │ │ Bash, File*, │ │  Initialize→Discuss→Plan→    │
│ OpenRouter+Zen │ │ Glob, Grep,  │ │  Execute→Verify→Ship         │
│ SSE streaming  │ │ Edit, Web,   │ │                              │
│ Model cache    │ │ Todo, Ask    │ │  ┌────────────────────────┐  │
└────────────────┘ └──────────────┘ │  │ prompts/*.md (embed)   │  │
                                    │  └────────────────────────┘  │
                                    └──────────────┬───────────────┘
                                                   │
                    ┌───────────────────────────────┼──────────────┐
                    ▼                               ▼              ▼
          ┌────────────────┐             ┌──────────────┐  ┌────────────┐
          │ pkg/session/   │             │ pkg/taskrunner│  │ pkg/git/   │
          │ Manager, CRUD, │             │ Topo sort,    │  │ internal/  │
          │ fork, archive  │             │ dependency    │  │ git/       │
          │ Checkpoints    │             │ graph         │  │ git wrapper│
          └────────────────┘             └──────────────┘  └────────────┘
                    │
     ┌──────────────┼───────────────┬──────────────┬──────────────┐
     ▼              ▼               ▼              ▼              ▼
┌──────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐ ┌────────────┐
│pkg/keychain│ │pkg/ledger/ │ │pkg/bisect/ │ │pkg/      │ │pkg/        │
│Linux/macOS│ │Cross-session│ │git bisect  │ │rollback/ │ │autodream/  │
│Windows    │ │learning    │ │wrapper     │ │commit    │ │context     │
│secret svc │ │LEDGER.md   │ │regression  │ │rollback  │ │consolidate │
└──────────┘ └────────────┘ └────────────┘ └──────────┘ └────────────┘
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
  ┌──────────────┐   ┌──────────────┐
  │ pkg/arbitrage│   │ internal/    │
  │ Cost scoring │   │ config/      │
  │ Model swap   │   │ TOML parser  │
  └──────────────┘   │ Env overrides│
                     └──────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `cmd/m31a/main.go` | Binary entry point — parses CLI flags, loads config, initializes providers, launches TUI. Contains zero business logic. | `cmd/m31a/main.go` |
| `internal/tui/` | Bubble Tea application — all screens, routing, message bus, theme system, key bindings, command palette. Single-threaded update loop. | `internal/tui/app.go`, `internal/tui/app_state.go`, `internal/tui/app_view.go`, `internal/tui/app_update.go` |
| `internal/tui/components/` | Reusable TUI widgets — tool cards, thinking blocks, permission modal, sparkline, progress bars, badges. | `internal/tui/components/toolcard.go`, `internal/tui/components/thinking.go`, `internal/tui/components/permission.go` |
| `internal/tui/theme/` | Color palette and theme definitions — dark/light/auto modes via `lipgloss` styles. | `internal/tui/theme/theme.go`, `internal/tui/theme/colors.go` |
| `internal/provider/` | LLM provider abstraction — `LLMProvider` interface, `Registry`, SSE stream parser, model cache, auto-fallback. | `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/sse.go`, `internal/provider/fallback.go` |
| `internal/provider/openrouter/` | OpenRouter client implementation — `GET /models`, `POST /chat/completions` with streaming. | `internal/provider/openrouter/client.go` |
| `internal/provider/zen/` | Zen (OpenCode) client implementation — identical interface, different base URL and response fields. | `internal/provider/zen/` |
| `internal/tools/` | Tool implementations and dispatcher — Bash (PTY on Unix), FileRead, FileWrite, Glob, Grep, Edit, WebFetch, TodoWrite, AskUserQuestion. Permission gate with risk levels. | `internal/tools/dispatcher.go`, `internal/tools/bash.go`, `internal/tools/fileread.go`, `internal/tools/filewrite.go` |
| `internal/workflow/` | Six-phase workflow engine — Initialize, Discuss, Plan, Execute, Verify, Ship. Orchestrates LLM calls, tool dispatch, self-healing, and git commits. | `internal/workflow/engine.go`, `internal/workflow/initialize.go`, `internal/workflow/discuss.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`, `internal/workflow/verify.go`, `internal/workflow/ship.go` |
| `internal/workflow/prompts/` | Embedded prompt templates for each phase — `base.md`, `tool-use.md`, `plan-format.md`, `execute-task.md`, `discuss-questions.md`, `self-heal.md`. | `internal/workflow/prompts/*.md` (embedded via `//go:embed`) |
| `internal/types/` | Shared core types — `Message`, `Task`, `ToolCall`, `ModelInfo`, `WorkflowPhase`, `RiskLevel`, constants. Zero internal imports. | `internal/types/types.go`, `internal/types/constants.go` |
| `internal/errors/` | Sentinel errors — all `var Err*` definitions plus `UserMessage()` for TUI-friendly error strings. Zero internal imports. | `internal/errors/errors.go` |
| `internal/config/` | TOML config parsing, env var overrides, defaults, hot-reload watcher. | `internal/config/types.go`, `internal/config/loader.go` |
| `internal/git/` | Git operations wrapper — init, add, commit, log, diff, reset, stash, branch, status (porcelain parser). | `internal/git/git.go` |
| `internal/log/` | Structured logger (`slog`) writing to `~/.m31a/m31a.log` with daily rotation, 7-day retention. | `internal/log/log.go` |
| `internal/tokens/` | Token estimation — `tiktoken-go` for GPT/Claude, character fallback, EMA calibration. | `internal/tokens/estimator.go` |
| `pkg/session/` | Session lifecycle — CRUD, fork, archive, checkpoints, workflow state persistence, recent models tracking. | `pkg/session/manager.go`, `pkg/session/session.go`, `pkg/session/planning.go`, `pkg/session/checkpoint.go` |
| `pkg/taskrunner/` | Task scheduling — topological sort via Kahn's algorithm, sequential group execution, dependency blocking. | `pkg/taskrunner/runner.go` |
| `pkg/ledger/` | Cross-session learning ledger — append-only markdown, filtered queries, aggregate stats. | `pkg/ledger/ledger.go` |
| `pkg/bisect/` | Git bisect wrapper — automated regression detection between session commits. | `pkg/bisect/bisect.go` |
| `pkg/rollback/` | Commit rollback chain — soft/hard reset with backup branches, diff preview. | `pkg/rollback/rollback.go` |
| `pkg/arbitrage/` | Cost-aware model arbitrage — complexity scoring (simple/moderate/complex), cost comparison matrix, recommendation engine. | `pkg/arbitrage/arbitrage.go` |
| `pkg/autodream/` | Context consolidation — summarizes oldest 50% of non-protected messages into a memory segment when context > 60%. | `pkg/autodream/autodream.go` |
| `pkg/keychain/` | OS keychain abstraction — Linux (secret-service via `godbus`), macOS (Keychain Services), Windows (Credential Manager). | `pkg/keychain/keychain.go`, `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go`, `pkg/keychain/keychain_windows.go` |

## Pattern Overview

**Overall:** Single-threaded event-loop (Bubble Tea) + goroutine message bus

**Key Characteristics:**
- All state mutations occur in `Update()` — goroutines emit `tea.Cmd` that return `tea.Msg` values
- Workflow phases run in background goroutines, communicating via `chan tea.Msg` channels
- Session state is persisted to disk as JSON + Markdown after every significant action
- Provider abstraction via `LLMProvider` interface — two concrete implementations (OpenRouter, Zen)
- Tool system uses permission gating with risk levels (safe/medium/dangerous/destructive)
- Context pruning per workflow phase — conversation history is discarded between phases; state read from planning/ files

## Layers

**TUI Layer (Presentation):**
- Purpose: Renders all screens, handles user input, manages screen routing
- Location: `internal/tui/`, `internal/tui/components/`, `internal/tui/theme/`
- Contains: Bubble Tea models, views, key bindings, command system, theme styles
- Depends on: `internal/workflow/`, `internal/tools/`, `internal/provider/`, `internal/config/`, all `pkg/` packages
- Used by: Terminal (user interaction)

**Provider Layer (LLM Gateway):**
- Purpose: Normalizes LLM API access across OpenRouter and Zen providers
- Location: `internal/provider/`, `internal/provider/openrouter/`, `internal/provider/zen/`
- Contains: `LLMProvider` interface, `Registry`, SSE parser, model cache, auto-fallback logic, reasoning normalization
- Depends on: `internal/types/`, `internal/errors/`
- Used by: `internal/workflow/`, `internal/tui/`

**Workflow Layer (Orchestration):**
- Purpose: Manages the six-phase workflow lifecycle, LLM context assembly, tool dispatch, self-healing
- Location: `internal/workflow/`
- Contains: `Engine` struct, phase implementations (`runInitialize`, `runDiscuss`, `runPlan`, `runExecute`, `runVerify`, `runShip`), embedded prompt templates
- Depends on: `internal/provider/`, `internal/tools/`, `internal/git/`, `internal/tokens/`, `pkg/session/`, `pkg/taskrunner/`, `pkg/bisect/`
- Used by: `internal/tui/`

**Tool Layer (Execution):**
- Purpose: Executes LLM-requested operations (file I/O, shell commands, web fetch, etc.)
- Location: `internal/tools/`
- Contains: Tool implementations, `Dispatcher`, permission gate, PTY management (Bash)
- Depends on: `internal/types/`, `internal/errors/`, `internal/config/` (permission rules)
- Used by: `internal/workflow/`

**State Layer (Persistence):**
- Purpose: Session lifecycle, file-based state persistence, checkpoints
- Location: `pkg/session/`
- Contains: `Manager`, `Session`, planning file I/O, checkpoint system, recent models
- Depends on: `internal/types/`, `internal/errors/`
- Used by: `internal/workflow/`, `internal/tui/`

**Infrastructure Packages:**
- Purpose: Cross-cutting concerns and domain-specific logic
- Location: `pkg/keychain/`, `pkg/ledger/`, `pkg/bisect/`, `pkg/rollback/`, `pkg/arbitrage/`, `pkg/autodream/`, `pkg/taskrunner/`
- Contains: OS keychain, cross-session learning, git bisect, commit rollback, cost arbitrage, context consolidation, task scheduling
- Depends on: `internal/types/`, `internal/errors/`, `internal/git/`
- Used by: `internal/workflow/`, `internal/tui/`

**Foundation Layer (Shared Types):**
- Purpose: Core type definitions and constants imported by all packages
- Location: `internal/types/`, `internal/errors/`
- Contains: `Message`, `Task`, `ToolCall`, `ModelInfo`, `WorkflowPhase`, `RiskLevel`, sentinel errors
- Depends on: Nothing (zero internal imports)
- Used by: All other packages

## Data Flow

### Primary Request Path (REPL Chat)

1. User types message in REPL textarea (`internal/tui/repl_model.go`)
2. `AppState.Update()` receives `submitMsg`, creates `ChatRequest` from message history (`internal/tui/app_update.go`)
3. `LLMProvider.ChatCompletionStream(ctx, req)` sends `POST /chat/completions` with `stream:true` (`internal/provider/interface.go`)
4. SSE stream parsed line-by-line by `StreamIterator.Next()` (`internal/provider/sse.go`)
5. `StreamChunk` events dispatched via `tea.Cmd` → `tea.Msg` → `AppState.Update()` (`internal/tui/app_channel.go`)
6. Content chunks appended to current message; thinking chunks open collapsible blocks (`internal/tui/repl_stream.go`)
7. Tool calls parsed from response, dispatched via `Dispatcher.Execute()` (`internal/tools/dispatcher.go`)
8. Tool results fed back into message history for next LLM turn

### Workflow Execution Path

1. User enters goal via `/workflow start` or GoalInput screen → `GoalSubmittedMsg` (`internal/tui/goalinput.go`)
2. `RunPhaseCmd()` creates per-phase goroutine + message channel (`internal/tui/app.go:RunPhaseCmd`)
3. `Engine.RunPhase(ctx, phase, goal)` dispatches to phase handler (`internal/workflow/engine.go:162`)
4. **Initialize:** Detects project type, inits git, writes `PROJECT.md` (`internal/workflow/initialize.go`)
5. **Discuss:** Streams LLM to generate clarifying questions, emits `StreamChunkMsg` to TUI (`internal/workflow/discuss.go`)
6. **Plan:** LLM generates JSON task list, schema-validated, serialized to `TASKS.md` (`internal/workflow/plan.go`)
7. **Execute:** `taskrunner.Runner` schedules tasks by dependency graph, executes sequentially per group (`internal/workflow/execute.go`, `pkg/taskrunner/runner.go`)
8. **Verify:** Checks task outputs, triggers self-heal loop (max 2 attempts), falls back to `git bisect` (`internal/workflow/verify.go`, `pkg/bisect/bisect.go`)
9. **Ship:** Final commit, ledger update, session archive (`internal/workflow/ship.go`)
10. `PhaseResultMsg` sent back to TUI via channel, `workflowMsgDrainer` delivers to `Update()` (`internal/tui/app.go:workflowMsgDrainer`)

### Streaming Chat Completion

```
User types message
        │
        ▼
AppState.Update() receives submitMsg
        │  Creates ChatRequest from message history
        ▼
LLMProvider.ChatCompletionStream(ctx, req)
        │  POST /chat/completions with stream:true
        ▼
SSE stream (HTTP response body)
        │  Line-by-line SSE parser
        ▼
StreamIterator.Next()
        │  yields typed StreamChunk events
        ▼
StreamChunk dispatched via tea.Cmd → tea.Msg
        │
        ├─ Type: "content"     → append to message content
        ├─ Type: "thinking"    → open/collapsible thinking block
        └─ Type: "done"        → finalize message, extract usage
        │
        ▼
TUI renders progressively via View()
```

### Tool Execution Flow

```
LLM response contains tool_calls JSON
        │
        ▼
Engine.parseToolCalls() extracts []ToolCall
        │
        ▼
Dispatcher.Execute(ctx, toolCall)
        │
        ├─ checkPermission() → rule-based allow/deny/ask
        ├─ RiskLevel check → PermissionRequestMsg to TUI
        ├─ User approves via PermissionModal
        │
        ▼
Tool.Execute(ctx, input) → ToolResult
        │
        ├─ Bash: exec.Cmd with PTY (Unix) or pipes (Windows)
        ├─ FileRead: encoding detection, binary check, 5MB limit
        ├─ FileWrite: atomic write (temp + rename), backup
        ├─ Glob: doublestar pattern matching, 1000 result limit
        ├─ Grep: rg with --json or pure-Go fallback
        ├─ Edit: search/replace with preview
        ├─ WebFetch: HTTP GET with SSRF protection
        │
        ▼
ToolResult fed back into message history
```

### Context Pruning Strategy

Each workflow phase runs in a fresh, pruned context. The system prompt is preserved
but all conversation history is discarded between phases. State is read from
structured planning/ files only.

| Phase | Typical Content |
|-------|----------------|
| Initialize | System prompt + MEMORY.md (if exists) |
| Discuss | System prompt + goal + PROJECT.md stub |
| Plan | System prompt + goal + Discuss Q&A + MEMORY.md + cwd schema |
| Execute (per task) | System prompt + TASKS.md + PROJECT.md + task spec |
| Verify | System prompt + TASKS.md + file contents |
| Ship | System prompt + TASKS.md (final status) + commit log |

### State Management

**Session State (Disk):**
- `session.json` — metadata (model, provider, phase, timestamps, workflow goal)
- `messages.json` — full message history
- `planning/PROJECT.md` — goal, project type, framework, discuss Q&A
- `planning/TASKS.md` — task list with status, dependencies, files
- `planning/STATE.md` — current phase, progress, last action
- `checkpoints/` — state snapshots for undo

**In-Memory State (Bubble Tea):**
- `AppState` struct — all TUI state, screens, models, health status
- `Engine` struct — workflow state, session ID, provider, model
- `Runner` struct — task execution state, status map, results
- `Consolidator` struct — message history for AutoDream

**Thread Safety:**
- Bubble Tea `Update()` is single-threaded — all state mutations go through it
- `Dispatcher` uses `sync.RWMutex` for concurrent tool registration and permission checks
- `Registry` uses `sync.RWMutex` for concurrent provider access
- `Ledger` uses `sync.RWMutex` for concurrent append/query
- `Consolidator` uses `sync.RWMutex` + `atomic.Bool` for reentrancy guard

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Abstracts LLM API access behind a common interface
- Examples: `internal/provider/interface.go`, `internal/provider/openrouter/client.go`, `internal/provider/zen/`
- Pattern: Strategy pattern with `Registry` for dynamic provider selection

**Tool Interface:**
- Purpose: Abstracts tool execution behind a common interface with risk levels
- Examples: `internal/types/types.go:Tool`, `internal/tools/bash.go`, `internal/tools/fileread.go`
- Pattern: Command pattern with `Dispatcher` for routing and permission gating

**WorkflowPhase Enum:**
- Purpose: Defines the six-phase workflow state machine
- Examples: `internal/types/types.go`, `internal/workflow/engine.go`
- Pattern: State machine with `validPhaseTransitions` map enforcing allowed transitions

**StreamIterator:**
- Purpose: Yields typed `StreamChunk` events from SSE response
- Examples: `internal/types/types.go`, `internal/provider/sse.go`
- Pattern: Iterator pattern with `Next()`/`Close()` methods

**Consolidator (AutoDream):**
- Purpose: Summarizes oldest messages to stay within context window
- Examples: `pkg/autodream/autodream.go`
- Pattern: Protected set (first message, system messages, tool calls, last 5) + candidate set consolidation

## Entry Points

**Binary Entry:**
- Location: `cmd/m31a/main.go`
- Triggers: `go run ./cmd/m31a` or compiled binary
- Responsibilities: CLI flag parsing, config loading, provider initialization, TUI launch

**TUI Entry:**
- Location: `internal/tui/app.go:NewApp()`
- Triggers: `cmd/m31a/main.go` calls `tui.NewApp()` then `tea.NewProgram().Run()`
- Responsibilities: Screen routing, theme initialization, session manager setup, workflow engine creation

**Workflow Entry:**
- Location: `internal/workflow/engine.go:RunPhase()`
- Triggers: TUI dispatches `RunPhaseCmd()` goroutine
- Responsibilities: Phase orchestration, LLM streaming, tool dispatch, state persistence

**Tool Entry:**
- Location: `internal/tools/dispatcher.go:Execute()`
- Triggers: `Engine.executeTaskWithTools()` or `Engine.healTask()`
- Responsibilities: Permission check, tool routing, result collection

## Architectural Constraints

- **Threading:** Bubble Tea is single-threaded. All state mutations go through `Update()`. Goroutines emit `tea.Cmd` functions that return `tea.Msg` values. Never mutate `AppState` from a goroutine.
- **Global state:** No module-level singletons except `slog.Default()` logger. `defaultLogger` in `internal/log/log.go` is set once at startup.
- **Circular imports:** None permitted. `internal/types/` and `internal/errors/` are leaf packages (zero internal imports). Known violation: `internal/tools/` imports `internal/config/` for `PermissionRule` type (deferred to Phase 26+).
- **CGO:** Forbidden. Binary must be static (`CGO_ENABLED=0`). Platform-specific code uses build tags, not CGO.
- **API keys:** Resolved in order: env var → OS keychain → config file. Never plaintext in committed files.
- **No telemetry:** No analytics, no external calls except to OpenRouter/Zen APIs.
- **Context pruning:** Each workflow phase discards prior conversation; reads state from `planning/` files only.

## Anti-Patterns

### Direct AppState Mutation from Goroutines

**What happens:** Goroutine writes to `AppState` fields directly (e.g., `app.replModel.AppendStreamChunk()`)
**Why it's wrong:** Violates Bubble Tea's single-threaded contract; causes data races and rendering glitches
**Do this instead:** Emit `tea.Cmd` that returns a `tea.Msg`, handle in `Update()`. See `internal/tui/app_channel.go` for `channelEmitter` pattern.

### Importing internal/config from internal/tools

**What happens:** `internal/tools/dispatcher.go` imports `internal/config` for `PermissionRule` type
**Why it's wrong:** Creates a dependency violation — `internal/tools/` should only import `internal/types/` and `internal/errors/`
**Do this instead:** Move `PermissionRule` to `internal/types/` (planned for Phase 26+)

### Blocking I/O in Update()

**What happens:** `Update()` calls a function that performs network or disk I/O
**Why it's wrong:** Blocks the TUI render loop, causing frame drops and unresponsive UI
**Do this instead:** Use `tea.Cmd` goroutines for I/O. See `HealthCheckTicker()` and `CacheRefreshTicker()` patterns in `internal/tui/health.go` and `internal/tui/cache_refresh.go`.

## Error Handling

**Strategy:** Sentinel errors defined in `internal/errors/errors.go`, user-friendly messages via `errors.UserMessage()`

**Patterns:**
- All sentinel errors are `var Err* = errors.New("...")` — use `errors.Is()` for comparison
- Provider errors normalized to sentinel types: HTTP 401 → `ErrInvalidKey`, HTTP 429 → `ErrRateLimited`, HTTP 503 → `ErrProviderUnreachable`
- TUI displays styled error messages via `UserMessage()` — never raw stack traces
- Context window protection: preflight check before LLM requests (80% warning, 95% blocking)
- Tool execution errors wrapped with `ErrToolExecution` for consistent handling

## Cross-Cutting Concerns

**Logging:** Structured `slog` logger writing to `~/.m31a/m31a.log` with daily rotation and 7-day retention. Never writes to stdout/stderr during TUI operation.

**Validation:** Schema validation on LLM-generated plan JSON. Session file validation on load (required fields, phase enum). Task dependency graph validation (cycle detection via Kahn's algorithm).

**Authentication:** API key resolution chain: env var → OS keychain → config file. Keychain integration via `pkg/keychain/` (Linux secret-service, macOS Keychain, Windows Credential Manager).

**Permissions:** Tool execution gated by risk level. `PermissionRequestMsg` sent to TUI when dangerous/destructive tool requested. User approves via modal with timeout auto-deny (default 300s).

**Config Hot-Reload:** File watcher polls `~/.m31a/config.toml` every 5 seconds. Changes trigger `ConfigReloadMsg` → TUI applies new settings without restart.

---

*Architecture analysis: 2026-06-07*
