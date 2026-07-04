# Architecture

**Analysis Date:** 2026-07-04

## Pattern Overview

**Overall:** Elm Architecture (Model-Update-View) with a 7-phase Workflow Engine

**Key Characteristics:**
- Bubble Tea TUI follows strict Elm architecture: all state mutations flow through `Update()` only; never mutate `AppState` from goroutines
- Workflow engine orchestrates a 7-phase state machine: Initialize -> Discuss -> Plan -> Execute -> Verify -> Runtime -> Ship
- Provider abstraction layer supports multiple LLM backends (OpenRouter, Zen, Nvidia) with dynamic model discovery
- Tool dispatcher enforces permissions, rate limiting, and concurrency for 18+ built-in tools
- Strict dependency boundary: `pkg/` must NOT import `internal/`
- `internal/types` serves as the shared type vocabulary across all layers

## Layers

**Entry Point (`cmd/m31a/`):**
- Purpose: CLI flag parsing, config load, provider registration, TUI construction, signal handling
- Location: `cmd/m31a/main.go`
- Contains: `main()`, `run()`, `runHeadless()`, `runHeadlessWorkflow()`, `restoreTerminal()`
- Depends on: `internal/config`, `internal/git`, `internal/log`, `internal/provider`, `internal/tools`, `internal/tui`, `internal/types`, `pkg/keychain`, `pkg/ledger`, `pkg/rollback`, `pkg/session`, `pkg/autodream`
- Used by: OS process

**Configuration (`internal/config/`):**
- Purpose: Load and merge TOML config from `~/.m31a/config.toml` with project-level overrides, resolve API keys from keychain
- Location: `internal/config/`
- Contains: `Config` struct, `Load()`, `LoadDotEnv()`, `Merge()`, provider/model/UI/permissions/agents/tools/git/verify/compaction config types
- Depends on: `internal/types`, `pkg/keychain`
- Used by: `cmd/m31a/main.go`, `internal/tui/`, `internal/workflow/`, `internal/tools/`

**Provider Layer (`internal/provider/`):**
- Purpose: Abstract LLM API communication behind `LLMProvider` interface; handle streaming, caching, health checks, fallback
- Location: `internal/provider/`
- Contains: `LLMProvider` interface, `Registry`, `BaseClient`, per-provider implementations (`openrouter/`, `zen/`, `nvidia/`), `ModelCache`, SSE streaming, capability detection
- Depends on: `internal/types`, `internal/errors`
- Used by: `internal/workflow/`, `internal/tui/`, `cmd/m31a/`

**Tools Layer (`internal/tools/`):**
- Purpose: Provide 18+ built-in tools (bash, file ops, search, web, etc.) with dispatcher that handles permissions, rate limiting, concurrency, output bounding
- Location: `internal/tools/`
- Contains: `Dispatcher`, tool implementations (`bash.go`, `edit.go`, `filewrite.go`, `grep.go`, `glob.go`, `webfetch.go`, `websearch.go`, etc.), `permissions.go`, `concurrency.go`, `OutputStore`
- Depends on: `internal/types`, `internal/config`, `internal/errors`, `pkg/metrics`
- Used by: `internal/workflow/`, `internal/tui/`

**Subagent System (`internal/tools/subagent/`):**
- Purpose: Orchestrate parallel child agent lifecycles with git worktree isolation, per-agent dispatchers, event channels
- Location: `internal/tools/subagent/`
- Contains: `Manager`, `Subagent`, `SubagentEvent`, agent profiles, worktree operations, event loop parsing
- Depends on: `internal/config`, `internal/provider`, `internal/types`
- Used by: `internal/tools/agent.go`, `internal/tui/`

**Workflow Engine (`internal/workflow/`):**
- Purpose: Orchestrate the 7-phase workflow lifecycle: Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship
- Location: `internal/workflow/`
- Contains: `Engine`, `StateMachine`, `PhaseCoordinator`, `WorkflowState`, `PromptBuilder`, `ContextBuilder`, `CostTracker`, per-phase logic (`initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go`), intent classification, self-healing, plan parsing
- Depends on: `internal/provider`, `internal/tools`, `internal/config`, `internal/types`, `internal/git`, `internal/codeintel`, `internal/tokens`, `internal/decision`, `internal/context`, `pkg/compaction`, `pkg/ledger`, `pkg/metrics`, `pkg/retry`, `pkg/session`
- Used by: `internal/tui/`

