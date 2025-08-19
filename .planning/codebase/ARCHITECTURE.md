<!-- refreshed: [YYYY-MM-DD] -->
# Architecture

**Analysis Date:** [YYYY-MM-DD]

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                          TUI Layer                          │
├──────────────────┬──────────────────┬───────────────────────┤
│    [App State]   │    [Components]  │       [Screens]       │
│ `internal/tui/`  │ `internal/tui/`  │    `internal/tui/`    │
└────────┬─────────┴────────┬─────────┴──────────┬────────────┘
         │                  │                    │
         ▼                  ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│                      Workflow Engine                        │
│                 `internal/workflow/`                        │
└────────┬──────────────────┬────────────────────┬────────────┘
         │                  │                    │
         ▼                  ▼                    ▼
┌──────────────────┬──────────────────┬───────────────────────┐
│     [Tools]      │   [LLM Provider] │   [Session/State]     │
│`internal/tools/` │`internal/provider` `pkg/session/`        │
└──────────────────┴──────────────────┴───────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| AppState | Orchestrates TUI models, manages BubbleTea event loop and holds global state | `internal/tui/app.go` |
| Engine | Orchestrates the 6-phase LLM workflow (Initialize, Discuss, Plan, Execute, Verify, Ship) | `internal/workflow/engine.go` |
| Dispatcher | Securely executes local operations (Bash, File I/O, Git) on behalf of LLM tools | `internal/tools/dispatcher.go` |
| LLMProvider | Interface standardizing Chat completions, streaming, and tool schemas | `internal/provider/interface.go` |
| Manager (Session) | Manages checkpointing, state persistence, and restoring past sessions | `pkg/session/manager.go` |

## Pattern Overview

**Overall:** Event-Driven TUI with State Machine Workflow

**Key Characteristics:**
- **BubbleTea Architecture:** The UI is completely driven by an event loop sending `tea.Msg` to update models.
- **Phased Agentic Workflow:** LLM interactions are strictly segmented into phases to prevent context drift and ensure verifiable steps.
- **Interface-based Abstraction:** Providers (`internal/provider`) and Tools (`internal/tools`) are hidden behind interfaces, allowing easy swapping.

## Layers

**TUI Layer:**
- Purpose: Provides the rich terminal interface for user interaction and visualizes workflow progress.
- Location: `internal/tui/`
- Contains: BubbleTea components, models (REPL, Settings, FirstRun, etc.).
- Depends on: Workflow, Provider, Session, Git.
- Used by: CLI Entrypoint (`cmd/m31a/main.go`).

**Workflow Layer:**
- Purpose: Manages structured multi-step LLM task execution.
- Location: `internal/workflow/`
- Contains: Phase transition logic, prompt definitions, workflow engine.
- Depends on: Tools, Provider, Session.
- Used by: TUI Layer.

**Provider Layer:**
- Purpose: Connects to external LLM services (OpenRouter, Zen).
- Location: `internal/provider/`
- Contains: API clients, response streaming iterators, registries.
- Depends on: Configuration, Types.
- Used by: Workflow Layer, TUI Layer (for direct REPL usage).

**Tools Layer:**
- Purpose: Executes system operations explicitly authorized by LLMs or Users.
- Location: `internal/tools/`
- Contains: Command dispatching, Unix/Windows Bash handlers, permission management.
- Depends on: OS, exec.
- Used by: Workflow Layer, TUI Layer.

## Data Flow

### Primary Request Path (Workflow Phase Execution)

1. TUI dispatches a run command via `RunPhaseCmd` (`internal/tui/app.go`)
2. `workflow.Engine` builds prompts, context, and tool schemas (`internal/workflow/engine.go`)
3. `Engine` requests streaming completion from `provider.LLMProvider` (`internal/workflow/engine.go`)
4. Provider streams chunks back, `Engine` emits `TaskUpdateMsg` to TUI via `channelEmitter` (`internal/tui/app.go`)
5. If the LLM requests a tool call, `Engine` delegates to `tools.Dispatcher` (`internal/tools/dispatcher.go`)

### TUI State Update Flow

1. User interaction generates a BubbleTea message (`tea.KeyMsg`)
2. Main `Update` method processes the message and updates relevant Sub-Model (`internal/tui/app.go`)
3. Sub-Model returns an updated model and a `tea.Cmd`
4. Global UI is re-rendered via the `View` method (`internal/tui/app.go`)

## Key Abstractions

**LLMProvider:**
- Purpose: Standardizes interactions with different remote models.
- Examples: `internal/provider/interface.go`, `internal/provider/openrouter/`
- Pattern: Adapter Pattern with a Registry (`internal/provider/registry.go`).

**MsgEmitter:**
- Purpose: Decouples the headless workflow engine from the BubbleTea event loop.
- Examples: `internal/workflow/workflow.go`, implemented by `channelEmitter` in `internal/tui/app.go`.
- Pattern: Observer / Event Bus.

## Entry Points

**Main Executable:**
- Location: `cmd/m31a/main.go`
- Triggers: User executing the `m31a` binary.
- Responsibilities: Parses CLI flags, sets up logging, loads config, initializes Providers and tools, and launches the TUI.

## Architectural Constraints

- **Threading:** Uses Go routines for background LLM requests and file operations, but UI state mutations must only happen sequentially through the BubbleTea `Update` function.
- **Global state:** Avoided; state is encapsulated within `AppState` and passed down. `slog.SetDefault` is used for global logging.
- **Security / Permissions:** All tool actions (especially file writes and bash commands) must go through the `tools.Dispatcher` to intercept and manage user permissions.

## Anti-Patterns

### Blocking the TUI Thread

**What happens:** Making direct, synchronous network calls or heavy I/O within the `Update` or `View` methods of BubbleTea models.
**Why it's wrong:** Freezes the UI, breaking animations, input handling, and terminal resizing.
**Do this instead:** Wrap the work in a `tea.Cmd` and return a message upon completion, as seen with `RunPhaseCmd` in `internal/tui/app.go`.

### Hardcoded Provider Logic in Workflow

**What happens:** Adding OpenRouter or Zen-specific logic inside `internal/workflow/engine.go`.
**Why it's wrong:** Breaks the provider abstraction, making it hard to add new API providers later.
**Do this instead:** Define necessary behaviors in `provider.LLMProvider` (`internal/provider/interface.go`).

## Error Handling

**Strategy:** Typed and structured errors, propagated to the user interface via dedicated messages.

**Patterns:**
- Errors from phases are returned as part of a structured `PhaseResultMsg` (`internal/tui/app.go`).
- Internal module errors define their own types or use `fmt.Errorf` wrapping (`internal/errors/errors.go`).

## Cross-Cutting Concerns

**Logging:** Handled by `log/slog` structured logging, initialized in `main.go`.
**Validation:** Config structures are validated upon loading (`internal/config/loader.go`).
**Security:** Managed by `pkg/keychain` for secrets and `tools.Dispatcher` for runtime operation permissions.

---

*Architecture analysis: [YYYY-MM-DD]*