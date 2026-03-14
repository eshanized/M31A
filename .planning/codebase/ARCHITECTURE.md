# Architecture

**Analysis Date:** 2026-06-12

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                    CLI Entry Point                          │
│  `cmd/m31a/main.go`                                        │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                    TUI Layer (Bubble Tea)                    │
│  `internal/tui/app.go` - AppState (top-level model)        │
│  `internal/tui/app_state.go` - State management            │
│  `internal/tui/repl.go` - REPL interface                   │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│              Workflow Engine (Orchestrator)                  │
│  `internal/workflow/engine.go` - Phase execution           │
│  `internal/workflow/plan.go` - Planning phase              │
│  `internal/workflow/execute.go` - Execution phase          │
│  `internal/workflow/verify.go` - Verification phase        │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                   Tool System                               │
│  `internal/tools/dispatcher.go` - Tool registry & dispatch │
│  `internal/tools/` - Individual tool implementations       │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Provider Layer                             │
│  `internal/provider/registry.go` - Provider registry       │
│  `internal/provider/interface.go` - LLMProvider interface  │
│  `internal/provider/openrouter/` - OpenRouter implementation│
│  `internal/provider/zen/` - Zen implementation            │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Supporting Packages                        │
│  `pkg/session/` - Session management                      │
│  `pkg/ledger/` - Action history                           │
│  `pkg/rollback/` - Git rollback                           │
│  `pkg/autodream/` - Context consolidation                 │
│  `pkg/keychain/` - OS keychain integration                │
│  `pkg/arbitrage/` - Model cost optimization               │
└─────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| **CLI Entry** | Parse flags, initialize config, create TUI app | `cmd/m31a/main.go` |
| **AppState** | Top-level Bubble Tea model, screen routing, state management | `internal/tui/app_state.go` |
| **Engine** | Orchestrate 6-phase workflow (Initialize → Discuss → Plan → Execute → Verify → Ship) | `internal/workflow/engine.go` |
| **Dispatcher** | Register tools, manage permissions, execute tool calls | `internal/tools/dispatcher.go` |
| **Registry** | Manage LLM providers, select active provider | `internal/provider/registry.go` |
| **Session Manager** | Persist sessions, messages, checkpoints, tasks | `pkg/session/manager.go` |
| **Config Loader** | Multi-layer config merging (defaults → global → env → project) | `internal/config/loader.go` |
| **Git Client** | Git operations for workflow phases | `internal/git/git.go` |

## Pattern Overview

**Overall:** Event-Driven TUI with Pipeline Workflow

**Key Characteristics:**
- Bubble Tea (Elm architecture) for UI: Model → Update → View
- 6-phase linear workflow with state machine transitions
- Tool system with permission gating and risk levels
- Provider abstraction for multiple LLM backends
- Session persistence with checkpoint/resume support

## Layers

**CLI Entry (`cmd/m31a/`):**
- Purpose: Application bootstrap and lifecycle
- Location: `cmd/m31a/main.go`
- Contains: Flag parsing, config loading, dependency injection, signal handling
- Depends on: All internal packages
- Used by: User via terminal

**TUI Layer (`internal/tui/`):**
- Purpose: Terminal user interface
- Location: `internal/tui/`
- Contains: Screen models, views, keybindings, command system
- Depends on: Workflow engine, provider, tools, session
- Used by: User interaction

**Workflow Engine (`internal/workflow/`):**
- Purpose: Orchestrate AI-assisted development phases
- Location: `internal/workflow/`
- Contains: Phase implementations (discuss, plan, execute, verify, ship), prompt templates
- Depends on: Provider, tools, session, git
- Used by: TUI layer

**Tool System (`internal/tools/`):**
- Purpose: Safe execution of file system and shell operations
- Location: `internal/tools/`
- Contains: Tool implementations, permission system, dispatcher
- Depends on: Config, types
- Used by: Workflow engine, LLM via tool calls

**Provider Layer (`internal/provider/`):**
- Purpose: Abstract LLM API interactions
- Location: `internal/provider/`
- Contains: Provider implementations, caching, fallback, resilience
- Depends on: Types, config
- Used by: Workflow engine

**Supporting Packages (`pkg/`):**
- Purpose: Reusable domain logic
- Location: `pkg/`
- Contains: Session, ledger, rollback, autodream, keychain, arbitrage
- Depends on: Internal types, git
- Used by: TUI, workflow engine

