<!-- refreshed: 2026-06-11 -->
# Architecture

**Analysis Date:** 2026-06-11

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                      Entry Point                             │
│              cmd/m31a/main.go                                │
│         (init, provider setup, signal handling)              │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    TUI Layer (MVU)                           │
│              internal/tui/                                   │
│    ┌──────────┬──────────┬──────────┬──────────┐            │
│    │ AppState │ ReplModel│ PlanModel│ ExecModel│ ... (26+)  │
│    │ app.go   │ repl_*.go│ plan_*.go│ exec_*.go│            │
│    └──────────┴──────────┴──────────┴──────────┘            │
│    ┌──────────────────────────────────────────┐             │
│    │ Components (internal/tui/components/)     │             │
│    │ permission, toolcard, thinking, badge...  │             │
│    └──────────────────────────────────────────┘             │
│    ┌──────────────────────────────────────────┐             │
│    │ Theme (internal/tui/theme/)              │             │
│    │ colors, borders, registry, shadows       │             │
│    └──────────────────────────────────────────┘             │
└────────────────────────────┬────────────────────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ Workflow Engine  │ │ Tool Dispatcher │ │ Session Manager │
│ internal/        │ │ internal/tools/ │ │ pkg/session/    │
│ workflow/        │ │ dispatcher.go   │ │ manager.go      │
│ engine.go        │ │ permissions.go  │ │ session.go      │
│ initialize.go    │ │ bash.go         │ │ checkpoint.go   │
│ discuss.go       │ │ fileread.go     │ │ planning.go     │
│ plan.go          │ │ filewrite.go    │ └─────────────────┘
│ execute.go       │ │ glob.go         │
│ verify.go        │ │ grep.go         │
│ ship.go          │ │ webfetch.go     │
└────────┬────────┘ │ edit.go          │
         │          │ question.go      │
         │          │ todo.go          │
         │          └────────┬────────┘
         │                   │
         ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   Provider Layer                             │
│              internal/provider/                              │
│    ┌──────────────┬──────────────┬──────────────┐           │
│    │ Registry     │ OpenRouter   │ Zen          │           │
│    │ registry.go  │ openrouter/  │ zen/         │           │
│    │ interface.go │ client.go    │ client.go    │           │
│    │ cache.go     │              │              │           │
│    │ fallback.go  │              │              │           │
│    │ reasoning.go │              │              │           │
│    └──────────────┴──────────────┴──────────────┘           │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                   Supporting Packages                        │
│              pkg/                                            │
│    ┌──────────┬──────────┬──────────┬──────────┐            │
│    │autodream │ ledger   │ rollback │arbitrage │            │
│    │consolid. │ sessions │ git hist │cost opt. │            │
│    └──────────┴──────────┴──────────┴──────────┘            │
│    ┌──────────┬──────────┬──────────┐                       │
│    │bisect    │taskrunner│ keychain │                       │
│    │git bisect│topo sort │ OS keys  │                       │
│    └──────────┴──────────┴──────────┘                       │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                   Foundation Layer                            │
│    ┌──────────┬──────────┬──────────┬──────────┐            │
│    │types/    │errors/   │config/   │fileutil/ │            │
│    │core types│sentinels │TOML load │atomic WR │            │
│    │constants │          │env vars  │          │            │
│    └──────────┴──────────┴──────────┴──────────┘            │
│    ┌──────────┬──────────┐                                  │
│    │git/      │log/      │                                  │
│    │git ops   │slog rot. │                                  │
│    └──────────┴──────────┘                                  │
└─────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| Entry Point | Binary init, provider setup, signal handling, TUI launch | `cmd/m31a/main.go` |
| AppState | Top-level Bubble Tea model, all state mutations via Update() | `internal/tui/app_state.go` |
| App Update | Message routing, screen transitions, workflow orchestration | `internal/tui/app_update.go` |
| App View | Rendering dispatch to active screen's View() | `internal/tui/app_view.go` |
| REPL Model | Chat interface, message history, input handling | `internal/tui/repl_model.go` |
| Workflow Engine | Six-phase workflow orchestration (Initialize→Discuss→Plan→Execute→Verify→Ship) | `internal/workflow/engine.go` |
| Provider Registry | Thread-safe provider management, active provider switching | `internal/provider/registry.go` |
| OpenRouter Client | OpenRouter API integration, SSE streaming | `internal/provider/openrouter/` |
| Zen Client | Zen API integration, SSE streaming | `internal/provider/zen/` |
| Tool Dispatcher | Tool registration, execution routing, permission gating | `internal/tools/dispatcher.go` |
| Permission Gate | Tool execution approval/denial, rule matching | `internal/tools/permissions.go` |
| Session Manager | Session CRUD, persistence, fork, archive | `pkg/session/manager.go` |
| AutoDream | Context consolidation, message compression | `pkg/autodream/autodream.go` |
| Ledger | Cross-session learning records, aggregate stats | `pkg/ledger/ledger.go` |
| Rollback | Git commit chain browsing, soft/hard reset | `pkg/rollback/rollback.go` |
| Arbitrage | Task complexity scoring, cost-aware model selection | `pkg/arbitrage/arbitrage.go` |
| Bisect | Git bisect wrapper for regression detection | `pkg/bisect/bisect.go` |
| Task Runner | Topological sort, dependency resolution, sequential execution | `pkg/taskrunner/runner.go` |
| Keychain | OS-specific secret storage (Linux/macOS/Windows) | `pkg/keychain/` |
| Token Estimator | Token counting, EMA calibration, context usage | `internal/tokens/estimator.go` |
| Config Loader | Multi-layer TOML config, env vars, validation | `internal/config/loader.go` |
| Logger | Structured slog with file rotation | `internal/log/log.go` |
| Git Client | Git operations wrapper | `internal/git/git.go` |
| Atomic Write | Crash-safe file writes (temp + rename) | `internal/fileutil/atomic.go` |

