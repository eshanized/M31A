# M31 Autonomous — Complete Architecture Audit

## Executive Summary

M31 Autonomous is a **terminal-native AI coding agent** written in Go. It orchestrates a seven-phase software development workflow (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship) through a Bubble Tea TUI, streaming LLM responses from three providers (OpenRouter, Zen, Nvidia) with automatic fallback. The system executes 18 built-in tools gated by a permission system with rate limiting, produces verified git commits with rollback chains, and records cross-session learning. It compiles to a single static binary with zero CGO and zero telemetry.

**Scale:** ~25,000+ lines of Go across 60+ source files in 22 packages. 33 TUI screens, 11 themes, 18 tools, 7 workflow phases, 3 LLM providers.

---

## Architecture Overview

### Layer Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                     cmd/m31a/ (entry point)                     │
│          main.go: flag parsing, config load, TUI launch         │
└───────────────────────────┬─────────────────────────────────────┘
                            │ imports
┌───────────────────────────▼─────────────────────────────────────┐
│                        internal/ (private)                      │
│                                                                 │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────────────────┐ │
│  │    tui/      │ │  workflow/    │ │       provider/          │ │
│  │  33 screens  │ │  7 phases    │ │  openrouter │ zen│nvidia │ │
│  │  11 themes   │ │  quality     │ │  SSE streaming           │ │
│  │  Bubble Tea  │ │  gates       │ │  model cache + fallback  │ │
│  └──────┬──────┘ └──────┬───────┘ └──────────┬───────────────┘ │
│         │               │                     │                  │
│  ┌──────▼───────────────▼─────────────────────▼───────────────┐ │
│  │                      tools/ (18 tools)                     │ │
│  │  permissions · rate limiting · concurrency · subagents      │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐ │
│  │ codeintel│ │  config/  │ │   git/   │ │    tokens/        │ │
│  │ 4 langs  │ │ TOML+env  │ │ commit   │ │ tiktoken + EMA    │ │
│  │ tree-sitter│ │ hot-reload│ │ rollback │ │ estimation       │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘ │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐ │
│  │ errors/  │ │ fileutil/ │ │   log/   │ │    types/         │ │
│  │ sentinels│ │ atomic I/O│ │ rotation │ │ shared types      │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘ │
└───────────────────────────┬─────────────────────────────────────┘
                            │ imports
┌───────────────────────────▼─────────────────────────────────────┐
│                        pkg/ (public)                            │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐ │
│  │ session/ │ │ ledger/  │ │rollback/ │ │   taskrunner/     │ │
│  │ lifecycle│ │ learning │ │ commits  │ │ Kahn's algorithm  │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘ │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐ │
│  │keychain/ │ │autodream/│ │arbitrage/│ │    history/       │ │
│  │ OS keys  │ │compress  │ │ cost opt │ │ frecency          │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘ │
│                                                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌───────────────────┐ │
│  │ bisect/  │ │metrics/  │ │compaction│ │   coordinator/    │ │
│  │ git bisect│ │ observe  │ │ context  │ │ drain management  │ │
│  └──────────┘ └──────────┘ └──────────┘ └───────────────────┘ │
│                                                                 │
│  ┌──────────┐ ┌──────────┐                                    │
│  │ retry/   │ │ skills/  │                                    │
│  │ backoff  │ │ discover │                                    │
│  └──────────┘ └──────────┘                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Dependency Rules

- `cmd/m31a/` imports only `internal/` and `pkg/`
- `internal/` may import `pkg/`
- `pkg/` must NOT import `internal/` (enforced by Go module system)
- `internal/types` is the shared type vocabulary across all layers

---

## Component Map

### Entry Point: `cmd/m31a/`

| File | Lines | Purpose |
|------|-------|---------|
| `main.go` | 418 | Flag parsing, config load, provider registration, TUI construction, signal handling |
| `usage.go` | 59 | Help text, slash command listing, env var documentation |

