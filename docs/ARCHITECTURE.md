# M31A — Architecture

## Package Dependency Graph

```
cmd/m31a/                     (binary entry point, no logic)
└── internal/log/             (structured slog logger, file rotation)

internal/tui/                 (Bubble Tea app, all screens)
├── internal/provider/        (LLMProvider interface + OpenRouter + Zen)
│   └── pkg/session/          (session lifecycle, file persistence)
├── internal/workflow/        (six workflow phases)
│   ├── pkg/taskrunner/       (dependency graph, topological sort, task lifecycle)
│   ├── pkg/bisect/           (git bisect wrapper)
│   ├── pkg/autodream/        (context consolidation)
│   └── pkg/session/
├── internal/tools/           (Bash, FileRead, FileWrite, Glob, Grep)
├── pkg/arbitrage/            (complexity scoring, model cost comparison)
├── pkg/ledger/               (cross-session learning ledger)
├── pkg/rollback/             (commit chain browser)
├── internal/config/          (toml parsing, env vars, keychain resolution)
│   └── pkg/keychain/         (OS-specific keychain: linux/darwin/windows)
└── internal/types/           (shared core types: Message, Task, ToolCall, etc.)

internal/errors/              (sentinel errors — imported by all packages)
```

### Dependency Rules

- `internal/types/` must have zero internal imports — it is the leaf package.
- `internal/errors/` must have zero internal imports.
- `internal/provider/` may import `internal/types/`, `internal/errors/`.
- `internal/config/` may import `internal/types/`.
- `internal/tools/` may import `internal/types/`, `internal/errors/`.
- `internal/log/` must import only stdlib packages.
- No circular dependencies are permitted.

## Data Flow: Provider Layer

### Streaming Chat Completion

```
User types message
        │
        v
TUI REPL screen
        │  User presses Enter
        v
AppState.Update() receives submitMsg
        │  Creates ChatRequest from message history
        v
LLMProvider.ChatCompletionStream(ctx, req)
        │  POST /chat/completions with stream:true
        v
SSE stream (HTTP response body)
        │  Line-by-line SSE parser
        v
StreamIterator.Next()
        │  yields typed StreamChunk events
        v
StreamChunk dispatched via tea.Cmd → tea.Msg
        │
        ├─ Type: "content"     → append to message content
        ├─ Type: "thinking"    → open/collapsible thinking block
        └─ Type: "done"        → finalize message, extract usage
        │
        v
TUI renders progressively via View()
```

### Reasoning/Thinking Segment Detection

Two patterns handled by `StreamIterator.Next()`:

1. **Pre-content reasoning** (DeepSeek R1, OpenAI o-series): All thinking tokens arrive before any content tokens. The iterator emits `StreamChunk{Type: "thinking"}` for each thinking token, then switches to `StreamChunk{Type: "content"}` when content begins.

2. **Interleaved reasoning** (Claude extended thinking): Thinking and content segments alternate. The iterator detects segment boundaries via SSE event type or special markers, emitting typed `StreamChunk` events accordingly.

## Data Flow: Workflow Engine

### Phase Sequence

```
Initialize ──► Discuss ──► Plan ──► Execute ──► Verify ──► Ship
    │            │           │           │           │          │
    ▼            ▼           ▼           ▼           ▼          ▼
 Create      Generate    Parse LLM   Execute     Run        Final
 session    clarifying  response →  tasks in   acceptance  commit +
 + detect   questions   validate    dependency checks +   archive
 project    → capture   → serialize cycle →     self-heal
 type       answers     to TASKS.md atomic      loop
                                    commits
```

### Context Injection Table

| Phase | Context Injected |
|-------|-----------------|
| Initialize → Discuss | Full session history + MEMORY.md + system prompt |
| Plan | System prompt + goal + Discuss Q&A + MEMORY.md + cwd file schema |
| Execute | System prompt + TASKS.md + PROJECT.md + current task spec only |
| Verify | System prompt + TASKS.md + file contents of all task outputs |
| Ship | System prompt + TASKS.md (final status) + commit log from git |

Each phase discards the prior conversation. No conversation history is carried between phases. State is read from structured Markdown files in `~/.m31a/sessions/<id>/planning/`.

## State Persistence Model

### Session Directory Structure

