# M31 Autonomous Architecture

## Overview

M31 Autonomous is a terminal-native AI coding agent written in Go. It orchestrates a seven-phase workflow through a Bubble Tea TUI, streaming LLM responses from three providers with automatic fallback. The system executes 18 built-in tools gated by a permission system, produces verified git commits, and records cross-session learning.

---

## Directory Layout

```
.
├── cmd/m31a/                  # Entry point (flag parsing, config, provider registration, TUI launch)
│   ├── main.go                # Startup sequence, signal handling, headless mode
│   └── usage.go               # CLI help text, slash command listing
├── docs/                      # User-facing documentation
├── internal/                  # Private packages (not importable)
│   ├── codeintel/             # 4-language parser (Go, TypeScript, Python, Rust), import graph, relevance
│   ├── config/                # TOML loader (6-layer cascade), hot-reload, project context detection
│   ├── context/               # Dynamic context system, registry, estimation
│   ├── decision/              # Decision logging and receipts
│   ├── errors/                # Sentinel errors with user-friendly messages
│   ├── fileutil/              # Atomic file write operations
│   ├── git/                   # Git operations (commit, rollback, diff, stash, branch)
│   ├── log/                   # Structured logging with daily rotation
│   ├── provider/              # LLM provider abstraction
│   │   ├── interface.go       # LLMProvider interface (8 methods)
│   │   ├── base_client.go     # Shared HTTP transport, model cache, cost estimation
│   │   ├── cache.go           # Thread-safe model cache with TTL + stale + singleflight
│   │   ├── capabilities.go    # Heuristic capability detection from model ID
│   │   ├── fallback.go        # Parallel health checks, priority-based provider switching
│   │   ├── registry.go        # Thread-safe provider registry
│   │   ├── sse.go             # SSE stream parser with watchdog timer
│   │   ├── openrouter/        # OpenRouter client (300+ models)
│   │   ├── zen/               # Zen/OpenCode client
│   │   └── nvidia/            # Nvidia NIM client
│   ├── tokens/                # Token estimation (tiktoken + EMA calibration)
│   ├── tools/                 # 18 tools + dispatcher + permissions + subagents
│   │   ├── dispatcher.go      # Execution pipeline: concurrency, rate limiting, permissions
│   │   ├── defaults.go        # DefaultDispatcher: registers all 18 tools
│   │   ├── permissions.go     # Rule-based permission evaluation
│   │   ├── subagent/          # Parallel subagent manager (git worktree isolation, depth=2)
│   │   └── *.go               # Individual tool implementations
│   ├── tui/                   # Bubble Tea TUI (33 screens)
│   │   ├── app.go             # AppState definition, NewApp constructor
│   │   ├── app_update.go      # Single dispatch point for all messages
│   │   ├── app_view.go        # View rendering with theme support
│   │   ├── commands/          # Slash command registry and handlers
│   │   ├── components/        # Reusable TUI components (spinners, lists, modals)
│   │   ├── layout/            # Layout helpers (chrome, sidebar sizing)
│   │   ├── streaming/         # Streaming response rendering
│   │   ├── theme/             # Lipgloss theming (M31A dark theme)
│   │   └── tuitypes/          # TUI type definitions and interfaces
│   ├── types/                 # Shared types, constants, workflow phases, risk levels
│   └── workflow/              # Seven-phase orchestration engine
│       ├── engine.go          # Core engine (phase dispatch, LLM streaming, prompt assembly)
│       ├── initialize.go      # Project detection, git init, code index
│       ├── discuss.go         # LLM-generated questions, quality scoring
│       ├── plan.go            # Task breakdown, plan checker, coverage gates
│       ├── execute.go         # Tool dispatch, self-healing, loop detection
│       ├── verify.go          # Build/test verification, bisect fallback
│       ├── runtime.go         # Dev server, HTTP smoke tests
│       ├── ship.go            # Final commit, changelog, ledger entry
│       └── prompts/           # Embedded prompt templates
├── pkg/                       # Public packages (importable)
│   ├── autodream/             # Context consolidation with reentrancy guard
│   ├── arbitrage/             # Model-cost optimizer with task classification
│   ├── bisect/                # Git-bisect wrapper for model comparison
│   ├── compaction/            # Context compaction utilities
│   ├── coordinator/           # Drain management for workflow→TUI communication
│   ├── history/               # Frecent prompt history with scoring
│   ├── keychain/              # OS keychain abstraction (Linux/macOS/Windows)
│   ├── ledger/                # Cross-session learning store (markdown-backed)
│   ├── metrics/               # Metrics collection and reporting
│   ├── retry/                 # Retry logic with exponential backoff
│   ├── rollback/              # Commit-chain manager (soft/hard/safe reset)
│   ├── session/               # Session lifecycle, persistence, checkpointing
│   ├── skills/                # Skill discovery and management
│   └── taskrunner/            # Kahn's algorithm for topological sort, bounded parallelism
├── scripts/                   # verify_v1.sh acceptance suite
├── install.sh                 # One-liner installer
├── Makefile                   # Build/test/lint/release targets
└── .goreleaser.yaml           # Cross-compile + release config
```

