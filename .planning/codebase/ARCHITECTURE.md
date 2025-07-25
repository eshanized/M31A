<!-- refreshed: 2026-06-04 -->
# Architecture

**Analysis Date:** 2026-06-04

## System Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│                        cmd/m31a/main.go                             │
│              Binary entry point — config, providers, TUI launch     │
└────────────┬───────────────────────────────────────┬────────────────┘
             │                                       │
             v                                       v
┌────────────────────────────┐     ┌──────────────────────────────────┐
│   internal/config/         │     │   internal/log/                  │
│   TOML config + env +      │     │   Structured slog logger with    │
│   keychain resolution      │     │   daily rotation, file-only      │
│   `loader.go`, `types.go`  │     │   `log.go`                       │
└────────────┬───────────────┘     └──────────────────────────────────┘
             │
             v
┌─────────────────────────────────────────────────────────────────────┐
│                     internal/tui/ (Bubble Tea App)                  │
│                                                                     │
│  AppState.Update() ◄──── tea.Msg ────► AppState.View()              │
│                                                                     │
│  ┌──────────┬──────────┬──────────┬──────────┬──────────┬─────────┐ │
│  │ REPL     │ Settings │ ModelSel │ Resume   │ Plan     │ Execute │ │
│  │ repl.go  │ settings │ modelsel │ resume   │ plan.go  │ exec.go │ │
│  │          │ .go      │ .go      │ .go      │          │         │ │
│  ├──────────┴──────────┴──────────┴──────────┴──────────┴─────────┤ │
│  │  components/ (MessageRenderer, ToolCard, PermissionModal,      │ │
│  │              ThinkingBlock, Sidebar, CommandPalette)            │ │
│  ├────────────────────────────────────────────────────────────────┤ │
│  │  theme/ (dark/light palette, all lipgloss styles)              │ │
│  └────────────────────────────────────────────────────────────────┘ │
└────────────┬──────────────┬────────────────┬───────────────────────┘
             │              │                │
             v              v                v
┌────────────────────┐ ┌──────────────┐ ┌─────────────────────────────┐
│ internal/provider/ │ │internal/tools│ │  internal/workflow/         │
│ LLMProvider iface  │ │Tool interface│ │  Engine — six-phase runner  │
│ OpenRouter + Zen   │ │Dispatcher    │ │  phases: init/discuss/plan/ │
│ SSE parsing        │ │Bash/Edit/    │ │  execute/verify/ship        │
│ Model cache        │ │FileRead/     │ │  prompts/ (embedded MD)     │
│ Fallback logic     │ │FileWrite/    │ │  parseToolCalls, validate   │
│ Reasoning normalize│ │Glob/Grep/    │ └──────────┬──────────────────┘
│ `openrouter/`,     │ │WebFetch/etc  │            │
│  `zen/`            │ └──────┬───────┘            │
└────────────────────┘        │                    v
                              │       ┌─────────────────────────────┐
                              │       │ pkg/taskrunner/             │
                              │       │ Topological sort + groups   │
                              │       │ Sequential execution        │
                              │       └─────────────────────────────┘
                              v
┌─────────────────────────────────────────────────────────────────────┐
│                        shared packages (pkg/)                       │
│                                                                     │
│  pkg/session/    pkg/keychain/    pkg/ledger/    pkg/rollback/      │
│  session CRUD    OS keychain      learning       commit chain       │
│  checkpoints     Linux/macOS/Win  ledger.md      soft/hard reset   │
│  planning files                                                │
│                                                                     │
│  pkg/arbitrage/    pkg/bisect/     pkg/autodream/                   │
│  cost scoring      git bisect      context consolidation            │
│  model suggest     wrapper         summarize oldest messages        │
└─────────────────────────────────────────────────────────────────────┘
             │
             v