**Startup sequence (main.go:110-418):**
1. Parse `--version`, `--help`, `--prompt`, `--model` flags
2. Load `.env` file (before logger to avoid goroutine race)
3. Initialize structured logger with daily rotation
4. Resolve config path (`M31A_CONFIG` env or `~/.m31a/config.toml`)
5. Load TOML config (6-layer cascade: defaults → global TOML → env vars → project TOML → variable substitution → validation)
6. Detect unclean shutdown via `.force-exit` sentinel file
7. Initialize OS keychain (best-effort), resolve API keys
8. Register providers (OpenRouter, Zen, Nvidia) based on available API keys
9. Create session manager, tool dispatcher, git client, ledger, rollback, AutoDream
10. Construct TUI app with all dependencies injected
11. Create subagent manager with git worktree support
12. Sweep stale agent worktrees (30s timeout)
13. Optionally resume previous session
14. Create Bubble Tea program with alt-screen + mouse tracking
15. Install signal handler (SIGTERM/SIGINT → graceful quit with 5s hard-fallback)
16. Run TUI

**Headless mode:** `--prompt` flag sends a single prompt to the active provider and prints the response to stdout, bypassing the TUI entirely.

### Provider Layer: `internal/provider/`

| File | Lines | Purpose |
|------|-------|---------|
| `interface.go` | 33 | `LLMProvider` interface (8 methods) |
| `base_client.go` | 256 | Shared HTTP transport, model lookup, cost estimation, stream iterator creation |
| `cache.go` | 139 | Thread-safe model cache with TTL + stale + singleflight dedup |
| `capabilities.go` | 152 | Heuristic capability detection from model ID |
| `common.go` | 356 | HTTP error handling, chat body building, context-exceeded detection |
| `fallback.go` | 192 | Parallel health checks, priority-based provider switching, rate-limit extraction |
| `model_metadata.go` | 286 | OpenRouter metadata enrichment, local metadata fallback |
| `reasoning.go` | 280 | Per-family reasoning config, SSE chunk parsing |
| `registry.go` | 116 | Thread-safe provider registry with active provider tracking |
| `sse.go` | 141 | SSE stream parser with watchdog timer, context cancellation |
| `openrouter/client.go` | 192 | OpenRouter client with referer/title headers, 402 credit handling |
| `zen/client.go` | 167 | Zen client with model enrichment, credit detection on 401 |
| `nvidia/client.go` | 264 | Nvidia client with model filtering, multimodal handling, extra_body nesting |

**Key design decisions:**
- **Two HTTP clients per provider:** `HTTPClient` (no timeout, for SSE streaming) and `CatalogClient` (hard timeout, for catalog/health checks)
- **Shared transport singleton:** All providers share a single `http.Transport` with connection pooling (100 max idle conns, 10/host, 90s idle timeout)
- **Singleflight model refresh:** Only one HTTP request made even when multiple goroutines call `FetchModels()` simultaneously
- **Two-tier cache TTL:** Fresh period (5 min) + stale maximum (24h). Stale cache returned as fallback when API fails
- **Parallel health checks for fallback:** All candidate providers checked concurrently with 10s timeout
- **Retry with exponential backoff:** OpenRouter and Nvidia retry up to 2 times (3 total attempts) with 1s, 2s backoff

### Tools Layer: `internal/tools/`

