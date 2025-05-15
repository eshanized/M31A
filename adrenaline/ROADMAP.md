# M31A — Implementation Roadmap

> **Source of truth:** This roadmap is derived directly from `adrenaline/idea.md` and `adrenaline/REFERENCE.md`. All estimates assume a **single senior Go developer**. Team multipliers are noted where applicable.
> **Version**: V1 (Dual-Provider: OpenRouter + OpenCode Zen)
> **Last Updated**: 2026-05-26
> **Status**: Planning Phase

---

## Project Metrics

| Metric | Value |
|--------|-------|
| **Complexity** | 8.2/10 (Very High) |
| **Estimated V1 Duration** | 21 weeks (1 senior Go developer) |
| **Estimated V1 Duration (2-person team)** | 14-15 weeks (Phases 1/2 and 4/5 parallelized) |
| **Estimated V1.1 Duration** | 6 weeks additional (week 27 total) |
| **Total Phases** | 12 (9 for V1, 2 for V1.1+, 1 for adaptations) |
| **Total Packages** | ~23 (internal + public) |
| **TUI Screens** | 10+ |
| **Core Tools** | 5 (V1) + 7 deferred (V1.1) |
| **Slash Commands** | 28+ |
| **Acceptance Criteria** | 26 |
| **Estimated V1 LOC** | ~24,400 (including ~9,000 test LOC) |
| **Estimated V1.1 LOC** | ~6,500 additional |

---

## Complexity by Subsystem

| Subsystem | Complexity | Justification |
|---|---|---|
| Provider abstraction layer | 7/10 | Two SSE clients, streaming normalization, TTL cache, auto-fallback |
| Reasoning token normalization | 8/10 | Pre-content vs. interleaved thinking; two provider quirks to unify |
| Bubble Tea TUI (10 screens) | 8/10 | Single-threaded state machine, 10 distinct views, 16ms frame budget |
| Streaming message renderer | 7/10 | Progressive assembly, collapsible thinking blocks, Glamour MD |
| Six-phase workflow engine | **9/10** | Context pruning per phase, schema validation, retry logic, phase gating |
| Task runner (dependency graph) | 6/10 | Topological sort, V1 sequential execution, V1.1 concurrency |
| Self-healing + git bisect | 8/10 | Two-attempt heal loop, programmatic bisect, diagnostic context injection |
| Tool system (Bash/FileRead/etc.) | 6/10 | PTY for Bash, atomic FileWrite, binary detection, permission modal |
| Config + OS keychain (3 platforms) | 6/10 | Linux secret-service / macOS Keychain / Windows Credential Manager |
| Token estimation + calibration | 5/10 | tiktoken-go for GPT/Claude, char÷4 fallback, EMA correction factor |
| AutoDream context consolidation | 5/10 | Trigger conditions, paused during execution, checkpoint/revert |
| Model arbitrage (`pkg/arbitrage`) | 5/10 | Complexity heuristic 0–1, cost comparison matrix, capability guard |
| Cross-session learning ledger | 4/10 | Append-only markdown, aggregate stats, context injection on init |
| Commit rollback chain | 5/10 | Git log parse, soft/hard reset, backup branch, task status sync |
| Ghost mode (V1.1) | 7/10 | `git worktree` lifecycle, full workflow isolation, merge/discard UX |
| Terminal PiP (V1.1) | 8/10 | Bubble Tea sub-window, goroutine stdout/stderr pipe, scroll buffer |
| Concurrent subagents (V1.1) | **9/10** | Channel-based BubbleTea safety, permission escalation, progress fan-in |

---

## Codebase Size Estimate (V1)

| Area | Packages | Est. LOC |
|---|---|---|
| `cmd/m31a/` | 1 | ~300 |
| `internal/config/` | 1 | ~500 |
| `internal/provider/` | 3 (interface, openrouter, zen) | ~2,200 |
| `internal/tui/` | 5 (app, repl, workflow screens, model selector, settings) | ~4,200 |
| `internal/workflow/` | 6 (phases × 1 each) | ~2,800 |
| `internal/tools/` | 5 (bash, fileread, filewrite, glob, grep) | ~1,600 |
| `pkg/taskrunner/` | 1 | ~650 |
| `pkg/arbitrage/` | 1 | ~450 |
| `pkg/bisect/` | 1 | ~320 |
| `pkg/ledger/` | 1 | ~420 |
| `pkg/rollback/` | 1 | ~350 |
| `pkg/autodream/` | 1 | ~550 |
| `pkg/session/` | 1 | ~650 |
| `pkg/keychain/` | 1 | ~450 |
| Test files | — | ~9,000 |
| **Total V1** | **~23 packages** | **~24,400 LOC** |

---

## Timeline Overview

```
Week  1 │███░░░░░░░░░░░░░░░░░░░░░░│ Phase 0 — Foundation
Week  2 │████░░░░░░░░░░░░░░░░░░░░░│
Week  3 │░░░░███████████░░░░░░░░░░│ Phase 1 — Provider Layer
Week  4 │░░░░███████████░░░░░░░░░░│
Week  5 │░░░░░░░░░░░░░███████░░░░░│ Phase 2 — TUI Shell (overlaps Phase 1 tail)
Week  6 │░░░░░░░░░░░░░░░░░░█████░░│
Week  7 │░░░░░░░░░░░░░░░░░░░░░░███│ Phase 3 — Rendering Pipeline
Week  8 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week  9 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 4 — Tool System
Week 10 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 5 — State & Config (overlaps Phase 4)
Week 11 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 12 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 6 — Workflow Engine (longest phase)
Week 13 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 14 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 15 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 16 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 17 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 7 — Signature Features
Week 18 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 19 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 20 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 8 — Polish & v1.0 Release
Week 21 │░░░░░░░░░░░░░░░░░░░░░░░░░│
─────── v1.0.0 released ──────────────────────────────────
Week 22 │░░░░░░░░░░░░░░░░░░░░░░░░░│ Phase 9 — V1.1 (Ghost, PiP, Subagents)
Week 23 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 24 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 25 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 26 │░░░░░░░░░░░░░░░░░░░░░░░░░│
Week 27 │░░░░░░░░░░░░░░░░░░░░░░░░░│
─────── v1.1.0 released ──────────────────────────────────
```