┌─────────────────────────────────────────────────────────────────────┐
│                  internal/types/ + internal/errors/                 │
│  Core types (Message, Task, ToolCall, WorkflowPhase, Session, ...)  │
│  Sentinel errors (ErrProviderUnreachable, ErrRateLimited, ...)      │
│  Leaf packages — zero internal imports                              │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | Key File(s) |
|-----------|----------------|-------------|
| `cmd/m31a` | Binary entry point; config loading, provider registry setup, TUI launch | `cmd/m31a/main.go` |
| `internal/config` | TOML config parsing, multi-layer merge (global→env→project), validation, variable substitution | `internal/config/loader.go`, `internal/config/types.go` |
| `internal/log` | Structured slog logger with daily file rotation (7-day retention) | `internal/log/log.go` |
| `internal/types` | Core domain types: `Message`, `Task`, `ToolCall`, `WorkflowPhase`, `ModelInfo`, `Session`, `StreamChunk` | `internal/types/types.go`, `internal/types/constants.go` |
| `internal/errors` | Sentinel errors (`ErrProviderUnreachable`, `ErrRateLimited`, etc.) + user-friendly messages | `internal/errors/errors.go` |
| `internal/provider` | `LLMProvider` interface, `Registry` (thread-safe map), `SSEParser`, `ModelCache`, fallback logic, reasoning config | `internal/provider/interface.go`, `internal/provider/registry.go`, `internal/provider/sse.go`, `internal/provider/cache.go`, `internal/provider/fallback.go`, `internal/provider/reasoning.go` |
| `internal/provider/openrouter` | OpenRouter API client — `FetchModels`, `ChatCompletionStream`, `HealthCheck` | `internal/provider/openrouter/client.go` |
| `internal/provider/zen` | OpenCode Zen API client — same interface, OpenAI-compatible endpoints | `internal/provider/zen/client.go` |
| `internal/tui` | Bubble Tea application: `AppState` (model), screen routing, `Update()`/`View()`, workflow orchestration | `internal/tui/app.go`, `internal/tui/app_update.go`, `internal/tui/app_view.go`, `internal/tui/app_workflow.go` |
| `internal/tui` (REPL) | Main chat screen: streaming, textarea input, message rendering, slash commands | `internal/tui/repl.go`, `internal/tui/repl_view.go`, `internal/tui/repl_stream.go`, `internal/tui/repl_thinking.go` |
| `internal/tui/components` | Reusable UI primitives: `MessageRenderer`, `ToolCard`, `PermissionModal`, `ThinkingBlock`, `QuestionModal` | `internal/tui/components/message.go`, `internal/tui/components/toolcard.go`, `internal/tui/components/permission.go`, `internal/tui/components/thinking.go` |
| `internal/tui/theme` | Color palette definition, dark/light/auto modes, all lipgloss styles | `internal/tui/theme/theme.go`, `internal/tui/theme/colors.go` |
| `internal/tools` | `Tool` interface implementations + `Dispatcher` (registry, permissions, execution gate) | `internal/tools/interface.go`, `internal/tools/dispatcher.go`, `internal/tools/defaults.go` |
| `internal/tools` (Bash) | PTY-based shell execution with 30-minute timeout, signal forwarding, output streaming | `internal/tools/bash.go`, `internal/tools/bash_unix.go`, `internal/tools/bash_windows.go` |
| `internal/workflow` | Six-phase workflow engine: `Engine.RunPhase()` orchestrates Initialize→Discuss→Plan→Execute→Verify→Ship | `internal/workflow/engine.go`, `internal/workflow/initialize.go`, `internal/workflow/discuss.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`, `internal/workflow/verify.go`, `internal/workflow/ship.go` |
| `internal/workflow/prompts` | Embedded prompt templates for all phases (7 markdown files) | `internal/workflow/prompts/base.md`, `internal/workflow/prompts/plan-format.md`, etc. |
| `internal/git` | Git wrapper: init, add, commit, log, diff, bisect, head hash, branch operations | `internal/git/git.go` |
| `internal/tokens` | Token estimation: tiktoken-go for GPT/Claude, rune-based fallback, EMA calibration, context warning | `internal/tokens/estimator.go` |
| `pkg/session` | Session lifecycle: ID generation, file persistence, planning file I/O (PROJECT.md, TASKS.md, STATE.md), checkpoints | `pkg/session/manager.go`, `pkg/session/planning.go`, `pkg/session/checkpoint.go` |
| `pkg/keychain` | OS-native keychain abstraction: Linux (D-Bus Secret Service), macOS (Keychain CLI), Windows (stub) | `pkg/keychain/keychain.go`, `pkg/keychain/keychain_linux.go`, `pkg/keychain/keychain_darwin.go` |
| `pkg/taskrunner` | Task scheduling: topological sort (Kahn's), dependency groups, sequential execution within groups | `pkg/taskrunner/runner.go` |
| `pkg/arbitrage` | Model cost optimization: complexity scoring, cost estimation, arbitrage recommendations | `pkg/arbitrage/arbitrage.go` |
| `pkg/bisect` | Git bisect wrapper: runs bisect between session start and HEAD to find offending commit | `pkg/bisect/bisect.go` |
| `pkg/ledger` | Cross-session learning ledger: append to `~/.m31a/LEDGER.md`, stats, query, context injection | `pkg/ledger/ledger.go` |
| `pkg/rollback` | Commit rollback: chain view, soft/hard reset, backup branch creation, diff preview | `pkg/rollback/rollback.go` |
| `pkg/autodream` | Context consolidation: summarize oldest 50% of messages when context > 60%, pause/resume/compress | `pkg/autodream/autodream.go` |

## Pattern Overview

**Overall:** Bubble Tea El-Update-View (EUV) with Workflow Engine + Provider Abstraction

**Key Characteristics:**
- Single-threaded TUI event loop (`Update()` receives `tea.Msg`, mutates state, returns `tea.Cmd`)
- All state mutations go through `Update()` only — goroutines communicate via `tea.Msg` channels
- Six-phase workflow engine runs in a separate goroutine, emits messages to TUI via `MsgEmitter`
- Provider layer abstracts OpenRouter/Zen behind `LLMProvider` interface with auto-fallback
- Tool system uses permission-gated dispatcher with risk-level classification
- All planning state persisted as human-readable Markdown files (PROJECT.md, TASKS.md, STATE.md)
- Configuration is TOML-based with env var and project-level overrides

## Layers

**Leaf Layer (zero internal imports):**
- Purpose: Core types and errors used by all other packages
- Location: `internal/types/`, `internal/errors/`
- Contains: Domain types (`Message`, `Task`, `WorkflowPhase`, etc.), sentinel errors, constants
- Depends on: nothing internal
- Used by: everything else

**Provider Layer:**
- Purpose: LLM API abstraction with streaming, caching, and auto-fallback
- Location: `internal/provider/`, `internal/provider/openrouter/`, `internal/provider/zen/`
- Contains: `LLMProvider` interface, `Registry`, SSE parser, model cache, reasoning normalization, fallback logic
- Depends on: `internal/types/`, `internal/errors/`
- Used by: `internal/tui/`, `internal/workflow/`

**Tool Layer:**
- Purpose: File system and shell tool execution with permission gating
- Location: `internal/tools/`
- Contains: `Dispatcher`, `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `TodoWrite`, `AskUserQuestion`
- Depends on: `internal/types/`, `internal/errors/`, `internal/config/`
- Used by: `internal/tui/`, `internal/workflow/`

**Config Layer:**
- Purpose: Multi-layer configuration with env var and project overrides
- Location: `internal/config/`
- Contains: TOML parsing, env var resolution, keychain integration, validation
- Depends on: `internal/types/`, `pkg/keychain/`
- Used by: `cmd/m31a/`, `internal/tui/`, `internal/tools/`

**Session Layer:**
- Purpose: Session lifecycle, file persistence, planning file I/O
- Location: `pkg/session/`
- Contains: `Manager`, checkpoint system, PROJECT.md/TASKS.md/STATE.md reader/writer
- Depends on: `internal/types/`, `internal/errors/`
- Used by: `internal/tui/`, `internal/workflow/`

**Workflow Layer:**
- Purpose: Six-phase workflow orchestration with context pruning
- Location: `internal/workflow/`
- Contains: `Engine`, phase implementations, prompt registry, task validation, JSON parsing
- Depends on: `internal/types/`, `internal/errors/`, `internal/provider/`, `internal/tools/`, `internal/git/`, `internal/tokens/`, `pkg/session/`, `pkg/taskrunner/`, `pkg/bisect/`
- Used by: `internal/tui/`

**TUI Layer:**
- Purpose: Terminal UI with screen routing, streaming rendering, and user interaction
- Location: `internal/tui/`, `internal/tui/components/`, `internal/tui/theme/`
- Contains: `AppState` (Bubble Tea model), REPL, settings, model selector, workflow screens, reusable components
- Depends on: all internal packages + `pkg/arbitrage/`, `pkg/ledger/`, `pkg/rollback/`, `pkg/autodream/`, `pkg/session/`, `pkg/keychain/`
- Used by: `cmd/m31a/`

## Data Flow

### Primary Chat Flow (REPL)

1. User types message in textarea → Enter pressed (`internal/tui/repl.go`)
2. `ReplModel.Update()` receives `tea.KeyMsg` → builds `ChatRequest` from message history
3. `StartStreamCmd()` launches goroutine calling `LLMProvider.ChatCompletionStream()` (`internal/tui/streaming.go`)
4. SSE stream yields `StreamChunk` events via `StreamIterator.Next()` (`internal/provider/sse.go`)
5. Goroutine sends `StreamMsg` into `chan tea.Msg` → Bubble Tea picks up via `tea.Cmd` continuation
6. `ReplModel.Update()` receives `StreamMsg` → appends delta to `streamContent` buffer (`internal/tui/repl_stream.go`)
7. On `"done"` chunk: finalize message, emit `StreamDoneMsg`, store in `messages` slice
8. `ReplModel.View()` renders messages via `MessageRenderer` + `Glamour` markdown (`internal/tui/components/message.go`)

### Workflow Execution Flow

1. User enters `/workflow <goal>` or triggers from REPL (`internal/tui/commands_workflow.go`)
2. `AppState.initWorkflowEngine()` creates `workflow.Engine` with session, provider, tools, git (`internal/tui/app_workflow.go`)
3. `RunPhaseCmd()` starts phase in goroutine via `tea.Batch(runner, drainer)` (`internal/tui/app.go:341`)
4. Phase goroutine calls `Engine.RunPhase()` → dispatches to `runInitialize`, `runDiscuss`, `runPlan`, `runExecute`, `runVerify`, `runShip` (`internal/workflow/engine.go:250`)
5. Each phase builds context messages → calls `streamLLM()` or `streamLLMStreaming()` → parses response
6. Phase emits `tea.Msg` values via `MsgEmitter` (channel → `workflowMsgDrainer`) for TUI rendering
7. `PhaseResultMsg` carries final result back to `AppState.Update()` for screen transitions
8. Phase transitions validated by `validPhaseTransitions` map → checkpoint saved → STATE.md updated

### Tool Execution Flow

1. LLM response parsed for tool calls by `Engine.parseToolCalls()` (`internal/workflow/engine.go:819`)
2. `ToolCall` dispatched via `Dispatcher.Execute()` → permission gate check (`internal/tools/dispatcher.go:75`)
3. Permission rules evaluated (glob-matched against tool params) → `PermissionRequest` sent to TUI via `requestCh`
4. TUI shows `PermissionModal` (`internal/tui/components/permission.go`) → user approves/denies
5. `PermissionResponse` sent back via `responseCh` → tool executes → `ToolResult` returned
6. Result fed back into LLM context for next response

### SSE Stream Processing

1. HTTP response body scanned by `SSEParser` (`internal/provider/sse.go`) — line-by-line, `[DONE]` sentinel
2. `SSEParser.Next()` returns `(eventType, data, error)` — handles `\r\n` normalization (H-18 fix)
3. Provider-specific client (`openrouter/client.go` or `zen/client.go`) parses JSON into `StreamChunk`
4. `ReasoningConfig` applied per model family — DeepSeek, OpenAI o-series, Anthropic, Qwen (`internal/provider/reasoning.go`)
5. `StreamChunk{Type: "thinking"}` vs `{Type: "content"}` segments detected for rendering

**State Management:**
- Bubble Tea `AppState` holds all mutable state — updated exclusively in `Update()`
- Workflow state persisted to disk: `session.json`, `messages.json`, `planning/PROJECT.md`, `planning/TASKS.md`, `planning/STATE.md`
- Checkpoints saved before phase transitions (max 2 retained) for `/undo`
- Phase generation counter (`phaseGen`) prevents stale goroutine messages

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Abstracts LLM API behind a unified interface for streaming chat
- Examples: `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go`
- Pattern: Interface + registry with auto-fallback on rate limit or unavailability

**Tool Interface:**
- Purpose: Defines executable tools with risk classification
- Examples: `internal/tools/bash.go`, `internal/tools/fileread.go`, `internal/tools/filewrite.go`
- Pattern: Interface + dispatcher with permission gate and risk-level gating

**Workflow Engine:**
- Purpose: Orchestrates six-phase development lifecycle
- Examples: `internal/workflow/engine.go`
- Pattern: State machine with validated transitions, context pruning per phase, embedded prompt templates

**MsgEmitter:**
- Purpose: Decouples workflow goroutine from TUI event loop
- Examples: `internal/tui/app.go` (channelEmitter), `internal/workflow/engine.go`
- Pattern: Channel-based message passing with timeout protection

**Dispatcher:**
- Purpose: Tool registry with permission evaluation and execution gating
- Examples: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`
- Pattern: Thread-safe registry + rule-based permission evaluation (glob matching)

## Entry Points

**Binary Entry:**
- Location: `cmd/m31a/main.go`
- Triggers: User runs `m31a` binary
- Responsibilities: Parse CLI flags, load config, resolve API keys, create provider registry, launch TUI

**TUI Application:**
- Location: `internal/tui/app.go`
- Triggers: `tea.NewProgram(app).Run()` from `cmd/m31a/main.go`
- Responsibilities: Bubble Tea `Init()`, `Update()`, `View()` — all UI state management

**Workflow Engine:**
- Location: `internal/workflow/engine.go`
- Triggers: `Engine.RunPhase()` called from TUI goroutine via `RunPhaseCmd()`
- Responsibilities: Phase execution, LLM streaming, tool dispatch, session persistence

**Tool Dispatcher:**
- Location: `internal/tools/dispatcher.go`
- Triggers: `Dispatcher.Execute()` called from workflow engine or REPL
- Responsibilities: Permission check, tool execution, result formatting

## Architectural Constraints

- **Threading:** Bubble Tea is single-threaded. All state mutations go through `Update()` only. Goroutines emit `tea.Msg` via channels; never mutate `AppState` from goroutines. The workflow engine runs in a goroutine but communicates only via `MsgEmitter` → `channelEmitter`.
- **Global state:** No module-level singletons. `AppState` is the single mutable state owner. `slog.Default()` is set once in `cmd/m31a/main.go` and used via package-level `slog.Info/Warn/Error` calls.
- **Circular imports:** `internal/types/` and `internal/errors/` are leaf packages with zero internal imports. `internal/provider/` imports only `types` + `errors`. `internal/tools/` imports `types`, `errors`, `config`. No circular dependencies exist.
- **CGO:** Binary must be static (`CGO_ENABLED=0`). Platform-specific keychain uses build tags, not CGO. Bash tool uses `creack/pty` on Linux/macOS.
- **Streaming:** HTTP client uses 30s dial timeout but NO body read timeout (streaming is unbounded). SSE parser handles `[DONE]` sentinel and `\r\n` normalization.

## Anti-Patterns

### Mutating AppState from Goroutines

**What happens:** Goroutine directly writes to `AppState` fields
**Why it's wrong:** Bubble Tea's update loop is single-threaded; concurrent mutation causes data races
**Do this instead:** Goroutine sends `tea.Msg` via channel → `Update()` receives and processes. See `channelEmitter` in `internal/tui/app.go:484`

### Blocking in View()

**What happens:** `View()` reads mutable state or performs I/O
**Why it's wrong:** Bubble Tea contract requires `View()` to be pure — it runs on every tick
**Do this instead:** Cache derived state in `Update()` (e.g., `headerCacheKey`/`headerCacheValue` in `AppState`). All rendering is pure string composition.

### Ignoring Phase Transition Guard

**What happens:** Code attempts to jump workflow phases out of order
**Why it's wrong:** `validPhaseTransitions` map enforces the only legal transitions; bypassing it corrupts session state
**Do this instead:** Always go through `Engine.Transition()` which validates and saves checkpoint. See `internal/workflow/engine.go:295`

## Error Handling

**Strategy:** Sentinel errors in `internal/errors/errors.go` with `errors.Is()` comparison. User-facing errors mapped via `UserMessage()` function.

**Patterns:**
- `errors.Is(err, ErrProviderUnreachable)` for provider failures → auto-fallback if enabled
- HTTP status code → sentinel error normalization (401→ErrInvalidKey, 429→ErrRateLimited, 503→ErrProviderUnreachable)
- Tool execution errors wrapped with `ErrToolExecution` for LLM feedback loop
- Context window exceeded → `ErrContextExceeded` → block request if > 95% usage

## Cross-Cutting Concerns

**Logging:** Structured `slog` writing to `~/.m31a/m31a.log` only (never stdout during TUI). JSON format by default; text via `M31A_LOG_FORMAT=text`. Daily rotation, 7-day retention.
**Validation:** Config validated on load (`validateConfig` in `internal/config/loader.go`). Task list validated after LLM plan generation (`validateTasks` in `internal/workflow/engine.go`). Phase transitions validated by guard map.
**Authentication:** API keys resolved in order: env var → OS keychain → config file. Keychain backed by D-Bus Secret Service (Linux), Keychain CLI (macOS), stub (Windows). Keys never stored in plaintext in committed files.

---

*Architecture analysis: 2026-06-04*