| File | Lines | Tool |
|------|-------|------|
| `bash.go` | 525 | Shell execution with dangerous command blocking, process groups, output capping |
| `fileread.go` | 251 | File reading with byte/line-level offsets, binary detection |
| `filewrite.go` | 220 | Atomic write with backup, append mode |
| `edit.go` | 860 | 7-strategy cascading replacement with Levenshtein fuzzy matching |
| `glob.go` | 207 | File pattern matching with ripgrep fallback |
| `grep.go` | 595 | Content search with ripgrep or pure-Go, ReDoS protection, gitignore caching |
| `webfetch.go` | 1136 | URL fetching with SSRF protection, DNS pinning, HTML-to-markdown conversion |
| `websearch.go` | 244 | SearXNG search with DNS cache for rebinding protection |
| `codemap.go` | 231 | Code intelligence interface: upstream/downstream/define/references/relevant/symbols |
| `codecomplexity.go` | 392 | Codebase complexity classification (<10K, 10K-50K, 50K+ lines) |
| `filedelete.go` | 118 | File deletion with backup |
| `filemove.go` | 153 | File move with containment checks |
| `filelist.go` | 228 | Tree-style directory listing |
| `todo.go` | 212 | TODO.md management with sidebar notification |
| `todoread.go` | 184 | TODO.md parsing |
| `devserver.go` | 587 | Dev server lifecycle: start/stop/restart/logs/port-check with crash monitoring |
| `httpcheck.go` | 301 | HTTP request with status/body/JSON path validation |
| `question.go` | 183 | Interactive user question with timeout and custom options |
| `agent.go` | 263 | Subagent spawning with depth limit (max 2) |
| `metrics.go` | 94 | Metrics summary tool |

**Dispatcher execution pipeline (dispatcher.go:200-312):**
1. Concurrency semaphore (max 8 concurrent tools)
2. General rate limiter (20 burst / 10 sustained)
3. Tool lookup by name
4. Per-risk-level rate limiter (5 burst / 2 sustained for dangerous tools)
5. JSON input parsing + normalization
6. Permission check (rule evaluation → agent default → risk-level fallback)
7. Tool execution
8. Metrics recording
9. Output bounding (2000 lines / 51200 bytes)

**Subagent system (tools/subagent/):**
- Manager orchestrates child agent lifecycles with max 8 concurrent
- Each subagent gets its own git worktree for isolation
- Max nesting depth: 2 levels
- Per-subagent budgets: 50 tools, 50K tokens, 25 turns
- Spawn rate limit: 10/minute, 50 total per session
- Built-in profiles: build, plan, general, explore, security, review

### Workflow Engine: `internal/workflow/`

| File | Lines | Purpose |
|------|-------|---------|
| `engine.go` | 1317+ | Core engine, phase dispatch, LLM streaming, system prompt building |
| `initialize.go` | 115 | Project detection, git init, planning directory setup |
| `discuss.go` | 294 | LLM-generated clarifying questions, quality scoring, completeness checks |
| `plan.go` | 509 | Task breakdown, plan checker with revision loops, coverage gates |
| `execute.go` | 1021 | Task execution with tool dispatch, loop detection, self-healing |
| `verify.go` | 316 | Build/test verification, self-healing, git bisect fallback |
| `runtime.go` | 505 | Dev server startup, HTTP smoke tests, route discovery |
| `ship.go` | 563 | Final commit, changelog generation, ledger entry, demonstration |
| `coverage_gates.go` | 294 | Granularity, security, gap analysis, requirements coverage gates |
| `plan_check.go` | 220 | LLM-based plan quality review with stall detection |
| `plan_chunk.go` | 221 | Chunked plan generation: outline → wave expansion |
| `discuss_check.go` | 293 | Question quality (yes/no, vague, duplicate detection) and answer completeness |
| `execute_preflight.go` | 126 | Dependency validation, tool call loop detection |
| `execute_quality.go` | 120 | Per-task acceptance criteria verification |
| `ship_preflight.go` | 92 | TODO/FIXME detection, debug statement detection, hardcoded secret scanning |
| `research.go` | 115 | Pre-plan research with web search |
| `classify.go` | 135 | Prompt complexity classification |
| `intent.go` | 188 | LLM-based intent classification (feature/bugfix/refactor/question/etc.) |
| `init_deep.go` | 488 | Deep project analysis, environment preflight |

**Phase transition map:**
```
Idle       → Initialize
Initialize → Discuss | Execute | Idle
Discuss    → Plan | Execute | Idle
Plan       → Execute | Plan (retry) | Discuss | Idle
Execute    → Verify | Ship | Idle
Verify     → Runtime | Ship | Execute (heal) | Idle
Runtime    → Ship | Execute | Idle
Ship       → Idle
```