## Data Flow

### Primary Request Path (User Input → LLM Response)

1. User types command/message in REPL (`internal/tui/repl.go:Update()`)
2. AppState routes message to appropriate handler (`internal/tui/app_state.go`)
3. Workflow engine processes phase (`internal/workflow/engine.go:RunPhase()`)
4. LLM provider sends chat completion (`internal/provider/registry.go:ChatCompletionStream()`)
5. Response chunks streamed back via MsgEmitter channel
6. TUI renders response in active screen

### Tool Execution Flow

1. LLM returns tool call in response (`internal/workflow/engine.go:consumeStream()`)
2. Dispatcher parses tool call (`internal/tools/dispatcher.go:Execute()`)
3. Permission gate checks risk level (`internal/tools/dispatcher.go:ensurePermission()`)
4. Tool executes operation (`internal/tools/*.go:Execute()`)
5. Result returned to LLM for next iteration

**State Management:**
- Bubble Tea single-threaded: All mutations in `Update()` method
- AppState holds all mutable state (`internal/tui/app_state.go`)
- Workflow state persisted via Session Manager (`pkg/session/manager.go`)
- Config hot-reload via file watching (`internal/config/loader.go:WatchConfig()`)

## Key Abstractions

**WorkflowPhase:**
- Purpose: Represents a stage in the development workflow
- Examples: `PhaseInitialize`, `PhaseDiscuss`, `PhasePlan`, `PhaseExecute`, `PhaseVerify`, `PhaseShip`
- Pattern: State machine with valid transitions (`internal/workflow/engine.go:validPhaseTransitions`)

**Tool:**
- Purpose: Safe execution of operations with risk assessment
- Examples: `bash.go`, `filewrite.go`, `grep.go`, `glob.go`
- Pattern: Interface-based with risk levels and permission gating (`internal/types/types.go:Tool`)

**LLMProvider:**
- Purpose: Abstract LLM API interactions
- Examples: OpenRouter, Zen
- Pattern: Interface with streaming support (`internal/provider/interface.go:LLMProvider`)

**Session:**
- Purpose: Persist workflow state across restarts
- Examples: Checkpoints, messages, tasks
- Pattern: File-based storage with JSON serialization (`pkg/session/manager.go`)

## Entry Points

**Main Entry:**
- Location: `cmd/m31a/main.go`
- Triggers: User runs `m31a` binary
- Responsibilities: Initialize config, create TUI app, handle signals, run event loop

**Workflow Start:**
- Location: `internal/tui/app.go:RunPhaseCmd()`
- Triggers: User command or agent mode
- Responsibilities: Create workflow engine context, execute phase, emit results

**Tool Execution:**
- Location: `internal/tools/dispatcher.go:Execute()`
- Triggers: LLM tool call in response
- Responsibilities: Parse input, check permissions, execute tool, return result

## Architectural Constraints

- **Threading:** Bubble Tea single-threaded event loop; all state mutations in `Update()`. Background goroutines communicate via `tea.Cmd` channels.
- **Global state:** No module-level singletons. All state flows through `AppState` struct.
- **Circular imports:** None detected. Clean dependency hierarchy: `cmd` → `tui` → `workflow` → `tools`/`provider` → `types`.
- **Permission system:** Tools with risk level ≥ "dangerous" require user approval via modal. Rules evaluated in order; first match wins.

## Error Handling

**Strategy:** Typed errors with sentinel values for known conditions.

**Patterns:**
- Sentinel errors: `m31errors.ErrPermissionDenied`, `m31errors.ErrContextExceeded`, `m31errors.ErrPhaseTransition`
- Error wrapping: `fmt.Errorf("context: %w", err)` for stack traces
- Tool errors: Return `ToolResult` with `Error` field for LLM consumption
- User-facing errors: `errors.UserMessage()` for TUI display

## Cross-Cutting Concerns

**Logging:** Structured logging via `log/slog` with configurable levels. Logger initialized in `cmd/m31a/main.go`.

**Validation:** Config validation in `internal/config/loader.go:validateConfig()`. Tool input validation in dispatcher.

**Authentication:** API keys resolved via priority chain: env vars → OS keychain → config file (`internal/config/loader.go:ResolveAPIKeys()`).

---

*Architecture analysis: 2026-06-12*
