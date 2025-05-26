<!-- refreshed: 2026-06-02 -->
# Architecture

**Analysis Date:** 2026-06-02

## System Overview

M31A is a Go 1.22+ terminal AI coding agent. It is structured as a **Bubble Tea single-threaded state machine** on top of a **layered internal package graph** that exposes a narrow public API through `pkg/`. A single binary (`m31a`) loads configuration, registers LLM providers, initializes a session manager, and launches the TUI. The TUI delegates goal-driven work to a **six-phase workflow engine** which streams chat completions, dispatches tools, and persists state to disk.

```text
┌──────────────────────────────────────────────────────────────────────────┐
│                          cmd/m31a/main.go (189 lines)                    │
│   flags → log init → config.Load → keychain.New → provider registry →    │
│   tui.NewApp → tea.NewProgram.Run()                                      │
└──────────────────────────────┬───────────────────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                    internal/tui/  (Bubble Tea app)                       │
│   app.go: AppState (1676 lines) — Init/Update/View + screen router      │
│   screens: FirstRun, REPL, Plan, Execute, Verify, Ship, Settings,       │
│            Resume, ModelSelector, Permission, Diff                       │
│   command palette, key chord registry, sidebar, statusbar, health,       │
│   diff viewer, streaming, history                                        │
└────┬──────────────┬──────────────┬──────────────────┬────────────────────┘
     │              │              │                  │
     ▼              ▼              ▼                  ▼
┌─────────┐  ┌──────────────┐  ┌────────────┐  ┌─────────────────────────┐
│provider │  │  workflow/   │  │  tools/    │  │  pkg/ (autodream,       │
│Registry │  │  Engine      │  │ Dispatcher │  │  ledger, rollback,      │
│+ OR/Zen │  │  6 phases    │  │ + 10 tools │  │  session, keychain,     │
│ clients │  │  + MsgEmitter│  │ + rules    │  │  taskrunner, arbitrage, │
│  + SSE  │  │              │  │            │  │  bisect)                │
└────┬────┘  └──────┬───────┘  └─────┬──────┘  └──────────┬──────────────┘
     │              │                │                    │
     ▼              ▼                ▼                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│            internal/types/  (leaf — Message, Task, ToolCall, etc.)      │
│            internal/errors/  (sentinel errors, leaf)                     │
│            internal/config/  (TOML, env, keychain resolution)            │
│            internal/tokens/  (estimator + EMA calibration)               │
│            internal/log/     (slog → file, never stdout during TUI)      │
│            internal/git/     (init, commit, log, diff, bisect wrapper)   │
└──────────────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  ~/.m31a/  (state on disk, atomic writes via .m31a_tmp_ + rename)       │
│   config.toml, m31a.log, LEDGER.md, recent_models.json,                  │
│   sessions/<id>/{session.json, messages.json, planning/, backups/}       │
└──────────────────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `main` | CLI parse, init logger, load config, register providers, launch TUI | `cmd/m31a/main.go` |
| `AppState` | Root Bubble Tea model; screen router; wires every component | `internal/tui/app.go` |
| `ReplModel` | Chat REPL: textarea, viewport, spinner, streaming, file refs, history, shell mode | `internal/tui/repl.go` |
| `KeyRegistry` | Chord-based keybindings (leader key + 1s timeout) | `internal/tui/keybindings.go` |
| `CommandRegistry` | 28 slash commands; context-driven handler dispatch | `internal/tui/commands.go` |
| `Provider Registry` | Thread-safe provider map, active provider pointer | `internal/provider/registry.go` |
| OpenRouter / Zen clients | LLMProvider impls: HTTP, SSE, model cache, health check | `internal/provider/openrouter/client.go`, `internal/provider/zen/client.go` |
| SSE parser | Generic line-by-line Server-Sent Events scanner | `internal/provider/sse.go` |
| `FallbackEvent` | Health-check-based provider failover | `internal/provider/fallback.go` |
| `ModelCache` | Per-provider 5-min TTL + 24h stale window | `internal/provider/cache.go` |
| `Workflow Engine` | Phase dispatch, context pruning, prompt registry, task execution | `internal/workflow/engine.go` |
| Phase files | `initialize`, `discuss`, `plan`, `execute`, `verify`, `ship` | `internal/workflow/{initialize,discuss,plan,execute,verify,ship}.go` |
| `tools.Dispatcher` | Tool registry, permission gate, rule matching, ask modal trigger | `internal/tools/dispatcher.go` |
| Tool impls | Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch, TodoWrite, AskUserQuestion | `internal/tools/*.go` |
| `session.Manager` | Session CRUD, atomic file writes, recent models, fork, siblings | `pkg/session/manager.go` |
| Planning files | `PROJECT.md`, `TASKS.md`, `STATE.md` writer/parser | `pkg/session/planning.go` |
| Checkpoints | Per-phase snapshot for `/undo` | `pkg/session/checkpoint.go` |
| `keychain` | Build-tag OS keychain (Linux D-Bus, macOS, Windows) | `pkg/keychain/keychain_{linux,darwin,windows}.go` |
| `taskrunner.Runner` | Topological sort, sequential group execution, lifecycle hooks | `pkg/taskrunner/runner.go` |
| `autodream` | Context consolidation (summarize oldest 50% messages) | `pkg/autodream/autodream.go` |
| `ledger` | Cross-session append-only markdown, aggregate stats | `pkg/ledger/ledger.go` |
| `rollback` | `git log` chain, soft/hard reset with backup branch | `pkg/rollback/rollback.go` |
| `bisect` | `git bisect` wrapper for regression pinpointing | `pkg/bisect/bisect.go` |
| `arbitrage` | Complexity 0–1 heuristic + cheaper-model suggestion | `pkg/arbitrage/arbitrage.go` |
| `config` | Multi-layer TOML (global → env → project m31a.toml), validation, `${VAR}` substitution | `internal/config/loader.go` |
| `tokens` | tiktoken-go + char÷4×1.3 fallback, EMA correction | `internal/tokens/estimator.go` |
| `git` | `git init`, `commit`, `log`, `diff`, `bisect`, `status`, `reset` | `internal/git/git.go` |
| `log` | `slog` JSON/text, daily rotation, 7-day retention | `internal/log/log.go` |
| `types` | Message, Task, Tool, StreamChunk, HealthStatus, enums | `internal/types/types.go` |
| `errors` | 15 sentinel errors (ErrProviderUnreachable, ErrRateLimited, …) | `internal/errors/errors.go` |
| `theme` | Lipgloss dark/light palette + `Manager.Cycle()` | `internal/tui/theme/theme.go` |
| `components` | Reusable Lipgloss renderers: `MessageRenderer`, `PermissionModal`, `ToolCard`, `ThinkingBlock` | `internal/tui/components/{message,permission,toolcard,thinking,toolrenderers,question}.go` |

## Pattern Overview

**Overall:** Elm-architecture / Bubble Tea `tea.Model` with central state machine + channel-mediated goroutine bridges.

**Key Characteristics:**

- **Single-threaded state machine.** All `AppState` mutations happen in `Update()`. Goroutines never touch `AppState` directly; they emit `tea.Cmd` returning `tea.Msg` values.
- **Provider-agnostic streaming.** `LLMProvider` interface abstracts OpenRouter and OpenCode Zen behind `ChatCompletionStream` → `*StreamIterator`. The TUI only sees `StreamChunk{Type, Delta, ThinkingDuration}`.
- **Tool dispatch with permission gate.** `Dispatcher.Execute` checks rules (allow/deny/ask) and `RiskLevel` before invoking tool implementations. Ask action sends `PermissionRequestMsg` through a channel listener; the modal blocks until a response arrives or 5 minutes elapse.
- **Phase pipeline with pruned context.** `Engine.RunPhase` dispatches to one of six `runXxx` methods. Each phase rebuilds its `[]Message` from the session's planning files — no conversation history leaks between phases.
- **Channel-emitted progress.** `Engine.SetMsgEmitter(channelEmitter{ch: msgCh})` lets task runner callbacks (`OnTaskStart`, `OnTaskUpdate`) push `TaskStartMsg`/`TaskUpdateMsg` into a buffered channel that a `workflowMsgDrainer` `tea.Cmd` reads on every tick.
- **Atomic file persistence.** Every JSON/Markdown write goes through `Session.atomicWrite` (`.m31a_tmp_<hex>` → `os.Rename`) to survive crashes mid-write.
- **Multi-layer config merge.** Defaults → global TOML → `M31A_*` env vars → project `m31a.toml` (walked up 3 levels) → `${VAR}` substitution → validation. Non-zero overlay fields override base.
- **Permission rules with glob matching.** `PermissionRule.Pattern` matches against any string value in `ToolInput.Params` via `doublestar.Match`. Per-agent profiles (e.g. `build`, `plan`) override the global rules via `Dispatcher.SelectAgent`.
- **No-CGO static binary.** All OS keychain work via CLI/D-Bus shims; `CGO_ENABLED=0` in the build tag.

## Layers

**Top Layer — Entry Point (`cmd/`):**
- Purpose: Binary entry, CLI flags, dependency wiring
- Location: `cmd/m31a/`
- Contains: `main()` only — no logic
- Depends on: `internal/{config,log,provider,provider/openrouter,provider/zen,tui,types}` + `pkg/keychain`
- Used by: End user (`m31a` invocation)

**TUI Layer (`internal/tui/`):**
- Purpose: Bubble Tea app, all screens, user interaction
- Location: `internal/tui/`
- Contains: `AppState`, screen models (`ReplModel`, `PlanModel`, `ExecuteModel`, `VerifyModel`, `ShipModel`, `SettingsModel`, `ResumeModel`, `FirstRunModel`, `ModelSelector`, `DiffModel`, `SidebarModel`, `CommandPaletteModel`), `KeyRegistry`, `CommandRegistry`, message types
- Depends on: `internal/{config,errors,git,provider,tokens,tools,types,workflow}` + `pkg/{autodream,keychain,ledger,rollback,session}`
- Used by: `cmd/m31a` only

**TUI Components (`internal/tui/components/`):**
- Purpose: Reusable, theme-aware Lipgloss renderers
- Location: `internal/tui/components/`
- Contains: `MessageRenderer`, `PermissionModal`, `ToolCard`, `ThinkingBlock`, `Question` (modal)
- Depends on: `internal/{tools,types}` + `internal/tui/theme`
- Used by: TUI layer

**Workflow Layer (`internal/workflow/`):**
- Purpose: Six-phase orchestrator with prompt templates, JSON task parsing, tool-call extraction, project-type-aware verification
- Location: `internal/workflow/`
- Contains: `Engine`, `PhaseResult`, per-phase `runXxx` methods, `PromptRegistry` (loaded via `//go:embed prompts/*.md`)
- Depends on: `internal/{git,provider,tokens,tools,types}` + `pkg/{session,taskrunner}`
- Used by: TUI layer (via `AppState.workflowEngine`)

**Provider Layer (`internal/provider/`):**
- Purpose: LLM provider abstraction, SSE parsing, model caching, health checking, auto-fallback
- Location: `internal/provider/`
- Contains: `LLMProvider` interface, `Registry`, `SSEParser`, `ModelCache`, `FallbackEvent`, `ChatRequest`, `ToolDefinition`; sub-packages `openrouter/` and `zen/` with `Client` impls
- Depends on: `internal/{errors,types}`
- Used by: TUI layer, workflow layer

**Tool Layer (`internal/tools/`):**
- Purpose: Tool implementations + permission-aware dispatcher
- Location: `internal/tools/`
- Contains: `Dispatcher`, `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `TodoWrite`, `AskUserQuestion`, plus build-tag split (`bash_unix.go` / `bash_windows.go`)
- Depends on: `internal/{config,errors,types}`
- Used by: TUI layer (slash commands), workflow layer (task execution)

**Public Packages (`pkg/`):**
- Purpose: Reusable, importable components with no TUI or workflow dependencies
- Location: `pkg/`
- Contains:
  - `pkg/session` — session + planning + checkpoint + fork/sibling navigation
  - `pkg/taskrunner` — topological sort + group execution + lifecycle hooks
  - `pkg/keychain` — platform keychain via build tags
  - `pkg/ledger` — append-only learning ledger with aggregate stats
  - `pkg/rollback` — git commit chain browser + soft/hard reset
  - `pkg/bisect` — `git bisect` wrapper
  - `pkg/arbitrage` — model cost optimization
  - `pkg/autodream` — context consolidation
- Depends on: `internal/{errors,types}` only
- Used by: TUI layer, workflow layer

**Foundation Layer (`internal/{config,log,tokens,git,types,errors}/`):**
- Purpose: Leaf packages with no M31A-internal dependencies
- Location: `internal/{config,log,tokens,git,types,errors}/`
- Contains: Config, slog logger, token estimator, git wrapper, core types, sentinel errors
- Depends on: stdlib + 3rd party (`BurntSushi/toml`, `pkoukk/tiktoken-go`)
- Used by: All other internal layers

**State Store (`~/.m31a/`):**
- Purpose: Cross-session persistence
- Location: `~/.m31a/` (user home, configurable via `M31A_CONFIG`)
- Contains: `config.toml`, `m31a.log`, `LEDGER.md`, `recent_models.json`, `sessions/<id>/{session.json,messages.json,planning/,backups/,checkpoints/}`
- Written by: `pkg/session` (sessions + planning), `pkg/ledger`, `internal/config`, `internal/log`
- Read by: All packages via `Session.Load*`, `Manager.Load*`

## Data Flow

### Primary Request Path (REPL message → provider → tool dispatch → response)

1. **User input** — `ReplModel.Update` (textarea submit) emits `submitMsg` (`internal/tui/repl.go`).
2. **Submit handler** — `ReplModel` builds a `ChatRequest` from `messages` history, attaches `Tools = buildToolDefinitions()`, calls `registry.ActiveProvider().ChatCompletionStream(ctx, req)` (`internal/tui/streaming.go`).
3. **HTTP / SSE** — `openrouter.Client` (or `zen.Client`) `POST`s to the provider base URL with `stream: true`; reads the response body with `SSEParser` (`internal/provider/sse.go`) which yields `eventType` + `data` per scan line.
4. **Reasoning normalization** — `internal/provider/reasoning.go` collapses pre-content vs. interleaved thinking into a single `StreamChunk` stream tagged `Type: "thinking"` or `Type: "content"`.
5. **Channel to Bubble Tea** — A goroutine sends each `StreamChunk` through `streamCh`; `streamNextCmd` reads and returns a `tea.Msg` per chunk.
6. **Re-render** — `ReplModel.Update` appends deltas to the active message; `View()` recomputes only the dirty message bubble. The 60 fps frame budget is held by `viewport.Model` from `charmbracelet/bubbles`.
7. **Tool dispatch** — When a `StreamChunk` carries a `tool_calls` payload, the engine (workflow) or REPL (ad-hoc) calls `Dispatcher.Execute(ctx, call)`. The dispatcher checks `rules` (allow/deny/ask), and if `ask` it pushes a `PermissionRequest` to `requestCh`. A listener `tea.Cmd` (`permissionListenerCmd`) blocks on the channel and emits `PermissionRequestMsg` to `AppState`.
8. **Permission modal** — `AppState` switches `screen = ScreenPermission`, spawns a `PermissionModal` with 5-min countdown, and arms a `PermissionTickMsg` ticker. The user presses `Y`/`A`/`N`/`E`; `ApprovePermission` unblocks the dispatcher.
9. **Tool result** — On approval, the dispatcher calls `tool.Execute(ctx, input)`. The `ToolResult` is appended to the message history and re-streamed back to the LLM on the next turn.

### Workflow Engine Path (`/workflow <goal>` or `/phase <name>`)

1. **Slash command** — `ReplModel` emits `SlashCommandMsg{Command: "/workflow ..."}` (`internal/tui/repl.go`).
2. **AppState intercept** — `app.go:Update` detects `/workflow` or `/phase <name>`, sets `workflowGoal`, `workflowRunning = true`, and calls `RunPhaseCmd(app, phase, goal)` (`internal/tui/app.go:322`).
3. **RunPhaseCmd** — Creates a buffered `chan tea.Msg` (capacity 64), wires `Engine.SetMsgEmitter(channelEmitter{ch: msgCh})`, and returns `tea.Batch(runner, workflowMsgDrainer(app))` so Bubble Tea runs both in parallel.
4. **Engine.RunPhase** — `internal/workflow/engine.go:141` dispatches to `runInitialize` / `runDiscuss` / `runPlan` / `runExecute` / `runVerify` / `runShip`. Each rebuilds context from `planning/*.md` files via `pkg/session` and streams a chat completion through `streamLLM`.
5. **Phase transitions** — On success, the TUI's `PhaseResultMsg` handler (`app.go:1004`) auto-advances: `initialize` → `discuss` → `plan` → `execute` → `verify` → `ship`. Each transition spawns a new `RunPhaseCmd` with a fresh context.
6. **Task execution** — `runExecute` (`internal/workflow/execute.go`) creates a `taskrunner.Runner` with the plan's tasks, calls `Schedule()` to get execution groups, and iterates `runner.ExecuteGroup(group, execFn)`. The `OnTaskStart` / `OnTaskUpdate` callbacks `e.emit(...)` `TaskStartMsg` / `TaskUpdateMsg` into the channel.
7. **Drain loop** — `workflowMsgDrainer(app)` returns a `tea.Cmd` that reads one message at a time and returns it. `AppState.Update` re-schedules the drainer on every workflow-related message.
8. **Verify & ship** — `runVerify` runs `Engine.verifyTask` per task (project-type aware: `go build`, `npm run build`, `python3 -m py_compile`, `cargo check`); `runShip` runs `git log` and writes `LEDGER.md` via `pkg/ledger`.

### Permission Request Path

1. Tool call → `Dispatcher.Execute` (`internal/tools/dispatcher.go:71`)
2. `checkPermission` evaluates `rules` then agent default then `RiskLevel` fallback
3. If `ask` action matches, `requestCh <- req` (buffered cap 8) — non-blocking send, errors if full
4. `permissionListenerCmd` blocks on `<-dispatcher.RequestCh()` and returns `PermissionRequestMsg`
5. `AppState.Update` switches to `ScreenPermission`, creates `PermissionModal`, arms `PermissionTickMsg` ticker (100 ms)
6. User presses key → `ApprovePermission` writes to `responseCh` → dispatcher unblocks and either runs the tool or returns `ErrPermissionDenied`

**State Management:**
- `AppState.screen` is a `Screen` enum (`internal/tui/types.go:10`); all rendering routes through it
- Per-screen models are independent `*Model` structs, not sub-`tea.Model`s — Bubble Tea's root model holds all of them and routes messages
- Workflow state is split: `AppState.currentPhase` for the TUI's view; `session.WorkflowPhase` for disk persistence
- Permission and question requests are blocking channels consumed by listener `tea.Cmd`s

## Key Abstractions

**`LLMProvider` interface** (`internal/provider/interface.go`):
- Purpose: Single contract for any LLM gateway (OpenRouter, OpenCode Zen, future providers)
- Examples: `openrouter.Client`, `zen.Client` — both satisfy the interface (`var _ provider.LLMProvider = (*Client)(nil)`)
- Pattern: Compile-time interface assertion; `Registry` stores by name string; `ActiveProvider()` returns the interface

**`Tool` interface** (`internal/types/types.go:112`):
- Purpose: Uniform contract for any executable tool
- Examples: `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `TodoWrite`, `AskUserQuestion`
- Pattern: Each tool exposes `Name() / Description() / RiskLevel() / Execute(ctx, input) (ToolResult, error)`; `Dispatcher.Register(tool)` adds to a `map[string]types.Tool`

**`Dispatcher`** (`internal/tools/dispatcher.go`):
- Purpose: Centralized tool routing with permission gate
- Holds: `map[string]types.Tool`, `map[string]bool` remembered permissions, `[]PermissionRule`, `map[string]PermissionsAgentConfig` per-agent profiles, request/response channels
- Pattern: One instance per TUI; workflow calls `Execute(ctx, call)` per tool invocation

**`workflow.Engine`** (`internal/workflow/engine.go`):
- Purpose: Drives the six-phase pipeline
- Holds: session ID, work dir, backup dir, planning dir, provider, model ID, git, dispatcher, token estimator, session manager, prompt registry, msg emitter, `execCommand` injection point
- Pattern: Long-lived; `RunPhase(ctx, phase, goal)` is the single entry; `SetMsgEmitter` and `SetGit` configure collaborators

**`pkg/session.Manager`** (`pkg/session/manager.go`):
- Purpose: Session lifecycle and atomic file persistence
- Holds: base dir, ID bytes count, max recent models
- Pattern: `NewSession / LoadSession / SaveSession / ListSessions / DeleteSession / ArchiveSession / ForkSession / SiblingSessions`

**`Registry`** (`internal/provider/registry.go`):
- Purpose: Multi-provider name-keyed lookup
- Pattern: `sync.RWMutex`-protected map; `SetActive` validates membership; `ActiveProvider()` returns the interface for direct use

**`KeyRegistry`** (`internal/tui/keybindings.go`):
- Purpose: Vim-style leader-key chord dispatch
- Holds: per-context `[]KeyBinding`, leader state, `time.Timer` for leader timeout
- Pattern: `Handle(key, ctx)` returns `(handled, cmd)`; supports `ctrl+x b` style two-key chords with 1-second default timeout

**`MsgEmitter` interface** (`internal/workflow/engine.go:29`):
- Purpose: Non-blocking goroutine → TUI bridge
- Impl: `channelEmitter{ch: msgCh}` writes to a buffered channel that `workflowMsgDrainer` reads on each tick
- Pattern: Used by `taskrunner.Runner.OnTaskStart` / `OnTaskUpdate` callbacks to push `TaskStartMsg` / `TaskUpdateMsg`

**`Keychain` interface** (`pkg/keychain/keychain.go`):
- Purpose: OS-agnostic secret storage
- Build tags: `keychain_linux.go` (D-Bus Secret Service + `pass` fallback), `keychain_darwin.go` (`/usr/bin/security`), `keychain_windows.go` (Credential Manager stub)
- Pattern: Each file's `init()` sets `newFunc = newXxxKeychain`; `keychain.New()` dispatches

## Entry Points

**`m31a` binary** — `cmd/m31a/main.go:24`:
- Triggers: User runs `m31a` (or `m31a --version`, `m31a --help`)
- Responsibilities: Parse flags, init logger, resolve config path (`$M31A_CONFIG` or `~/.m31a/config.toml`), `config.Load`, `keychain.New` (graceful on failure), `provider.NewRegistry` + register clients, `tui.NewApp`, `tea.NewProgram(app, tea.WithAltScreen()).Run()`

**TUI REPL** — `internal/tui/app.go:97`:
- Triggers: App boot, after `FirstRunModel` completes
- Responsibilities: Chat loop, slash command dispatch, streaming, tool card rendering, status bar, header

**Slash commands** — `internal/tui/commands.go:159` `DefaultCommands`:
- Triggers: User types `/` in REPL; `ParseCommand` splits name + args; `CommandRegistry.Execute` looks up handler
- Responsibilities: 28 commands across session management (`/fork`, `/prev`, `/next`, `/sessions`), model selection (`/model`, `/models`, `/provider`, `/fallback`), workflow (`/workflow`, `/phase`, `/goal`, `/status`), tools (`/tools`, `/compress`, `/rollback`, `/undo`), config (`/config`, `/theme`, `/key`, `/log`, `/tokens`, `/health`), UI (`/help`, `/clear`, `/reset`, `/quit`, `/diff`, `/history`, `/save`, `/settings`, `/ledger`)

**Workflow `/phase <name>`** — `internal/tui/app.go:628`:
- Triggers: `SlashCommandMsg` with `/phase initialize|discuss|plan|execute|verify|ship`
- Responsibilities: Set `workflowGoal`, spawn `RunPhaseCmd`, await `PhaseResultMsg`, auto-advance

**First-run flow** — `internal/tui/firstrun.go`:
- Triggers: `AppState` constructed with empty `apiKey`
- Responsibilities: Welcome → provider select → key input → keychain prompt → transition to `ScreenREPL`

## Architectural Constraints

- **Threading:** Single-threaded Bubble Tea event loop. `AppState` is mutated only in `Update()`. Goroutines (stream reader, health check, workflow runner, permission listener, question listener, cache refresh ticker) emit `tea.Cmd` returning `tea.Msg`. Goroutines may NOT touch `AppState` directly.
- **Global state:** None at the package level. `AppState` is the only mutable singleton, passed by pointer. `theme.Manager` is a per-app value. `Registry` and `Manager` are the only package-level locks (`sync.RWMutex`), both guarding maps.
- **Circular imports:** `internal/types/` and `internal/errors/` are leaf packages with no M31A imports. The `provider` package depends only on `types` and `errors`. No circular chains exist.
- **CGO:** None. `CGO_ENABLED=0` enforced. All OS keychain work via CLI/D-Bus shims; bash tool uses `creack/pty` was removed — `internal/tools/bash_unix.go` is a 27-line build-tag stub, with the actual implementation using `os/exec` and pipes.
- **Telemetry:** None. No analytics, no phone-home. `internal/log` writes only to `~/.m31a/m31a.log`; never stdout during TUI operation.
- **Single binary embed:** Prompt templates at `internal/workflow/prompts/*.md` are compiled into the binary via `//go:embed`. `LoadPrompts` reads them at `Engine.NewEngine` time.
- **Atomic file writes:** All state files written via temp-file + `os.Rename` pattern. Random hex suffix on temp name (`.m31a_tmp_<16hex>`) to avoid collisions.
- **Permission model:** `Dispatcher.requestCh` is buffered to 8; if full, the tool returns `ErrPermissionDenied`. This prevents a stalled modal from blocking the tool goroutine.
- **Stream backpressure:** `channelEmitter` uses non-blocking `select { case ch <- msg: default: }` — if the TUI isn't draining fast enough, messages are dropped (logged via `slog.Warn`).
- **Provider health gating:** Health check skipped if last status indicates rate-limit or offline; interval backed off to 120s in those cases (`calculateNextInterval` in `app.go:1527`).
- **No direct Anthropic/OpenAI clients:** Only OpenRouter and OpenCode Zen via their `LLMProvider` interface (`internal/provider/interface.go`).

## Anti-Patterns

### Direct `AppState` mutation from `tea.Cmd` body

**What happens:** Some command handlers in `app.go` modify `AppState` fields (e.g. `m.activeModel = ...`, `m.fallbackNotification = ...`) directly in the `Update` method — which is the *intended* pattern — but the same code path also schedules a `tea.Cmd` whose body mutates the TUI via a callback that eventually calls into `m.something` from a goroutine.

**Why it's wrong:** Bubble Tea's contract is that the goroutine return is a pure `tea.Msg` value, with all state mutation happening only when the resulting message is processed by `Update`. If the `tea.Cmd` body touches `AppState` directly, the access is unsynchronized and can race with `Update`.

**Do this instead:** Have the goroutine return the `tea.Msg` with the data; mutate `AppState` only inside the `Update` switch case that matches that message type. See `app.go:864` `case FallbackEventMsg:` for the correct pattern.

### Mutex copy

**What happens:** `Dispatcher.mu sync.RWMutex` is embedded by value in the struct. If a `Dispatcher` is ever passed by value, the mutex is copied, breaking synchronization.

**Why it's wrong:** Go's `go vet` flags this with `copylocks`. Embedded mutexes must only be on heap-allocated values passed by pointer.

**Do this instead:** Always pass `*Dispatcher` (pointer). `NewDispatcher` returns `*Dispatcher`; `cmd/m31a/main.go:160` stores it in `AppState.dispatcher` as a pointer.

### Sync write to `requestCh`

**What happens:** `Dispatcher.Execute` uses `select { case d.requestCh <- req: default: return ErrPermissionDenied }` — non-blocking, but does not honor a per-request timeout.

**Why it's wrong:** A stuck modal could (theoretically) cause the request to be dropped if the channel is full when the user has 8 other pending requests; the tool call returns `ErrPermissionDenied` and the workflow marks the task failed.

**Do this instead:** The current non-blocking send is intentional — the alternative would be a blocking send that could deadlock the workflow goroutine. The fix is the 5-minute `PermissionTickMsg` countdown that auto-denies, freeing the channel slot.

## Error Handling

**Strategy:** All sentinel errors live in `internal/errors/errors.go` as `var Err* = errors.New("...")`. Code uses `errors.Is(err, m31errors.ErrXxx)` for comparison. No custom error types; no type assertions on sentinels.

**Patterns:**
- **Provider errors** — HTTP 401 → `ErrInvalidKey`; 402 → `ErrProviderUnreachable`; 429 → `ErrRateLimited`; 503 → `ErrProviderUnreachable`. The TUI displays a styled modal; for 429/503, `StreamErrorMsg` triggers `FindFallbackProvider` and emits `FallbackEventMsg` (`app.go:877`).
- **Workflow errors** — `runExecute` returns `fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, ...)` after exhausting `MaxHealAttempts` retries.
- **Session errors** — Missing/corrupt `session.json` → `ErrSessionCorrupted`. Recovered via `/resume` screen `[!CORRUPT]` badge.
- **Permission errors** — `Dispatcher.Execute` returns `m31errors.ErrPermissionDenied` when rule says `deny` or user clicks `N`. The workflow layer surfaces it as a task failure.
- **Tool errors** — Tool impls populate `ToolResult.Error` string for partial failures; `Execute` returns `(ToolResult, nil)` if recoverable or `(ToolResult, err)` for hard failures.
- **Context errors** — If estimated tokens > 95% of model's context, the request is blocked with `ErrContextExceeded` (also a 60% threshold triggers AutoDream consolidation).

## Cross-Cutting Concerns

**Logging:** `internal/log` uses `log/slog`. Two formatters: `json` (default) and `text` (controlled by `M31A_LOG_FORMAT`). Log file at `~/.m31a/m31a.log` with daily rotation and 7-day retention. **Never writes to stdout/stderr** during TUI operation; stdout is reserved for `--version` and `--help` flags.

**Validation:** Multi-layer config validation in `internal/config/loader.go:validateConfig` — collects all `ValidationError`s and returns them joined. Range checks on floats (0–1), enum values (`"dark"|"light"|"auto"`), non-negative integers.

**Authentication:** API keys resolved in strict order: `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY` env vars → OS keychain via `keychain.Get("openrouter")` / `"zen"` → `config.toml` `provider.openrouter.api_key` field. Config file is the last-resort fallback; `Config.Save` *clears* the API key fields before writing so they never hit disk. `KeychainUnavailable` errors are non-fatal — the resolver falls through to the next tier.

**Persistence:** `pkg/session.Manager.atomicWrite` is the canonical write path: temp file with random `.m31a_tmp_<16hex>` name in the same directory → `os.Rename` for atomicity → `os.Remove` on any intermediate failure. Used for `session.json`, `messages.json`, `PROJECT.md`, `TASKS.md`, `STATE.md`, `LEDGER.md`, `recent_models.json`, and `config.toml`.

**Context pruning:** Each workflow phase discards prior conversation and rebuilds its `[]Message` from `~/.m31a/sessions/<id>/planning/*.md`. The `Engine` exposes `buildExecuteContext(task, allTasks)` which produces a task-specific slice containing `system_prompt + TASKS.md + PROJECT.md + task spec + (optional) file contents`.

**Theme:** `theme.Manager` holds the active `Theme` (Lipgloss color values). On `ThemeChangedMsg`, the manager is replaced; all sub-models re-render with the new palette. `/theme` and `ctrl+x t` cycle between `dark` → `light` → `auto` (auto-detect via `termenv`).

**AutoDream:** `pkg/autodream.Consolidator` runs in response to `/compress` (manual) or when context usage exceeds 60% (auto). It summarizes the oldest 50% of non-protected messages into a single `Summary` segment. **Never runs during tool execution** — checks `Consolidator.paused` flag.

---

*Architecture analysis: 2026-06-02*