**Quality gates:**
- **Plan:** Granularity (task size), Security (auth/crypto file awareness), Gap Analysis (missing files/goals), Requirements Coverage (goal phrase coverage)
- **Discuss:** Question quality (yes/no, vague, duplicate detection via Jaccard similarity), Answer completeness (0-100 score)
- **Execute:** Pre-flight (dependency validation), Loop detection (repeated tool call signature), Per-task acceptance criteria
- **Ship:** Pre-ship (TODO/FIXME, debug statements, hardcoded secrets), Post-ship validation
- **Verify:** File existence, build/test execution, security file scanning

**Self-healing:** Failed tasks get error output fed back to the LLM with enhanced context (git diff, code intelligence, acceptance criteria), up to 2 retries. Git bisect used as fallback to find offending commit.

### Code Intelligence: `internal/codeintel/`

| File | Lines | Purpose |
|------|-------|---------|
| `codeintel.go` | 438 | Indexer facade: Build, Upstream, Downstream, Define, RelevantFiles, FormatContext |
| `parser.go` | 861 | Tree-sitter + regex parsers for 13 languages |
| `graph.go` | 415 | Import graph with BFS traversal, language-specific import resolution |
| `index.go` | 215 | Symbol index with hash map + trie for O(1)/O(K) lookups |
| `trie.go` | 148 | ASCII trie for prefix search |
| `relevance.go` | 240 | Multi-factor scoring: direct mention (+10), neighbors (+5), symbols (+7), same-package (+3), transitive deps (2/(depth+2)) |
| `cache.go` | 194 | Incremental index cache with mtime-based invalidation |

**Languages supported:** Go, TypeScript, JavaScript, Python, Rust, Java, C, C++, C#, Ruby, PHP, Swift, Kotlin (tree-sitter), with dedicated regex fallbacks for Python and Rust.

**Import resolution:** Go (go.mod), TypeScript (.ts/.tsx/.js/.jsx extensions), Python (__init__.py, relative imports), Rust (crate:: paths).

### TUI: `internal/tui/`

The TUI is a Bubble Tea application following the Elm architecture (Model → Update → View).

**AppState** is the single top-level model containing ~50+ fields including:
- Screen routing with back-stack (33 screens)
- Theme manager (11 themes)
- Key registry with leader key (`Ctrl+X`)
- Command registry (60+ slash commands)
- Provider/model state
- Workflow engine integration via `channelEmitter`
- Permission/question modals
- Subagent management
- Toast notifications
- Health monitoring

**Message flow:**
1. Sub-models emit `KeyActionMsg`, `SlashCommandMsg`, or `AppMsg`
2. `AppState.Update()` is the single dispatch point
3. Workflow engine communicates via buffered channel (128 messages) → `emitterCh`
4. `drainEmitterCmd()` reads one message per tick to prevent backpressure

**Key bindings:** Vim-style with leader key (`Ctrl+X` + chord). 18 key contexts covering all screens.

### Configuration: `internal/config/`

**6-layer loading cascade:**
1. `DefaultConfig()` — zero-valued defaults
2. Global TOML (`~/.m31a/config.toml`)
3. `.env` file auto-load from cwd
4. Environment variable overrides (`M31A_THEME`, `M31A_DEFAULT_MODEL`, etc.)
5. Project-level `m31a.toml` (walked up from cwd, max 3 levels)
6. `${VAR}` variable substitution → validation

**Hot-reload:** fsnotify primary + polling fallback (5s interval). 50ms debounce. `ConfigReloadMsg` carries reloaded config + error.

**24 boolean feature flags** controlling workflow phases, quality gates, and behavioral features.

### Session Management: `pkg/session/`

- Sessions stored as JSON + Markdown under `<workDir>/.m31a/`
- Atomic writes via temp-file + rename
- File locking for concurrent access
- Backup-on-overwrite (`.bak` suffix)
- Checkpoint pruning (keep 2 most recent)
- Markdown-based persistence for project state, tasks, plans
- Recent models MRU list with favorites