## Pattern Overview

**Overall:** Layered Architecture with MVU (Model-View-Update) for TUI and Engine Pattern for workflow orchestration.

**Key Characteristics:**
- Single-threaded TUI event loop (Bubble Tea) — all state mutations via `Update()`
- Goroutines emit `tea.Cmd` functions, never mutate `AppState` directly
- Strict dependency injection via constructors — no global mutable state
- Sentinel errors for all error matching (`errors.Is()`)
- Atomic file writes throughout (temp file + rename)
- Phase-gated workflow with strict state transitions
- Provider abstraction with auto-fallback on failures

## Layers

**Entry Point (`cmd/m31a/`):**
- Purpose: Binary bootstrap, dependency wiring, signal handling
- Location: `cmd/m31a/main.go`
- Contains: `main()`, `run()`, CLI flag parsing
- Depends on: All internal packages (wiring layer)
- Used by: OS exec

**TUI Layer (`internal/tui/`):**
- Purpose: User interface, screen routing, message handling
- Location: `internal/tui/`
- Contains: 26+ screen models, 40+ components, theme system, command registry
- Depends on: `internal/workflow/`, `internal/tools/`, `internal/provider/`, `pkg/session/`, `pkg/arbitrage/`, `pkg/autodream/`
- Used by: Entry point

