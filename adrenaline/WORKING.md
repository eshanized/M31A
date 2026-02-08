# WORKING.md — How M31A Works in the Real World

A deep technical walkthrough of M31A's architecture, runtime behavior, and data flow — from binary execution to shipped code.

---

## Table of Contents

1. [What M31A Is](#1-what-m31a-is)
2. [Boot Sequence](#2-boot-sequence)
3. [Configuration System](#3-configuration-system)
4. [Provider Architecture](#4-provider-architecture)
5. [The Terminal UI](#5-the-terminal-ui)
6. [The Six-Phase Workflow](#6-the-six-phase-workflow)
7. [Tool System](#7-tool-system)
8. [Session Persistence](#8-session-persistence)
9. [Self-Healing and Verification](#9-self-healing-and-verification)
10. [Cross-Session Intelligence](#10-cross-session-intelligence)
11. [Cost and Budget Controls](#11-cost-and-budget-controls)
12. [Security Model](#12-security-model)
13. [Auxiliary Systems](#13-auxiliary-systems)
14. [Real-World Usage Flow](#14-real-world-usage-flow)

---

## 1. What M31A Is

M31A is a terminal-based AI coding agent written in Go 1.24+. It compiles to a single static binary (`CGO_ENABLED=0`) with zero external runtime dependencies. The user launches it from a terminal, interacts through a full-screen TUI built on Bubble Tea, and drives software development tasks through a structured six-phase workflow.

M31A does not connect directly to Anthropic or OpenAI. It routes all LLM traffic through two gateway providers:

- **OpenRouter** (`https://openrouter.ai/api/v1`) — a multi-model aggregator
- **Zen** (`https://opencode.ai/zen/v1`) — an alternative gateway

Models are never hardcoded. The application discovers available models dynamically from provider APIs at runtime, caches them with a configurable TTL, and lets the user switch freely.

**Key constraints:**
- No telemetry, no analytics, no external calls beyond the two provider APIs
- No CGO — the binary is fully static
- Bubble Tea is single-threaded — all state mutations go through `Update()`, never from goroutines
- Task execution is sequential (V1) — no concurrent tool dispatch

---

## 2. Boot Sequence

When a user runs `m31a`, the following happens in `cmd/m31a/main.go`:

```
1. Parse CLI flags (--version, --help)
2. Initialize structured logger (slog with rotation) → ~/.m31a/logs/
3. Resolve config path:
   - $M31A_CONFIG env var → ~/.m31a/config.toml (default)
4. Load config (multi-layer merge, see §3)
5. Initialize OS keychain (D-Bus Secret Service on Linux, Keychain on macOS, Credential Manager on Windows)
6. Resolve API keys: env var → keychain → config file
7. Create provider registry:
   - Register OpenRouter client (if API key available)
   - Register Zen client (if API key available)
   - Set active provider from config
8. Create session manager → ~/.m31a/sessions/
9. Create tools dispatcher with all 12 tools registered
10. Create git client (wraps working directory)
11. Create ledger client → ~/.m31a/LEDGER.md
12. Create rollback client (wraps git)
13. Create autodream consolidator (starts empty, REPL injects messages later)
14. Initialize theme (dark/light/auto)
15. Build TUI app with all dependencies injected
16. Optionally auto-resume most recent session (if features.resume_on_startup = true)
17. Launch Bubble Tea program with alt-screen
18. Start signal handler goroutine (SIGTERM/SIGINT → tea.QuitMsg through the program channel)
19. Run event loop until quit
20. Call app.Shutdown() for cleanup
```

The signal handler sends `tea.QuitMsg{}` through the Bubble Tea program channel rather than mutating state directly — this preserves the single-threaded contract.

---

## 3. Configuration System

### Multi-Layer Loading

Config is loaded through five ordered layers, each overriding the previous:

```
Layer 1: DefaultConfig()        — hardcoded sane defaults
Layer 2: Global TOML            — ~/.m31a/config.toml
Layer 3: Environment variables  — M31A_THEME, M31A_DEFAULT_MODEL, M31A_PROVIDER, etc.
Layer 4: Project TOML           — m31a.toml walked up from cwd (max 3 levels)
Layer 5: Variable substitution  — ${VAR} patterns replaced with env values
Layer 6: Validation             — type/range checks on all known fields
```

Additionally, `.env` files in the current working directory are auto-loaded (without overriding existing env vars).

### Config Structure

```toml
[provider]
default = "openrouter"
auto_fallback = true           # auto-switch to backup provider on failure

[provider.openrouter]
# api_key is resolved from env/keychain, never stored in config

[model]
default = "anthropic/claude-sonnet-4"
context_warning_threshold = 0.80
auto_arbitrage = false         # suggest cheaper models for simple tasks

[ui]
theme = "dark"                 # dark, light, auto
compact_mode = false
show_token_usage = true
show_cost_estimate = true
leader_key = "ctrl+x"          # chord shortcut leader
sidebar_auto_show = true

[permissions]
default_mode = "prompt"        # prompt, allow, deny
timeout_seconds = 300

[[permissions.rules]]
tool = "Bash"
pattern = "rm -rf *"
action = "deny"

[agents]
plan = "google/gemini-2.5-flash"     # cheap model for planning
execute = "anthropic/claude-sonnet-4" # powerful model for execution
verify = "google/gemini-2.5-flash"

[tools]
max_glob_results = 1000
max_grep_results = 100
bash_kill_grace_secs = 5

[verify]
build_command = "make build"   # custom verification commands
test_command = "make test"

[features]
budget_limit_usd = 5.00       # per-session spending cap
resume_on_startup = true
session_retention_days = 30

[git]
commit_prefix = "feat"
fix_prefix = "fix"
ship_prefix = "chore"
user_name = "M31A"
user_email = "m31a@local"
```

### Hot Reload

The config file is polled every 5 seconds (`ConfigWatchInterval`). When a change is detected, a `ConfigReloadMsg` is sent into the TUI event loop, which re-applies settings without restarting.

### API Key Resolution

For each provider, keys are resolved in priority order:

```
1. M31A_OPENROUTER_API_KEY / M31A_ZEN_API_KEY env var
2. OPENROUTER_API_KEY / ZEN_API_KEY env var (fallback)
3. OS keychain: keychain.Get("openrouter") / keychain.Get("zen")
4. Config file field (only if the above all miss)
```

API keys are **never** persisted to the config file — `Save()` explicitly strips them before writing.

---

## 4. Provider Architecture

### LLMProvider Interface

```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]ModelInfo, error)
    CachedModels() []ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*StreamIterator, error)
    EstimateCost(modelID string, usage Usage) float64
    HealthCheck(ctx context.Context) HealthStatus
    GetModel(id string) (*ModelInfo, error)
}
```

### OpenRouter Client

- Base URL: `https://openrouter.ai/api/v1`
- Model discovery: `GET /models` — fetches all available models, normalizes pricing to per-million-token rates
- Chat: `POST /chat/completions` with `stream: true`
- Headers: `Authorization: Bearer <key>`, `HTTP-Referer`, `X-Title`, `User-Agent: M31A/<version>`
- Retry logic: up to 2 retries with exponential backoff on 5xx/connection errors
- Error mapping: 429 → `ErrRateLimited`, 401 → `ErrInvalidKey`, 402 → `ErrNoCredits`, 503 → `ErrProviderUnreachable`

### Zen Client

- Base URL: `https://opencode.ai/zen/v1`
- Same interface, different endpoint and header conventions

### SSE Streaming

Both providers use Server-Sent Events for streaming. The SSE parser (`internal/provider/sse.go`):

- Reads lines from the HTTP response body via `bufio.Scanner`
- Handles `data:`, `event:`, `id:`, and `retry:` fields
- Detects `[DONE]` sentinel to signal end-of-stream
- Trims `\r` from line endings (compatibility with providers sending `\r\n`)
- 1MB max line length, 5-minute stream timeout
- Context-aware: checks cancellation between lines

### Model Cache

```
ModelCache:
  - Thread-safe (sync.RWMutex)
  - singleflight deduplication — concurrent Refresh() calls share one HTTP request
  - Two-tier expiry: TTL (5 min default) + stale TTL (24 hours)
  - Within TTL → fresh data returned
  - Between TTL and stale TTL → stale data returned as fallback
  - Beyond stale TTL → ErrProviderUnreachable
```

### Provider Registry

The registry manages multiple providers with atomic switching:

```
Registry:
  - Register(name, provider)
  - SetActive(name) — switches the active provider
  - TrySetActive(name) — atomic check-and-set (prevents TOCTOU races)
  - RollbackActive(from, to) — reverts if health check fails after switch
  - ActiveProvider() — returns the current LLMProvider
```

### Auto-Fallback

When `provider.auto_fallback = true`, the system monitors provider health and can automatically switch to a backup provider when the active one becomes unreachable. Health checks run at configurable intervals with three status levels:

- **Live**: latency < `healthcheck_live_ms` (default 500ms)
- **Slow**: latency < `healthcheck_slow_ms` (default 2000ms)
- **Degraded/Offline**: above threshold or unreachable

---

## 5. The Terminal UI

### Framework Stack

```
charmbracelet/bubbletea  — Elm-like Model/Update/View architecture
charmbracelet/lipgloss   — CSS-like terminal styling
charmbracelet/bubbles    — reusable components (textarea, viewport, spinner, list)
charmbracelet/glamour    — markdown rendering
```

### Screen Architecture

M31A has a multi-screen architecture with a central `App` model that delegates to screen-specific models:

```
Screens:
  ScreenREPL        — main chat interface (default)
  ScreenSettings    — config editor with tabs
  ScreenPlan        — task plan viewer
  ScreenExecute     — live task execution dashboard
  ScreenShip        — session completion summary
  ScreenMetrics     — session analytics
  ScreenFirstRun    — onboarding wizard
  ScreenModelSelect — model browser/picker
  ScreenKeybindings — keybinding reference
  ScreenHistory     — conversation history browser
  Session browser   — session list with search/filter/rename/export
```

### The REPL (Main Chat Interface)

The `ReplModel` is the primary interaction surface. It contains:

- **Viewport**: scrollable message history with rendered markdown
- **Textarea**: multi-line input with slash command autocomplete
- **Spinner**: Unicode-based loading indicator
- **Streaming state**: progressive token rendering from SSE chunks
- **Thinking blocks**: collapsible reasoning display (opacity-controlled)
- **Tool cards**: collapsible cards showing tool execution results
- **Slash command palette**: fuzzy-search command browser
- **Frecent history**: input history sorted by frequency + recency
- **Quick actions panel**: collapsible shortcut hints
- **@mention support**: `@filepath` expansion for file references

### Message Flow

```
User types → textarea captures input
  → If starts with '/': command registry lookup → CommandHandler → CommandResult
  → Otherwise: appended as user message
    → If workflow active: sent to workflow engine phase
    → If REPL chat: sent to LLM via provider → SSE stream → progressive render
      → If response contains tool calls: parse → dispatch → render results
```

### Slash Commands

Over 30 slash commands organized by category:

```
Core:      /help, /clear, /status, /reset, /quit, /undo, /history, /health, /tools
Config:    /settings, /config, /theme, /cost, /log, /key, /tokens
AI/Model:  /compress, /memory, /optimize, /model, /models, /fallback, /provider
Git:       /diff, /rollback, /bisect
Session:   /sessions, /export, /fork, /prev, /next, /save, /goal, /resume, /ledger
Workflow:  /new, /workflow, /plan, /execute, /verify, /ship, /phase, /pause, /metrics
```

Commands support fuzzy matching with Levenshtein distance suggestions for typos.

### Keybinding System

- **Leader key** (default `ctrl+x`): chord-style shortcuts with configurable timeout
- **Screen-specific bindings**: each screen defines its own keymap
- **Global bindings**: `ctrl+c` quit, `tab` autocomplete, `esc` cancel

### Theme System

```
Theme modes: dark, light, auto (follows terminal)
Components: borders, shadows, colors, tabs, unicode symbols
Configurable: accent color, background, border style, animation speed, spinner style
```

### Responsive Layout

The layout engine adapts to terminal size:
- Sidebar auto-shows when width >= `sidebar_width_threshold` (default 120 columns)
- Minimum screen size enforcement
- Page-based layout for complex screens

---

## 6. The Six-Phase Workflow

The workflow is M31A's core value proposition. Every coding task flows through six sequential phases, each with context pruning — the LLM only sees what's relevant to the current phase.

### Phase Transitions

```
idle → initialize → discuss → plan → execute → verify → ship → idle
                                                ↑         |
                                                └─────────┘
                                                (re-execute on failure)
```

Transitions are validated against an explicit allowlist. Invalid transitions return `ErrPhaseTransition`. Each transition saves a checkpoint and writes `STATE.md`.

### Phase 1: Initialize

**Purpose**: Detect project type, initialize git, capture goal.

**What happens**:
1. Parse the user's goal string into a `ProjectState`
2. Detect project type by scanning for indicator files:
   - `go.mod` → Go, `package.json` → Node.js, `Cargo.toml` → Rust
   - `pyproject.toml`/`requirements.txt` → Python, `pom.xml` → Java
   - `Makefile`/`CMakeLists.txt` → C/C++
3. Initialize git repository if not already a repo (with configured user name/email)
4. Create `planning/` directory for session artifacts
5. Write `PROJECT.md` (goal, project type, framework)
6. Write `STATE.md` (current phase + progress)
7. Save checkpoint

**No LLM calls** — this phase is purely deterministic.

### Phase 2: Discuss

**Purpose**: Ask 2-4 clarifying questions to understand requirements.

**What happens**:
1. Build context: system prompt (`discuss-questions.md`) + project info + cross-session memory
2. Stream LLM response with progressive token emission to the TUI
3. Parse numbered questions from the response (regex-based, 3 fallback strategies)
4. Store questions in engine state
5. TUI presents questions to user for Q&A collection
6. User answers each question (or skips)
7. Answers are saved to `PROJECT.md`

**LLM prompt** (`discuss-questions.md`):
- Ask specific, actionable questions
- Avoid yes/no questions — ask "how" or "what"
- Focus on technical decisions affecting implementation
- Max 4 questions (hard cap)
- Can skip entirely for trivial goals

**Streaming**: This phase uses `streamLLMStreaming()` — the underlying `StreamIterator` is returned to the caller, and each SSE chunk is emitted to the TUI as a `StreamChunkMsg` for real-time rendering.

### Phase 3: Plan

**Purpose**: Generate a structured task list with dependencies.

**What happens**:
1. Build context: system prompt (`base.md` + `tool-use.md` + `plan-format.md`) + project info + file schema + discuss Q&A + cross-session memory
2. Stream LLM response
3. Parse JSON task array from response (strips markdown fences, bracket-depth JSON extraction)
4. Validate tasks:
   - Unique IDs, non-empty descriptions/actions
   - No self-references, no circular dependencies (iterative DFS)
   - All dependency IDs reference existing tasks
5. On validation failure: retry up to 3 times with error feedback injected into context
6. Set all tasks to `pending` status
7. Save `TASKS.md` (markdown table format)
8. Save checkpoint + `STATE.md`

**Task Schema**:
```json
{
  "id": 1,
  "action": "Create",
  "description": "Initialize Go module and create main.go",
  "dependencies": [],
  "files": ["go.mod", "main.go"],
  "acceptance_criteria": ["go mod init succeeds", "main.go compiles"]
}
```

**Context pruning**: The plan phase does NOT see the discuss conversation. It reads project state from `PROJECT.md` and file schema from directory walking (max 3 levels deep, skipping `node_modules`, `vendor`, `.git`, etc.).

### Phase 4: Execute

**Purpose**: Implement each task using tools with self-healing.

**What happens**:
1. Load tasks from `TASKS.md`
2. Create a `taskrunner.Runner` with the task list
3. Schedule via topological sort (Kahn's algorithm):
   - Builds adjacency list and in-degree count
   - Groups tasks by dependency depth
   - Detects circular dependencies
4. Execute each group sequentially; tasks within a group also run sequentially (V1)
5. For each task:
   a. Save checkpoint (for rollback on heal failure)
   b. Build execute context: system prompt (`base.md` + `tool-use.md` + `execute-task.md`) + project context + task list + current task spec
   c. Stream LLM response with tools enabled
   d. Parse tool calls from response (JSON extraction with name normalization)
   e. Dispatch tool calls through the permission-gated dispatcher
   f. Feed tool results back as `tool` role messages
   g. On any failure (LLM, parse, tool): trigger self-heal loop
   h. On success: commit changes scoped to task files (`feat: <description>`)
6. Update `TASKS.md` and `STATE.md` after each group

**Tool call parsing**: The engine extracts tool calls from LLM responses using two strategies:
1. JSON inside markdown code blocks (```` ``` ```)
2. Standalone JSON objects with `"name"` or `"tool"` fields (bracket-depth scanning, 64KB scan limit)

Tool names are normalized — the LLM can say `"shell"`, `"exec"`, or `"run"` and it maps to `"Bash"`.

**Self-healing**: On failure, the engine:
1. Increments `healsAttempted` (max 2)
2. Emits `SelfHealStartMsg` to the TUI
3. Makes a fresh LLM call with the self-heal prompt + error context + current file state
4. Parses and dispatches fix tool calls
5. Commits the fix (`fix: <description>`)
6. Re-enters the main execution loop

**Per-phase model assignment**: The `agents` config section allows different models per phase:
```toml
[agents]
plan = "google/gemini-2.5-flash"      # cheap for planning
execute = "anthropic/claude-sonnet-4"  # powerful for execution
```

### Phase 5: Verify

**Purpose**: Validate task outputs for correctness.

**What happens**:
1. Load tasks from `TASKS.md`
2. For each completed task, run verification:
   a. **File existence**: check that all files listed in the task exist on disk
   b. **Syntax/build check**: project-type-specific validation:
      - Go: `go build ./...`
      - Node.js: `npm run build` → fallback `tsc --noEmit`
      - Python: `python3 -m py_compile <file>` (with path traversal protection)
      - Rust: `cargo check`
      - Custom: `verify.build_command` from config
   c. **Test execution**: if test files exist (`*_test.go`, `.test.js`, `_test.py`):
      - Go: `go test ./...`
      - Node.js: `npm test`
      - Python: `python3 -m pytest`
      - Custom: `verify.test_command` from config
3. On verification failure:
   a. Trigger self-heal (same mechanism as execute phase)
   b. Re-verify after heal
   c. If heal fails: try git bisect as fallback
   d. If bisect heal fails: mark task as `failed` or `unrecoverable`
4. Save updated `TASKS.md`, checkpoint, `STATE.md`

**Verification timeout**: 5 minutes per task (configurable).

**Git bisect fallback**: When self-heal fails, the engine runs `git bisect` between the session start hash (good) and HEAD (bad), using the verification check as the pass/fail function. If bisect identifies the offending commit, that diff is fed into a targeted heal call.

### Phase 6: Ship

**Purpose**: Finalize the session, commit remaining changes, archive.

**What happens**:
1. Load tasks for summary statistics
2. `git add -A` + `git commit` with `chore: ship <session_id>`
3. Build `ShipSummary`: tasks done/failed/skipped, commit log, duration
4. Update the learning ledger (`~/.m31a/LEDGER.md`) with session metrics
5. Write final `STATE.md` and checkpoint
6. Archive session: move `~/.m31a/sessions/<id>/` → `~/.m31a/sessions/archived/<id>/`
7. Collect diff stats (insertions, deletions, files added/modified/deleted)

---

## 7. Tool System

### Registered Tools (12 total)

| Tool | Risk Level | Purpose |
|------|-----------|---------|
| **Bash** | Dangerous | Execute shell commands (30-min timeout, 50K output cap) |
| **FileRead** | Safe | Read file contents (5MB max, binary detection) |
| **FileWrite** | Medium | Create/overwrite files atomically (temp + rename, backup) |
| **Edit** | Medium | Targeted string/line-range replacement in existing files |
| **Glob** | Safe | Find files by pattern (`**` recursive, doublestar library) |
| **Grep** | Safe | Search file contents (ripgrep with pure Go fallback) |
| **WebFetch** | Medium | Fetch URL content as text/markdown (30s timeout, SSRF protection) |
| **TodoWrite** | Safe | Write structured TODO list to session directory |
| **AskUserQuestion** | Safe | Pause and ask user a question (interactive only, never automated) |
| **FileList** | Safe | List directory contents with metadata |
| **FileDelete** | Dangerous | Delete file with automatic backup |
| **FileMove** | Medium | Rename/move file (creates parent dirs) |

### Dispatcher Architecture

```
ToolCall from LLM
  → Rate limiter (token bucket: burst + sustained rate)
  → Tool lookup by name
  → Input JSON unmarshaling (normalizes direct args vs nested params)
  → Permission check (rules → agent defaults → risk-level fallback)
  → Permission gate (allow / deny / ask user via modal)
  → Tool.Execute(ctx, input)
  → ToolResult returned to LLM
```

### Permission System

Three layers of permission evaluation:

```
1. Rules (from config):
   - Match tool name (glob patterns: "Bash", "*")
   - Match parameter values (path, command, url, pattern)
   - Action: allow, deny, ask

2. Agent profiles:
   - Per-agent default action (allow/deny)
   - Per-agent rule sets
   - Selected via dispatcher.SelectAgent()

3. Risk-level fallback:
   - Safe/Medium: auto-allow
   - Dangerous/Destructive: prompt user via permission modal
```

**Permission modal**: When `action = "ask"`, the dispatcher sends a `PermissionRequest` through a channel to the TUI. The TUI displays a modal showing the tool name, command, risk level, and matched rule. The user can:
- Allow (once)
- Allow and remember (caches decision for tool name)
- Deny
- Timeout after `permission_timeout` seconds (default 300)

Per-request channels prevent response mix-ups when multiple tools request permission concurrently.

### Rate Limiting

Token bucket algorithm:
- Burst capacity: `ToolRateLimitBurst` tokens
- Refill rate: `ToolRateLimitPerSec` tokens/second
- Prevents resource exhaustion from buggy/malicious LLMs generating thousands of tool calls

### Bash Tool Details

The Bash tool is the most complex:

- **Process groups**: commands run in their own process group for clean signal delivery
- **Output limiting**: 50,000 character cap per stream (stdout + stderr), tracked via `limitWriter`
- **Binary detection**: null-byte check in first 512 bytes → replaces output with `[binary output, N bytes]`
- **Signal forwarding**: context cancellation → SIGINT → grace period (5s) → SIGKILL
- **Concurrent reads**: stdout and stderr read concurrently via goroutines + pipes
- **Timeout**: configurable per-call (max 30 minutes), enforced via `context.WithTimeout`

### Edit Tool Details

The Edit tool supports two modes:

1. **String mode**: exact `old_string` → `new_string` replacement with cascading match strategies:
   - Exact match → line-trimmed → whitespace-normalized → fuzzy-anchor
2. **Line-range mode**: replace lines `start_line` to `end_line` with `new_string`

Files are backed up before modification (configurable max backups per file, default 10).

---

## 8. Session Persistence

### Directory Structure

```
~/.m31a/
├── config.toml                    # global configuration
├── LEDGER.md                      # cross-session learning ledger
├── recent_models.json             # recent/favorite model selections
├── logs/                          # structured log files with rotation
├── backups/                       # file backups from Edit/FileWrite/FileDelete
└── sessions/
    ├── a1b2c3d4/                  # active session (8-char hex ID)
    │   ├── session.json           # metadata (model, provider, phase, timestamps)
    │   ├── messages.json          # full conversation history
    │   ├── checkpoint.json        # last workflow checkpoint
    │   ├── MEMORY.md              # cross-session memory (persisted learnings)
    │   └── planning/
    │       ├── PROJECT.md         # goal, project type, framework, Q&A
    │       ├── TASKS.md           # task list as markdown table
    │       └── STATE.md           # current phase, progress, timestamp
    ├── e5f6a7b8/
    │   └── ...
    └── archived/                  # completed sessions moved here by Ship phase
        └── c9d0e1f2/
            └── ...
```

### Session Lifecycle

```
NewSession(model, provider)
  → Generate 8-char hex ID (crypto/rand, collision retry)
  → Create directory + planning/ subdirectory
  → Write session.json atomically (temp + rename)

LoadSession(id)
  → Validate ID format (lowercase hex, exact length)
  → Read session.json (50MB size limit to prevent OOM)
  → Read messages.json (graceful degradation if missing)
  → Set ResumedAt timestamp
  → Validate required fields (ID, StartedAt, WorkflowPhase)

SaveSession(session)
  → Atomic write session.json
  → Atomic write messages.json

ForkSession(parentID)
  → Deep-copy messages and project state
  → Link parent/child via ParentID/ChildrenIDs
  → Generate new session ID

ArchiveSession(id)
  → Move session directory to archived/
```

### Planning Files

**PROJECT.md** — written by Initialize, updated by Discuss:
```markdown
# Project
**Goal:** Build a REST API for user management
**Type:** go
**Framework:**

## Questions
- **Q:** What database should be used? → **A:** PostgreSQL
- **Q:** How should authentication work? → **A:** JWT tokens
```

**TASKS.md** — written by Plan, updated by Execute/Verify:
```markdown
# Tasks
| ID | Action | Description | Deps | Status | Files |
|----|--------|-------------|------|--------|-------|
| 1 | Create | Initialize Go module | - | done | go.mod, main.go |
| 2 | Add | Add database models | 1 | done | models.go |
| 3 | Add | Add HTTP handlers | 2 | pending | handlers.go |
```

**STATE.md** — written on every phase transition:
```markdown
# State
**Phase:** execute
**Progress:** executing tasks
**Last Action:** group complete
**Timestamp:** 2026-06-10T14:30:00Z
```

### Checkpoint System

Checkpoints capture workflow state at key moments:
- Before each phase transition
- Before each task execution (for per-task rollback)
- After each phase completion

Stored as `checkpoint.json` with phase, timestamp, and task count.

### Atomic Writes

All file writes use atomic temp-file-then-rename to prevent corruption on crash:
1. Write data to `<file>.tmp`
2. `fsync()` the file
3. Close the file
4. `os.Rename(tmp, file)`

File reads are size-limited (`MaxSessionFileSize = 50MB`) to prevent OOM from corrupted or malicious files.

---

## 9. Self-Healing and Verification

### Self-Heal Loop

When a task fails (LLM error, parse error, tool error, or verification failure), the self-heal loop activates:

```
1. Increment healsAttempted (max 2)
2. Emit SelfHealStartMsg to TUI
3. Build heal context:
   - System prompt (self-heal.md)
   - Failure reason
   - Current file contents (read from disk)
4. Stream LLM with tools enabled
5. Parse and dispatch fix tool calls
6. Commit fix (fix: <description>)
7. Verify fix applied (check file existence)
8. Return result to caller
```

### Bisect-Enhanced Healing

When self-heal fails, git bisect provides targeted context:

```
1. Mark session start hash as "good"
2. Mark HEAD as "bad"
3. Binary search commits using verifyTask() as the check function
4. When offending commit found:
   - Extract diff of that commit
   - Feed diff + verification errors into heal prompt
   - LLM can see exactly what changed and why it broke
5. Re-verify after bisect-guided heal
```

### Verification Pipeline

For each completed task:

```
File Existence Check
  → For each file in task.Files: os.Stat(path)

Build/Syntax Check (project-type-specific):
  Go:      go build ./...
  Node.js: npm run build → tsc --noEmit
  Python:  python3 -m py_compile <file>
  Rust:    cargo check
  Custom:  config verify.build_command

Test Execution (if test files detected):
  Go:      go test ./...
  Node.js: npm test
  Python:  python3 -m pytest
  Custom:  config verify.test_command
```

All verification commands run with a 5-minute timeout and are scoped to the working directory.

---

## 10. Cross-Session Intelligence

### Learning Ledger

`~/.m31a/LEDGER.md` — a markdown table recording every shipped session:

```markdown
| Session ID | Timestamp | Model | Project Type | Tasks | Failed | Skipped | Cost | Duration | Commits |
|---|---|---|---|---|---|---|---|---|---|
| a1b2c3d4 | 2026-06-10T14:30:00Z | claude-sonnet-4 | go | 5 | 0 | 0 | 0.42 | 23 | 6 |
```

**Features**:
- Deduplication by session ID (idempotent appends)
- Aggregate statistics: avg tasks, avg cost, avg duration, top failures, top frameworks
- Filtered queries: by project type, by goal keywords (substring match)
- Mtime-based cache: skips re-computation if file hasn't changed
- Truncation: caps at configurable `max_entries` (default 100)

### AutoDream Context Consolidation

When conversation history grows too long, AutoDream compresses it:

```
Protected messages (never consolidated):
  - First message (initial goal/context)
  - All system messages
  - Messages with tool calls
  - Last 5 messages

Candidates = all other messages

Consolidation:
  1. Take oldest 50% of candidates
  2. Concatenate their content
  3. Truncate to ~500 tokens
  4. Create summary: "[AutoDream Context Summary] <timeframe> — <truncated content>"
  5. Replace consolidated messages with single summary
  6. Track statistics (total consolidations, tokens saved)
```

**Reentrancy guard**: `atomic.Bool` CAS prevents nested `/compress` calls from corrupting state.

**Threshold**: Consolidation only fires when the candidate pool is large enough that compressing `(1 - AutoDreamThreshold)` of it removes at least one message (default threshold = 0.60).

### MEMORY.md

Per-session memory file that persists learnings across phases. Loaded by Discuss and Plan phases as additional context. Enables the LLM to reference decisions and patterns from earlier in the session without carrying the full conversation history.

---

## 11. Cost and Budget Controls

### Token Estimation

The `tokens.Estimator` uses tiktoken-go for model-specific tokenization:
- Supports GPT and Claude tokenizer families
- EMA (Exponential Moving Average) calibration: compares estimated vs actual token counts from API responses and adjusts future estimates (configurable alpha, default 0.3)
- Context warning: logs when estimated usage exceeds `context_warning_threshold` (default 80%)
- Preflight check: before each LLM call, estimates token count and rejects if > 95% of model context window

### Cost Estimation

```
cost = (prompt_tokens / 1M) * input_per_m_token + (completion_tokens / 1M) * output_per_m_token
```

Pricing data comes from the provider's model catalog (OpenRouter returns per-token pricing, normalized to per-million-token rates).

### Budget Limit

When `features.budget_limit_usd > 0`:
- Cumulative cost is tracked across all phases in the workflow engine
- Before each `RunPhase()`, the engine checks if `totalCost >= budgetLimitUSD`
- If exceeded, the phase returns an error and the workflow halts
- Cost accumulates from `PhaseResult.Cost` fields

### Arbitrage (Model Cost Optimization)

The `arbitrage` package provides cost-aware model selection:

```
1. Score task complexity (simple/moderate/complex) via keyword analysis
2. Boost complexity for tasks with many files (>3) or dependencies (>3)
3. Estimate token usage based on complexity:
   - Simple: 2000 input + 1000 output
   - Moderate: 5500 input + 2750 output
   - Complex: 14000 input + 7000 output
4. Compare costs across all available models
5. For complex tasks: reject models with < 64K context window
6. Recommend cheapest suitable model
7. ShouldArbitrage(): switch if savings > threshold proportion
```

---

## 12. Security Model

### API Key Security

- Keys resolved from env vars or OS keychain — never stored in config files
- Config `Save()` strips all API keys before writing
- Error messages are sanitized: regex-based API key detection + masking (`sk-ab****efgh`)
- Provider error bodies are stripped of HTML tags and truncated to 200 chars

### Permission System

- Four risk levels: Safe, Medium, Dangerous, Destructive
- Rule-based evaluation with glob pattern matching (doublestar library)
- Per-agent permission profiles (e.g., "build agent" allows Bash, "plan agent" denies it)
- Remember-decision caching per tool name
- Permission timeout (default 300s) prevents indefinite blocking

### Path Safety

- File tools resolve paths relative to `workDir` — no absolute path traversal
- Python verification includes explicit path traversal check (`filepath.Rel` + `..` prefix detection)
- Glob/Grep skip known heavy directories (`node_modules`, `vendor`, `.git`, etc.)
- File reads capped at 5MB, session files at 50MB

### Bash Safety

- Process group isolation (signals don't leak to parent)
- Output capping at 50K characters per stream
- Binary output detection and replacement
- Grace period (5s) between SIGINT and SIGKILL
- Context-based timeout (max 30 minutes)
- No interactive shell — each command is a fresh process

### WebFetch Safety

- SSRF protection (configurable, rejects private IP ranges)
- Max redirect limit (configurable, default 5)
- 30-second timeout
- No JavaScript execution — text/markdown extraction only

### LLM Response Safety

- Max response size: 1MB (`MaxLLMResponseBytes`) — enforced incrementally during streaming
- Max tool calls per response: 16 (`MaxToolsPerCall`)
- JSON scan limit: 64KB when searching for tool call objects
- Malformed JSON detection: if tool-call-like patterns are found but can't be parsed, returns error instead of silently failing

---

## 13. Auxiliary Systems

### Task Runner (`pkg/taskrunner`)

- Topological sort via Kahn's algorithm
- Groups tasks by dependency depth
- Sequential execution within groups (V1)
- Per-task timeout (default 30 minutes, matching Bash)
- Retry support with cancellable backoff
- Lifecycle callbacks: `OnTaskStart`, `OnTaskUpdate`
- Dependency failure propagation: if a dependency fails/skips, dependents are automatically skipped

### Git Wrapper (`internal/git`)

- Wraps all git operations through `exec.Command`
- `CommitWithFiles`: stages specific files + commit (atomic per-task commits)
- `LogSince`: commits since a timestamp (for Ship summary)
- `StatusPorcelain`: machine-readable status
- `HasUncommittedChanges`: dirty tree detection
- `DiffRefs`: diff between two refs (for Ship stats and rollback preview)
- Configurable commit prefixes (`feat:`, `fix:`, `chore:`)

### Rollback (`pkg/rollback`)

- `Chain(limit)`: returns N newest commits with diffs to HEAD
- `Preview(hash)`: diff between commit and HEAD (50K cap)
- `SoftReset(hash)`: git reset --soft with auto-stash
- `HardReset(hash)`: git reset --hard with auto-stash
- `SafeReset(hash)`: hard reset + stash pop (preserves uncommitted changes)
- Commit counting between refs for user-friendly messages

### Keychain (`pkg/keychain`)

Platform-specific implementations via build tags:

| Platform | Backend |
|----------|---------|
| Linux | D-Bus Secret Service API with `pass` CLI fallback |
| macOS | `/usr/bin/security` CLI (Keychain Access) |
| Windows | Windows Credential Manager |

Interface: `Get(service)`, `Set(service, value)`, `Delete(service)`
Error types: `ErrKeyNotFound`, `ErrKeychainUnavailable`

### Structured Logging (`internal/log`)

- `log/slog` based with JSON and text format support
- Log rotation (size-based)
- Configurable level: debug, info, warn, error
- Format: `M31A_LOG_FORMAT` env var (json/text)
- Level: `M31A_LOG_LEVEL` env var

---

## 14. Real-World Usage Flow

### Scenario: Building a REST API

Here's what happens when a user runs M31A to build a Go REST API:

```
1. User launches: $ m31a
   → TUI opens on first-run screen (or REPL if configured)
   → Models fetched from OpenRouter in background
   → Session cleanup runs (removes sessions older than 30 days)

2. User types: "Build a REST API for user management with PostgreSQL"
   → /new command triggers workflow

3. INITIALIZE PHASE
   → Detects go.mod → project type = "go"
   → Git repo already exists → skips init
   → Creates planning/ directory
   → Writes PROJECT.md with goal and type
   → Phase transitions to Discuss

4. DISCUSS PHASE
   → LLM generates 3 questions:
     1. What authentication mechanism should be used?
     2. Which ORM/database driver do you prefer?
     3. Should the API include pagination and filtering?
   → User answers each question in the TUI
   → Answers saved to PROJECT.md
   → Phase transitions to Plan

5. PLAN PHASE
   → LLM sees: project type (go), file schema, Q&A answers
   → Returns 6 tasks with dependencies:
     Task 1: Create DB models (no deps)
     Task 2: Create DB migration (depends on 1)
     Task 3: Create HTTP handlers (depends on 1)
     Task 4: Add router and middleware (depends on 3)
     Task 5: Add authentication (depends on 3, 4)
     Task 6: Write tests (depends on 1-5)
   → Tasks validated, saved to TASKS.md
   → Phase transitions to Execute

6. EXECUTE PHASE
   → Topological sort produces groups:
     Group 1: [Task 1]
     Group 2: [Task 2, Task 3]  (both depend only on Task 1)
     Group 3: [Task 4]
     Group 4: [Task 5]
     Group 5: [Task 6]
   → Task 1: LLM reads existing files, creates models.go, commits
   → Task 2: LLM creates migration, commits
   → Task 3: LLM creates handlers, go build fails →
     SELF-HEAL: LLM sees build error, fixes import, commits
   → Tasks 4, 5, 6: execute normally
   → After each task: permission prompts for Bash (if needed),
     tool cards render in TUI, progress updates

7. VERIFY PHASE
   → For each completed task:
     - File existence: all files present ✓
     - go build ./... → passes ✓
     - go test ./... → Task 6 test fails
     → Self-heal triggered with test output
     → LLM fixes assertion, re-verifies → passes
   → All tasks verified
   → Phase transitions to Ship

8. SHIP PHASE
   → git add -A && git commit "chore: ship a1b2c3d4"
   → Summary: 6/6 tasks done, 0 failed, 8 commits, 45 minutes
   → Ledger updated: session recorded in LEDGER.md
   → Session archived to sessions/archived/
   → Ship view renders with diff stats
   → User returns to REPL

9. POST-SESSION
   → User can browse session history (/sessions)
   → Export session as markdown or JSON (/export)
   → View learning ledger (/ledger)
   → Start a new workflow (/new)
   → Switch models (/model) for the next task
```

### Session Resume

If the user closes M31A mid-workflow (Ctrl+C or terminal close):

```
1. Signal handler sends tea.QuitMsg through program channel
2. Update() processes the quit message
3. app.Shutdown() saves current session state
4. On next launch (with resume_on_startup = true):
   → Most recent session is auto-loaded
   → Workflow state (goal, phase, questions) restored from session.json
   → User can continue from where they left off
```

### Provider Fallback in Production

When the active provider goes down:

```
1. Health check detects offline status (latency > threshold or connection error)
2. If auto_fallback = true:
   → Registry.TrySetActive(backup_provider)
   → Health check on backup
   → If backup healthy: switch complete, banner shown in TUI
   → If backup unhealthy: RollbackActive to original provider
3. Rate limit handling:
   → 429 responses trigger backoff (configurable, default 120s)
   → Retry-After header respected (max 120s wait)
4. Model cache degrades gracefully:
   → Fresh data within TTL → used normally
   → Stale data within stale TTL → used with warning
   → Beyond stale TTL → ErrProviderUnreachable
```

---

## Appendix A: File Layout Reference

```
cmd/m31a/
  main.go              — entry point, dependency wiring, signal handling
  usage.go             — CLI help text with dynamic command listing

internal/
  config/
    loader.go          — multi-layer config loading, validation, var substitution
    types.go           — Config struct and all sub-struct definitions
  provider/
    interface.go       — LLMProvider interface, ChatRequest, ToolDefinition
    registry.go        — provider registry with atomic switching
    cache.go           — model cache with singleflight + two-tier TTL
    sse.go             — SSE parser with context cancellation
    common.go          — shared HTTP helpers, error sanitization, cost estimation
    reasoning.go       — model-specific reasoning parameter tuning
    capabilities.go    — model capability detection from ID patterns
    fallback.go        — auto-fallback orchestration
    openrouter/
      client.go        — OpenRouter API client
    zen/
      client.go        — Zen API client
  tui/
    repl_model.go      — main REPL model (chat interface)
    repl_view.go       — REPL rendering
    repl_stream.go     — SSE stream handling in REPL
    repl_keys.go       — keyboard input processing
    repl_commands.go   — slash command dispatch in REPL
    commands.go        — command registry + all command handlers
    commands_*.go      — categorized command implementations
    app_channel.go     — channel-based message emitter for workflow
    settings_*.go      — settings editor screens
    modelselector*.go  — model picker UI
    ship_view.go       — ship phase summary view
    sidebar.go         — sidebar rendering
    header.go          — header bar rendering
    statusbar.go       — status bar rendering
    health.go          — health check display
    streaming.go       — streaming state management
    toast.go           — toast notification system
    help.go            — help screen
    diff_model.go      — git diff viewer
    mention*.go        — @file mention support
    theme/             — theme registry, colors, borders, shadows, tabs
    layout/            — responsive layout, min screen, pagination
    components/        — reusable UI components (cards, badges, spinners, etc.)
  workflow/
    engine.go          — workflow engine: phase dispatch, LLM streaming, context building
    engine_messages.go — all tea.Msg types for TUI communication
    engine_parse.go    — tool call parsing, task validation, JSON extraction
    engine_verify.go   — task verification, file checking, build/test commands
    initialize.go      — Phase 1: project detection, git init
    discuss.go         — Phase 2: clarifying questions, streaming Q&A
    plan.go            — Phase 3: task generation with retry
    execute.go         — Phase 4: task execution with self-heal
    verify.go          — Phase 5: correctness validation + bisect fallback
    ship.go            — Phase 6: finalization, ledger, archive
    prompts/           — embedded markdown prompt templates
  tools/
    dispatcher.go      — tool registry, permission gating, rate limiting
    permissions.go     — permission evaluation, rule matching, modal protocol
    bash.go            — Bash tool: process management, output capping
    fileread.go        — FileRead tool: file reading with size/binary checks
    filewrite.go       — FileWrite tool: atomic writes with backup
    edit.go            — Edit tool: string/line-range replacement
    glob.go            — Glob tool: pattern matching with doublestar
    grep.go            — Grep tool: content search (ripgrep + Go fallback)
    webfetch.go        — WebFetch tool: URL fetching with SSRF protection
    todo.go            — TodoWrite tool: structured TODO management
    question.go        — AskUserQuestion tool: interactive Q&A
    filelist.go        — FileList tool: directory listing
    filedelete.go      — FileDelete tool: safe deletion with backup
    filemove.go        — FileMove tool: rename/move
    defaults.go        — DefaultDispatcher: registers all 12 tools
    constants.go       — tool-specific constants
  types/
    types.go           — core types: Message, Task, Tool, ToolCall, etc.
    constants.go       — global constants and limits
    git.go             — git-related types
  tokens/
    estimator.go       — token estimation with EMA calibration
  git/
    git.go             — git command wrapper
  log/
    log.go             — structured logging with rotation
  errors/
    errors.go          — sentinel error types
  fileutil/
    atomic.go          — atomic file write utility

pkg/
  session/
    session.go         — Session type with runtime state
    manager.go         — session CRUD, forking, archiving, cleanup
    planning.go        — PROJECT.md, TASKS.md, STATE.md read/write
    checkpoint.go      — checkpoint save/load
  taskrunner/
    runner.go          — topological sort, sequential execution, retry
  arbitrage/
    arbitrage.go       — complexity scoring, cost comparison, model recommendation
  bisect/
    bisect.go          — git bisect wrapper for regression finding
  ledger/
    ledger.go          — cross-session learning ledger (markdown table)
  rollback/
    rollback.go        — commit chain browser, soft/hard/safe reset
  autodream/
    autodream.go       — context consolidation (oldest 50% → summary)
  keychain/
    keychain.go        — Keychain interface
    keychain_linux.go  — D-Bus Secret Service + pass fallback
    keychain_darwin.go — macOS Keychain via /usr/bin/security
    keychain_windows.go — Windows Credential Manager
```

## Appendix B: Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxFileSize` | 5 MB | Max file size for FileRead |
| `MaxToolOutputChars` | 10,000 | Tool output display limit |
| `BashOutputLimit` | 50,000 | Bash stdout/stderr cap |
| `BashTimeout` | 30 min | Max Bash command duration |
| `MaxHealAttempts` | 2 | Self-heal retry limit |
| `MaxPlanRetries` | 3 | Plan generation retry limit |
| `MaxToolsPerCall` | 16 | Tool calls per LLM response |
| `MaxLLMResponseBytes` | 1 MB | Max LLM response size |
| `MaxSessionFileSize` | 50 MB | Max session file size (OOM guard) |
| `SessionIDLength` | 8 | Hex chars in session ID |
| `HTTPDialTimeout` | 30 s | HTTP connection timeout |
| `DefaultContextLength` | 128,000 | Fallback model context window |
| `AutoDreamThreshold` | 0.60 | Min compression ratio |
| `ContextWarningThreshold` | 0.80 | Token usage warning level |
| `DefaultPermissionTimeout` | 300 s | Permission modal timeout |
| `MaxRetryAfterWait` | 120 s | Max Retry-After header wait |
| `ConfigWatchInterval` | 5 s | Config file poll interval |
| `VerifyTaskTimeout` | 5 min | Per-task verification timeout |