### Cross-Session Learning: `pkg/ledger/`

- Append-only markdown table at `~/.m31a/LEDGER.md`
- Dedup by SessionID
- Mtime-based stats cache with double-checked locking
- Stop-word filtered keyword extraction
- Aggregate statistics (avg tasks, avg cost, avg duration, top failures)

### Commit Rollback: `pkg/rollback/`

- Commit chain listing with diff preview
- Soft/hard/safe reset with backup branch creation
- Stash-if-dirty pattern
- O(1) commit counting via `git rev-list --count`

### Task Runner: `pkg/taskrunner/`

- Kahn's algorithm for topological sort with cycle detection
- Bounded parallelism via semaphore (default 4 concurrent)
- Per-task timeouts
- Self-healing retry with exponential backoff
- Dependency-aware skipping (failed deps → skip dependents)

### OS Keychain: `pkg/keychain/`

- Linux: D-Bus Secret Service + `pass` CLI fallback
- macOS: `/usr/bin/security` CLI
- Windows: Win32 `advapi32.dll` API with `unsafe.Pointer`
- Service name validation via regex to prevent command injection

### Context Consolidation: `pkg/autodream/`

- Reentrancy guard via `atomic.Bool` CAS
- Protected messages (first, system, tool calls, last 5) never consolidated
- Targets oldest 50% of non-protected messages
- Role-sampled summary with ~500 token budget
- Configurable threshold (default 60% context usage)

### Model Arbitrage: `pkg/arbitrage/`

- Keyword-based complexity classification (simple/moderate/complex)
- Token estimation by complexity tier + per-file bonus
- Price-sorted model ranking with threshold comparison
- Complex task context window guard (rejects <64K context models)

### Retry Logic: `pkg/retry/`

- Exponential backoff with configurable factor
- Retry-After header support (both ms and seconds)
- Error classification: ContextOverflow (no retry), RateLimit/Overloaded/ServerError/Network (retryable)
- Default: 3 attempts, 1s initial, 30s max, 2x factor

### Token Estimation: `internal/tokens/`

- tiktoken-go for OpenAI models
- Fallback: `utf8.RuneCountInString(text)/4 * 1.3`
- Lock-free EMA calibration via `atomic.Uint64`
- Per-message overhead (~4 tokens for metadata)

---

## Data Flow

### 1. User Input → LLM Response

```
User types in REPL
  → REPL model parses input
  → Intent classification (LLM or fallback heuristic)
  → If workflow-worthy: start workflow engine
  → Phase dispatch (Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship)
  → Each phase: build context → stream LLM → parse response → emit progress
  → LLM may call tools → Dispatcher executes → results fed back to LLM
  → Final phase: git commit → ledger entry → session save
```

### 2. Tool Execution Flow

```
LLM emits tool_call
  → Dispatcher.Execute()
  → Concurrency semaphore (max 8)
  → Rate limiter (token bucket)
  → Tool lookup
  → Risk-level rate limiter (for dangerous tools)
  → JSON input parsing
  → Permission check → modal if needed
  → Tool.Execute()
  → Output bounding
  → Metrics recording
  → Result returned to LLM
```

### 3. Provider Fallback Flow

```
Chat request fails
  → InspectResponse() extracts rate-limit/retry-after
  → If rate-limited: FindFallbackWithRetryAfter()
  → Else: FindFallbackProvider()
    → Parallel health checks (10s timeout)
    → Priority-based selection (live > slow > degraded)
    → TrySetActive() atomic switch
  → Retry request with new provider
  → Emit FallbackEvent to TUI
```

### 4. Session Persistence Flow

```
Session state changes
  → Manager.SaveSession() acquires file lock
  → Serialize to JSON
  → AtomicWrite() (temp file + sync + rename)
  → Release file lock
  → Messages saved separately via SaveMessages()
  → Workflow state saved via UpdateWorkflowState()
  → Checkpoints pruned to 2 most recent
```

