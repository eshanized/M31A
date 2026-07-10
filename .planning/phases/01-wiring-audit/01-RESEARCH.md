# Phase 1: Wiring Audit - Research

**Phase:** 01 - Wiring Audit
**Generated:** 2026-07-10

---

## Executive Summary

This research analyzes the M31A codebase (Go TUI with Bubble Tea, 7-phase workflow engine, 3 LLM providers, 18 tools, session persistence) to determine how to build a complete dependency graph and verify all wiring connections. The audit must trace every module, package, interface, service, workflow, configuration, event, command, state transition, provider, tool, UI component, and runtime system.

---

## Codebase Architecture Overview

### Entry Point & Startup (cmd/m31a/main.go)
- **Main initialization sequence**: flag parsing → config load → keychain init → provider registration → tool registration → workflow engine init → session init → TUI construction → run loop
- **Key dependencies**: `internal/config`, `internal/keychain`, `internal/provider`, `internal/tools`, `internal/workflow`, `internal/session`, `cmd/m31a/tui`
- **Signal handling**: SIGINT/SIGTERM for graceful shutdown
- **Background workers**: session persistence, metrics, hot reload watchers

### Package Structure & Dependency Rules

| Package | Purpose | Can Import | Cannot Import |
|---------|---------|------------|---------------|
| `cmd/m31a/` | Entry point, TUI | internal/*, pkg/* | - |
| `internal/` | Private implementation | pkg/*, internal/* (same level) | - |
| `internal/provider/` | LLM provider abstraction | internal/types, pkg/* | internal/tools (circular risk) |
| `internal/tools/` | Built-in tools & dispatcher | internal/types, internal/provider, pkg/* | - |
| `internal/workflow/` | 7-phase engine | internal/types, internal/tools, internal/provider | - |
| `internal/session/` | Persistence, checkpoints | internal/types, pkg/* | - |
| `internal/types/` | Shared type vocabulary | pkg/* | internal/* |
| `pkg/*` | Public APIs | stdlib, pkg/* | **NEVER internal/*** |

**CRITICAL**: `pkg/` must NOT import `internal/` — enforced by Go module system

### Workflow Engine (internal/workflow/engine.go)
- **7 Phases**: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship
- **Phase registration**: Each phase registers via `RegisterPhase(name, handler)`
- **Transitions**: Explicit phase → phase via `engine.NextPhase()`
- **Events**: `PhaseStarted`, `PhaseCompleted`, `PhaseFailed` emitted via event bus
- **Checkpoints**: Serialized to disk at phase boundaries
- **Resume**: Load checkpoint, restore engine state, continue from failed phase
- **Cancellation**: Context propagation through all phases
- **Retries**: Configurable per-phase with exponential backoff

### Provider Layer (internal/provider/)
Three providers implementing `LLMProvider` interface:
1. **OpenRouter** (`openrouter.go`): Dynamic model discovery via `/models` endpoint
2. **Zen** (`zen.go`): Anthropic-compatible API with model listing
3. **Nvidia** (`nvidia.go`): Nvidia NIM API with model catalog

**Registry pattern**: `provider.Registry` holds all registered providers, `ActiveProvider` field tracks current
**Fallback**: Automatic fallback chain on provider failure
**Health checks**: Periodic `/health` or model listing calls
**Model discovery**: Dynamic — never hardcoded
**Streaming**: All providers implement `StreamChat()` for token-by-token responses
**Retry**: Built-in with configurable attempts/backoff

### Tools System (internal/tools/)
- **18 built-in tools** in `internal/tools/` registered in `defaults.go`
- **Dispatcher** (`dispatcher.go`): Handles permissions, rate limiting, concurrency, output store
- **Permission system**: `ToolPermission` (Allow/Deny/Ask) per tool, persisted in config
- **Rate limiter**: Token bucket per tool, configurable via config
- **Concurrency**: Semaphore-based, max concurrent executions per tool
- **Output store**: Tool results cached with TTL, keyed by invocation ID
- **Schema**: JSON Schema for each tool (name, description, parameters)
- **Execution**: `Dispatch(ctx, toolName, params)` → validates → checks permission → rate limits → executes → stores result
- **Agent spawning**: Tools can spawn sub-agents via `task` tool
- **AskUser**: Special tool for human-in-the-loop approval
- **Batch approvals**: Multiple tool calls can be approved in one prompt

### Configuration (internal/config/)
- **Config structs**: `Config`, `ProviderConfig`, `ToolConfig`, `WorkflowConfig`, `TUIConfig`, `KeychainConfig`
- **TOML loading**: `config.Load(path)` with environment variable expansion
- **Defaults**: `config.Defaults()` provides zero-value safe defaults
- **Validation**: `config.Validate()` checks required fields, valid providers, tool names
- **Hot reload**: `config.Watch(path, callback)` uses fsnotify
- **Keychain resolution**: `config.ResolveKeychain()` substitutes `${KEYCHAIN:key}` refs

### Bubble Tea TUI (cmd/m31a/tui/)
- **Elm architecture**: `Model` (state), `Update(msg)`, `View()`, `Init()`
- **AppState**: Single source of truth, **never mutated from goroutines**
- **Messages**: `tea.Msg` subtypes for every async event (provider response, tool result, workflow event, keypress)
- **Commands**: `tea.Cmd` for async work (API calls, file I/O, subprocesses)
- **Screen stack**: Push/pop navigation, each screen has own `Update/View`
- **Event emitters**: Channels from background workers → `tea.Cmd` → `Update()`
- **Shutdown**: `tea.Quit` on SIGINT, cleanup in `Model.Close()`

### Persistence (internal/session/)
- **Sessions**: Full conversation + workflow state serialized to JSON
- **Checkpoints**: Phase-boundary snapshots for resume
- **Ledger**: Append-only event log for audit/replay
- **Rollback**: `session.RollbackTo(checkpointID)` restores state
- **History**: `session.History()` returns chronological events
- **Metrics**: Token usage, latency, tool invocations per session
- **Narrative**: Human-readable session summary
- **AutoDream**: Periodic background compaction of old sessions
- **Compaction**: Merge related events, drop verbose logs

### Public Packages (pkg/)
| Package | Exports | Test Coverage Target |
|---------|---------|---------------------|
| `pkg/taskrunner` | `TaskRunner`, `Task`, `Result` | **90%** |
| `pkg/bisect` | `Bisect`, `Commit`, `TestFunc` | **90%** |
| `pkg/rollback` | `RollbackManager`, `Snapshot` | **90%** |
| `pkg/keychain` | `Keychain`, `Store`, `Retrieve` | 75% |
| `pkg/...` | Other utilities | 75% |

---

## Wiring Audit Scope Mapping

### 1. Package Wiring (Scope: PackageWiring)
- **Imports**: Scan all `import` statements, build directed graph
- **Boundaries**: Verify no `pkg/*` imports `internal/*`
- **Direction**: Dependencies flow inward (cmd → internal → pkg → stdlib)
- **Circular**: Detect cycles via graph analysis
- **Unused**: Packages with no incoming edges (except main)
- **Duplicates**: Multiple implementations of same interface

### 2. Application Boot (Scope: ApplicationBoot)
**Trace from `cmd/m31a/main.go`:**
1. `flag.Parse()` → config path
2. `config.Load()` → validates, resolves keychain refs
3. `keychain.Init()` → OS keyring backend (libsecret/keychain/credman)
4. `provider.RegisterAll()` → discovers models from each provider API
5. `tools.RegisterDefaults()` → 18 tools + dispatcher init
6. `workflow.NewEngine()` → registers 7 phases
7. `session.NewManager()` → loads last session or creates new
8. `tui.NewModel()` → builds initial AppState
9. `tea.NewProgram(model).Run()` → enters Bubble Tea loop
10. Signal handlers → `program.Send(tea.Quit)`
11. Background workers started (persistence, metrics, watcher)
12. On shutdown: `session.Save()`, `keychain.Close()`, worker cleanup

**Must verify**: Nothing skipped, correct order, error handling at each step

### 3. Workflow Engine (Scope: WorkflowEngine)
- Phase registration: all 7 phases present in registry
- Transitions: Initialize→Discuss, Discuss→Plan, Plan→Execute, Execute→Verify, Verify→Runtime, Runtime→Ship
- Event emission: every phase start/complete/fail emits event
- Message routing: workflow events → TUI via channel → `Update()`
- Checkpoints: saved at each phase boundary
- Resume: loads checkpoint, reconstructs engine state
- Metrics: phase duration, token usage, tool calls recorded
- Persistence: checkpoints survive process restart
- Cancellation: context cancelled propagates to all running work
- Retries: configurable, exponential backoff, max attempts
- Self-healing: failed phase can be retried from checkpoint
- Completion: Ship phase marks workflow complete, triggers session finalize

### 4. State Machine (Scope: StateMachine)
- **States**: `Initializing`, `Discussing`, `Planning`, `Executing`, `Verifying`, `Running`, `Shipping`, `Complete`, `Failed`
- **Transitions**: Valid next states for each state
- **Unreachable**: States with no incoming transitions
- **Missing**: Required transitions not defined (e.g., Failed→Retry)
- **Duplicated**: Multiple paths to same state
- **Impossible**: Transitions that violate invariants
- **Dead states**: Terminal states not Complete/Failed
- **Orphan states**: States not reachable from Initializing
- **Invalid mutations**: Direct AppState field writes outside Update()
- **Race possibilities**: Shared mutable state accessed from goroutines

### 5. Bubble Tea (Scope: BubbleTea)
- `Init()`: Returns initial `tea.Cmd` for startup async work
- `Update(msg)`: **Single state mutation point** — all changes here
- `View()`: Pure render, no side effects
- Commands: All async work returns `tea.Cmd`
- Messages: All external events are `tea.Msg` subtypes
- Event emitters: Background workers send via channel → `tea.Cmd` wrapper
- Channel routing: Single `msgChan` multiplexes all async sources
- Async goroutines: ONLY communicate via channels → `tea.Cmd`
- Navigation: Screen stack push/pop, each screen has `Update/View`
- Listeners: Subscriptions to external events (file watch, provider health)
- Shutdown: `tea.Quit` triggers `Model.Close()` for cleanup
- **INVARIANT**: No goroutine mutates `AppState` directly — MUST use `tea.Cmd` → `Update()`

### 6. Provider Layer (Scope: ProviderLayer)
- Registration: `provider.Register(p)` called for all 3 providers
- Registry: `provider.Get(name)` returns registered provider
- Active provider: `provider.SetActive(name)` / `provider.Active()`
- Fallback: `provider.FallbackChain()` returns ordered list
- Health checks: `provider.HealthCheck(ctx)` for each
- Model discovery: `provider.ListModels(ctx)` called at startup
- Cost estimation: `provider.EstimateCost(model, tokens)`
- Streaming: `provider.StreamChat(ctx, req)` returns channel
- Retry: `provider.WithRetry(attempts, backoff)` middleware
- Capability detection: `provider.Capabilities()` returns struct
- Reasoning: `provider.SupportsReasoning()` bool
- Context handling: `provider.MaxContextTokens(model)` int

### 7. Tools (Scope: Tools)
- Registration: All 18 tools in `tools.Defaults()` map
- Dispatcher: Single `Dispatcher` instance, thread-safe
- Permissions: `config.ToolPermissions` map[tool]Permission
- Rate limiter: Token bucket per tool, refill rate from config
- Concurrency: Semaphore per tool, max from config
- Output store: `dispatcher.Store` with TTL cleanup
- Schema: Each tool has `Schema()` returning JSON Schema
- Execution: `Dispatch()` validates → permits → rate-limits → executes → stores
- Tool results: Stored with invocation ID, retrievable for context
- Agent spawning: `task` tool creates sub-agent with own context
- AskUser: Blocks until user responds via TUI prompt
- Batch approvals: Multiple `AskUser` calls batched into single prompt
- Metrics: Invocation count, latency, errors per tool
- Shutdown: `Dispatcher.Close()` waits for in-flight, flushes store

### 8. Configuration (Scope: Configuration)
- Config structs match TOML keys exactly
- Defaults: Zero values safe, `Defaults()` provides overrides
- Env vars: `M31A_PROVIDER_API_KEY` etc. override TOML
- Validation: `Validate()` returns error for missing required
- Hot reload: `Watch()` callback fires on file change
- Keychain resolution: `${KEYCHAIN:service/key}` → OS keyring
- Missing fields: Detected by validation, not silent zero-values
- Dead fields: TOML keys not mapped to struct fields
- Unused settings: Struct fields not read anywhere
- Broken overrides: Env var set but not used in code

### 9. Persistence (Scope: Persistence)
- Sessions: `session.Save()` / `session.Load(id)`
- Checkpoints: `engine.Checkpoint()` / `engine.Restore(id)`
- Ledger: `session.Ledger.Append(event)` / `session.Ledger.Query()`
- Rollback: `session.RollbackTo(checkpointID)`
- History: `session.History()` returns chronological events
- Metrics: `session.Metrics` aggregated on save
- Narrative: `session.Narrative()` generates summary
- AutoDream: Background goroutine compacts old sessions
- Compaction: `session.Compact(olderThan)` merges events

### 10. Public Packages (Scope: PublicPackages)
- `pkg/taskrunner`: Exported API, callers in internal/workflow, tests
- `pkg/bisect`: Exported API, callers in internal/tools (git-bisect tool)
- `pkg/rollback`: Exported API, callers in internal/session
- `pkg/keychain`: Exported API, callers in cmd/m31a/main, internal/config
- Verify no `internal/` imports in any `pkg/` file

### 11. Subagents (Scope: Subagents)
- Creation: `task` tool spawns `internal/subagent/Runner`
- Lifecycle: Start → work → emit events → complete/fail
- Dispatcher: Subagent uses parent's dispatcher (shared)
- Worktrees: `git worktree add` for isolated file ops
- Cancellation: Parent context cancellation propagates
- Cleanup: Worktree removed on completion
- Concurrency: Max subagents configurable
- Event propagation: Subagent events → parent's event bus

### 12. Testing (Scope: Testing)
**Coverage targets**: 75% overall, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback

| Layer | Test Files | Current Status |
|-------|------------|----------------|
| `pkg/taskrunner` | `*_test.go` | Need verification |
| `pkg/bisect` | `*_test.go` | Need verification |
| `pkg/rollback` | `*_test.go` | Need verification |
| `internal/provider` | `*_test.go` | Need verification |
| `internal/tools` | `*_test.go` | Need verification |
| `internal/workflow` | `*_test.go` | Need verification |
| `internal/session` | `*_test.go` | Need verification |
| `cmd/m31a` | `e2e_test.go` | Real API tests (require keys) |
| All exported APIs | `*_test.go` | Need coverage audit |

### 13. Documentation Alignment (Scope: DocumentationAlignment)
| Doc | Source of Truth | Drift Risk |
|-----|-----------------|------------|
| ARCHITECTURE.md | Code structure | High if refactored |
| STRUCTURE.md | Package layout | Medium |
| STACK.md | go.mod, imports | Low |
| INTEGRATIONS.md | Provider APIs | High (external APIs change) |
| CONVENTIONS.md | Code style, patterns | Medium |
| TESTING.md | Test patterns, targets | Low |
| CONCERNS.md | Known issues | High |

---

## Key Technical Findings for Planning

### 1. Graph Construction Strategy
- Use `go list -json -deps ./...` for import graph
- Parse AST for: interface declarations, struct embeddings, channel types, function signatures
- Build bipartite graph: (packages) ↔ (types/interfaces)
- Trace runtime: channel sends/receives, goroutine spawns, `tea.Cmd` returns

### 2. Critical Wiring Patterns to Verify
| Pattern | Detection Method |
|---------|------------------|
| Missing registration | Interface in registry but no `Register()` call |
| Unused registration | `Register()` called but never `Get()`/`Active()` |
| Orphan interface | Interface defined, no struct implements it |
| Nil path | Channel/interface used without nil check |
| Goroutine leak | `go func()` without context cancellation |
| Channel leak | Channel created but no sender/receiver |
| Context leak | `context.Background()` used in request path |
| Race | Shared mutable state without mutex/channel |

### 3. Phase 1 Deliverables (3 Plans)
**Plan 01-01**: Package wiring + Application boot trace
**Plan 01-02**: Workflow engine + Provider layer + Tools + Bubble Tea + Config + Persistence
**Plan 01-03**: Public packages + Subagents + Testing gaps + Doc drift + Report generation

### 4. Required Tooling for Audit
- `go list`, `go vet`, `golangci-lint` for static analysis
- Custom AST visitor for interface/implementation mapping
- Channel/goroutine tracing via `go tool trace` or runtime instrumentation
- Graph visualization: GraphViz DOT export

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Incomplete graph due to dynamic imports (plugins) | Low | Medium | M31A has no plugin system |
| False positives on dead code (test-only code) | High | Low | Exclude `_test.go` from dead code analysis |
| Goroutine leak detection false negatives | Medium | High | Use `runtime.NumGoroutine()` before/after test runs |
| Config drift between TOML and structs | Medium | Medium | Automated validation in CI |
| Provider API changes breaking discovery | High | High | Version pinning, integration tests with recorded responses |

---

## Recommended Plan Structure

### Plan 01-01: Package Wiring & Application Boot
- Build import graph from `go list -json`
- Verify pkg/internal boundary
- Trace main.go initialization sequence
- Identify unused packages, circular deps, wrong direction
- Output: Dependency graph (DOT), issue list with file:line

### Plan 01-02: Runtime Systems Wiring
- Workflow engine: phase registry, transitions, events, checkpoints
- Provider layer: registration, fallback, health, discovery
- Tools: dispatcher, all 18 tools registered, permissions, rate limits
- Bubble Tea: Init/Update/View, message types, channel routing, no direct mutations
- Config: structs↔TOML↔env↔keychain, validation, hot reload
- Persistence: sessions, checkpoints, ledger, rollback, compaction
- Output: Wiring issues per system with severity

### Plan 01-03: Cross-Cutting & Report
- Public packages: exported API vs callers, internal import violations
- Subagents: lifecycle, worktrees, cancellation, event propagation
- Testing: coverage audit per package, missing test files
- Documentation: drift detection (doc vs code)
- Generate 20-section report with severity matrix, file locations, root causes, fixes, priority
- Output: Complete wiring audit report

---

## Conclusion

The M31A codebase is well-structured with clear boundaries (pkg/internal), single-threaded TUI (Bubble Tea), and explicit registration patterns (providers, tools, workflow phases). The audit must systematically trace: (1) static import graph, (2) runtime initialization sequence, (3) all registration points and their consumers, (4) channel/goroutine communication patterns, (5) state machine transitions, and (6) persistence save/load paths. Three plans provide logical separation: static structure, runtime systems, cross-cutting concerns + report.