# M31A — Deep Research Report

> **Generated:** 2026-06-13 | **Version:** v1.0.0 | **Status:** Core feature complete
> **Repository:** github.com/eshanized/M31A | **License:** MIT

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Project Identity & Philosophy](#2-project-identity--philosophy)
3. [Codebase Statistics](#3-codebase-statistics)
4. [Architecture Analysis](#4-architecture-analysis)
5. [TUI Layer Deep Dive](#5-tui-layer-deep-dive)
6. [Workflow Engine Deep Dive](#6-workflow-engine-deep-dive)
7. [Provider Layer Deep Dive](#7-provider-layer-deep-dive)
8. [Tool System Deep Dive](#8-tool-system-deep-dive)
9. [Public Packages Analysis](#9-public-packages-analysis)
10. [Configuration System](#10-configuration-system)
11. [Error Handling Strategy](#11-error-handling-strategy)
12. [Security Model](#12-security-model)
13. [Testing & Quality Assurance](#13-testing--quality-assurance)
14. [Performance Profile](#14-performance-profile)
15. [Strengths & Weaknesses](#15-strengths--weaknesses)
16. [Recommendations](#16-recommendations)

---

## 1. Executive Summary

M31A is a **terminal-based AI coding agent** written in **Go 1.24** that implements a structured six-phase software development workflow (Initialize → Discuss → Plan → Execute → Verify → Ship) powered by large language models. It is designed as a complete, autonomous coding assistant that runs entirely in the terminal with a polished Bubble Tea TUI.

The project is authored by **Eshan Roy** with **1,612 commits** and represents a significant engineering effort — **97,366 lines of Go** across **335 Go source files** (including **115 test files** with 21,656 test lines, yielding a **32% test-to-code ratio**).

### Key Differentiators

- **Six-Phase Workflow Engine**: A deterministic, phase-gated development cycle that mirrors professional software engineering practices — not just a chat interface
- **Dual-Provider Architecture**: First-class support for both OpenRouter and OpenCode Zen gateways with transparent auto-fallback, model discovery, and cost arbitrage
- **Bubble Tea TUI with 10 Screens**: An immersive terminal UI with dark/light themes, model selector, plan review, execution tracking, verification dashboards, and session management
- **Inline Thinking System**: Streaming reasoning display for models that support explicit thinking (DeepSeek R1, OpenAI o-series, Anthropic Claude extended thinking)
- **Cross-Session Learning Ledger**: Persistent pattern tracking and learning across sessions
- **Git Bisect Integration**: Automated debugging through commit history with self-heal mechanisms
- **Cost Arbitrage Engine**: Real-time model pricing comparison and recommendation across providers
- **AutoDream Context Consolidation**: Smart conversation pruning to prevent context window saturation
- **OS Keychain Integration**: Secure credential storage via dbus (Linux), Keychain (macOS), and wincred (Windows)

---

## 2. Project Identity & Philosophy

### 2.1 Design Philosophy: "Gemini in the Terminal, Spec-Driven in the Engine"

M31A is engineered around four core UX pillars and six engine principles:

**UX Pillars:**
- **Spatial Clarity**: Every pixel of terminal space is intentional — generous padding, visual hierarchy, whitespace management
- **Inline Cognition**: Model reasoning is surfaced as a first-class, collapsible artifact within the message stream
- **Fluid Motion**: All state transitions use eased animations (spinner→text, thinking→answer, tool call→result)
- **Contextual Density**: Model badges, token counters, and cost estimates appear only when relevant

**Engine Principles:**
1. **Six-phase cycle** — Initialize → Discuss → Plan → Execute → Verify → Ship
2. **Context rot prevention** — Each phase gets a fresh, pruned context window
3. **File-based state** — All planning data in `planning/` as readable Markdown/JSON
4. **Parallelize independent work** — Task dependency graph with topological sort
5. **Deterministic logic in code, not prompts** — Native Go tool implementations
6. **Auto-verify with self-healing** — Verification triggers automatic diagnostic + remediation loop

### 2.2 FOSS Mandate

M31A is MIT-licensed, free, open source, and community-driven. No vendor lock-in, no telemetry, no paid tiers. Design decisions prioritize user sovereignty over monetization.

---

## 3. Codebase Statistics

### 3.1 Overview

| Metric | Value |
|--------|-------|
| **Language** | Go 1.24 |
| **Total Go Source Files** | 335 |
| **Lines of Go Code** | 97,366 |
| **Test Files** | 115 |
| **Test Lines of Code** | 21,656 |
| **Test-to-Code Ratio** | 32% |
| **Functions** | ~2,519 |
| **Structs** | ~376 |
| **Interfaces** | ~48 |
| **Total Commits** | 1,612 |
| **Branch** | master |
| **Cross-platform targets** | linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64, windows/arm64 |

### 3.2 Package Breakdown

| Package | Files | Lines of Code | Test Files | Test Lines |
|---------|-------|--------------|------------|------------|
| `internal/tui/` | ~109 | ~15,000 | ~20 | ~4,500 |
| `internal/tools/` | ~40 | ~8,500 | ~10 | ~2,800 |
| `internal/workflow/` | ~32 | ~4,500 | ~14 | ~4,200 |
| `internal/provider/` | ~21 | ~5,200 | ~8 | ~1,800 |
| `internal/types/` | 6 | ~1,200 | 3 | ~600 |
| `internal/config/` | ~8 | ~2,500 | ~3 | ~800 |
| `internal/errors/` | ~3 | ~800 | ~1 | ~200 |
| `internal/fileutil/` | ~3 | ~300 | ~1 | ~100 |
| `internal/git/` | ~2 | ~400 | ~1 | ~150 |
| `internal/log/` | ~2 | ~200 | ~1 | ~50 |
| `internal/tokens/` | ~2 | ~400 | ~1 | ~150 |
| `pkg/session/` | ~8 | ~3,200 | ~4 | ~1,200 |
| `pkg/taskrunner/` | 2 | ~800 | ~2 | ~400 |
| `pkg/ledger/` | 1 | ~400 | ~2 | ~200 |
| `pkg/rollback/` | 1 | ~300 | ~2 | ~200 |
| `pkg/arbitrage/` | 1 | ~300 | ~1 | ~100 |
| `pkg/autodream/` | 1 | ~350 | ~2 | ~200 |
| `pkg/bisect/` | 1 | ~200 | ~1 | ~100 |
| `pkg/keychain/` | 3 | ~500 | ~1 | ~100 |
| `cmd/m31a/` | 3 | ~500 | — | — |
| **Total** | **~335** | **~97,366** | **~115** | **~21,656** |

### 3.3 Dependency Graph

```
Module: github.com/eshanized/M31A (Go 1.24)

Core Dependencies:
├── github.com/BurntSushi/toml v1.6.0        — Config parsing
├── github.com/bmatcuk/doublestar/v4 v4.10.0 — Glob matching
├── github.com/charmbracelet/bubbles v0.20.0 — TUI components
├── github.com/charmbracelet/bubbletea v1.3.0 — TUI framework
├── github.com/charmbracelet/glamour v0.6.0  — Markdown rendering
├── github.com/charmbracelet/lipgloss v1.1.0 — Style engine
├── github.com/godbus/dbus/v5 v5.2.2         — Linux keychain
├── github.com/pkoukk/tiktoken-go v0.1.8     — Token estimation
├── golang.org/x/sync v0.10.0                — Concurrency primitives
├── github.com/atotto/clipboard v0.1.4       — Clipboard access
└── github.com/fsnotify/fsnotify v1.10.1     — File watching

Indirect (key):
├── github.com/alecthomas/chroma v0.10.0     — Syntax highlighting
├── github.com/microcosm-cc/bluemonday v1.0.21 — HTML sanitizer
├── github.com/yuin/goldmark v1.5.2          — Markdown parser
├── github.com/olekukonko/tablewriter v0.0.5 — Table rendering
└── golang.org/x/net v0.0.0-20221002022538   — Networking
```

---

## 4. Architecture Analysis

### 4.1 Layered Architecture

M31A follows a strict **layered architecture** with 0 circular dependencies and a maximum import depth of 5 layers:

```
Layer 0: cmd/m31a/main.go
    ↓
Layer 1: internal/tui/ (presentation)
    ↓
Layer 2: internal/workflow/ (orchestration)
    ↓
Layer 3: internal/tools/ + internal/provider/ (capabilities)
    ↓
Layer 4: pkg/ (infrastructure: session, ledger, rollback, etc.)
    ↓
Layer 5: internal/types/, internal/config/, internal/errors/ (foundation)
```

**Cross-cutting concerns** (used by all layers):
- `internal/log/` — Structured logging
- `internal/fileutil/` — Atomic file operations
- `internal/git/` — Git abstraction

### 4.2 Data Flow Patterns

**Primary data flow (six-phase workflow):**
```
User Goal → Initialize → Discuss → Plan → Execute → Verify → Ship
                ↓            ↓         ↓        ↓         ↓       ↓
           PROJECT.md  PROJECT.md  TASKS.md  STATE.md  TASKS.md  ARCHIVE
                ↑            ↑         ↑        ↑         ↑       ↑
              File-based state persists between phases in sessions/<id>/planning/
```

**Streaming data flow (LLM interaction):**
```
User Input → Bubble Tea update loop → Workflow Engine → Provider Client
                                                           ↓
                                                     HTTP POST /chat/completions
                                                           ↓
                                                     SSE stream parsing
                                                           ↓
                                                     Thinking/Content segment routing
                                                           ↓
                                                     Tool call detection & dispatch
                                                           ↓
                                                     TUI re-render (throttled 5fps)
```

**Session persistence flow:**
```
Session.Create() → ~/.m31a/sessions/<id>/
    ├── session.json     (metadata, messages, config)
    ├── planning/
    │   ├── PROJECT.md   (goal, requirements)
    │   ├── TASKS.md     (task breakdown)
    │   └── STATE.md     (execution state)
    ├── checkpoints/     (save states for rollback)
    ├── backups/         (pre-modification file backups)
    └── archived/        (shipped/finalized sessions)
```

### 4.3 Concurrency Model

- **Bubble Tea single-threaded event loop**: All TUI state mutations go through `Update()` — the canonical single goroutine
- **Goroutine pool for parallel tool execution**: Bounded concurrency (max 4 concurrent), controlled by `internal/tools/subagent/`
- **Streaming via channels**: SSE parser runs in its own goroutine, emits typed events through `chan StreamChunk`
- **Provider fallback**: Parallel health checks via `errgroup`
- **Session operations**: RWMutex for concurrent-safe reads, sync.Map for global caches
- **Lock-free atomics**: Token estimators use `atomic.Uint64` with `math.Float64bits` to avoid mutex contention

---

## 5. TUI Layer Deep Dive

### 5.1 Screen Architecture (10 Screens)

`internal/tui/` — ~109 files, ~15,000 LOC — the largest package in the codebase.

| Screen | File(s) | Purpose |
|--------|---------|---------|
| **REPL** | `repl.go`, `repl_state.go`, `repl_view.go`, `repl_model.go`, `repl_keys.go`, `repl_stream.go`, `repl_thinking.go`, `repl_commands.go`, `repl_welcome.go`, `repl_clipboard.go`, `repl_quickactions.go` | Main chat interface — message stream, input area, model badge, thinking blocks, tool cards |
| **Dashboard** | `dashboard_model.go` | Session statistics and performance metrics |
| **Settings** | `settings_model.go`, `settings_view.go`, `settings_edit.go`, `settings_tabs.go`, `config_model.go` | Configuration with tabbed interface |
| **Plan** | `plan_model.go`, `plan_view.go`, `plan_refine.go` | Task breakdown review with dependency graph |
| **Execute** | `execute_model.go`, `execute_view.go` | Live task execution with progress, tool cards, and thinking display |
| **Verify** | `verify.go` | Verification results with pass/fail per task and self-heal options |
| **Ship** | `ship_model.go`, `ship_view.go` | Final commit summary and session archive |
| **Diff** | `diff_model.go`, `diff_view.go` | Side-by-side and unified diff viewer |
| **Resume** | `resume_model.go`, `resume_view.go` | Session browser with search and filtering |
| **Model Selector** | `modelselector.go`, `modelselector_view.go`, `modelselector_list.go` | Full-screen model search with provider filtering, pricing, and capability badges |
| **Theme Picker** | `themepicker_model.go` | Visual theme selection |
| **Metrics** | `metrics.go` | Session history statistics |

**Supporting models (not full screens):**
- `notification_model.go` — Toast notification system (3 visible max, slide-in animation, auto-dismiss with progress bar)
- `header.go` — Persistent header with model badge, context bar, provider tag
- `statusbar.go` — Status bar with connection state, operation status
- `sidebar.go` — File explorer sidebar with tree navigation
- `mention.go` — `@file` mention autocompletion with fuzzy search
- `cmdpalette.go` — Command palette with slash command browsing
- `goalinput.go` — Goal input widget for workflow initiation
- `help.go` — Scrollable help screen with 8 context-sensitive sections
- `toast.go` — Toast notification rendering
- `truncate.go` — Output truncation utilities
- `health.go` — Connection health monitoring
- `history.go` — Input history with frecency search

### 5.2 Rendering Architecture

The TUI uses a **model-view-update** architecture per Bubble Tea conventions:

- **`app.go`**: Root model — holds program instance, screen stack, initialization
- **`app_state.go`**: Central state — active screen, theme, dimensions, all sub-models
- **`app_update.go`**: Main update loop (~2,191 lines) — routes messages to active screen, handles window resize, key dispatch, tick events, theme propagation
- **`app_view.go`**: Root view — composites active screen into viewport with overlays (permissions, fallback banners, notifications)
- **`app_channel.go`**: Channel-based event bus for goroutine→TUI communication
- **`app_update_commands.go`**: tea.Cmd factory functions
- **`app_update_phase.go`**: Phase-related update logic
- **`transition.go`**: Phase transition animations and screen stack management

**Render optimization** (recently added):
- **5fps throttle** in `repl_state.go` via `lastRenderTime` + `minRenderInterval` — prevents viewport rebuild on every streaming token
- **Message rendering cache** — rendered strings cached per message hash (planned per architecture recommendations)
- **Welcome screen cache** — static content cached, not rebuilt per render

### 5.3 Key Components

**`components/message.go`**: Renders assistant messages with:
- Content segments (normal text with glamour markdown)
- Thinking segments (collapsible `[v]`/`[^]` toggle, duration badge, gray background)
- Tool call segments (rich cards with status, output, timing)
- Error segments (red-bordered cards with error details)

**`sidebar.go`**: File explorer with:
- Tree-based directory navigation
- `@file` mention integration
- Git status indicators
- File size and line count metadata
- Map-based child lookup for O(1) traversal (recently optimized from O(F×P×C))

**`keybindings.go`** + `keybindings_screens.go`: Centralized keybinding definition with:
- 200+ configurable bindings
- Leader key system (Ctrl+X with configurable timeout)
- Which-key overlay
- Per-screen context-sensitive bindings
- Screen stack routing

---

## 6. Workflow Engine Deep Dive

### 6.1 Six-Phase Cycle

`internal/workflow/` — ~32 files, ~4,500 LOC — the orchestrator of the entire system.

#### Phase 1: Initialize (`initialize.go`)
- **Trigger**: User types a goal or `/workflow <goal>`
- **Actions**: Parse goal, detect project type (package.json, go.mod, Cargo.toml, etc.), scaffold session directory, auto `git init` if not a repo
- **Output**: `PROJECT.md` created in `sessions/<id>/planning/`

#### Phase 2: Discuss (`discuss.go`)
- **Trigger**: Auto after Initialize
- **Actions**: LLM generates 2-4 clarifying questions, rendered inline for user to answer
- **Output**: `PROJECT.md` updated with Q&A
- **Key mechanism**: Questions parsed from numbered markdown list in LLM response; `skip` triggers sensible defaults

#### Phase 3: Plan (`plan.go`, `plan_parser.go`)
- **Trigger**: Auto after Discuss or `/plan`
- **Actions**: LLM produces JSON task array → schema-validated → dependency graph built → rendered in Plan screen
- **Output**: `TASKS.md` with task breakdown
- **Validation**: Circular dependency detection (iterative DFS), self-reference checks, dangling reference detection
- **Retry**: Max 3 retries on parse failure; user can manually edit or skip to REPL

#### Phase 4: Execute (`execute.go`)
- **Trigger**: User approves plan or `/execute`
- **Actions**: Tasks run in dependency order (sequential in V1, parallel in V1.1); each task gets atomic commits
- **Key mechanism**: Per-task context pruning — only current task spec + dependency outputs in LLM context
- **Self-heal**: `healTask()` — sends failure details + original spec + current file state back to LLM; max 2 heal attempts

#### Phase 5: Verify (`verify.go`, `engine_verify.go`)
- **Trigger**: Auto after Execute completes
- **Checks**: File existence, syntax validation, test execution, import/export consistency
- **Actions**: Each task scored PASS/FAIL; self-heal or bisect on failure
- **Bisect integration**: `tryBisectHeal()` — uses `pkg/bisect` to find the commit that introduced the failure

#### Phase 6: Ship (`ship.go`)
- **Trigger**: Auto after Verify passes
- **Actions**: Final git commit, update ledger, summary banner, session archived to `sessions/<id>/archived/`

### 6.2 Engine Core (`engine.go`)

The central `Engine` struct orchestrates phase transitions:

```go
type Engine struct {
    started     bool
    phase       WorkflowPhase       // Current phase
    phases      []WorkflowPhase     // Phase pipeline
    stateDir    string              // Session state directory
    gitClient   types.GitClient     // Git abstraction
    // Cached state (recently optimized)
    plan        *Plan               // Parsed task plan
    modelInfo   *types.ModelInfo    // Current model metadata
    // ...
}
```

**Key methods:**
- `RunPhase()` — dispatches to phase-specific handler
- `Transition()` — validates and executes phase transition
- `consumeStream()` — processes SSE stream chunks
- `consumeStreamWithTools()` — stream processing with tool call detection
- `streamLLM()` — initiates LLM streaming request
- `HealTask()` — self-heal loop for failed tasks
- `preflightContextCheck()` — estimates context usage before LLM calls

### 6.3 Plan Parsing (`plan_parser.go`, `engine_parse.go`)

The plan parser is responsible for extracting structured tasks from LLM responses:

- **`ParsePlan()`** — Main entry: section extraction → review notes → proposed changes → task list
- **`parseTasksFromJSON()`** — JSON task array parsing with validation
- **`validateTasks()`** — Dependency graph validation (3 build validators)
- **`hasCycle()`** — Iterative DFS cycle detection (O(V+E) optimal)
- **`extractJSONObject()`** — Brace-matching JSON extraction from LLM output
- **`stripJSONComments()`** — Comment removal with string literal awareness
- **`normalizeTrailingCommas()`** — JSONC-style trailing comma normalization

### 6.4 Prompt Templates

`internal/workflow/prompts/` — 9 prompt templates:

Each phase has a dedicated prompt template injected into the LLM context:
- `initialize.md`, `discuss.md`, `plan.md`, `execute.md`, `verify.md`, `ship.md`
- Supporting: `tool_definitions.md`, `system.md`, `plan_fix.md`

---

## 7. Provider Layer Deep Dive

### 7.1 Dual-Provider Architecture

`internal/provider/` — ~21 files, ~5,200 LOC.

The provider layer implements a clean abstraction over two LLM gateways:

```go
// interface.go
type LLMProvider interface {
    Name() string
    FetchModels() ([]types.ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*StreamIterator, error)
    EstimateCost(usage types.Usage) float64
}
```

**OpenRouter Client** (`openrouter/client.go`):
- Base URL: `https://openrouter.ai/api/v1`
- Implements retry with 3 attempts, exponential backoff
- Fetches model catalog with pricing, context length, architecture, top_provider
- Health check every 60s via `/v1/models` (lightweight)

**Zen Client** (`zen/client.go`):
- Base URL: `https://opencode.ai/zen/v1`
- OpenAI-compatible endpoints
- No retry logic in current implementation (identified as gap)

**Provider Registry** (`registry.go`):
- Thread-safe provider registration and activation
- RWMutex for concurrent reads
- Active provider switching

**Fallback** (`fallback.go`):
- Sequential provider health checks on failure (identified as bottleneck — should be parallelized)
- Configurable fallback chain
- Timeout-based failover

### 7.2 Model Discovery & Caching

**`cache.go`**: TTL-based model cache (5-minute refresh):
- `Refresh()` — Fetches model catalog from active provider
- `Get()` — Thread-safe model lookup
- `Models()` — Returns all cached models
- Uses `singleflight` from `golang.org/x/sync` to coalesce concurrent refresh requests

**`capabilities.go`**: Model capability detection:
- Architecture detection (deepseek, openai, anthropic, qwen, gemini, mistral)
- Feature flag extraction (tools, reasoning, vision)
- Context length normalization

### 7.3 Inline Thinking System (`reasoning.go`)

The reasoning system normalizes per-architecture thinking parameters into a unified streaming format:

```go
type ReasoningConfig struct {
    ModelFamily   string         // "deepseek" | "openai" | "anthropic" | "qwen"
    RequestParams map[string]any // Parameters injected into request body
    SSEField      string         // JSON path for reasoning token extraction
}
```

**Per-architecture thinking support:**
- **DeepSeek**: Automatic reasoning; SSE field `choices[0].delta.reasoning_content`
- **OpenAI o-series**: Configurable `reasoning_effort` (low/medium/high); SSE field `choices[0].delta.reasoning`
- **Anthropic Claude**: `thinking` block with `budget_tokens`; SSE field `choices[0].delta.content` with type discriminator
- **Qwen**: Automatic for thinking-enabled variants; SSE field `choices[0].delta.reasoning_content`

**SSE Parsing (`sse.go`)**:
- Custom SSE parser with `strings.Builder` for event accumulation
- Watchdog timer for stream health monitoring
- Configurable scanner buffer (currently 1MB — could be reduced to 64KB)

### 7.4 Wire Format (`common.go`)

Request/response serialization layer:
- `messagesToWire()` — Message array to wire format
- `BuildChatBody()` — Full request body construction
- `toolCallToWire()` — Tool call serialization
- `IsContextExceeded()` — Error message parsing for 413/context window exceeded
- **Identified optimization**: Currently uses `map[string]any` throughout — switching to typed structs would reduce allocations 3-5x

---

## 8. Tool System Deep Dive

### 8.1 Tool Interface & Registry

`internal/tools/` — ~40 files, ~8,500 LOC.

```go
// interface.go
type Tool interface {
    Name() string
    Description() string
    Execute(ctx context.Context, params map[string]any) (result string, err error)
    Parameters() json.RawMessage
}
```

**Dispatcher** (`dispatcher.go`):
- Central tool execution hub with permission checking, input validation, and output formatting
- Double JSON unmarshal identified as MEDIUM optimization opportunity
- Retry logic for transient tool failures

### 8.2 Implemented Tools (13 total)

| Tool | File | Purpose | Risk Level |
|------|------|---------|------------|
| **Bash** | `bash.go` | Shell command execution, 30-min timeout, signal forwarding | Dangerous |
| **FileRead** | `fileread.go` | File reading with binary detection, 5MB limit, line counting | Safe |
| **FileWrite** | `filewrite.go` | Atomic write with backup, directory auto-creation | Destructive |
| **Edit** | `edit.go` | Search/replace with cascading strategy (exact→trimmed→whitespace-normalized→fuzzy) | Destructive |
| **Glob** | `glob.go` | File pattern matching via doublestar | Safe |
| **Grep** | `grep.go` | Code search via ripgrep (fallback to pure Go) | Safe |
| **WebFetch** | `webfetch.go` | URL content fetch with HTML→Markdown conversion, SSRF protection | Safe |
| **FileDelete** | `filedelete.go` | File deletion with backup | Destructive |
| **FileMove** | `filemove.go` | File rename/move | Destructive |
| **FileList** | `filelist.go` | Directory listing | Safe |
| **TodoWrite** | `todo.go` | Task list management | Safe |
| **AskUser** | `question.go` | User question modal with timeout | Safe |
| **Agent** | `agent.go` | Subagent goroutine spawning | Dangerous |

### 8.3 Edit Strategies (`edit.go`)

The Edit tool implements a sophisticated cascading strategy for source code modifications:

1. **Exact match** (`cascadingReplace` — first attempt)
2. **Line-trimmed match** (`lineTrimmedReplace` — whitespace-insensitive)
3. **Whitespace-normalized match** (`whitespaceNormalizedReplace` — all whitespace flexible)
4. **Fuzzy anchor match** (`fuzzyAnchorReplace` — Levenshtein distance with context lines)

Each strategy falls through to the next on failure. Total pipeline complexity: O(N × L × K) where K = 4 strategies.

### 8.4 Permission System (`permissions.go`)

- **Actions**: `ask`, `allow`, `deny`
- **Risk levels**: `low`, `medium`, `high`, `destructive`
- **Per-tool rules** with glob patterns
- **Per-agent profiles** (main vs subagent)
- **Rate limiting** per tool
- **Permission timeout** (configurable, default 300s)
- **"Allow always"** and **"Allow for session"** scoping

### 8.5 Output Formatting

Tool call outputs render as **rich cards** in the TUI:
- Color-coded badge per tool type (Bash = amber, FileRead = blue, FileWrite = brand, etc.)
- Auto-collapse for outputs >20 lines with `[+N lines]` toggle
- 10,000 character output cap with truncation message
- Binary content detection — shows `[binary file, size]`
- Status badges: `[..]` spinner → `[OK]` (green) or `[ERR]` (red)
- Syntax highlighting via Glamour/Chroma

---

## 9. Public Packages Analysis

### 9.1 `pkg/session/` — Session Management (~8 files, ~3,200 LOC)

The largest public package. Manages the full session lifecycle:

- **`manager.go`** (904 lines): Core session operations — Create, Save, Load, List, Delete, Fork, Resume
- **`checkpoint.go`**: Checkpoint save/load for undo support
- **`types.go`**: Session metadata structures

Key features:
- File-based persistence in `~/.m31a/sessions/<uuid>/`
- Session forking (full deep copy)
- Session listing with search/filter
- Recent model tracking
- Backup management (atomic with rollback on failure)
- Configurable cache TTL via `ManagerOpts`
- **Known optimization**: `ListSessions()` reads every `session.json` — O(N×fileSize). A `sessions_index.json` index would reduce to O(1).

### 9.2 `pkg/taskrunner/` — Task Execution Engine (~2 files, ~800 LOC)

Dependency-aware task scheduler:

- **`runner.go`**: Core scheduling with topological sort, group-based execution, retry logic, status tracking
- Task states: Pending → Running → Completed / Failed / Skipped
- Dependency graph built via Kahn's algorithm
- **Known issue**: 3 data race conditions in status reading (identifed as M44-M46)

### 9.3 `pkg/ledger/` — Cross-Session Learning (~1 file, ~400 LOC)

Persistent ledger for tracking patterns across sessions:

- `Append()` — Add entry with deduplication (currently O(N) rewrite — should be append-only)
- `Entries()` — Sorted listing (re-sorts every call — cache recommended)
- `Stats()` — Aggregated statistics by type
- `Search()` — Full-text search across entries
- **Critical optimization**: Rewrites entire file on every append — O(N) I/O. Append-only with `O_APPEND|O_WRONLY` reduces to O(1).

### 9.4 `pkg/rollback/` — Commit Rollback (~1 file, ~300 LOC)

Git history navigation and rollback:

- `Chain()` — Builds commit chain with diffs (spawns `git diff` per commit — optimize with `git log --patch`)
- `countCommitsBetween()` — Commit counting via `git rev-list --count`
- `Checkout()` — Safe checkout with branch protection
- Hard/soft reset options

### 9.5 `pkg/arbitrage/` — Model Cost Arbitrage (~1 file, ~300 LOC)

Real-time model pricing optimization:

- `Score()` — Multi-factor model scoring (cost, context length, capabilities)
- `CompareModels()` — Side-by-side model comparison
- `Recommend()` — Top-N model recommendation with context requirements
- `EstimateTokens()` — Token estimation for prompt/files
- **Optimization needed**: O(M²) nested loop in `Recommend()` — hashmap key would reduce to O(M).

### 9.6 `pkg/autodream/` — Context Consolidation (~1 file, ~350 LOC)

Intelligent conversation pruning:

- `Consolidate()` — Summarizes older messages into condensed form
- `protectedIndices()` — Identifies messages to preserve verbatim (system prompts, tool results)
- `candidateIndices()` — Identifies messages safe for consolidation
- `Stats()` — Token count estimation for all messages

### 9.7 `pkg/bisect/` — Git Bisect Integration (~1 file, ~200 LOC)

Automated binary search through commit history:

- `Run()` — Orchestrates `git bisect` with custom check command
- Poll-based iteration (O(N·logN × C))

### 9.8 `pkg/keychain/` — OS Keychain (~3 files, ~500 LOC)

Cross-platform secure credential storage:

- `keychain.go` — Unified interface
- `keychain_linux.go` — D-Bus Secret Service via `godbus/dbus`
- `keychain_darwin.go` — macOS Keychain via cgo
- `keychain_windows.go` — Windows Credential Manager
- **Known issue**: Every operation opens a new D-Bus connection on Linux — connection caching would improve performance.

---

## 10. Configuration System

### 10.1 Configuration Layering

`internal/config/` — 5-layer precedence:

1. **Default values** (hardcoded in `defaults.go`)
2. **Global config** (`~/.m31a/config.toml`)
3. **Environment variables** (parsed from `.env`)
4. **Environment overrides** (`M31A_*` prefixed env vars)
5. **Project config** (walks up 3 parent dirs looking for `.m31a.toml`)

**Variable substitution**: Config values support `${VAR}` and `$VAR` interpolation from environment.

### 10.2 Configuration Schema

Key configuration sections (defined in `internal/types/constants.go` and loaded in `internal/config/`):

| Section | Key Fields |
|---------|------------|
| `[provider]` | `default`, `auto_fallback` |
| `[provider.openrouter]` | `api_key`, `base_url` |
| `[provider.zen]` | `api_key`, `base_url` |
| `[model]` | `default`, `context_warning_threshold`, `show_thinking_by_default`, `auto_collapse_tools`, `auto_arbitrage` |
| `[ui]` | `theme` (dark/light/auto), `compact_mode`, `show_token_usage`, `show_cost_estimate` |
| `[permissions]` | `default_mode` (ask/allow/deny), `timeout_seconds` |
| `[features]` | `autodream_enabled`, `subagent_enabled`, `auto_backup`, `resume_on_startup` |
| `[ledger]` | `enabled`, `max_entries` |
| `[ghost]` | `enabled` (V1.1 feature) |

### 10.3 Configuration Features

- **TOML format** with full validation
- **Graceful degradation** — unknown keys warn but don't crash
- **Config diff** — `/settings diff` shows differences from defaults
- **Config freeze** — export current effective config to a file
- **Accent color override** — hex color for UI accent
- **Border style selection** — rounded, double, thin, split

---

## 11. Error Handling Strategy

### 11.1 Error Architecture (`internal/errors/`)

M31A uses **sentinel errors** with user-friendly message mapping:

```go
var (
    ErrPlanValidationFailed  = errors.New("plan validation failed")
    ErrProviderNotAvailable  = errors.New("provider not available")
    ErrModelNotFound         = errors.New("model not found")
    ErrSessionLoadFailed     = errors.New("session load failed")
    // ... 12+ sentinel errors total
)
```

**Error classification**:
- **User-facing errors**: Mapped to `UserMessage()` with recovery hints
- **Internal errors**: Logged via structured logger with stack traces
- **Recoverable errors**: Provider timeouts, rate limits → auto-retry with backoff
- **Fatal errors**: Config parse failures, missing API keys → graceful shutdown with diagnostic message

### 11.2 Error Categories

| Category | Examples | Handling |
|----------|----------|----------|
| Provider | Timeout, 429, 401, context exceeded | Retry (3 attempts), fallback to alternate provider |
| Tool | Permission denied, file not found, invalid params | Return error to LLM, display in tool card |
| Workflow | Plan validation failure, task execution error | Self-heal loop (2 attempts), bisect fallback |
| Config | Missing key, invalid TOML, unknown key | Warning + graceful default, never crash |
| Session | I/O error, corrupt JSON, permission error | Recover from backup, user notification |

### 11.3 Error Display

Errors render as:
- **Inline error segments** in message stream (red-bordered cards)
- **Plain error banners** (recently fixed from ANSI escape mangling)
- **Fallback banners** when provider is unreachable (dismiss with `x`)
- **Toast notifications** for transient errors (auto-dismiss with progress bar)

---

## 12. Security Model

### 12.1 API Key Protection

- **OS keychain storage**: Linux (dbus secret service), macOS (Keychain), Windows (wincred)
- **In-memory**: Stored in secure string type with explicit zeroing
- **Logging**: API keys redacted in all logs (14 subtests verify this)
- **Config file**: Optional plaintext storage with user warning

### 12.2 SSRF Protection (`webfetch.go`)

- IPv6 private range detection via `net.IP.IsPrivate()`
- DNS-based URL validation
- Configurable allowed domains and blocklists

### 12.3 File System Safety

- **Path traversal prevention**: All file operations validate paths against work directory
- **Atomic writes**: Backup-before-write pattern with rollback on failure
- **Size limits**: 50MB cap on all file reads, 5MB on files read by LLM
- **Binary detection**: `strings.IndexByte(content, 0)` to prevent binary exposure
- **Output truncation**: 10,000 character cap on tool outputs

### 12.4 Command Execution Safety

- **Timeouts**: 30-minute maximum for Bash commands
- **Signal forwarding**: Ctrl+C properly forwarded to child processes
- **Context cancellation**: All tool executions respect context cancellation
- **Blacklist removed**: Bash command blacklist was removed (bypassable substring matching)

### 12.5 Permission System

- **Risk-level classification**: Every tool tagged with risk level
- **Granular rules**: Per-tool, per-pattern rules with glob support
- **Approval modes**: `ask` (default), `allow`, `deny`
- **Timeouts**: Permission requests auto-deny after configurable timeout
- **Subagent isolation**: Subagents have separate permission profiles with restricted defaults

---

## 13. Testing & Quality Assurance

### 13.1 Test Coverage

- **76 test files** across all packages
- **21,656 test lines** — 32% test-to-code ratio
- **Race detector enabled** in CI (`-race` flag)
- **Coverage reports** in `cover.out`, `cov2.out`, `coverage_final.out`

### 13.2 CI/CD Pipeline

`.github/workflows/ci.yml`:
- Go build and test with race detector
- golangci-lint with 5-minute timeout
- Cross-platform validation

**`.golangci.yml`**: Custom linter configuration with strict rules
**`.goreleaser.yaml`**: Multi-platform release automation

### 13.3 Test Organization

| Package | Test Files | Focus Areas |
|---------|------------|-------------|
| `internal/tui/` | ~20 | Model behavior, view rendering, command parsing, streaming |
| `internal/tools/` | ~10 | Tool execution, permission system, security, bash kill |
| `internal/workflow/` | ~14 | Engine transitions, plan parsing, verification, integration |
| `internal/provider/` | ~8 | SSE parsing, caching, reasoning, wire format |
| `pkg/session/` | ~4 | Session lifecycle, checkpoint, fork |
| `pkg/taskrunner/` | ~2 | Task scheduling, dependency resolution |

**Notable tests:**
- `bash_security_test.go` — Command injection and sandbox escape tests
- `bash_kill_test.go` — Process termination and signal handling tests
- `webfetch_security_test.go` — SSRF and URL parsing security tests
- `api_key_redaction_test.go` (14 subtests) — Comprehensive key redaction verification
- `types_test.go` — Type serialization/deserialization roundtrip tests
- `integration_test.go` — End-to-end workflow integration test

### 13.4 Code Quality Tooling

- **`golangci-lint`** — Comprehensive linting with custom config
- **`go vet`** — Static analysis
- **`go fmt`** + **`goimports`** — Format enforcement
- **`Makefile`** — 30+ targets: `test`, `test-fast`, `test-verbose`, `bench`, `cover`, `lint`, `lint-fix`, `vet`, `fmt`, `check`

---

## 14. Performance Profile

### 14.1 Critical Hot Paths

Based on static analysis of the 103 identified performance issues:

**Streaming Hot Path** (highest impact):
- `ParseSSEChunk()` — 200 map allocations per 200-chunk stream
- `messagesToWire()` — 12+ map allocations per LLM request
- `BuildChatBody()` — Double allocation (map → marshal)
- `strings.Split(cfg.SSEField)` per chunk — allocation for static data

**TUI Render Path** (frequent):
- Viewport rebuild on every 200ms tick (mitigated by 5fps throttle)
- Glamour markdown re-render per message per frame (no LRU cache)
- ThinkingBlock/ToolCard recreation per render (no segment cache)

**Disk I/O Path** (blocking):
- `ListSessions()` — Reads every session.json → O(N×fileSize)
- Ledger `Append()` — Rewrites entire file → O(N)
- Rollback `Chain()` — Spawns `git diff` per commit → O(L×subprocess)

### 14.2 Allocation Hotspots

| Rank | Location | Allocations/Call | Impact |
|------|----------|-----------------|--------|
| 1 | `ParseSSEChunk` — map per chunk | 1 map + N keys + N values | ~800 heap allocs per stream |
| 2 | `messagesToWire` — map per message | 12+ allocs per request | Heavy under load |
| 3 | `BuildChatBody` — body map + tools | 20+ allocs per request | Heavy under load |
| 4 | `cascadingReplace` — content copies | 3-4× O(N) | 30K-40K allocs for large files |
| 5 | `renderMessages` — full rebuild | 1 alloc per tick | 1 alloc per 200ms |

### 14.3 Optimizations Completed (This Session)

6 issues already fixed:

| Issue | File | Fix |
|-------|------|-----|
| O(N²) HTML processing — 16 redundant calls | `webfetch.go` | Batched match-then-mutate |
| Full viewport rebuild per token | `repl_state.go` | 5fps throttle |
| Regex recompilation in ParsePlan | `plan_parser.go` | Package-level cached vars |
| Tool def JSON re-parse per request | `common.go` | ParametersParsed cache |
| Sequential tool execution (4× slower) | `execute.go` | Goroutine pool (max 4) |
| Token estimator mutex contention | `estimator.go` | Lock-free atomic.Uint64 |

### 14.4 Top 10 Recommended Fixes by ROI

| Rank | Description | Impact | Effort |
|------|-------------|--------|--------|
| 1 | Typed SSE/wire structs (replace map[string]any) | 3-5× fewer allocations | Low |
| 2 | Cache regex compilation | Eliminates per-call recompilation | Trivial |
| 3 | Split content once in cascadingReplace | 3-4× O(N) eliminated | Low |
| 4 | LRU cache for glamour renders | 60-80% viewport rebuild reduction | Medium |
| 5 | Async LoadStats (move to tea.Cmd) | Eliminates UI freeze | Low |
| 6 | Ledger append-only writes | O(N)→O(1) per append | Low |
| 7 | Map-based sidebar child lookup | O(F×P×C)→O(F×P) | Low |
| 8 | Strip JSON comments once | O(N+C²)→O(N+C) | Low |
| 9 | Session index file | O(N×fileSize)→O(1) | Medium |
| 10 | Parallelize fallback + grep ctx | 3-10× faster failover | Low |

---

## 15. Strengths & Weaknesses

### 15.1 Strengths

1. **Architectural Purity**: Clean 5-layer architecture with 0 circular dependencies — rare in a project of this scale
2. **Comprehensive Workflow**: Six-phase pipeline with self-healing is genuinely novel for a CLI coding agent
3. **Dual-Provider Strategy**: True provider-agnostic design with auto-fallback, model discovery, and arbitrage — avoids vendor lock-in
4. **Invisible Thinking**: Streaming reasoning display is a major UX differentiator vs spinner-based alternatives
5. **Security-Conscious Design**: OS keychain integration, path traversal protection, SSRF guards, output limits
6. **Cross-Platform**: Full linux/macOS/Windows support with platform-specific keychain implementations
7. **Session Persistence**: File-based with checkpoints, backups, archives, and cross-session learning ledger
8. **Test Coverage**: 32% test-to-code ratio with race detection, security tests, and integration tests
9. **Performance Awareness**: 103 issues identified with clear ROI-based prioritization
10. **Documentation**: 11 user-facing docs + extensive code comments + architecture documentation

### 15.2 Weaknesses

1. **Wire Format Overhead**: Heavy reliance on `map[string]any` for LLM request/response serialization causes significant allocation pressure in the hot path
2. **Sequential Session Listing**: `ListSessions()` reads every session file — doesn't scale beyond ~100 sessions
3. **Ledger Rewrite Pattern**: Every ledger append rewrites the entire file — will not scale with heavy use
4. **No Plugin System**: Extensibility limited compared to OpenCode's dual server/TUI plugin architecture
5. **Rollback Subprocess Spam**: One `git diff` subprocess per commit — expensive for commit chains
6. **Missing Retry in Zen Client**: Unlike OpenRouter, Zen has no transient error retry logic
7. **Keychain Connection Reuse**: Linux dbus connection opened per operation instead of cached
8. **No Concurrent Execution in V1**: Tasks run sequentially despite dependency graph supporting parallel execution
9. **Context Pruning Simplistic**: Per-phase pruning strategy is effective but doesn't use semantic chunking
10. **No Built-in Themes**: Dark/light modes available but no pre-built theme library (10+ themes from spec unimplemented)

---

## 16. Recommendations

### 16.1 Immediate (P0 — Next Sprint)

1. **Typed Wire Formats**: Replace `map[string]any` with `ChatCompletionRequest`, `wireMessage`, `wireToolCall` structs — single biggest allocation reduction opportunity
2. **Append-Only Ledger**: Switch to `O_APPEND|O_WRONLY` writes with periodic compaction — O(1) instead of O(N)
3. **Session Index File**: Maintain `sessions_index.json` with lightweight metadata — O(1) listing
4. **Markdown Render Cache**: LRU cache keyed on `(contentHash, width, themeVersion)` — 60-80% viewport reduction
5. **Async Metrics Loading**: Move `LoadStats` to `tea.Cmd` background goroutine — eliminate UI freeze
6. **Map-Based Sidebar**: O(F×P×C) → O(F×P) child lookup in sidebar tree

### 16.2 Short-term (P1 — V1.1)

1. **SSE Field Pre-computation**: Pre-split field paths at config creation, lock reasoning config keys at init
2. **Tool Execution Parallelism**: Full concurrent task execution (dependency-aware) with bounded goroutine pool
3. **Zen Retry Logic**: Add 3-attempt retry with exponential backoff matching OpenRouter
4. **Command Palette Enhancement**: Fuzzy search, category grouping, keybinding hints, frecency sorting
5. **1-Click Retry in Rollback**: Replace per-commit subprocess with single `git log --patch`
6. **Keychain Connection Pool**: Cache D-Bus connection for Linux keychain operations

### 16.3 Medium-term (P2 — V1.2)

1. **Plugin System**: Go plugin interface with tool hooks, TUI slots, event bus, KV store
2. **10 Built-in Themes**: Catppuccin, Dracula, Gruvbox, Nord, Tokyo Night, Rose Pine, Solarized, GitHub Dark, Monokai, Material
3. **Session Forking**: `/fork` for branching sessions, `/resume search`, session tags
4. **Per-File Extension Permission Rules**: Like OpenCode's `pattern → action` mapping
5. **Config Deep Merge**: Nested config section merging with array concatenation
6. **Interactive Tutorial**: First-run walkthrough of key features

### 16.4 Long-term (P3 — V2.0)

1. **Semantic Context Pruning**: Replace fixed phase-based pruning with semantic chunking and summarization
2. **Multi-Model Orchestration**: Route different phases to different models based on task requirements
3. **Concurrent Multi-Provider**: Simultaneous OpenRouter + Zen requests with first-response wins
4. **Web Search Integration**: Deferred tool for real-time web information retrieval
5. **Visual Mode**: Image input/output support (vision models, diagram generation)
6. **Collaborative Sessions**: Multi-user session sharing with real-time sync

### 16.5 Architecture Evolution

```
V1.0 (Current)          V1.1 (Next)               V1.2 (Future)
┌──────────────────┐   ┌────────────────────┐   ┌─────────────────────┐
│ Sequential Tasks  │   │ Parallel Execution  │   │ Multi-Model Router  │
│ map[string]any    │   │ Typed Wire Formats  │   │ Semantic Context    │
│ File-based State  │   │ Session Index       │   │ Plugin System       │
│ 2 Providers       │   │ Theme Library       │   │ TUI Slots           │
│ No Plugin System  │   │ Command Palette     │   │ Web Search Tool     │
└──────────────────┘   └────────────────────┘   └─────────────────────┘
```

---

## Appendix A: File Tree

```
M31A/
├── cmd/
│   ├── firstrunpreview/          — First-run screen preview tool
│   └── m31a/
│       └── main.go               — Entry point
├── internal/
│   ├── config/                   — Configuration loading & validation
│   ├── errors/                   — Sentinel error definitions
│   ├── fileutil/                 — Atomic file I/O utilities
│   ├── git/                      — Git abstraction layer
│   ├── log/                      — Structured logging
│   ├── provider/                 — LLM provider abstraction
│   │   ├── openrouter/           — OpenRouter client implementation
│   │   └── zen/                  — OpenCode Zen client implementation
│   ├── tokens/                   — Token estimation
│   ├── tools/                    — Tool implementations (13 tools)
│   │   └── subagent/             — Subagent manager
│   ├── tui/                      — Bubble Tea TUI (10 screens)
│   │   ├── components/           — Reusable TUI components
│   │   ├── layout/               — Layout utilities
│   │   └── theme/                — Theme definitions
│   ├── types/                    — Shared type definitions
│   └── workflow/                 — Six-phase workflow engine
│       └── prompts/              — LLM prompt templates (9)
├── pkg/
│   ├── arbitrage/                — Model cost comparison
│   ├── autodream/                — Context consolidation
│   ├── bisect/                   — Git bisect integration
│   ├── keychain/                 — OS keychain abstraction
│   ├── ledger/                   — Cross-session learning
│   ├── rollback/                 — Commit rollback chain
│   ├── session/                  — Session persistence
│   └── taskrunner/               — Task execution engine
├── docs/                         — 11 user-facing documents
├── images/                       — Screenshot assets
├── scripts/                      — Utility scripts
├── .github/workflows/            — CI pipeline
├── Makefile                      — Build automation (30+ targets)
├── go.mod / go.sum               — Go module definition
├── README.md                     — Project overview
├── CHANGELOG.md                  — Version history
└── .goreleaser.yaml              — Release automation
```

## Appendix B: Key Interfaces

```go
// Provider abstraction (internal/provider/interface.go)
type LLMProvider interface {
    Name() string
    FetchModels() ([]types.ModelInfo, error)
    ChatCompletionStream(ctx context.Context, req types.ChatRequest) (*StreamIterator, error)
    EstimateCost(usage types.Usage) float64
}

// Tool interface (internal/tools/interface.go)
type Tool interface {
    Name() string
    Description() string
    Execute(ctx context.Context, params map[string]any) (string, error)
    Parameters() json.RawMessage
}

// Plugin interface (planned for V1.2)
type Plugin interface {
    Name() string
    Version() string
    Activate(ctx PluginContext) error
    Deactivate() error
}

// Git abstraction (internal/types/git.go)
type GitClient interface {
    Init() error
    Add(paths ...string) error
    Commit(msg string) error
    Log(limit int) ([]CommitInfo, error)
    Status() (string, error)
    Diff(head string) (string, error)
    Bisect() *BisectSession
}

// Session manager (pkg/session/manager.go)
type Manager interface {
    Create(ctx context.Context, opts CreateOpts) (*Session, error)
    Load(id string) (*Session, error)
    Save(session *Session) error
    List(ctx context.Context) ([]SessionSummary, error)
    Delete(id string) error
    Fork(id string) (*Session, error)
}
```

## Appendix C: References

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) — Package dependency graph, data flows, threading model
- [SCREENS.md](docs/SCREENS.md) — Complete screen reference with screenshots
- [WORKFLOW.md](docs/WORKFLOW.md) — Six-phase workflow detailed guide
- [TOOLS.md](docs/TOOLS.md) — Complete tool reference with parameters
- [SLASH_COMMANDS.md](docs/SLASH_COMMANDS.md) — All 45+ slash commands
- [CONFIG.md](docs/CONFIG.md) — Full configuration reference
- [KEYBINDINGS.md](docs/KEYBINDINGS.md) — Complete keybinding reference
- [INTERFACES.md](docs/INTERFACES.md) — All exported interfaces
- [TYPES.md](docs/TYPES.md) — Complete type reference
- [TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) — Common issues and solutions
- [QUICKSTART.md](docs/QUICKSTART.md) — Getting started guide