---

## Control Flow

### Bubble Tea Event Loop

```
tea.NewProgram(app)
  → app.Init()
    → Session cleanup (remove old sessions)
    → Start background listeners (health, permissions, questions, subagents, file watcher, config watcher)
    → Restore or create session
  → app.Update(msg) [single dispatch point]
    → tea.WindowSizeMsg → layout recalculation
    → tea.KeyMsg → input handling with double Ctrl+C protection
    → StreamMsg/StreamDoneMsg → LLM response handling
    → PhaseResultMsg → workflow phase completion
    → TaskStartMsg/ToolStartMsg → progress updates
    → PermissionRequestMsg → modal display
    → ... (50+ message types)
  → app.View()
    → renderFrameWithTheme()
    → Command palette overlay (if open)
    → Permission modal overlay (if active)
    → Model selector overlay (if open)
    → Sidebar + main page composition
    → Toast overlay
```

### Workflow Phase Execution

```
RunPhase(ctx, phase, goal)
  → Phase transition validation (validPhaseTransitions map)
  → modelForPhase() resolves per-phase model
  → buildSystemPrompt() assembles system context
  → buildContext() assembles messages
  → preflightContextCheck() estimates tokens, auto-compacts if needed
  → Phase-specific handler:
    → Initialize: detect project, build code index, deep analysis
    → Discuss: LLM questions, quality check, completeness check
    → Plan: research → plan generation → plan checker → coverage gates
    → Execute: task runner (Kahn's sort) → per-task LLM+tools → self-heal
    → Verify: build/test → self-heal → bisect fallback → security scan
    → Runtime: dev server → smoke tests
    → Ship: commit → changelog → ledger → demonstration
  → PhaseResult returned with tasks, messages, usage, cost
```

---

## Important Design Patterns

### 1. Elm Architecture (Bubble Tea)
All TUI state lives in `AppState`. State mutations only happen in `Update()`. `View()` is a pure function of state. Background work returns `tea.Cmd` that produces `tea.Msg`.

### 2. Sentinel Error Pattern
24 sentinel errors in `internal/errors/errors.go` with `UserMessage()` function that maps errors to user-friendly strings via `errors.Is()` + regex fallback.

### 3. Atomic File Operations
All file writes use temp-file + `Sync()` + `Rename()` pattern to prevent corruption on crash. Used in session, ledger, metrics, config.

### 4. Singleflight Deduplication
Model cache refresh uses `golang.org/x/sync/singleflight` to ensure only one HTTP request is made even under concurrent access.

### 5. Token Bucket Rate Limiting
Two token bucket channels: general (20 burst / 10 sustained) and dangerous (5 burst / 2 sustained). Refilled by goroutine tickers.

### 6. Channel-Based Event Emission
Workflow engine communicates with TUI via buffered channel (128 messages). `drainEmitterCmd()` reads one message per tick to prevent backpressure while preserving Bubble Tea's single-threaded contract.

### 7. Reentrancy Guard
AutoDream uses `atomic.Bool` CAS to prevent nested `/compress` calls from double-summarizing.

### 8. Cascading Fallback
Edit tool uses 7-strategy replacement cascade: exact → line-trimmed → whitespace-normalized → indent-normalized → line-skip fuzzy → fuzzy anchor → Levenshtein similarity.

### 9. Defensive Copying
Message slices are defensively copied on entry/exit in AutoDream and session management to prevent aliasing bugs.

### 10. Platform Abstraction via Build Tags
Keychain (Linux/macOS/Windows), shell (Unix/Windows), file locking (Unix/Windows) all use build tags for platform-specific implementations.

---

## Critical Files