**TUI Layer (`internal/tui/`):**
- Purpose: Full-screen terminal UI following Bubble Tea Elm architecture; screen routing, REPL, sidebar, permissions modal, streaming
- Location: `internal/tui/`
- Contains: `AppState` (top-level tea.Model), `Init()`/`Update()`/`View()`, screen-specific models (`repl_model.go`, `plan_model.go`, `execute_model.go`, etc.), command registry, key bindings, handlers split by concern (`handler_*.go`, `app_handlers*.go`)
- Depends on: `internal/config`, `internal/provider`, `internal/tools`, `internal/workflow`, `internal/types`, `internal/tui/components`, `internal/tui/layout`, `internal/tui/theme`, `pkg/narrative`, `pkg/arbitrage`, `pkg/metrics`, `pkg/history`, `pkg/keychain`, `pkg/ledger`, `pkg/rollback`, `pkg/session`
- Used by: `cmd/m31a/`

**TUI Components (`internal/tui/components/`):**
- Purpose: Reusable UI building blocks (cards, badges, spinners, permissions modal, file trees, etc.)
- Location: `internal/tui/components/`
- Contains: `PermissionModal`, `QuestionModel`, `Badge`, `Card`, `Spinner`, `GlamourCache`, `FileTree`, `EmptyState`, `VirtualViewport`, etc.
- Depends on: `internal/tui/theme`, `internal/tui/layout`
- Used by: `internal/tui/`

**TUI Theme (`internal/tui/theme/`):**
- Purpose: Design token system with light/dark/auto modes, accent colors, cached style lookups
- Location: `internal/tui/theme/`
- Contains: `Theme`, `Manager`, token types (colors, typography, spacing, elevation, motion, semantic), `Cache`, registry
- Depends on: `github.com/charmbracelet/lipgloss`
- Used by: `internal/tui/`, `internal/tui/components/`

**TUI Layout (`internal/tui/layout/`):**
- Purpose: Responsive layout system with Box/Stack/Page abstractions, minimum screen enforcement, constraint solvers
- Location: `internal/tui/layout/`
- Contains: `Box`, `Stack`, `PageChrome`, `RenderPage()`, constraint system, responsive helpers, minimum screen
- Depends on: `internal/tui/theme`
- Used by: `internal/tui/`, `internal/tui/components/`

**Shared Types (`internal/types/`):**
- Purpose: Common type vocabulary used across all layers (Message, ToolCall, Task, ModelInfo, WorkflowPhase, RiskLevel, etc.)
- Location: `internal/types/`
- Contains: Core domain types, constants, Tool interface, WorkflowPhase enum, RiskLevel enum, IntentType/IntentResult, Task, Session, StreamChunk
- Depends on: nothing (leaf package)
- Used by: everything

**Supporting Internal Packages:**
- `internal/codeintel/` - Code intelligence: AST parsing, relevance scoring, trie-based indexing for workspace context
- `internal/context/` - Dynamic system context sources (datetime, environment, git) for prompt injection
- `internal/decision/` - Decision logging with receipts, redaction, query support
- `internal/errors/` - Sentinel errors (`ErrPhaseTransition`, `ErrInvalidProvider`, etc.)
- `internal/fileutil/` - Atomic file writes, platform-specific file locking
- `internal/git/` - Git operations (commit, diff, branch, worktree)
- `internal/log/` - Structured slog-based logger with file + stderr output
- `internal/logging/` - Audit logging subsystem
- `internal/shell/` - Platform-specific shell detection
- `internal/tokens/` - Token estimation with EMA calibration
- `internal/scaffold/` - Project scaffolding for website generation
- `internal/testutil/` - Test environment helpers