```
~/.m31a/
├── config.toml              (user configuration)
├── m31a.log                 (structured log, daily rotation, 7-day retention)
├── LEDGER.md                (cross-session learning ledger)
└── sessions/
    └── <session-id>/        (8-char lowercase alphanumeric)
        ├── session.json     (metadata: model, provider, phase, timestamps)
        ├── messages.json    (message history)
        ├── checkpoints/     (state snapshots for undo)
        │   ├── checkpoint-1.json
        │   └── checkpoint-2.json
        ├── planning/        (human-readable state files)
        │   ├── PROJECT.md   (goal, project type, framework, discuss Q&A)
        │   ├── TASKS.md     (task list with status, dependencies, files)
        │   └── STATE.md     (current phase, progress, last action)
        ├── backups/         (pre-overwrite file backups)
        └── archives/        (post-ship archived sessions)
```

### File Format Rules

- All planning files are human-readable Markdown.
- session.json and messages.json are JSON for machine parsing.
- All writes are atomic: write to temp file, then rename.
- Session resume must parse all files and reconstruct exact state.

## Threading Model

Bubble Tea is single-threaded. All state mutations go through `Update()` only.
Never mutate `AppState` from a goroutine. Use `tea.Cmd` and `tea.Msg`.

### Thread Boundaries

| Goroutine | Purpose | Communication |
|-----------|---------|---------------|
| Main (BT update loop) | All state mutations, rendering | N/A |
| Stream iterator | Reads SSE, emits chunks | Sends tea.Cmd with StreamChunkMsg |
| Health check ticker | Polls provider every 60s | Sends tea.Cmd with HealthUpdateMsg |
| Permission request | Blocking user approval | Sends tea.Cmd with PermissionResponseMsg |

### Key Rules

- Goroutines emit `tea.Cmd` functions that return `tea.Msg` values.
- `Update()` receives messages in the main loop and mutates state.
- `View()` never reads mutable state without synchronization (use atomic values or snapshot pattern).
- Stream reading and health checks never block `Update()` or `View()`.

## Context Pruning Strategy

Each workflow phase runs in a fresh, pruned context. The system prompt is preserved
but all conversation history is discarded between phases. State is read from
structured planning/ files only.

### Phase Context Budget

| Phase | Typical Tokens | Content |
|-------|---------------|---------|
| Initialize | ~2K | System prompt + MEMORY.md (if exists) |
| Discuss | ~4K | System prompt + goal + PROJECT.md stub |
| Plan | ~8K | System prompt + goal + Discuss Q&A + MEMORY.md + cwd schema |
| Execute (per task) | ~6K | System prompt + TASKS.md + PROJECT.md + task spec |
| Verify | ~10K | System prompt + TASKS.md + file contents |
| Ship | ~4K | System prompt + TASKS.md + commit log |

### AutoDream Consolidation

When context usage exceeds 60%, AutoDream consolidates the oldest 50% of messages
into a single summary segment. Never runs during tool execution. The `/compress`
command triggers consolidation manually.

## Error Handling Strategy

### Sentinel Errors

All sentinel errors are defined in `internal/errors/errors.go` and are of the form:

```go
var Err* = errors.New("...")
```

Use `errors.Is()` for comparison. Never use type assertions on sentinel errors.

### Provider Error Normalization

| HTTP Status | Normalized Error | Action |
|-------------|-----------------|--------|
| 401          | ErrInvalidKey    | Show "Invalid API key" modal |
| 402          | ErrProviderUnreachable | Log, suggest checking key status |
| 429          | ErrRateLimited   | Auto-fallback if enabled; retry-after header respected |
| 503          | ErrProviderUnreachable | Auto-fallback if enabled |
| 400 (other)  | ErrToolExecution | Return to LLM for correction |

### Context Window Protection

Before sending a chat completion request, M31A estimates total tokens via
tiktoken-go (for GPT/Claude families) or `len(runes) / 4 * 1.3` fallback.

If estimated tokens exceed 80% of the model's context window:
- A warning banner is shown in the TUI header
- AutoDream consolidation is triggered automatically
- If tokens exceed 95%, the request is blocked with `ErrContextExceeded`

## Known Architecture Violations

### CR-09: internal/tools imports internal/config (Deferred to Phase 26+)

**Status:** Known violation, documented for tracking.

The following files in `internal/tools/` import `internal/config`:
- `internal/tools/dispatcher.go`
- `internal/tools/permissions.go`
- `internal/tools/defaults.go`

**Rule:** `internal/tools/` may only import `internal/types/` and `internal/errors/`.

**Root cause:** `PermissionRule` type is defined in `internal/config/types.go` but is consumed by `internal/tools/permissions.go`. Moving the type to `internal/types/types.go` would resolve the violation but cascades across 6+ files.

**Planned fix:** Phase 26+ will move `PermissionRule` to `internal/types/` and update all import paths.

**Phase 25 action:** Document only; do NOT modify the imports.