| File | Why Critical |
|------|-------------|
| `cmd/m31a/main.go` | Entry point; orchestrates entire startup, dependency injection, and shutdown |
| `internal/workflow/engine.go` | Core orchestration engine; 1300+ lines of phase dispatch, LLM streaming, prompt assembly |
| `internal/workflow/execute.go` | Most complex phase; 1000+ lines of tool dispatch, self-healing, loop detection |
| `internal/tui/app_state.go` | TUI state definition; 220+ fields, 30+ sub-models |
| `internal/tui/app_update.go` | Single dispatch point for all TUI messages; 1400+ lines |
| `internal/tools/dispatcher.go` | Tool execution pipeline with rate limiting, permissions, concurrency |
| `internal/provider/base_client.go` | Shared provider infrastructure; HTTP transport, caching, streaming |
| `internal/config/loader.go` | 6-layer config cascade with reflection-based merge and hot-reload |
| `internal/codeintel/codeintel.go` | Code intelligence facade; import graph, symbol index, relevance scoring |
| `pkg/session/manager.go` | Session lifecycle; 640+ lines of persistence, locking, cleanup |
| `pkg/taskrunner/runner.go` | Kahn's algorithm with bounded parallelism and self-healing |
| `internal/types/types.go` | Shared type vocabulary; all interfaces, enums, and core structs |

---

## Hidden Knowledge

1. **Tiktoken-go is unmaintained** since 2024. New tokenizers silently fall back to `RuneCountInString/4 * 1.3` heuristic (tokens/estimator.go:14).

2. **Bubble Tea is strictly single-threaded.** All state mutations must go through `Update()`. The workflow engine uses a channel-based emitter with 128-message buffer and one-per-tick draining to maintain this invariant.

3. **The `.force-exit` sentinel file** is written when the program is forcefully killed (5s timeout after SIGTERM). On next launch, it's detected and cleaned up, enabling session resume.

4. **Config hot-reload has a polling fallback** (5s interval) because fsnotify doesn't work reliably on all filesystems (NFS, Docker mounts).

5. **Edit tool's 7-strategy cascade** exists because LLMs frequently produce slightly different whitespace/indentation than the actual file content. The fuzzy matching with Levenshtein similarity (threshold 0.7) handles this gracefully.

6. **Plan-Discuss oscillation guard** limits Plan→Discuss transitions to 3 cycles to prevent infinite refinement loops.

7. **Code intelligence caches to disk** (`.m31a/codeintel.cache`) with mtime-based invalidation. Incremental rebuild only parses changed files.

8. **Session files are stored per-project** in `<workDir>/.m31a/` rather than globally in `~/.m31a/`. This enables project-local session isolation.

9. **The ledger uses append-only writes** (O_APPEND) rather than rewriting the entire file, fixing a performance bug (H9) with large ledgers.

10. **Metric collection is disabled by default** in the code path but enabled in config defaults (`MetricsEnabled: true`). The `Collector` returns immediately on every call when disabled.

11. **WebFetch uses DNS pinning** (resolves DNS once, pins the IP) to prevent TOCTOU DNS rebinding attacks. The DNS cache has a 5-minute TTL.

12. **Subagent profiles have allow/deny tool lists** that are applied by unregistering tools from the child dispatcher, not by filtering at call time.

---

## Risks

### Architectural Risks

1. **Single-binary constraint** means all 13 language parsers (tree-sitter grammars) are statically linked, increasing binary size. The tree-sitter dependency (`gotreesitter`) is a fork with limited maintenance.

2. **Channel-based workflow→TUI communication** has a fixed 128-message buffer. Under heavy parallel tool execution, messages are silently dropped (timeout-based), which could lose progress updates.

3. **No persistent plugin system.** Skills are discovered from filesystem but have no lifecycle management. The skill loader uses simple YAML frontmatter parsing without schema validation.

### Technical Debt

1. **`engine.go` is 1300+ lines** with 47+ fields on the `Engine` struct. This is a complexity hotspot that would benefit from decomposition.

2. **`app_update.go` is 1400+ lines** with a massive switch statement. Message handling could be decomposed into per-concern handlers.

