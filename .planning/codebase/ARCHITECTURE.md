# Architecture — M31A

> Mapped: 2026-07-09

## Architectural Pattern

M31A follows the **Elm architecture** (Model-View-Update) via Bubble Tea for the TUI layer, with a **pipeline/workflow** pattern for the task execution engine. The codebase is divided into three layers:

```
cmd/m31a/        Entry point (thin)
  └─ internal/   Private packages — all business logic
       └─ pkg/   Public packages — reusable components (importable by external consumers)
```

**Key architectural rules:**
- `pkg/` MUST NOT import `internal/` (enforced by Go module convention)
- All state mutations go through `tea.Update()` only — never from goroutines
- Provider model lists are dynamic (never hardcoded)

## System Layers

### 1. Entry Point (`cmd/m31a/main.go`)

- Flag parsing (`usage.go`)
- Config loading (`~/.m31a/config.toml`)
- Provider registration
- TUI construction and `tea.Program` launch
- Headless mode (`--prompt`, `--goal` flags) bypasses TUI entirely

### 2. TUI Layer (`internal/tui/`)

33-screen Bubble Tea application with:

| Sub-component | Files | Purpose |
|--------------|-------|---------|
| App model | `app.go`, `app_state.go` | Root model, state management |
| Routing | `app_routing.go` | Screen navigation |
| REPL | `repl.go`, `repl_view.go`, `repl_model.go` | Main chat interface |
| Command palette | `cmdpalette.go`, `commandpalette_model.go` | `/command` dispatch |
| Sidebar | `sidebar_model.go` | Session/file navigation |
| Streaming | `streaming.go`, `streaming/` | Real-time LLM response rendering |
| Layout | `layout/` | Responsive layout system |
| Commands | `commands/` | Slash command implementations |
| Models | `dashboard_model.go`, `diff_model.go`, `plan_model.go`, etc. | Per-screen models |
| Themes | `theme/` | M31A dark theme |
| Accessibility | `a11y/` | Reduced motion, animation speed |

**Routing:** The `app_routing.go` handler manages screen transitions. The app uses a centralized message-passing pattern — all events flow through `app_update.go`.

**State management:** `AppState` in `app_state.go` is the single source of truth. Each screen model is embedded and swapped based on the current route.

### 3. Workflow Engine (`internal/workflow/`)

Seven-phase pipeline:

```
Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship
```

| Phase | Entry Point | Purpose |
|-------|------------|---------|
| Initialize | `initialize.go` | Project detection, code index, env pre-flight |
| Discuss | `discuss.go` | Clarifying questions, quality scoring |
| Plan | `plan.go`, `plan_chunk.go`, `plan_check.go` | Task breakdown, revision loops |
| Execute | `execute.go`, `execute_preflight.go`, `execute_quality.go` | Task execution, loop detection |
| Verify | `verify.go`, `verify_report.go` | Build/test, self-heal, security scan |
| Runtime | `runtime.go` | Dev server lifecycle, HTTP smoke tests |
| Ship | `ship.go`, `ship_preflight.go` | Pre-ship checklist, commit, changelog |

**Engine decomposition:** The engine (`engine.go`) delegates to:
- `phase_coordinator.go` — Phase lifecycle management
- `state_machine.go` — State transitions
- `workflow_cache.go` — Cached workflow data
- `context_builder.go` — Dynamic context assembly
- `cost_tracker.go` — Per-phase cost tracking
- `prompt_builder.go` — Prompt template assembly
- `retry.go` — Self-heal retry logic
- `intent.go` — Intent classification

### 4. Provider Layer (`internal/provider/`)

Three provider implementations with a common interface:

```
interface.go (Provider interface)
  ├── base_client.go (shared: SSE, caching, streaming)
  ├── openrouter/    (OpenRouter REST client)
  ├── zen/           (OpenCode Zen client)
  └── nvidia/        (Nvidia NIM client)
```

**Cross-cutting:**
- `cache.go` — Model metadata cache with TTL and `singleflight` dedup
- `fallback.go` — Auto-fallback on provider degradation
- `capabilities.go` — Dynamic model capability detection
- `registry.go` — Provider registration and health checks
- `sse.go` — SSE stream parsing
- `reasoning.go` — Extended thinking/reasoning content