| Version | Duration | Target Date (from start) |
|---|---|---|
| **v0.1.0** (provider + TUI shell) | 6 weeks | Week 6 |
| **v0.3.0** (tools + state + rendering) | 11 weeks | Week 11 |
| **v0.6.0** (full workflow engine) | 17 weeks | Week 17 |
| **v1.0.0** (all signature features, release-ready) | 21 weeks | Week 21 |
| **v1.1.0** (ghost mode, PiP, concurrent subagents) | 27 weeks | Week 27 |

> **2-person team:** Phases 1/2 and 4/5 can be parallelized. Estimated V1.0 in **14-15 weeks**, V1.1 in **20 weeks**.

---

## Phase 0 — Foundation

**Duration:** 1.5 weeks  
**Complexity:** 2/10  
**Milestone:** Repository compiles, CI passes, interfaces in place

### Goals

Establish the project skeleton so every future phase has a clean base. No user-facing features — only infrastructure.

### Tasks

**P0.1 — Go module & project structure**
- Initialize `github.com/eshanized/M31A` with Go 1.22+
- Create directory layout: `cmd/m31a/`, `internal/`, `pkg/`, `docs/`
- `cmd/m31a/main.go` with stub `main()` that prints version and exits
- `Makefile` with targets: `build`, `test`, `lint`, `clean`, `release`
- `.gitignore`, `LICENSE` (MIT), `README.md` stub

**P0.2 — Core interfaces & shared types**
- Define `internal/provider/interface.go`: `LLMProvider`, `ModelInfo`, `ChatRequest`, `StreamIterator`, `Usage`, `CapFlags`, `Pricing`
- Define `internal/tools/interface.go`: `Tool`, `ToolCall`, `ToolResult`, `RiskLevel`
- Define `internal/session/types.go`: `Session`, `Message`, `MessageSegment`, `WorkflowPhase`, `Task`, `TaskStatus`, `FilePrediction`, `ProjectState`
- Shared enumerations and sentinel errors in `internal/errors/`
- No implementations yet — interfaces only

**P0.3 — CI/CD pipeline**
- GitHub Actions: lint (`golangci-lint`), test (`go test ./...`), build matrix (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64)
- `CGO_ENABLED=0` enforced in all build steps
- `go vet` + `staticcheck` in lint job
- Release workflow: tag-triggered, produces binaries + checksums via `goreleaser`

**P0.4 — Logging & observability**
- Structured logger (`log/slog`) writing to `~/.m31a/m31a.log` only — never stdout/stderr during TUI operation
- Log rotation: keep last 7 days
- **Strictly no telemetry**, no analytics, no phone-home (mandated by FOSS requirements)

### Deliverables

- Repository with `go build ./...` passing cleanly
- All shared interfaces committed and documented
- CI green on all three platforms

---

## Phase 1 — Provider Abstraction Layer

**Duration:** 3 weeks  
**Complexity:** 7/10  
**Milestone:** Both providers stream tokens; model catalog populates; auto-fallback works

### Goals

Build the full LLM gateway layer. Everything else depends on it working correctly, especially streaming and reasoning normalization.

### Tasks