3. **Config merge uses reflection** (`reflect.Value`, `reflect.Type`) which is fragile, hard to debug, and slow. A code-generated merge would be more reliable.

4. **Tiktoken-go dependency** is unmaintained. The fallback heuristic (`RuneCount/4 * 1.3`) may drift significantly from actual token counts for non-OpenAI models.

5. **Edit tool's 7-strategy cascade** is 860 lines of increasingly complex fuzzy matching. Each strategy adds maintenance burden and potential for false positives.

6. **Per-message overhead estimation** (~4 tokens) is a rough heuristic that doesn't account for tool call JSON serialization or role prefix variations across providers.

### Potential Bugs

1. **Race in provider registry:** `TrySetActive()` and `RollbackActive()` use separate lock acquisitions, which could allow a third goroutine to observe an intermediate state.

2. **Config hot-reload race:** The `sendReload()` function retries for 100ms before blocking, but if the TUI is busy processing, the config change could be delayed indefinitely.

3. **Stale worktree sweep** uses a 30-second timeout context. If the parent repo has many stale worktrees, this could timeout and leave orphans.

---

## Testing Strategy

### Unit Tests
- `go test -race ./...` with coverage profiling
- Coverage targets: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`
- Test files excluded from `errcheck` and `unused` linter rules

### E2E Tests (`e2e_test.go`)
- Binary-level tests: version, help, headless prompt mode
- Real API calls (skipped without env vars): Nvidia, Zen, OpenRouter
- Timeout handling verification

### Acceptance Tests (`scripts/verify_v1.sh`)
- 170-line bash script covering: build, test, coverage, cross-compile, docs, architecture compliance, TUI screens, workflow phases, tools, keychain, features, commands

### CI Pipeline
- **Lint:** gofmt, golangci-lint (govet, staticcheck, errcheck, ineffassign, unused), goreleaser validation
- **Test:** race-enabled with coverage upload
- **Security:** govulncheck
- **Build:** Matrix of 5 platform combinations (linux/darwin/windows × amd64/arm64, excluding windows/arm64)
- **Release:** GoReleaser on `v*` tags with draft release

---

## Future Development Guide

### Adding a New Tool
1. Create `internal/tools/mytool.go` implementing `types.Tool` interface
2. Add `ParameterSchema()` returning JSON Schema
3. Implement `Execute()` with proper error handling
4. Register in `DefaultDispatcher()` in `defaults.go`
5. Add to `BuildToolDefs()` in `tooldefs.go`
6. Add risk level, rate limiting profile
7. Write tests in `mytool_test.go`

### Adding a New Provider
1. Create `internal/provider/myprovider/client.go` implementing `provider.LLMProvider`
2. Embed `provider.BaseClient`
3. Implement `FetchModels()`, `ChatCompletionStream()`, `HealthCheck()`
4. Register in `cmd/m31a/main.go` provider registration
5. Add to `provider_registration.go` in TUI

### Adding a New Workflow Phase
1. Add phase constant to `internal/types/types.go`
2. Add to `validPhaseTransitions` map in `engine.go`
3. Implement `runMyPhase()` in new file `internal/workflow/myphase.go`
4. Add to `RunPhase()` dispatch in `engine.go`
5. Add phase model slot to `modelForPhase()`
6. Add TUI screen in `internal/tui/`
7. Add phase transition in `app_update_phase.go`

### Modifying Configuration
1. Add field to appropriate config struct in `internal/config/types.go`
2. Add default in `DefaultConfig()` in `loader.go`
3. Add validation in `validateConfig()` in `loader.go`
4. Add env var override if needed in `Load()` in `loader.go`
5. Document in `docs/CONFIG.md`

### Code Conventions
- No CGO (`CGO_ENABLED=0`)
- No emojis in code
- All file writes must be atomic (temp + rename)
- All shared state must be mutex-protected
- All errors returned, never panicked
- `gofmt`-clean, imports grouped (stdlib / third-party / project)
- Conventional commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`