**Public Packages (`pkg/`):**
- `pkg/arbitrage/` - Model cost/quality arbitrage scoring
- `pkg/autodream/` - Auto-suggestion consolidation engine
- `pkg/bisect/` - Git bisect automation
- `pkg/compaction/` - Session context compaction (auto-summarize old messages)
- `pkg/coordinator/` - Generic concurrency coordinator (used by session manager)
- `pkg/history/` - Frecent history tracking
- `pkg/keychain/` - OS keychain integration (macOS Keychain, Linux D-Bus Secret Service, Windows Credential Manager)
- `pkg/ledger/` - Markdown-based session ledger (`LEDGER.md`)
- `pkg/metrics/` - Metrics collector (tool calls, LLM usage, phase durations)
- `pkg/narrative/` - Narrative engine for progressive UI storytelling
- `pkg/retry/` - Retry policies with backoff
- `pkg/rollback/` - Git-based session rollback
- `pkg/session/` - Session persistence (messages, tasks, checkpoints, planning)
- `pkg/skills/` - Skill discovery and loading (composable slash commands)
- `pkg/taskrunner/` - Parallel task execution with dependency ordering

## Data Flow

**User Prompt -> LLM Response (Interactive REPL):**

1. User types in REPL -> `ReplModel.Update()` receives `tea.KeyMsg`
2. Message sent to active provider via `provider.ChatCompletionStream()`
3. SSE stream chunks emitted as `StreamChunkMsg` via `channelEmitter`
4. TUI renders chunks incrementally in `repl_view.go`
5. Tool calls parsed from stream, dispatched via `Dispatcher.Execute()`
6. Permission checks intercept dangerous tools -> `PermissionRequestMsg` shown to user
7. Tool results appended to message history -> next LLM turn continues

**Workflow Execution (Goal-Based):**

1. User submits goal via `GoalInputModel` or `--goal` flag
2. `Engine.SetIntentResult()` classifies intent (feature/bugfix/refactor/etc.)
3. Workflow mode selected (auto/full/fast/direct) based on complexity
4. `Engine.RunPhase()` called for each phase in sequence
5. Each phase: pre-flight checks -> build context -> stream LLM response -> parse tool calls -> execute tools -> verify -> transition
6. `StateMachine.Transition()` validates phase ordering and prevents oscillation
7. `PhaseCoordinator` handles checkpoint save, metrics recording, budget checks
8. TUI receives `PhaseResultMsg` and updates screen

**Tool Execution Pipeline:**

1. LLM returns tool call in stream
2. `Dispatcher.Execute()` called with tool name and input
3. Rate limiter token bucket checked (10/sec general, 2/sec dangerous)
4. Concurrency semaphore checked (max 8 concurrent)
5. Permission rules evaluated: auto-allow safe tools, prompt for dangerous, deny blocked
6. Tool's `Execute()` method runs
7. Output bounded by `OutputStore` (2000 lines / 51KB default)
8. `ToolResult` returned to LLM as tool message

**Subagent Spawning:**

1. LLM calls `Agent` tool on parent dispatcher
2. `subagent.Manager.Spawn()` acquires semaphore slot (max 8 concurrent)
3. Git worktree created for isolation (if git repo)
4. Child `Dispatcher` created with `NewDispatcherFactory()`
5. Child agent loop runs in goroutine with own provider/model
6. Events streamed back to parent via `eventCh` -> `subagentListenerCmd()`
7. TUI renders child agent progress in `SubagentsModel`

**State Management:**
- Bubble Tea guarantees single-threaded access to `AppState` via `Update()` channel
- No mutexes on `AppState` fields; all mutations in `Update()`
- Workflow state (`WorkflowState`) holds mutable session data (plan, messages, intent, decisions)
- `sync.Mutex` used only for cross-goroutine data: `perPhaseModels`, `modelIDMu`, `workflowModeMu`
- Session persistence: messages, tasks, checkpoints saved to `<workDir>/.m31a/` as JSON

## Key Abstractions

**LLMProvider Interface:**
- Purpose: Abstract LLM API so the workflow engine and TUI are provider-agnostic
- Examples: `internal/provider/interface.go`
- Pattern: Interface with `Name()`, `FetchModels()`, `ChatCompletionStream()`, `EstimateCost()`, `HealthCheck()`, `GetModel()`

**Tool Interface:**
- Purpose: Abstract tool execution so the dispatcher can manage any tool uniformly
- Examples: `internal/types/types.go` (interface), `internal/tools/bash.go` (implementation)
- Pattern: Interface with `Name()`, `Description()`, `RiskLevel()`, `Execute(ctx, ToolInput) (ToolResult, error)`