**P1.1 — `LLMProvider` interface + `ProviderRegistry`**
- `ProviderRegistry`: thread-safe map of name → provider, active provider field, `SetActive()` / `Active()` / `Get()` methods
- Factory function: `NewProvider(name, apiKey string) (LLMProvider, error)`
- Error types: `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, `ErrContextExceeded`, `ErrModelNotFound`

**P1.2 — OpenRouter client (`internal/provider/openrouter/`)**
- `OpenRouterClient` struct: `apiKey`, `baseURL = "https://openrouter.ai/api/v1"`, `httpClient` with 30s dial-only timeout (no body read timeout — streaming is unbounded), `modelCache`
- `FetchModels()`: `GET /models`, parse JSON, populate `[]ModelInfo`. Cache with 5-minute TTL (`sync.RWMutex`-protected). Non-blocking refresh via goroutine.
- `ChatCompletionStream()`: `POST /chat/completions` with `stream: true`. Return `*StreamIterator` that yields `StreamChunk`. Parse SSE line-by-line.
- `GetModel(id string)`: Return `ModelInfo` from cache; trigger refresh if stale.
- `EstimateCost(usage Usage)`: Multiply token counts against `ModelInfo.Pricing`.
- `HealthCheck()`: `GET /auth/key` → 200 = live; parse latency.

**P1.3 — OpenCode Zen client (`internal/provider/zen/`)**
- `ZenClient` struct: identical shape to OpenRouter but `baseURL = "https://opencode.ai/zen/v1"`. Uses OpenAI-compatible endpoints.
- Zen-specific: reasoning params may differ slightly; normalize to the same `MessageSegment{Type: "thinking"}` structure.
- `HealthCheck()`: `GET /models` (lightweight catalog fetch doubles as auth validation).
- `FetchModels()`: Same 5-minute TTL cache, same `ModelInfo` output — different source JSON field names.

**P1.4 — Reasoning normalization**
- Two patterns to handle:
  1. **Pre-content reasoning** (DeepSeek R1, OpenAI o-series): All thinking tokens arrive before any content tokens.
  2. **Interleaved reasoning** (Claude extended thinking): Thinking and content segments alternate.
- `StreamIterator.Next()` must detect segment boundaries and emit typed `StreamChunk` events.

**P1.5 — Model cache & dynamic discovery**
- `ModelCache`: per-provider, `TTL = 5 * time.Minute`, background refresh via `time.Ticker`
- Offline resilience: use stale cache for up to 24 hours on network failure
- Cross-provider model resolution: when switching providers, attempt to match model ID; if missing, find closest name match and prompt user

**P1.6 — Auto-fallback logic**
- On `ErrRateLimited` (HTTP 429) or `ErrProviderUnreachable` (HTTP 503): if `auto_fallback` enabled, switch `ProviderRegistry.active` to the other provider
- Emit `FallbackEvent` so TUI can display a banner
- Do not switch if the other provider is also unhealthy
- Resume the pending request on the new provider (rebuild `ChatRequest` from current history)

**P1.7 — Unit tests**
- Mock HTTP server for both providers (record/replay fixtures)
- Test: streaming chunked SSE, thinking segment detection, cost estimation, model cache TTL expiry, auto-fallback triggering, health check latency
- Coverage target: 80%

### Deliverables

- `internal/provider/` with both clients passing all tests
- Both providers stream tokens correctly against mocked servers
- Auto-fallback switches provider and emits `FallbackEvent` on 429/503

---

## Phase 2 — TUI Foundation

**Duration:** 2.5 weeks (starts Week 4, overlaps Phase 1 tail)  
**Complexity:** 8/10  
**Milestone:** Bubble Tea app launches; REPL screen renders; model badge live

### Goals

Build the shell of the TUI — the application skeleton, routing between screens, theme system, and the main REPL view.

### Tasks

**P2.1 — Bubble Tea application skeleton**
- `internal/tui/app.go`: top-level `AppState` struct, `Init()`, `Update()`, `View()` implementing `tea.Model`
- Screen routing: `activeScreen` enum — `ScreenREPL`, `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, `ScreenShip`, `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, `ScreenFirstRun`, `ScreenPermission`
- Message bus: `tea.Cmd` / `tea.Msg` types for all cross-component events (stream delta, tool result, phase transition, fallback event, health update)
- 60fps frame budget: `View()` must return in < 16ms; heavy operations cached

**P2.2 — Theme system (`internal/tui/theme/`)**
- `Theme` struct containing all named Lipgloss `Style` values from spec §9.1
- Dark palette: `Background #0D0D0D`, `Surface #1A1A1A`, `Brand #D77757`, `Thinking #8AB4F8`, `Success #81C995`, `Error #F28B82`, `Warning #FDD663`
- Light palette: all counterparts as specified
- `auto` mode: detect terminal background via `termenv`
- `ThemeManager.Cycle()` for `/theme` command (dark → light → auto)
- All styles defined once in `theme/` — no raw hex strings in rendering code

**P2.3 — REPL screen (`internal/tui/repl.go`)**
- **Header** (1 line): brand, model badge `[OR]`/`[ZEN]` with color-coded fg, context bar `used/total`, connection status
- **Message area** (flex): `bubbles/viewport` for scrollable history; user vs. assistant distinction; auto-scroll to bottom
- **Input area** (3-6 lines): `bubbles/textarea` with placeholder, character count
- **Status bar** (1 line): current operation, timestamp
- **Spinner**: `bubbles/spinner` using `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` cycle at 10fps

**P2.4 — Health check ticker**
- Background goroutine: poll active provider every 60s
- Adaptive: increase to 120s on rate-limit headers; stop on 401
- Emit `HealthUpdateMsg{Status, Latency}` to TUI update loop
- Non-blocking: never interrupt active streaming

**P2.5 — First-run screen (`internal/tui/firstrun.go`)**
- Shown when no config file and no stored keys exist
- Provider selection: [1] OpenRouter, [2] Zen, [3] Both, [4] Skip
- Key input + immediate validation via health check
- OS keychain storage prompt
- On skip: load REPL with "No API key configured" banner

### Deliverables

- `m31a` binary launches, renders REPL screen, accepts input
- Theme cycles with `/theme`; connection badge updates live
- First-run screen completes setup and transitions to REPL

---

## Phase 3 — Message Rendering Pipeline

**Duration:** 2 weeks  
**Complexity:** 7/10  
**Milestone:** Streaming tokens render token-by-token; thinking blocks collapsible; tool cards rich

### Goals

Build the visual rendering layer that makes M31A look premium.

### Tasks

**P3.1 — Streaming token renderer**
- As `StreamChunk` events arrive, progressively append to current message
- Content segments render token-by-token with no flicker
- No full re-render on each token — only active message bubble dirtied

**P3.2 — Inline thinking/reasoning blocks**
- When `StreamChunk{Type: "thinking"}` arrives, open thinking block above content
- Render: `[▼ Thinking (N.Ns)]` header in `Thinking #8AB4F8` with collapsible body
- Duration counter updates live; final duration stamped when thinking closes
- Toggle: `T` key or Enter on block header; state persisted in `MessageSegment.Visible`