---

## Dependency Rules

- `cmd/m31a/` imports only `internal/` and `pkg/`
- `internal/` may import `pkg/`
- `pkg/` must NOT import `internal/` (enforced by Go module system)
- `internal/types` is the shared type vocabulary across all layers

---

## Entry Point

`cmd/m31a/main.go` handles:

1. Flag parsing (`--version`, `--prompt`, `--goal`, `--model`)
2. Config loading (6-layer TOML cascade)
3. Provider registration (OpenRouter, Zen, Nvidia)
4. Session manager, tool dispatcher, git client creation
5. TUI construction with all dependencies injected
6. Signal handling (SIGTERM/SIGINT with graceful shutdown)
7. Headless mode (`--prompt` for single-shot LLM interaction)

---

## Provider Layer

### Interface (`internal/provider/interface.go`)

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```

### Implementations

- **OpenRouter** (`internal/provider/openrouter/`) — Aggregates 300+ models, configurable referer/title headers
- **Zen** (`internal/provider/zen/`) — Zen/OpenCode API provider with model enrichment
- **Nvidia** (`internal/provider/nvidia/`) — Nvidia NIM gateway with multimodal handling

### Shared Infrastructure

- **BaseClient** (`base_client.go`) — Shared HTTP transport (100 max idle conns, 10/host, 90s idle timeout), model lookup, cost estimation, stream iterator creation
- **Model Cache** (`cache.go`) — TTL-based in-memory cache (5 min fresh, 24h stale) with singleflight deduplication
- **Fallback** (`fallback.go`) — Parallel health checks (10s timeout), priority-based provider switching, rate-limit extraction
- **SSE Parser** (`sse.go`) — Server-Sent Events parser with watchdog timer for streaming responses
- **Capability Detection** (`capabilities.go`) — Heuristic inference from model ID (tool use, reasoning, vision)

---

## Workflow Engine

Seven-phase orchestration in `internal/workflow/`:

| Phase | File | Purpose |
|-------|------|---------|
| Initialize | `initialize.go` | Project detection, git init, code intelligence index, deep analysis |
| Discuss | `discuss.go` | LLM-generated clarifying questions, quality scoring, completeness checks |
| Plan | `plan.go` | Task breakdown, plan checker with revision loops, coverage gates, chunked generation |
| Execute | `execute.go` | Tool dispatch with self-healing, loop detection, per-task quality gates |
| Verify | `verify.go` | Build/test execution, self-healing, git bisect fallback, security scanning |
| Runtime | `runtime.go` | Dev server lifecycle, HTTP smoke tests, route discovery |
| Ship | `ship.go` | Pre-ship checklist, final commit, changelog generation, ledger entry |

**Engine** (`engine.go`) — Core orchestration: phase dispatch, LLM streaming, system prompt building, context management, checkpoint persistence.

**Workflow Modes** (`internal/types/types.go`):
- `auto` — Adaptive (default), classifies intent and chooses phases
- `full` — All 7 phases
- `fast` — Skip Plan phase
- `direct` — Skip Discuss, Plan, Verify

---

## Tools Layer

18 built-in tools registered in `internal/tools/defaults.go`:

| Tool | Risk | Purpose |
|------|------|---------|
| Bash | dangerous | Shell execution with dangerous command blocking |
| FileRead | safe | File reading with byte/line offsets, binary detection |
| FileWrite | medium | Atomic write with backup, append mode |
| Edit | medium | 7-strategy cascading replacement with fuzzy matching |
| Glob | safe | File pattern matching with ripgrep fallback |
| Grep | safe | Content search with regex, gitignore caching |
| WebFetch | safe | URL fetching with SSRF protection, HTML-to-markdown |
| WebSearch | safe | SearXNG search with DNS cache |
| CodeMap | safe | Code intelligence (upstream/downstream/define/references) |
| CodeComplexity | safe | Codebase complexity classification |
| FileDelete | destructive | File deletion with backup |
| FileMove | medium | File move with containment checks |
| FileList | safe | Tree-style directory listing |
| TodoWrite | safe | TODO.md management with sidebar notification |
| TodoRead | safe | TODO.md parsing |
| DevServer | dangerous | Dev server lifecycle (start/stop/restart/logs) |
| HTTPCheck | safe | HTTP request with status/body validation |
| AskUserQuestion | safe | Interactive user question with timeout |

**Dispatcher** (`dispatcher.go`) — Execution pipeline:
1. Concurrency semaphore (max 8 concurrent tools)
2. Rate limiter (token bucket: 20 burst / 10 sustained)
3. Risk-level rate limiter (dangerous tools: 5 burst / 2 sustained)
4. Permission check (rule evaluation → agent default → risk-level fallback)
5. Tool execution
6. Output bounding (2000 lines / 51200 bytes)

**Subagent System** (`tools/subagent/`):
- Max 8 concurrent subagents
- Git worktree isolation per subagent
- Max nesting depth: 2
- Per-subagent budgets: 50 tools, 50K tokens, 25 turns

---

## Context System

`internal/context/` — Dynamic context management:
- Registry for context providers
- Token estimation and budget management
- Context change detection and caching

---

## Decision System

`internal/decision/` — Decision logging and receipts:
- Records architectural decisions during workflow execution
- Snapshots available at ship phase
- Browseable via `/decisions` command

---

## Checkpoint System

Workflow state persistence:
- Plan state (content, version, refinement feedback)
- Conversation messages
- Intent classification results
- Dynamic context snapshots
- Decision log state

Checkpoints saved at phase transitions and available for resume after interruption.

---

## Knowledge System

Code intelligence in `internal/codeintel/`:
- 4-language parser (Go, TypeScript, Python, Rust) via regex fallbacks
- Import dependency graphs with BFS traversal
- Symbol index with hash map + trie for O(1)/O(K) lookups
- Relevance scoring (direct mention, neighbors, symbols, transitive deps)
- Incremental index cache with mtime-based invalidation

---

## Rollback System

`pkg/rollback/` — Git commit chain management:
- Commit listing with diff preview
- Soft/hard/safe reset with backup branch creation
- Stash-if-dirty pattern

---

## TUI Layer

Built with **Bubble Tea** (`tea.Program`) following the Elm architecture.

**AppState** (`internal/tui/app.go`) — Single top-level model with ~50+ fields:
- Screen routing with back-stack (33 screens)
- Theme manager (M31A dark theme)
- Key registry with leader key (`Ctrl+X`)
- Command registry (60+ slash commands)
- Provider/model state
- Workflow engine integration via channel-based emitter

**Message flow:**
1. Sub-models emit `KeyActionMsg`, `SlashCommandMsg`, or `AppMsg`
2. `AppState.Update()` is the single dispatch point
3. Workflow engine communicates via buffered channel (128 messages)
4. `drainEmitterCmd()` reads one message per tick (backpressure control)

**Themes:** Midnight, Daylight, Catppuccin Mocha, Nord Frost, Tokyo Night, Gruvbox Dark, Rose Pine, Dracula, Solarized Dark, Pure Mono, High Contrast

---

## Headless Mode

```bash
# Single prompt (no TUI)
m31a --prompt "What files are in the project?"

# Full workflow execution
m31a --goal "Create a REST API with authentication"
```

Bypasses the TUI entirely. Sends prompts directly to the active provider and prints responses to stdout.

---

## Release Flow

`/ship` phase:
1. Pre-ship checklist (TODO/FIXME detection, debug statement scanning)
2. Git commit with configured prefix
3. Changelog generation
4. Ledger entry (cross-session learning)
5. Session metrics recording