### 5. Tool Layer (`internal/tools/`)

18 built-in tools registered in `defaults.go`, dispatched via `dispatcher.go`:

| Tool | File | Category |
|------|------|----------|
| Bash | `bash.go` + platform variants | Execution |
| FileRead | `fileread.go` | File system |
| FileWrite | `filewrite.go` | File system |
| Edit | `edit.go` | File system |
| FileDelete | `filedelete.go` | File system |
| FileMove | `filemove.go` | File system |
| FileList | `filelist.go` | File system |
| Glob | `glob.go` | File system |
| Grep | `grep.go` | File system |
| WebFetch | `webfetch.go` | Network |
| WebSearch | `websearch.go` | Network |
| HTTPCheck | `httpcheck.go` | Network |
| CodeMap | `codemap.go` | Code intelligence |
| CodeComplexity | `codecomplexity.go` | Code intelligence |
| TodoWrite | `todo.go` | Task tracking |
| TodoRead | `todoread.go` | Task tracking |
| DevServer | `devserver.go` | Runtime |
| AskUserQuestion | `question.go` | Interaction |

**Security layers on tools:**
- Permission system (`permissions.go`) — modal allow/deny per command
- Rate limiting (`concurrency.go`) — token bucket (20/10 burst, 5/2 dangerous)
- Concurrency control — semaphore (max 8)
- Command blocklist — dangerous patterns at tool boundary
- SSRF/DNS rebinding protection on webfetch/websearch
- Path traversal prevention on file tools
- 7-strategy cascade on Edit tool (`edit.go`)

**Subagent manager:** `tools/subagent/` — spawns child agents in isolated git worktrees, max depth 2.

### 6. Code Intelligence (`internal/codeintel/`)

- 4-language parser (Go via tree-sitter, TypeScript, Python, Rust)
- Import dependency graph construction (`graph.go`)
- Symbol indexing (`index.go`)
- Relevance scoring (`relevance.go`)
- Trie-based prefix matching (`trie.go`)
- Cache layer (`cache.go`)

### 7. Configuration (`internal/config/`)

- TOML loader (`loader.go`)
- Project context detection (`project_context.go`)
- Config merging (`merge.go`)
- Instructions extraction (`instructions.go`)

### 8. Dynamic Context (`internal/context/`)

- Context registry with tracking/reconciliation
- Diff notifications between LLM calls
- Dynamic context assembly for prompts

## Data Flow

```
User Input
  → TUI (app_update.go)
    → Slash command dispatch (commands/)
      → Workflow engine (workflow/)
        → Provider (provider/ → external LLM API)
          → Tool calls (tools/dispatcher.go)
            → File system / Network / Git
          → Response streamed back
        → Phase progression
      → Session persistence (pkg/session/)
      → Ledger entry (pkg/ledger/)
    → TUI render (app_view.go)
```

## Cross-cutting Concerns

| Concern | Location | Description |
|---------|----------|-------------|
| Logging | `internal/logging/`, `internal/log/` | Structured + audit logging with secret redaction |
| Errors | `internal/errors/` | Sentinel errors with user-friendly messages |
| Metrics | `pkg/metrics/` | Thread-safe session metrics with JSON persistence |
| Decision logging | `internal/decision/` | Structured decision receipts, ring buffer |
| Retry | `pkg/retry/` | Exponential backoff with error classification |
| Session management | `pkg/session/` | Lifecycle, persistence, resume support |
| Keychain | `pkg/keychain/` | OS-native secret storage abstraction |
| Model arbitrage | `pkg/arbitrage/` | Cost-optimized model selection |
| AutoDream | `pkg/autodream/` | Context consolidation with reentrancy guard |
| Compaction | `pkg/compaction/` | LLM-summarized context replacement |
| Narrative | `pkg/narrative/` | Event → human-readable progress descriptions |
| Coordinator | `pkg/coordinator/` | Concurrent drain session management |