**P3.3 — Rich tool cards (`internal/tui/toolcard.go`)**
- Card layout: tool label badge → command/input → output → status badge
- Label colors: Bash `#FDD663`, FileRead `#8AB4F8`, FileWrite `#D77757`, Glob `#9AA0A6`, Grep `#8AB4F8`
- Status cycle: `[..]` spinner → `[OK]` fg(#81C995) → `[ERR]` fg(#F28B82)
- Auto-collapse: outputs > 20 lines show `[+N lines]` toggle. Output cap at 10,000 chars.
- Binary content: show `[binary file, N bytes]` placeholder
- Syntax highlighting via Glamour/Chroma

**P3.4 — Markdown rendering**
- Use `charmbracelet/glamour` with custom dark/light stylesheet
- Render cached per-message; only re-render if content changes

**P3.5 — Permission modal (`internal/tui/permission.go`)**
- Centered overlay: tool name, command/args with syntax highlight, risk badge, timeout countdown (default 300s)
- Keys: `Y`/`Enter` = approve, `N`/`Esc` = deny, `A` = approve and remember
- Timeout auto-denies; only triggers for `Dangerous` or `Destructive` tools

### Deliverables

- Streaming renders smoothly with visible token-by-token appearance
- Thinking blocks render, animate duration, collapse/expand on `T`
- Tool cards render with correct colors, collapsible output, status badges
- Permission modal appears and correctly gates tool execution

---

## Phase 4 — Tool System

**Duration:** 2 weeks  
**Complexity:** 6/10  
**Milestone:** All 5 V1 tools execute correctly; permission gate enforced

### Tasks

**P4.1 — `Bash` tool (`internal/tools/bash.go`)**
- `exec.Cmd` with 30-minute absolute timeout via `context.WithTimeout`
- Signal forwarding: `SIGINT` propagates to child process
- PTY allocation on Linux/macOS (via `creack/pty`); plain pipes on Windows
- Output streaming: stdout/stderr piped to `chan string` for real-time tool card updates
- Binary output detection; working directory is user's `cwd`

**P4.2 — `FileRead` tool (`internal/tools/fileread.go`)**
- Open file, detect encoding (UTF-8 vs. binary via mime sniff on first 512 bytes)
- If binary: return `[binary file, mime-type, N bytes]`
- Size limit: 5MB; path safety: resolve symlinks, reject paths outside cwd

**P4.3 — `FileWrite` tool (`internal/tools/filewrite.go`)**
- Atomic write: write to `.m31a_tmp_<random>` sibling file, then `os.Rename`
- Backup: copy existing file to `~/.m31a/sessions/<id>/backups/<filename>.<timestamp>` before overwrite
- Directory creation: `os.MkdirAll` on parent directory
- Path safety: same restrictions as FileRead

**P4.4 — `Glob` tool (`internal/tools/glob.go`)**
- `filepath.Glob` wrapper with recursive support (`**` via `doublestar` library)
- Output: sorted relative paths with sizes and last-modified dates
- Hard limit: 1,000 results; truncated with `[... N more files]`

**P4.5 — `Grep` tool (`internal/tools/grep.go`)**
- Detect `rg` in `$PATH`; shell out with `--json` output for structured results
- Fallback: pure-Go `bufio.Scanner` line-by-line regex search
- Output: file path, line number, line content; formatted as tool card table
- Respect `.gitignore` (via `rg`) or manual parse in fallback

**P4.6 — Tool dispatcher (`internal/tools/dispatcher.go`)**
- Parse `ToolCall` from LLM response; route to appropriate implementation
- Enforce permission gate: check `RiskLevel`; emit `PermissionRequestMsg` if dangerous
- Serialize tool result to `ToolResult` and feed into active `ChatRequest`

### Deliverables

- All 5 tools execute correctly in isolation via `go test`
- Bash streams output to tool card in real-time
- Permission modal blocks Bash until approved/denied
- FileWrite produces `.m31a_tmp_*` → rename pattern verifiable in tests

---

## Phase 5 — Session State & Configuration

**Duration:** 2 weeks (starts Week 10, overlaps Phase 4 tail)  
**Complexity:** 5/10  
**Milestone:** Sessions persist, resume, and are correctly reconstructed from disk

### Tasks

**P5.1 — Config system (`internal/config/`)**
- Parse `~/.m31a/config.toml` using `BurntSushi/toml`
- Env var override layer; resolution order: env var → OS keychain → config file
- Defaults for all fields; missing config file triggers first-run flow

**P5.2 — OS keychain integration (`pkg/keychain/`)**
- Compile-tag separated implementations:
  - `keychain_linux.go`: freedesktop Secret Service via `godbus/dbus`; fallback to `pass` CLI
  - `keychain_darwin.go`: macOS Keychain Services via `keyring` package (CGO-less)
  - `keychain_windows.go`: Windows Credential Manager via `go-wincred`
- Unified interface: `Get()`, `Set()`, `Delete()`
- CLI subcommands: `m31a keychain setup`, `m31a keychain rotate <provider>`, `m31a keychain remove <provider>`

**P5.3 — Session lifecycle (`pkg/session/`)**
- `Session.New()`: generate 8-char ID via `crypto/rand`; create session directory
- `Session.Save()`: atomic writes to `session.json` and `messages.json`
- `Session.Load(id)`: parse all session files; reconstruct state
- `Session.Archive()`: move to `archived/` post-Ship

**P5.4 — File-based state persistence**
- `planning/PROJECT.md` writer/parser: goal, project type, framework, discuss Q&A
- `planning/TASKS.md` writer/parser: Markdown table with ID, action, description, deps, status, files
- `planning/STATE.md` writer/parser: current phase, progress, last action, timestamp
- All writes atomic (temp file + rename); parse tolerates extra whitespace

**P5.5 — Checkpoint system**
- Snapshot `AppState` to `checkpoint.json` before each phase transition
- Only last 2 checkpoints retained
- `/undo` reads checkpoint and restores previous phase

**P5.6 — Resume browser screen (`internal/tui/resume.go`)**
- On startup: scan sessions directory; sort by last-modified
- Render session list with metadata; corrupted session detection with `[!]` badge
- Keys: `Enter` = resume, `N` = new session, `D` = delete with confirmation

**P5.7 — Token estimation + calibration**
- Client-side: `tiktoken-go` for GPT/Claude; `len(runes) / 4 * 1.3` fallback
- Server calibration: extract `usage` from final SSE chunk; EMA correction (alpha=0.3)
- Context warning: banner at `contextUsed / contextTotal > threshold` (default 80%)

### Deliverables

- Session files written after every message; resume reconstructs exact state
- All three OS keychain backends compile and pass unit tests
- Token estimation converges to < 5% error after 3 turns

---

## Phase 6 — Six-Phase Workflow Engine

**Duration:** 6 weeks  
**Complexity:** 9/10  
**Milestone:** Full Initialize → Discuss → Plan → Execute → Verify → Ship cycle completes end-to-end

This is the largest and most complex phase. Build and test each phase in order.

### Tasks

**P6.1 — Task runner (`pkg/taskrunner/`)**
- `TaskRunner.Schedule()`: topological sort by dependency graph. Cycle detection → error.
- V1 execution: groups iterated sequentially; within group, one task at a time
- Per-task lifecycle: emit `TaskStartMsg` → stream LLM → dispatch tool calls → `git commit` → emit result
- Dependency blocking: failed/skipped dependency → mark dependent `StatusSkipped`

**P6.2 — Initialize phase (`internal/workflow/initialize.go`)**
- Parse goal from user input
- Project type detection: scan cwd for `package.json`, `go.mod`, `Cargo.toml`, etc.
- If cwd not a git repo: `git init` automatically
- Create `planning/` directory; write initial `PROJECT.md`
- Transition to Discuss automatically

**P6.3 — Discuss phase (`internal/workflow/discuss.go`)**
- System prompt + goal + `MEMORY.md` (if exists) + `PROJECT.md` context
- Stream LLM; parse 2-4 numbered clarifying questions
- Render questions inline; user answers one at a time
- `skip` fills remaining with defaults; append Q&A to `PROJECT.md`
- Transition to Plan automatically

**P6.4 — Plan phase (`internal/workflow/plan.go`)**
- Pruned context: system prompt + goal + Discuss Q&A + `MEMORY.md` + cwd file schema
- LLM produces JSON array of tasks; schema validation (no cycles, no self-refs, valid deps, all fields present)
- On validation failure: send errors back to LLM (max 3 retries)
- After 3 failures: prompt user to enter tasks manually or skip to REPL
- Serialize to `TASKS.md`; emit `PlanReadyMsg`

**P6.5 — Plan screen (`internal/tui/plan.go`)**
- Split-pane: task list (left) + cost/time/model panel (right)
- Task list: `[ACTION] description · deps: N · ~duration`
- Right panel: total estimated cost, estimated time, active model
- Per-task arbitrage suggestion if complexity score < threshold
- Keys: `A` = accept, `E` = edit inline, `R` = retry, `D` = diff preview overlay, `Tab` = dependency graph
- Diff preview overlay: `predicted_files` as file tree with `+`/`~`/`-` indicators

**P6.6 — Execute phase (`internal/workflow/execute.go`)**
- Pruned context per task: system prompt + `TASKS.md` + `PROJECT.md` + current task spec
- Task progress tracker; write `TASKS.md` and `STATE.md` to disk after each task
- Atomic git commits: `git add -A && git commit -m "feat: <description>"`

**P6.7 — Execute screen (`internal/tui/execute.go`)**
- Task list with status indicators: `[x]` done, `[>]` running, `[ ]` queued, `[ ]` blocked
- Progress bar in header: `N of M complete X%`
- Live tool cards for current task; keys: `P` = pause, `R` = resume, `S` = skip

**P6.8 — Verify phase (`internal/workflow/verify.go`)**
- Pruned context: system prompt + `TASKS.md` + file contents of all task outputs
- Deterministic checks: file existence, syntax validation, test execution
- Self-heal loop (max 2 attempts): LLM receives failure + task spec + current file state → apply fix → re-verify
- After 2 failed heals: mark `StatusUnrecoverable`

**P6.9 — git bisect integration (`pkg/bisect/`)**
- Triggered when task is `StatusUnrecoverable`
- `Bisect.Run(sessionStartHash, headHash, checkFn)`: bisect session commits, parse output, extract offending commit diff
- Return `BisectResult{OffendingCommit, Diff}` for 3rd targeted heal attempt

**P6.10 — Verify screen (`internal/tui/verify.go`)**
- Pass/fail checklist per task; `[H]` self-heal, `[S]` skip
- BISECT result displayed inline after `[UNRECOVERABLE]`
- Auto-transition to Ship after all tasks pass (or are skipped)

**P6.11 — Ship phase (`internal/workflow/ship.go`)**
- Final git commit: `chore: ship <session-id>`
- Summary: task count, commit log from `git log --oneline`
- Ledger update: append entry to `~/.m31a/LEDGER.md`
- Session archive: move to `archived/`

**P6.12 — Ship screen (`internal/tui/ship.go`)**
- Summary banner with task count, commit log
- `[O]` open in browser (detect dev server URL), `[N]` new session, `[R]` return to REPL

**P6.13 — Integration test: full workflow**
- Mocked LLM provider with scripted responses for each phase
- End-to-end: Initialize → Discuss → Plan → Execute → Verify → Ship against temp git repo
- Assert: all planning files written; git commit produced; session archived

### Deliverables

- `m31a` completes full workflow from goal to Ship with real OpenRouter key
- All session files written and correctly parsed on resume
- git bisect integration pinpoints offending commit in integration test fixture

---

## Phase 7 — Signature Features

**Duration:** 3 weeks  
**Complexity:** 7/10  
**Milestone:** All 26 acceptance criteria pass

### Tasks

**P7.1 — Model selector UI (`internal/tui/modelselector.go`)**
- Full-screen overlay; provider filter cycles with `P`: All → OpenRouter → Zen
- Real-time fuzzy search over `ModelInfo.Name` and `ModelInfo.ID`
- Each result: name, `[OR]`/`[ZEN]` badge, context length, pricing, capability badges
- Detail pane on `Tab`: full description, architecture, latency stats
- Same model on two providers shown as separate entries

**P7.2 — Cost-aware model arbitrage (`pkg/arbitrage/`)**
- `Arbiter.Score(task)`: complexity score 0-1 from file count, operation type, token budget, dependency depth
- `Arbiter.Suggest(task, catalog)`: find cheapest model where `cost < currentCost` and capabilities met
- `Arbiter.SuggestAll(tasks)`: batch suggestions for Plan screen
- `/optimize` command; `O` key in Plan screen: accept all suggestions

**P7.3 — Cross-session learning ledger (`pkg/ledger/`)**
- `Ledger.Append(entry)`: after each Ship, write to `~/.m31a/LEDGER.md`. Prune to `max_entries`.
- `Ledger.Stats()`: parse all entries; compute avg tasks, cost, time, top failures, top frameworks
- `Ledger.Query(projectType, goalKeywords)`: return at most 3 relevant past sessions for context injection
- Context injection during Initialize phase
- `/ledger` screen: filterable viewer; `/ledger stats`: aggregate stats

**P7.4 — Commit rollback chain (`pkg/rollback/`)**
- `Rollback.SessionCommits(sessionStartHash)`: `git log --oneline --since=<session-start>`
- Rollback screen (`/rollback`): interactive list with `[HEAD]` marker; `Enter` = rollback, `D` = view diff
- Before rollback: create backup branch `m31a/rollback-backup-<timestamp>`
- Soft rollback: `git reset --soft`; rolled-back tasks return to `StatusPending`
- Hard rollback: prompt confirmation then `git reset --hard`
- Diff view: `git diff <selected>..HEAD` with Lipgloss syntax highlighting

**P7.5 — AutoDream context consolidation (`pkg/autodream/`)**
- Trigger: context > 60% full, or 15 minutes of active conversation. Never during tool execution.
- Consolidation: summarize oldest 50% of messages; replace with memory segment
- Status bar: `[DREAM]` tag during consolidation
- `/memory review`, `/memory revert`, `/memory pause`, `/memory resume`, `/compress`

**P7.6 — Slash command system (`internal/tui/commands.go`)**
- Parse all 30+ slash commands; route to handler or screen transition
- `/status`: inline display of active model, provider, context usage, cost, session duration, phase
- `/help`: formatted command table
- Tab-completion for slash command names

**P7.7 — Settings screen (`internal/tui/settings.go`)**
- Two-column layout with tab navigation: General, Provider, Model, Permissions, Features, Ledger
- All config fields editable inline; atomic save to `~/.m31a/config.toml`
- API key fields: masked by default; "Store in keychain" button

### Deliverables

- All 26 acceptance criteria pass
- `/optimize` reduces estimated plan cost in Plan screen
- `/rollback` correctly creates backup branch and updates task statuses
- Ledger file grows after each Ship and injects context on subsequent Initialize phases

---

## Phase 8 — Polish, Testing & v1.0 Release

**Duration:** 2 weeks  
**Complexity:** 4/10  
**Milestone:** v1.0.0 tagged; binary installable; README complete

### Tasks

**P8.1 — Error handling hardening**
- Audit every `error` return path; styled TUI messages (never raw stack traces)
- Graceful shutdown: `Ctrl+C` saves session state; incomplete tool calls emit `[INTERRUPTED]`
- Offline mode: if both providers unreachable, load REPL with history viewer but no streaming

**P8.2 — Test coverage pass**
- Target: 75% overall coverage, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`
- Integration test suite: full workflow against mocked provider, corrupt session recovery, keychain fallback, auto-fallback
- Race detector: `go test -race ./...` must pass on all platforms

**P8.3 — Cross-platform verification**
- macOS: test keychain on Keychain Services, verify `open` browser launch
- Windows: test Credential Manager, verify `start` browser launch, PTY fallback for Bash
- Linux: test secret-service + `pass` fallback, verify `xdg-open`, test GNOME and KDE keyring

**P8.4 — Documentation**
- `README.md`: quick start (30-second install), first-run walkthrough, all slash commands, config reference
- `CONTRIBUTING.md`: development setup, test instructions, PR conventions
- `docs/ARCHITECTURE.md`: package dependency graph, data flow diagram, phase lifecycle
- `--help` flag: all CLI flags, subcommands, env vars

**P8.5 — Release pipeline**
- `goreleaser` config: builds for linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
- Homebrew tap formula (`homebrew-m31a`)
- Install script: `curl -fsSL ... | bash`
- GitHub release: binaries + checksums + signed with `cosign`

**P8.6 — v1.0.0 tag**
- All 26 acceptance criteria verified by release checklist script
- `CHANGELOG.md` v1.0.0 entry written; tag and push

### Deliverables

- `curl | bash` install produces working binary on Linux, macOS, Windows
- `go test -race ./...` passes on all three platforms
- GitHub release published with checksums

---

## Phase 9 — V1.1: Ghost Mode, Terminal PiP & Concurrent Subagents

**Duration:** 6 weeks (post v1.0 release)  
**Complexity:** 9/10  
**Milestone:** v1.1.0 tagged

### V1.1.1 — Deferred Tools (2 weeks)

| Tool | Description |
|------|-------------|
| **FileEdit** | Unified diff or search/replace with preview modal; triggers permission modal |
| **WebFetch** | HTTP GET with SSRF protection; text extraction via `goquery` |
| **WebSearch** | SerpAPI or Tavily integration; top-N result summaries |
| **AgentTool** | Spawn subagent goroutine with independent context; parent renders progress |
| **TaskTool** | Task lifecycle management from within LLM tool call stream |
| **AskUserQuestion** | Blocking question modal for structured mid-execution questions |
| **GitTool** | `git status`, `diff`, `log`, `stash`; dangerous ops require permission modal |

### V1.1.2 — Ghost Mode (2 weeks)

`pkg/ghost/` wraps `git worktree` lifecycle:

1. `/ghost` activates ghost mode for next workflow
2. On Initialize: `git worktree add .m31a/ghost/<session-id>/ <current-branch>`
3. Full workflow executes inside worktree directory
4. After Ship: diff browser shows changes vs. current branch
5. `M` = merge, `D` = discard (`git worktree remove` + branch deletion)

Zero-risk guarantee: main working directory untouched until Merge chosen.

### V1.1.3 — Terminal PiP (1.5 weeks)

- Long-running Bash commands offer "Pin to PiP panel" prompt
- Fixed-size Bubble Tea sub-window in bottom-right corner (configurable position)
- Receives lines from `chan string` fed by background goroutine reading process stdout/stderr
- Main REPL remains fully interactive; keys: `E` = expand, `X` = close, `M` = cycle position

### V1.1.4 — Concurrent subagents (2.5 weeks)

**The hardest piece in V1.1.** Bubble Tea's single-threaded update loop must coexist with multiple goroutines.

Architecture:
- Each subagent goroutine receives `chan SubagentEvent` from parent `AppState`
- Subagents own their message history; cannot read/write `AppState` directly
- All state mutations sent as typed events through channel
- Parent `Update()` drains all pending `SubagentEvent` on each tick; renders as indented sub-task cards
- Task runner's `Schedule()` groups consumed by subagent pool — one goroutine per task in a group
- Permission escalation: modal blocks main TUI; subagent channel pauses (buffered, never dropped)

### Deliverables for V1.1

- All 7 deferred tools execute and render correctly
- Ghost mode runs full workflow in isolation; merge/discard both function
- PiP panel displays live output without blocking REPL
- Concurrent subagents complete tasks in parallel; race detector passes
- v1.1.0 tagged and released

---

## Phase 10 — Provider & Message Layer Adaptations

**Duration:** 1.5 weeks  
**Complexity:** 5/10  
**Milestone:** Provider connection status, slash command routing, session initialization from context

### Goals

Adapt the provider and message layers with OpenCode-compatible enhancements: connection status display, slash command /fork routing infrastructure, and session initialization from command context.

### Tasks

**P10.1 — Provider connection status**
- Display connection status (connected/reconnecting/disconnected) in REPL header
- Provider health check integration with status indicator
- Auto-reconnect banner when provider connection drops

**P10.2 — Slash command routing for session management**
- `/fork` command infrastructure (routed to Phase 11 implementation)
- Command parsing updates for session-switching commands
- CommandResult.SessionID plumbing for session transition

**P10.3 — Session initialization from context**
- CommandContext.SessionID field population during session creation
- Session ID propagation through message lifecycle

### Deliverables

- Provider connection status visible in TUI header
- Slash command routing supports session-switching commands
- Session ID flows through command context correctly

---

## Phase 11 — Session & Config Adaptations

**Duration:** 1.5 weeks  
**Complexity:** 6/10  
**Milestone:** Session forking creates child sessions; multi-layer config with project-level override; permission rules use glob matching

### Goals

Implement three OpenCode adaptations: session branching for exploration workflows, multi-layer configuration for per-project settings, and permission ruleset completion for declarative tool gating.

### Requirements

- **P11-ADAPT-02** — Session forking: ParentID/ChildrenIDs on session, ForkSession, /fork command, sibling navigation via /prev/next
- **P11-ADAPT-10** — Multi-layer configuration: project-level m31a.toml, schema validation, ${VAR} substitution, walk-up discovery (max 3 levels)
- **P11-ADAPT-09** — Permission ruleset completion: glob matching of PermissionRule.Pattern against tool params, per-agent profiles, allow/deny/ask actions

### Plans

- [ ] 11-01-PLAN.md — Session Forking (Wave 1)
- [ ] 11-02-PLAN.md — Multi-Layer Configuration (Wave 2)
- [ ] 11-03-PLAN.md — Permission Ruleset Completion (Wave 2)

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1 | 11-01 | yes |
| 2 | 11-02, 11-03 | yes, yes |

### Deliverables

- `/fork`, `/prev`, `/next` commands for session branching
- Config loaded from three layers (global → env → project) with validation
- Permission rules match against tool inputs via doublestar glob patterns
- All adaptations pass `go test -race -count=1 -cover ./...`

---

## Deferred to Future (V1.2+)

| Feature | Reason for Deferral | Notes |
|---------|-------------------|-------|
| **Vision support** | No terminal UX for image input/output | `CapFlags.Vision` field exists but unused |
| **Voice interaction** | Terminal not suited for audio | Could explore TTS/STT integration later |
| **Multi-modal outputs** | Terminal cannot render images/video | ASCII art previews possible |
| **Plugin system** | Adds complexity to core | Consider after V1.1 stabilization |
| **Team collaboration** | Requires server component | Out of scope for CLI-only tool |

---

## Critical Path

```
Phase 0 (Foundation)
  → Phase 1 (Provider Layer)
    → Phase 2 (TUI Shell)
      → Phase 3 (Rendering Pipeline)
        → Phase 6 (Workflow Engine)
          → Phase 7 (Signature Features)
            → Phase 8 (Polish & Release)
```

**Parallel tracks:**
- Phase 4 (Tool System) runs parallel to Phase 5 (State & Config)
- Phase 8 (Background Systems) starts once Phase 6 is 50% complete

---

## Package Dependency Graph

```
cmd/m31a/
└── internal/tui/           (Bubble Tea app, all screens)
    ├── internal/provider/  (LLMProvider interface + OpenRouter + Zen)
    │   └── pkg/session/    (session state + file persistence)
    ├── internal/workflow/  (six phases: init, discuss, plan, execute, verify, ship)
    │   ├── pkg/taskrunner/ (topological sort, task lifecycle)
    │   ├── pkg/bisect/     (git bisect wrapper)
    │   ├── pkg/autodream/  (context consolidation)
    │   └── pkg/session/
    ├── internal/tools/     (bash, fileread, filewrite, glob, grep)
    ├── pkg/arbitrage/      (complexity scoring, model cost comparison)
    ├── pkg/ledger/         (cross-session learning ledger)
    ├── pkg/rollback/       (commit chain browser)
    └── internal/config/    (toml + env + pkg/keychain/)
        └── pkg/keychain/   (Linux/macOS/Windows secret storage)
```

---

## Package Map

### Internal Packages (`internal/`)

| Package | Purpose | Phase | Est. LOC |
|---------|---------|-------|----------|
| `internal/types/` | Core types: Message, Task, ToolCall, WorkflowPhase | 0 | ~300 |
| `internal/config/` | TOML config parsing, env resolution, defaults | 5 | ~500 |
| `internal/keychain/` | OS keychain integration (Linux/macOS/Windows) | 5 | ~450 |
| `internal/session/` | Session ID generation, directory management | 5 | ~350 |
| `internal/state/` | File-based state read/write (PROJECT.md, TASKS.md, STATE.md) | 5 | ~300 |
| `internal/git/` | Git operations: init, commit, log, diff, bisect, reset | 0, 6 | ~400 |
| `internal/errors/` | Sentinel errors, error types | 0 | ~100 |
| `internal/provider/` | LLM provider interface, registry, SSE parser, cache | 1 | ~800 |
| `internal/provider/openrouter/` | OpenRouter-specific client | 1 | ~700 |
| `internal/provider/zen/` | OpenCode Zen-specific client | 1 | ~700 |
| `internal/tui/` | Bubble Tea app, key bindings, command parser | 2 | ~600 |
| `internal/tui/theme/` | Color palette, theme definitions | 2 | ~300 |
| `internal/tui/components/` | Reusable TUI components (header, bubble, toolcard, etc.) | 2, 3, 4 | ~1,500 |
| `internal/tui/screens/` | Full TUI screens (REPL, plan, execute, etc.) | 2, 6, 7, 8 | ~1,800 |
| `internal/tokens/` | Token estimation, calibration, budget enforcement | 5 | ~300 |
| `internal/tools/` | Tool implementations (Bash, FileRead, etc.) + dispatcher | 4 | ~1,600 |
| `internal/workflow/` | State machine, phase orchestration, context pruning | 6 | ~500 |
| `internal/workflow/phases/` | Individual phase implementations | 6, 7 | ~2,300 |
| `internal/autodream/` | Context consolidation engine | 7 | ~400 |

### Public Packages (`pkg/`)

| Package | Purpose | Phase | Est. LOC |
|---------|---------|-------|----------|
| `pkg/taskrunner/` | Task scheduling, dependency resolution, execution | 6 | ~650 |
| `pkg/verify/` | Verification checks (file, syntax, tests, imports) | 6 | ~400 |
| `pkg/bisect/` | Git bisect wrapper for regression detection | 6 | ~320 |
| `pkg/arbitrage/` | Cost optimization engine, complexity heuristic | 7 | ~450 |
| `pkg/ledger/` | Cross-session learning ledger, aggregate stats | 7 | ~420 |
| `pkg/rollback/` | Commit rollback chain, soft/hard reset | 7 | ~350 |
| `pkg/session/` | Session lifecycle, ID generation, archive | 5 | ~650 |
| `pkg/keychain/` | OS keychain abstraction (Linux/macOS/Windows) | 5 | ~450 |
| `pkg/autodream/` | Context consolidation (shared with internal) | 7 | ~550 |
| `pkg/subagent/` | Subagent goroutine system (V1.1) | 9 | ~800 |
| `pkg/ghost/` | Git worktree lifecycle for ghost mode (V1.1) | 9 | ~600 |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| OpenRouter/Zen API changes break streaming | Medium | High | Abstract SSE parsing behind `StreamIterator`; pin tested response fixtures in tests |
| Bubble Tea frame budget exceeded on large outputs | Medium | Medium | Cache all Glamour renders; cap tool output at 10,000 chars; profile before release |
| OS keychain unavailable (headless CI, Docker) | High | Low | Env var → config file fallback is always available; keychain failure is non-fatal |
| git bisect state corruption on interrupted session | Low | High | `git bisect reset` called in all exit paths including `Ctrl+C`; backed by checkpoint |
| Context window exceeds limit mid-workflow | Medium | High | AutoDream at 60%; per-phase context pruning; `/compress` manual trigger |
| Concurrent subagent (V1.1) race condition | Medium | High | All state via typed channels; `go test -race` in CI; no direct `AppState` mutation from goroutines |
| Windows PTY not available for Bash | Medium | Medium | Plain pipe fallback already implemented; affects only interactive commands |
| Scope creep from V1.1 features | High | Medium | Strict V1 feature freeze; V1.1 deferred tools tracked separately |
| Provider API changes break compatibility | Medium | Medium | Abstract provider interface; minimal assumptions about response format |

---

## Testing Strategy

| Test Type | Coverage Target | Tools | Phase |
|-----------|----------------|-------|-------|
| **Unit tests** | 80%+ | `go test`, `testify` | All phases |
| **Integration tests** | All acceptance criteria | Mock HTTP, mock filesystem, mock git | 6, 8, 9 |
| **E2E tests** | Full workflow cycle | Real provider (test key), temp directory | 8 |
| **Performance tests** | 16ms/frame TUI redraw | `go tool pprof`, custom benchmarks | 8 |
| **Cross-platform tests** | Linux/macOS/Windows | GitHub Actions matrix, real binaries | 8 |

---

## Change Log

| Date | Change |
|------|--------|
| 2026-05-26 | Initial roadmap created from V1 specification |
| 2026-05-26 | Merged Claude roadmap improvements: complexity scores, LOC estimates, milestone versions, team multipliers, package dependency graph |
| 2026-06-01 | Added Phase 10 (Provider & Message Layer Adaptations) and Phase 11 (Session & Config Adaptations) from OpenCode adaptation report |
| 2026-06-01 | Phase 11 planned: 3 plans (Session Forking, Multi-Layer Config, Permission Ruleset Completion) |