**StateMachine:**
- Purpose: Enforce valid workflow phase transitions and prevent oscillation
- Examples: `internal/workflow/state_machine.go`
- Pattern: Directed graph with `Transition(from, to)` validation, history tracking, cycle counting

**Bubble Tea Model (AppState):**
- Purpose: Top-level Elm architecture model holding all TUI state
- Examples: `internal/tui/app_state.go`, `internal/tui/app.go`, `internal/tui/app_update.go`
- Pattern: `Init()` returns initial commands, `Update()` dispatches messages, `View()` renders frame

**Message Emitter (MsgEmitter):**
- Purpose: Bridge between workflow goroutines and TUI event loop without breaking Elm purity
- Examples: `internal/tui/app_channel.go`, `internal/workflow/engine.go` (MsgEmitter interface)
- Pattern: Channel-based emitter; workflow writes `tea.Msg` to channel, TUI reads via `tea.Cmd`

**Dispatcher:**
- Purpose: Central tool execution coordinator with permissions, rate limiting, concurrency control
- Examples: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`
- Pattern: Registry of tools + channel-based permission requests + token bucket rate limiter + semaphore

**PromptBuilder / ContextBuilder:**
- Purpose: Construct LLM system prompts from templates and dynamic context sources
- Examples: `internal/workflow/prompt_builder.go`, `internal/workflow/context_builder.go`
- Pattern: Prompt templates loaded via `embed.FS`; context enriched from environment, git, codeintel

## Entry Points

**`cmd/m31a/main.go` (primary):**
- Location: `cmd/m31a/main.go`
- Triggers: OS process execution (`go run`, compiled binary)
- Responsibilities: CLI flag parsing, config loading, keychain init, provider registration, TUI construction, signal handling, graceful shutdown with 5-second force-exit fallback

**`e2e_test.go` (testing):**
- Location: `e2e_test.go`
- Triggers: `go test -run TestBinary`
- Responsibilities: Compiles binary, runs E2E tests including real API tests (require API key env vars)

**Headless mode (`--prompt`):**
- Location: `cmd/m31a/main.go` -> `runHeadless()`
- Triggers: `m31a --prompt "question"`
- Responsibilities: Single-turn LLM query without TUI, streams response to stdout

## Error Handling

**Strategy:** Return errors, never panic. Wrap with `fmt.Errorf("%w", err)`. Use sentinel errors from `internal/errors/`.

**Patterns:**
- Sentinel errors: `ErrPhaseTransition`, `ErrInvalidProvider`, `ErrProviderNotFound`, `ErrProviderUnreachable` defined in `internal/errors/errors.go`
- `ToolError` type carries both error message and actionable hint for LLM self-recovery (`internal/types/types.go`)
- Phase failures return `PhaseResult` with `Success: false` and `Error` string
- Config load failures return early with exit code 1
- Tool execution errors wrapped as `ToolResult.Error` string (not Go error) for LLM consumption
- Self-healing: `Engine.HealTask()` retries failed tasks up to `MaxHealAttempts` (2)

## Cross-Cutting Concerns

**Logging:** `log/slog` with structured JSON output to both file (`~/.m31a/m31a.log`) and stderr. Logger initialized in `cmd/m31a/main.go` via `internal/log.NewLogger()`.

**Validation:** Config validation at load time in `internal/config/loader.go`. Tool input validated per-tool. Permission rules validated against config schema. Phase transitions validated by `StateMachine`.

**Authentication:** API keys stored in OS keychain (`pkg/keychain/`). Resolved at startup via `cfg.ResolveAPIKeys(kc)`. Keys never written to disk in plaintext. `.env` files gitignored except `.env.example`.

**Metrics:** `pkg/metrics/Collector` captures tool execution counts, LLM token usage, phase durations, heal attempts. Written to `METRICS.json` per session when `MetricsEnabled` is true.

**Security:** Permission system with risk levels (safe/medium/dangerous/destructive). Tools checked against rules before execution. Dangerous tools rate-limited (2/sec). Bash commands can be denied/asked. Persistent per-project permission overrides.

---

*Architecture analysis: 2026-07-04*
