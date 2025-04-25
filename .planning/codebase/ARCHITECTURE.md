---
focus: arch
last_mapped: 2026-05-30
---

# M31A — Architecture

## Pattern: Model-View-Update (Bubble Tea)

The entire application follows the Bubble Tea MVU pattern:

- **Model** (`AppState` in `internal/tui/app.go`): holds all state — current screen, provider, models, messages, workflow state, health status, permissions
- **Update** (`AppState.Update()`): single entry point for all state mutations. Receives typed messages (`tea.Msg`) and returns new model + commands.
- **View** (`AppState.View()`): renders the current screen from immutable state. Must complete in <16ms.

## Package Layers

```
cmd/m31a/             — binary entry point (parses flags, inits deps, launches TUI)
  │
  ├── internal/log/   — structured logger (slog, file rotation)
  │
  └── internal/tui/   — Bubble Tea app, all screens
        │
        ├── internal/provider/  — LLMProvider interface + OpenRouter + Zen clients
        │     └── pkg/session/  — session lifecycle, file persistence
        │
        ├── internal/workflow/  — six-phase workflow engine
        │     ├── pkg/taskrunner/  — dependency graph, topological sort
        │     ├── pkg/bisect/      — git bisect wrapper
        │     ├── pkg/autodream/   — context consolidation
        │     └── pkg/session/
        │
        ├── internal/tools/   — 5 core tools (Bash, FileRead, FileWrite, Glob, Grep)
        │
        ├── pkg/arbitrage/    — model cost comparison + complexity scoring
        ├── pkg/ledger/       — cross-session learning ledger
        ├── pkg/rollback/     — commit chain browser
        │
        ├── internal/config/  — TOML config parsing + keychain resolution
        │     └── pkg/keychain/  — OS keychain backends
        │
        └── internal/types/   — shared core types (leaf package, zero internal imports)
```

## Data Flow: TUI App Lifecycle

```
main.go
  │  parses flags, loads config, creates providers
  v
tui.NewApp()  ← initializes all sub-models (REPL, settings, resume, workflow engine)
  │
  v
AppState.Init()  ← starts health ticker, cache refresher
  │
  v
tea.NewProgram(app, tea.WithAltScreen()).Run()
  │
  ├── Update() loop  ← receives keys, stream chunks, tool results, phase results
  │     │
  │     └── Tea.Batch(cmds...)  ← dispatches next commands
  │
  └── View()  ← renders based on active screen (Switch on screen type)
```

## Data Flow: Streaming Chat

```
User types message → REPL Update
  │  creates ChatRequest with message history
  v
LLMProvider.ChatCompletionStream(ctx, req)
  │  POST /chat/completions with stream:true
  v
SSE stream → StreamIterator.Next()
  │  yields StreamChunk{Type: "content"|"thinking"|"done"}
  v
StreamChunk dispatched as tea.Cmd → tea.Msg
  │  message progressively appended to ReplModel
  v
View() renders token-by-token via Glamour
```

## Data Flow: Workflow Engine

```
/phase <name> or /workflow <goal>
  │
  v
AppState.RunPhaseCmd() → engine.RunPhase()
  │
  ├── PhaseInitialize: create session, detect project type, init git
  ├── PhaseDiscuss:    generate clarifying questions, capture answers
  ├── PhasePlan:       LLM generates task list → validate → serialize to TASKS.md
  ├── PhaseExecute:    for each task group (sequential V1): LLM → tool calls → git commit
  ├── PhaseVerify:     file existence, syntax check, test execution, self-heal loop
  └── PhaseShip:       final commit, update ledger, archive session
```

## Screen Routing

AppState holds a `screen` field (enum `Screen`). `Update()` and `View()` switch on it:

| Screen | Model | Purpose |
|--------|-------|---------|
| `ScreenFirstRun` | `FirstRunModel` | API key setup on first launch |
| `ScreenREPL` | `ReplModel` | Main chat interface |
| `ScreenSettings` | `SettingsModel` | 6-tab config editor |
| `ScreenResume` | `ResumeModel` | Session browser/loader |
| `ScreenPermission` | `PermissionModal` | Approval modal for dangerous tools |
| `ScreenModelSelector` | `ModelSelector` | Full-screen model browser with fuzzy search |
| `ScreenPlan` | `PlanModel` | Task list with cost/time panel |
| `ScreenExecute` | `ExecuteModel` | Task progress with live tool cards |
| `ScreenVerify` | `VerifyModel` | Pass/fail checklist per task |
| `ScreenShip` | `ShipModel` | Summary banner, session archive |

## Key Architecture Decisions

1. **Context pruning**: Each workflow phase discards prior conversation. Reads only structured state files (PROJECT.md, TASKS.md, STATE.md).
2. **Sequential execution (V1)**: Tasks execute one at a time within dependency groups. No concurrency.
3. **Atomic file writes**: All file writes use temp-file-then-rename pattern (`internal/config/loader.go:atomicWrite`).
4. **Sentinel errors**: All error constants in `internal/errors/errors.go`. Compared with `errors.Is()`.
5. **No CGO**: Build constraint `CGO_ENABLED=0` enforced everywhere.