**Workflow Engine (`internal/workflow/`):**
- Purpose: Six-phase workflow orchestration, LLM interaction, tool dispatch
- Location: `internal/workflow/`
- Contains: `engine.go`, `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `ship.go`
- Depends on: `internal/provider/`, `internal/tools/`, `internal/tokens/`, `internal/git/`, `pkg/session/`
- Used by: TUI layer

**Provider Layer (`internal/provider/`):**
- Purpose: LLM API abstraction, streaming, caching, fallback
- Location: `internal/provider/`
- Contains: `interface.go`, `registry.go`, `cache.go`, `fallback.go`, `reasoning.go`, `sse.go`, `common.go`
- Depends on: `internal/types/`, `internal/errors/`
- Used by: Workflow engine, TUI layer

**Tool Layer (`internal/tools/`):**
- Purpose: Tool implementations, permission gating, execution dispatch
- Location: `internal/tools/`
- Contains: `dispatcher.go`, `permissions.go`, `bash.go`, `fileread.go`, `filewrite.go`, `glob.go`, `grep.go`, `webfetch.go`, `edit.go`, `question.go`, `todo.go`
- Depends on: `internal/types/`, `internal/errors/`, `internal/config/`
- Used by: Workflow engine, TUI layer

**State Layer (`pkg/session/`, `internal/config/`):**
- Purpose: Session persistence, configuration management
- Location: `pkg/session/`, `internal/config/`
- Contains: `manager.go`, `session.go`, `checkpoint.go`, `planning.go`, `loader.go`, `types.go`
- Depends on: `internal/types/`, `internal/errors/`, `pkg/keychain/`
- Used by: TUI layer, workflow engine

**Supporting Packages (`pkg/`):**
- Purpose: Autonomous domain logic
- Location: `pkg/autodream/`, `pkg/ledger/`, `pkg/rollback/`, `pkg/arbitrage/`, `pkg/bisect/`, `pkg/taskrunner/`, `pkg/keychain/`
- Contains: Domain-specific logic with minimal internal dependencies
- Depends on: `internal/types/`, `internal/errors/`, `internal/git/`
- Used by: TUI layer, workflow engine

**Foundation Layer (`internal/types/`, `internal/errors/`, `internal/fileutil/`, `internal/log/`, `internal/git/`, `internal/tokens/`):**
- Purpose: Shared types, sentinel errors, utilities
- Location: `internal/types/`, `internal/errors/`, `internal/fileutil/`, `internal/log/`, `internal/git/`, `internal/tokens/`
- Contains: Core type definitions, error constants, atomic writes, structured logging, git operations, token estimation
- Depends on: Standard library only (leaf packages)
- Used by: All other packages

## Data Flow

### Primary Request Path (Chat)

1. User types message in REPL textarea (`internal/tui/repl_model.go`)
2. User presses Enter → `submitMsg` dispatched via `tea.Cmd`
3. `AppState.Update()` creates `ChatRequest` from message history (`internal/tui/app_update.go`)
4. `LLMProvider.ChatCompletionStream()` called with `ChatRequest` (`internal/provider/interface.go`)
5. HTTP POST to provider API with `stream: true` (`internal/provider/openrouter/client.go` or `zen/client.go`)
6. SSE stream parsed line-by-line by `StreamIterator.Next()` (`internal/provider/sse.go`)
7. `StreamChunk` events dispatched as `tea.Msg` to TUI (`internal/types/types.go`)
8. TUI renders progressively via `View()` (`internal/tui/repl_view.go`)
9. Tool calls extracted → dispatched via `Dispatcher.Execute()` (`internal/tools/dispatcher.go`)
10. Permission gate checks risk level → emits `PermissionRequestMsg` if dangerous (`internal/tools/permissions.go`)
11. User approves/denies → `PermissionResponseMsg` sent back
12. Tool executes → result fed back into `ChatRequest`
13. Loop continues until LLM sends final content

### Workflow Phase Path

1. User submits goal → `GoalSubmittedMsg` (`internal/tui/goalinput.go`)
2. `AppState.RunPhaseCmd(PhaseInitialize)` creates `tea.Cmd` (`internal/tui/app.go`)
3. Goroutine calls `engine.RunPhase(ctx, phase, goal)` (`internal/workflow/engine.go`)
4. Phase-specific handler runs (e.g., `runInitialize`, `runPlan`)
5. LLM called via `streamLLM()` or `streamLLMStreaming()` (`internal/workflow/engine.go`)
6. Tool calls dispatched via `Dispatcher.Execute()`
7. Results emitted via `MsgEmitter` → `tea.Msg` → TUI Update()
8. Phase completes → `PhaseResultMsg` emitted
9. TUI transitions to next phase screen

### Context Pruning Strategy

Each workflow phase runs in a fresh, pruned context:
- System prompt preserved across phases
- Conversation history discarded between phases
- State read from `planning/` files only
- Phase context budget: ~2K-10K tokens depending on phase

**State Management:**
- Bubble Tea's `AppState` is the single source of truth for UI state
- Session data persisted to `~/.m31a/sessions/<id>/` as JSON
- Planning state persisted to `~/.m31a/sessions/<id>/planning/` as Markdown
- No in-memory database — all state is either in `AppState` or on disk

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Abstract LLM API access behind a common interface
- Examples: `internal/provider/interface.go`, `internal/provider/openrouter/`, `internal/provider/zen/`
- Pattern: Strategy pattern with registry for runtime switching

**Tool Interface:**
- Purpose: Abstract tool execution behind a common interface
- Examples: `internal/tools/bash.go`, `internal/tools/fileread.go`, `internal/tools/grep.go`
- Pattern: Command pattern with risk levels and permission gating

**Engine:**
- Purpose: Orchestrate six-phase workflow with context pruning
- Examples: `internal/workflow/engine.go`, `internal/workflow/plan.go`, `internal/workflow/execute.go`
- Pattern: State machine with strict phase transitions

**Dispatcher:**
- Purpose: Route tool calls to implementations, enforce permissions
- Examples: `internal/tools/dispatcher.go`, `internal/tools/permissions.go`
- Pattern: Mediator pattern with permission gate

**SessionManager:**
- Purpose: CRUD operations for session persistence
- Examples: `pkg/session/manager.go`, `pkg/session/session.go`
- Pattern: Repository pattern with atomic writes

**Consolidator (AutoDream):**
- Purpose: Context consolidation when conversation grows large
- Examples: `pkg/autodream/autodream.go`
- Pattern: Strategy pattern with pause/resume support

**Registry:**
- Purpose: Thread-safe provider management
- Examples: `internal/provider/registry.go`
- Pattern: Service locator with active provider tracking

## Entry Points

**Binary Entry Point:**
- Location: `cmd/m31a/main.go`
- Triggers: `go run ./cmd/m31a` or compiled binary
- Responsibilities: CLI parsing, config loading, provider setup, TUI launch, signal handling

**TUI Application:**
- Location: `internal/tui/app.go`
- Triggers: `tea.NewProgram(app).Run()`
- Responsibilities: `Init()` starts health ticker and permission listener, `Update()` processes all messages, `View()` renders active screen

**Workflow Engine:**
- Location: `internal/workflow/engine.go`
- Triggers: `engine.RunPhase(ctx, phase, goal)`
- Responsibilities: Execute workflow phases, manage LLM interactions, dispatch tool calls

## Architectural Constraints

- **Threading:** Bubble Tea is single-threaded. All state mutations go through `Update()` only. Goroutines emit `tea.Cmd` functions that return `tea.Msg` values. Never mutate `AppState` from a goroutine.

- **Global state:** No module-level mutable state except `slog.SetDefault()` in `main.go`. All state is held in `AppState` or passed via function parameters.

- **Circular imports:** Not permitted. Dependency direction: `cmd/m31a/` → `internal/tui/` → `internal/workflow/` → `internal/provider/` → `internal/types/`. Foundation packages (`types/`, `errors/`) have zero internal imports.

- **CGO:** Binary must be static (`CGO_ENABLED=0`). No CGO allowed.

- **External calls:** Only to OpenRouter and Zen APIs. No telemetry, no analytics, no other external calls.

## Anti-Patterns

### Direct AppState Mutation from Goroutine

**What happens:** Goroutine directly sets fields on `AppState` (e.g., `m.messages = append(...)`)
**Why it's wrong:** Violates Bubble Tea's single-threaded contract, causes race conditions
**Do this instead:** Goroutine sends `tea.Cmd` that returns `tea.Msg`, `Update()` handles it: `func() tea.Msg { return MyMsg{Data: result} }`

### Using errors.Is() with Type Assertions

**What happens:** `switch e := err.(type) { case *MyError: ... }`
**Why it's wrong:** Doesn't work with wrapped errors or sentinel errors
**Do this instead:** Use `errors.Is(err, ErrSentinel)` for sentinel errors, `errors.As()` for typed errors

### Storing API Keys in Config File

**What happens:** API key stored in `config.toml` in plaintext
**Why it's wrong:** Security risk — config file may be committed or shared
**Do this instead:** Use env var → OS keychain → config file fallback order. Config file `api_key` field is last resort.

### Blocking in View()

**What happens:** `View()` calls `os.Stat()`, `time.Sleep()`, or any I/O
**Why it's wrong:** `View()` must render in <16ms. I/O causes frame drops.
**Do this instead:** Cache results in `Update()`, return cached values in `View()`

## Error Handling

**Strategy:** Sentinel errors in `internal/errors/errors.go` with `errors.Is()` matching. User-friendly messages via `errors.UserMessage()`.

**Patterns:**
- Sentinel errors: `var ErrProviderUnreachable = errors.New("provider unreachable")`
- Error wrapping: `fmt.Errorf("context: %w", err)`
- User messages: `errors.UserMessage(err)` returns actionable strings
- Provider errors: Normalized to sentinels based on HTTP status codes

## Cross-Cutting Concerns

**Logging:** Structured `log/slog` writing to `~/.m31a/m31a.log` with daily rotation. Never stdout/stderr during TUI operation. Log level configurable via `M31A_LOG_LEVEL`.

**Validation:** Config validation via `validateConfig()` in `internal/config/loader.go`. Collects all errors and returns joined error. Task graph validation via topological sort in `pkg/taskrunner/runner.go`.

**Authentication:** API keys resolved in order: env var → OS keychain → config file. Keys never stored in plaintext config. Keychain uses platform-specific backends (Linux: secret-service, macOS: Keychain, Windows: Credential Manager).

**Security:** SSRF protection blocks private/loopback IPs. Permission gate blocks dangerous tool execution until user approves. API keys masked in error messages. Session files size-limited to prevent OOM.

---

*Architecture analysis: 2026-06-11*
