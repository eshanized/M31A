# M31A — Implementation Roadmap

> **Source of truth:** This roadmap is derived directly from `adrenaline/idea.md` and `adrenaline/REFERENCE.md`. All estimates assume a **single senior Go developer**. Team multipliers are noted where applicable.
> **Version**: V1 (Dual-Provider: OpenRouter + OpenCode Zen)
> **Last Updated**: 2026-06-06
> **Status**: Phase 24 planned — TUI Redesign

---

## Project Metrics

| Metric | Value |
|--------|-------|
| **Complexity** | 8.2/10 (Very High) |
| **Estimated V1 Duration** | 21 weeks (1 senior Go developer) |
| **Estimated V1 Duration (2-person team)** | 14-15 weeks (Phases 1/2 and 4/5 parallelized) |
| **Estimated V1.1 Duration** | 6 weeks additional (week 27 total) |
| **Total Phases** | 12 (9 for V1, 1 for V1.1, 2 for bug fixes) |
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

### Goal

Build the shell of the TUI — the application skeleton, routing between screens, theme system, and the main REPL view.

**Plans:** 5 plans in 3 waves

Plans:
- [ ] 02-01-PLAN.md — Theme + Types Foundation (Wave 1)
- [ ] 02-02-PLAN.md — Layout Components + Health Ticker (Wave 2)
- [ ] 02-03-PLAN.md — REPL Screen (Wave 2)
- [ ] 02-04-PLAN.md — First Run Setup Screen (Wave 2)
- [ ] 02-05-PLAN.md — App State + Screen Routing (Wave 3)

### Deliverables

- `internal/tui/` package with tea.Model app, screen routing, theme system, REPL, first-run, header, statusbar, health ticker
- All components compile and pass `go test -race ./internal/tui/...`
- Binary continues to print version and exit (no wiring into main.go)



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
**Plans:** 5 plans in 3 waves

### Plans

| Plan | Wave | Objective | Files | Requirements |
|------|------|-----------|-------|--------------|
| `05-01` | 1 | OS keychain integration — `pkg/keychain/` with Linux/macOS/Windows backends, unified Keychain interface, tests | `pkg/keychain/` | P5.2 |
| `05-02` | 1 | Session lifecycle — `pkg/session/` Manager, Session CRUD, atomic writes, archive, tests | `pkg/session/` | P5.3 |
| `05-03` | 2 | File-based state persistence (PROJECT.md, TASKS.md, STATE.md writers/parsers) + checkpoint system | `pkg/session/` | P5.4, P5.5 |
| `05-04` | 2 | Config loader (BurntSushi/toml + env resolution) + token estimator (tiktoken-go + fallback) | `internal/config/loader.go`, `internal/tokens/` | P5.1, P5.7 |
| `05-05` | 3 | Settings screen (tabbed config editor) + Resume screen (bubbles/list session browser) + AppState wiring | `internal/tui/settings.go`, `internal/tui/resume.go`, `internal/tui/types.go`, `internal/tui/app.go` | P5.6 |

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1 | 05-01, 05-02 | yes, yes |
| 2 | 05-03, 05-04 | yes, yes |
| 3 | 05-05 | no (checkpoint for visual verification) |

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

**Plans:** 8 plans

```
Plans:
- [ ] 07-01-PLAN.md — Slash Command System (commands.go, 16+ handlers, tests)
- [ ] 07-02-PLAN.md — Commit Rollback Chain (rollback.go, 3 reset modes, tests)
- [ ] 07-03-PLAN.md — Cost-Aware Model Arbitrage (arbitrage.go, scorer, tests)
- [ ] 07-04-PLAN.md — AutoDream Context Consolidation (autodream.go, pause/resume, tests)
- [ ] 07-05-PLAN.md — Cross-Session Learning Ledger (ledger.go, stats, filtering, tests)
- [ ] 07-06-PLAN.md — Gap Fixes (FallbackEvent wiring, T key toggle, cache ticker)
- [ ] 07-07-PLAN.md — Model Selector UI (full-screen overlay, search, provider filter)
- [ ] 07-08-PLAN.md — Settings Screen (6 tabs, inline editing, masked API keys, tests)
```

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

**Duration:** 2 weeks  
**Complexity:** 7/10  
**Milestone:** Structured tool calls from provider responses; session undo restores conversation state; auto-compaction triggers at threshold  
**Source:** `rush/opencode_adaptation_report.md` (items 1, 3, 4)
**Depends on:** Phase 7 (Signature Features)

### Adaptations

1. **Structured Tool Call Handling** (adoption #4) — Replace regex-based JSON extraction from streaming text with native provider API tool call handling. Update `StreamChunk` to carry structured tool call events. Update SSE parser to extract `tool_use` blocks from Anthropic/OpenAI response formats. Remove fragile `parseToolCalls()`, `extractJSONObject()`, `stripCodeBlocks()` from engine.go.

2. **Session Undo/Revert** (adoption #3) — Complete the partial `/undo` implementation. `/undo` currently only *displays* checkpoint info but does NOT restore conversation state. Add message-level revert with `reverted_to` field on messages. Add `/redo` for revert undo.

3. **Context Compaction Auto-Trigger** (adoption #1) — Context compaction already exists in `pkg/autodream/` but has no auto-trigger in the workflow engine. Add automatic consolidation when context exceeds 60% threshold. Add `/compact` alias alongside existing `/compress`.

### Plans

```
Plans:
- [x] 10-01-PLAN.md — Structured Tool Call Handling (Wave 1)
- [x] 10-02-PLAN.md — Session Undo/Revert Completion (Wave 1)
- [x] 10-03-PLAN.md — Context Compaction Auto-Trigger (Wave 2)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 10-01, 10-02 | yes, yes |
| 2    | 10-03 | yes |

### Deliverables

- Provider SSE parser extracts native `tool_use` blocks instead of regex JSON
- `StreamChunk` carries typed tool call events
- `/undo` restores conversation state from checkpoints; `/redo` restores
- Auto-compaction triggers in workflow engine at 60% threshold
- All existing tests pass; new tests for structured tool calls, undo/redo, auto-compact

---

## Phase 11 — Session & Config Adaptations

**Duration:** 2 weeks  
**Complexity:** 6/10  
**Status:** ✅ COMPLETE  
**Milestone:** Session forking creates child sessions; multi-layer config with project-level override; permission rules use glob matching  
**Source:** `rush/opencode_adaptation_report.md` (items 2, 9, 10)
**Depends on:** Phase 10

### Adaptations

1. **Session Forking** (adoption #2) — ✅ `Session.ParentID/ChildrenIDs` implemented. `ForkSession()`, `SiblingSessions()`, `ListChildren()` in session Manager. `/fork`, `/prev`, `/next` slash commands registered in TUI. 14 tests passing.

2. **Multi-Layer Configuration** (adoption #10) — ✅ `findProjectConfig()` walks up from cwd (max 3 levels). `mergeConfig()` merges project toml over global/env. `validateConfig()` with typed `ValidationError`. `applyVarSubstitution()` for `${VAR}` patterns across all string fields. 17 tests, 72.8% coverage.

3. **Permission Ruleset Completion** (adoption #9) — ✅ Glob matching via `doublestar` in `checkPermission()`. Per-agent permission profiles (`PermissionsAgentConfig` + `SelectAgent()`). `matchAnyParamValue()` stringifies non-string types before glob. 11 test functions with 30+ subtests.

### Plans

```
Plans:
- [x] 11-01-PLAN.md — Session Forking (Wave 1)
- [x] 11-02-PLAN.md — Multi-Layer Configuration (Wave 2)
- [x] 11-03-PLAN.md — Permission Ruleset Completion (Wave 2)
```

### Wave Structure

| Wave | Plans | Autonomous | Status |
|------|-------|------------|--------|
| 1    | 11-01 | yes        | ✅ Done |
| 2    | 11-02, 11-03 | yes, yes | ✅ Done |

### Code Review

Phase 11 underwent deep code review (14 source files, 15 test files):
- **2 critical** — CR-01 (permission rules silently ignored), CR-02 (stale session ID after fork/prev/next) — **both fixed**
- **7 warnings** — boolean merge asymmetry, var substitution order, agent ask bypass, missing agent DefaultAction substitution, non-string param matching — **all fixed**
- **3 info** — unused interface, doc comment, test coverage gap — **documented for future work**
- All fixes verified with `go test -race -count=1 ./...` (23 packages passing) and `CGO_ENABLED=0 go build`

### Deliverables

- ✅ `Session.ParentID` field populated on fork; `/fork` creates child session
- ✅ `/prev`, `/next` navigate sibling sessions
- ✅ `m31a.toml` in project root overrides `~/.m31a/config.toml` fields
- ✅ Malformed config produces clear validation errors
- ✅ Dispatcher matches file paths against `PermissionRule.Pattern` globs
- ✅ Per-agent permission profiles in config
- ✅ Code review findings addressed — all critical/warning items resolved

---

## Phase 12 — UX & Editor Experience Adaptations

**Duration:** 2.5 weeks  
**Complexity:** 6/10  
**Milestone:** Model variants and favorites persist; shell mode bypasses LLM; prompt history persists with frecency; diff viewer renders styled output; `@file` syntax includes file content  
**Source:** `rush/opencode_adaptation_report.md` (items 8, 11, 12, 13, 14)
**Depends on:** Phase 10

### Adaptations

1. **Model Variants & Favorites** (adoption #8) — Add `Variant` field to `ModelInfo`. Add recent model list (up to 10) and favorites list persisted to `~/.m31a/recent_models.json`. Add `Ctrl+M` / `Ctrl+Shift+M` keyboard cycling through recent models. Per-agent model assignments.

2. **Shell Mode** (adoption #12) — In REPL, detect `!` prefix and execute commands directly via Bash tool without LLM involvement. Bypasses the entire tool-use loop for quick commands.

3. **Prompt History with Frecency Ranking** (adoption #11) — Persist prompts to `~/.m31a/prompt_history.json`. Add frecency scoring (frequency + recency). Arrow-up/down navigates persistent history with frecency-based ordering.

4. **Diff Viewing Enhancement** (adoption #13) — Create dedicated diff viewer screen with syntax highlighting using lipgloss/glamour. Split/unified diff format selection. Interactive scrolling for long diffs.

5. **Editor Context Auto-Include** (adoption #14) — Add `@filepath` syntax in REPL that auto-includes file contents in the next LLM prompt. File content injection before sending chat request.

### Plans

```
Plans:
- [ ] 12-01-PLAN.md — Model Variants & Favorites System (Wave 1)
- [ ] 12-02-PLAN.md — Shell Mode (Wave 1)
- [ ] 12-03-PLAN.md — Prompt History with Frecency (Wave 2)
- [ ] 12-04-PLAN.md — Diff Viewer Screen (Wave 2)
- [ ] 12-05-PLAN.md — Editor Context Auto-Include (Wave 2)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 12-01, 12-02 | yes, yes |
| 2    | 12-03, 12-04, 12-05 | yes, yes, yes |

### Deliverables

- `ModelInfo.Variant` field; recent/favorite model lists persist to disk
- `Ctrl+M` cycles through recent models inline
- `!command` executes directly without LLM involvement
- Prompt history persists across sessions; frecency-ranked
- `/diff` renders interactive styled diff view
- `@filepath` in REPL includes file content in next prompt

---

## Phase 13 — Infrastructure & Sharing Adaptations

**Duration:** 1.5 weeks  
**Complexity:** 5/10  
**Milestone:** Session export produces shareable markdown; pub/sub decouples internal events  
**Source:** `rush/opencode_adaptation_report.md` (items 15, 16)
**Depends on:** Phase 12

### Adaptations

1. **Session Sharing/Export** (adoption #15) — Export session history to markdown or HTML for sharing. `pkg/session/export.go` with format options. `/export` command writes to file or stdout.

2. **Bus/PubSub Event System** (adoption #16) — Introduce a lightweight pub/sub system for internal events (session changes, tool executions, phase transitions). Decouples the TUI message handling from direct channel references.

### Plans

**Plans:** 2 plans

```
Plans:
- [x] 13-01-PLAN.md — Session Sharing/Export (Wave 1)
- [x] 13-02-PLAN.md — Bus/PubSub Event System (Wave 1)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 13-01, 13-02 | yes, yes |

### Deliverables

- `/export` command writes session as markdown/html to stdout or file
- `internal/bus/bus.go` with typed event channels and wildcard subscriptions
- Existing message patterns transition to bus where appropriate

---

## Phase 14 — TUI ↔ Core Wiring Fixes

**Duration:** 3 weeks  
**Complexity:** 8/10  
**Milestone:** Multi-phase workflow `Initialize → Discuss → Plan → Execute → Verify → Ship` runs end-to-end without dead-ends, screen flashes, or message drops
**Source:** `rush/tui_core_wiring_report.md` (11 wiring issues, D-01 through D-11)
**Depends on:** Phase 13

### Background

The TUI ↔ Core wiring audit (`rush/tui_core_wiring_report.md`) followed
a previous walkthrough that fixed 13 issues (W-01 through W-13) but
missed workflow-phase wiring — specifically the Discuss/Plan/Execute/
Verify phase transitions and the `msgChan` synchronization that
backstops them. The audit identified 11 additional issues ranging from
CRITICAL (Discuss phase deadlock) to LOW (style/cleanup).

### Wiring Issues Fixed

| ID | Severity | Component | Type | Plan |
|----|----------|-----------|------|------|
| D-01 | CRITICAL | Discuss phase Q&A | `dead_end` | 14-01 |
| D-02 | HIGH | Plan screen | `unreachable_screen` | 14-02 |
| D-03 | HIGH | PlanModel dimensions | `missing_init` | 14-02 |
| D-04 | HIGH | msgChan race | `data_race` | 14-03 |
| D-05 | MEDIUM | Execute/Verify flash | `unreachable_screen` | 14-02 |
| D-06 | MEDIUM | Workflow state non-persistent | `missing_persistence` | 14-04 |
| D-07 | MEDIUM | Discuss not streamed | `inconsistent_patterns` | 14-05 |
| D-08 | LOW | PlanModel field access | `tight_coupling` | 14-05 |
| D-09 | LOW | Sidebar threshold | `magic_number` | 14-05 |
| D-10 | LOW | streamCh not closed | `resource_leak` | 14-05 |
| D-11 | MEDIUM | AppState god object | `architectural_smell` | deferred |

### Plans

```
Plans:
- [ ] 14-01-PLAN.md — Discuss Phase Q&A Wiring (Wave 1, D-01)
- [ ] 14-02-PLAN.md — Workflow Screen Wiring (Wave 1, D-02/D-03/D-05)
- [ ] 14-03-PLAN.md — msgChan Drainer Synchronization (Wave 1, D-04)
- [ ] 14-04-PLAN.md — Workflow State Persistence (Wave 2, D-06)
- [ ] 14-05-PLAN.md — Discuss Streaming + Low Severity (Wave 2, D-07/D-08/D-09/D-10)
- [ ] 14-06-PLAN.md — AppState Refactor (Wave 3, D-11, optional/stretch)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 14-01, 14-02, 14-03 | yes, yes, yes | — |
| 2    | 14-04, 14-05 | yes, yes | Wave 1 |
| 3    | 14-06 (stretch) | yes | Wave 2 |

### Deliverables

- Discuss phase Q&A: 5-minute timeout, sequential questions via existing
  `QuestionRequestMsg` flow, no deadlock
- Plan screen reachable with non-zero dimensions; user accepts via 'a' key
- Execute/Verify/Ship screens wait for user confirmation (no flash)
- `msgChan` synchronized via per-phase `done` channel and `phaseGen` counter
- Workflow state (goal, phase, questions) persisted to `session.json`
- `/workflow resume` command restores from persisted state
- Discuss phase LLM response streams into REPL token-by-token
- `streamCh` properly closed on stream end
- `ModelSelector` setters (`SetRegistry`, `SetTheme`) replace direct field access
- `SidebarWidthThreshold` configurable via `~/.m31a/config.toml` (default 120)
- All existing tests still pass; new tests cover each fix

### Out of Scope

- **D-11 AppState refactor** — listed as Plan 14-06 but marked
  optional/stretch. The coordinator pattern (WorkflowCoordinator,
  ScreenRouter, StreamCoordinator) is recommended for the next major
  release but is not required for v1.0 stability. May be deferred
  to v1.1.
- **Shell mode permission bypass** (loophole report H1) — already
  documented as intentional; not a Phase 14 fix.
- **Vision / multi-modal support** — out of M31A V1 scope.

---

## Phase 15 — Comprehensive Deep Audit Fixes

**Duration:** 3 weeks  
**Complexity:** 8/10  
**Milestone:** All 72 findings from the comprehensive deep codebase audit are fixed, with regression tests for every Critical and High issue, and the binary is ready for v1.0.0 release
**Source:** `rush/comprehensive_deep_audit_2026.md` (72 findings, 7 Critical, 19 High, 28 Medium, 18 Low)
**Depends on:** Phase 14 (TUI ↔ Core Wiring Fixes)

### Background

The comprehensive deep audit (`rush/comprehensive_deep_audit_2026.md`,
auditor: `opencode / MiniMax-M3`, dated 2026-06-02) read every Go file
in the 171-file tree across `cmd/`, `internal/`, and `pkg/`. It found
**72 distinct issues** and identified five cross-cutting themes:

1. **Channel ownership in the streaming pipeline is broken** (C-3, H-8,
   H-9, H-14, M-21) — the REPL holds `streamCh`/`streamDone` and passes
   them to a goroutine that closes them. Two owners, one channel.
2. **Lint-by-string-match in lieu of typed errors** (C-5, H-11, M-29) —
   `strings.Contains(errStr, "rate limit")` is fragile and locale/
   wording dependent. `internal/errors` already defines 15 sentinels.
3. **LLM output is untrusted but parsing code is fragile** (C-4, H-5,
   M-22) — ReDoS, JSON comments, empty `done` case for usage tracking.
4. **Configuration fields declared but unwired** (L-7, L-8, M-9) — the
   package boundaries look complete but the wiring is partial.
5. **Concurrency primitives in single-threaded contexts** (H-9, H-14,
   C-3) — `sync.Once` and `sync.Mutex` are used in places where the
   Bubble Tea single-thread model would have been sufficient, while
   real concurrency bugs go unaddressed.

The audit's recommended fix order (Critical first, then High, then
Medium, then Low) is reflected in the wave structure below. **Every
Critical and High must land with a regression test** — the audit's
closing note: *"none of these findings have a regression test that
would have caught them."*

### Issues Fixed

| ID | Sev | Component | Plan |
|----|-----|-----------|------|
| C-1 | CRITICAL | `replModel` nil deref (TUI panic) | 15-01 |
| C-2 | CRITICAL | `AutoFallback` dead (resilience) | 15-05 |
| C-3 | CRITICAL | Stream channel double-close | 15-02 |
| C-4 | CRITICAL | `parseToolCalls` ReDoS / OOM | 15-03 |
| C-5 | CRITICAL | `isDBusUnavailable` over-matches | 15-04 |
| C-6 | CRITICAL | WebFetch SSRF DNS rebinding | 15-04 |
| C-7 | CRITICAL | Bash `NaN` timeout + `go build ./...` | 15-04 |
| H-1 | HIGH | `Schedule` group order non-deterministic | 15-06 |
| H-2 | HIGH | Empty `Status` re-runs done tasks | 15-06 |
| H-3 | HIGH | `healTask` verifies by `os.Stat` only | 15-06 |
| H-4 | HIGH | Bisect loop unbounded | 15-06 |
| H-5 | HIGH | JSON comment stripping | 15-03 |
| H-6 | HIGH | `SetSessionID` `..` traversal | 15-06 |
| H-7 | HIGH | `resolvedAPIKey` stale after fallback | 15-05 |
| H-8 | HIGH | Segment-boundary logic divergent | 15-07 |
| H-9 | HIGH | `safeClose` race | 15-02 |
| H-10 | HIGH | Emitter drops silently | 15-07 |
| H-11 | HIGH | Health interval substring check | 15-08 |
| H-12 | HIGH | Zen key ignored at startup | 15-05 |
| H-13 | HIGH | Slash command case sensitivity | 15-07 |
| H-14 | HIGH | Stream goroutine concurrency | 15-02 |
| H-15 | HIGH | Prompts embed not tested | 15-06 |
| H-16 | HIGH | `collectDiffStats` heuristic broken | 15-06 |
| H-17 | HIGH | `verifyTask` runs `go build ./...` | 15-06 |
| H-18 | HIGH | `listCwdFiles` depth count fragile | 15-06 |
| H-19 | HIGH | `session_id_length` silently clamped | 15-08 |
| M-1..M-33 | MEDIUM | Various (28 issues) | 15-08/09/10 |
| L-1..L-18 | LOW | Style/dead code (18 issues) | 15-08/10 |

Full per-issue mapping is in the CONTEXT.md and individual PLAN.md
files.

### Plans

```
Plans:
- [ ] 15-01-PLAN.md — Critical TUI Nil-Safety Guards (Wave 1, C-1)
- [ ] 15-02-PLAN.md — Stream Pipeline Channel Ownership Refactor (Wave 1, C-3/H-9/H-14/M-21)
- [ ] 15-03-PLAN.md — LLM Input Safety & Output Parsing Hardening (Wave 1, C-4/H-5/M-22)
- [ ] 15-04-PLAN.md — Tool Security: Keychain, WebFetch, Bash Hardening (Wave 2, C-5/C-6/C-7)
- [ ] 15-05-PLAN.md — Provider Resilience: Autofallback & Key Resolution (Wave 2, C-2/H-7/H-12)
- [ ] 15-06-PLAN.md — Workflow Engine Correctness (Wave 2, H-1..H-6/H-15..H-18)
- [ ] 15-07-PLAN.md — TUI Segment Logic & Concurrency Cleanup (Wave 3, H-8/H-10/H-11/H-13)
- [ ] 15-08-PLAN.md — Typed Errors, Dead Config & Unwired Packages (Wave 3, H-19/M-*)
- [ ] 15-09-PLAN.md — Session, Ledger, Rollback, AutoDream Hardening (Wave 4, M-6/M-7/M-16/M-19/M-20/M-27/M-28/M-31)
- [ ] 15-10-PLAN.md — Low Priority Polish & Tool Cleanup (Wave 4, L-1..L-18/M-30/M-33)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 15-01, 15-02, 15-03 | yes, yes, yes | — |
| 2    | 15-04, 15-05, 15-06 | yes, yes, yes | Wave 1 |
| 3    | 15-07, 15-08 | yes, yes | Wave 2 |
| 4    | 15-09, 15-10 | yes, yes | Wave 3 |

### Deliverables

- **Zero TUI panics** in normal user flow (C-1, H-9 guards)
- **Stream pipeline single-owner** — channel allocation owned by
  `StartStreamCmd`; REPL does not hold `streamCh`/`streamDone` (C-3,
  H-14, M-21)
- **LLM input bounded** — `parseToolCalls` capped at 1 MB; max tools
  per call limited; JSON comments stripped (C-4, H-5)
- **WebFetch SSRF safe** — `DialContext` resolves once, pins IP,
  re-checks after connect (C-6)
- **Bash `NaN`/`Inf` rejected** before cast; `verifyTask` scopes to
  task packages (C-7, H-17)
- **`AutoFallback` wired** — 429/503 trigger fallback; key resolution
  follows active provider (C-2, H-7, H-12)
- **Workflow engine deterministic** — sorted groups, `Status` →
  `Pending` only if no `CommitHash`, `os.Stat`+`AcceptanceCriteria`
  heal verify, `git bisect run`, prompts embed test, `git diff
  --name-status` for DiffStats, `SetSessionID` uses
  `baseSessionsDir`, `NewEngine` validates (H-1..H-6, H-15..H-18)
- **Typed errors throughout** — `errors.Is(err,
  m31errors.ErrRateLimited)` replaces string matching (H-11, C-5, M-29)
- **TUI segment logic single-source** — `streaming.go` is the only
  segment-boundary path; `repl_stream.go` reuses it (H-8, M-22, M-26)
- **All regression tests pass** — `go test -race -count=1 -cover
  ./...` green on Linux/macOS/Windows; coverage for `pkg/taskrunner`,
  `pkg/bisect`, `pkg/rollback`, `internal/workflow`, `internal/provider`
  ≥ 80%

### Out of Scope

- **AppState refactor (D-11 / M-32)** — already deferred from Phase 14;
  coordinator pattern is a v1.1 concern.
- **V1.1 features** (Ghost mode, Terminal PiP, Concurrent subagents) —
  separate phase.
- **MCP / Plugin / Vision** — explicit v2.0+.

### Test Strategy

Every Critical and High lands with at least one regression test that
**would have caught the bug**. The audit's closing note is the basis
for this rule: *"none of these findings have a regression test that
would have caught them."* Mediums get a test where the cost is
proportional to the value; Lows are spot-checked.

---

## Phase 16 — UX Polish

**Duration:** 2 weeks  
**Complexity:** 5/10  
**Status:** ✅ COMPLETE  
**Milestone:** Permission timeout urgency, question timeout warnings, /help categorized, shell mode docs, tool card improvements, permission syntax highlighting, config validation formatting, autodream message improvements

### Plans

```
Plans:
- [x] 16-01-PLAN.md — Trust & Safety Fixes (Wave 1)
- [x] 16-02-PLAN.md — Error UX Fixes (Wave 1)
- [x] 16-03-PLAN.md — Feedback & Loading (Wave 2)
- [x] 16-04-PLAN.md — Theme & Visual Fixes (Wave 2)
- [x] 16-05-PLAN.md — Navigation & Help (Wave 2)
- [x] 16-06-PLAN.md — Screen-Specific (Wave 3)
- [x] 16-07-PLAN.md — Tool & Provider (Wave 3)
- [x] 16-08-PLAN.md — Config & Polish (Wave 3)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 16-01, 16-02 | yes, yes |
| 2    | 16-03, 16-04, 16-05 | yes, yes, yes |
| 3    | 16-06, 16-07, 16-08 | yes, yes, yes |

---

## Phase 17 — Post-Phase-16 Audit Fixes

**Duration:** 1 day  
**Complexity:** 4/10  
**Status:** ✅ COMPLETE  
**Milestone:** All 36 issues from comprehensive codebase audit resolved

### Background

A comprehensive codebase audit performed after Phase 16 found 36 issues
(4 Critical, 9 High, 12 Medium, 11 Low) across build, test, correctness,
security, and code quality. All fixes verified with `go test -race` and
`go build ./...`.

### Plans

```
Plans:
- [x] 17-01-PLAN.md — Critical Build & Test Fixes (Wave 1, C-1 through C-4)
- [x] 17-02-PLAN.md — High Severity Correctness Fixes (Wave 2, H-1 through H-9)
- [x] 17-03-PLAN.md — Medium Severity Fixes (Wave 3, M-1 through M-11)
- [x] 17-04-PLAN.md — Low Severity Polish (Wave 4, L-1 through L-11)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 17-01 | yes |
| 2    | 17-02 | yes |
| 3    | 17-03 | yes |
| 4    | 17-04 | yes |

### Key Fixes

- **C-1:** go.mod bumped to Go 1.24 (testing.Context support)
- **C-2:** ModelCache.Refresh uses atomic.Bool (deadlock prevention)
- **C-3:** Edit.atomicWrite uses crypto/rand temp filenames (race prevention)
- **C-4:** Zen client error patterns verified, tests passing
- **H-4:** WebFetch shares http.Client (connection reuse)
- **H-5:** SSEParser trims whitespace before [DONE]
- **H-9:** SetProvider returns tea.Cmd (async, no TUI blocking)
- **M-4:** WebFetch SSRF DNS pinning (TOCTOU prevention)
- **M-10:** Config.Save copies struct before clearing API keys
- **L-5:** config.atomicWrite uses 0600 permissions
- **L-6:** Registry.SetActive returns ErrProviderNotFound

### Deliverables

- All 36 audit findings resolved
- `go build ./...` clean
- `go vet ./...` zero errors
- `go test -race ./...` passes

---

## Phase 18 — Welcome Page Rebuild

**Duration:** 1 day  
**Complexity:** 3/10  
**Status:** ✅ COMPLETE  
**Milestone:** Clean, professional welcome page with proper layout and no broken UI elements  
**Source:** User request — rebuild welcome/landing page shown in screenshot  
**Depends on:** None

### Background

The current welcome/landing page has visual issues:
1. Pixelated ASCII art logo using Unicode block characters (░███) that render poorly
2. Broken UI elements (horizontal lines visible in screenshot)
3. Poor layout and visual hierarchy
4. "No provider configured" warning needs better styling

### Plans

```
Plans:
- [x] 19-01-PLAN.md — Tech Debt & Bug Fixes (Wave 1)
- [x] 19-02-PLAN.md — Security Hardening (Wave 1)
- [x] 19-03-PLAN.md — Performance & Fragile Area Improvements (Wave 2)
- [x] 19-04-PLAN.md — Missing Features & Test Coverage (Wave 2)
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1    | 18-01 | yes |

### Deliverables

- Clean ASCII art logo (4 lines max, not pixelated)
- No broken UI elements (horizontal lines fixed)
- Centered, balanced layout
- Provider status card with proper styling
- Input box with placeholder text
- Keyboard shortcut hints
- Bottom bar (cwd + version)
- All existing tests pass
- No compile errors

---

## Phase 19 — Comprehensive Codebase Concerns Fixes

**Duration:** 3 weeks  
**Complexity:** 7/10  
**Status:** ✅ COMPLETE  
**Milestone:** All items from `.planning/codebase/CONCERNS.md` addressed  
**Source:** Codebase audit — CONCERNS.md generated 2026-06-04  
**Depends on:** Phase 18

### Background

The codebase audit produced 36 identified concerns across 9 categories:
- 6 Tech Debt items (sync.Map leak, dead Stream field, hardcoded maps, deprecated aliases, manual config merge)
- 5 Known Bugs (HEAD~50 fallback, discuss timeout, permission timeout mismatch, silent git errors)
- 4 Security Considerations (SSRF TOCTOU, os.Exit in library, API key in memory, backup accumulation)
- 4 Performance Bottlenecks (filepath.Walk, HTML conversion, JSON extraction, sync.Map lookup)
- 4 Fragile Areas (workflow engine, app_update.go, streaming pipeline, permission system)
- 3 Scaling Limits (session accumulation, ledger growth, model cache)
- 3 Dependencies at Risk (singleflight, doublestar, BurntSushi/toml)
- 3 Missing Critical Features (backup pruning, session auto-cleanup, config hot-reload)
- 7 Test Coverage Gaps (app_update, permissions, commands, repl_stream, quickactions, keychain, error paths)

### Plans

```
Plans:
- [ ] 19-01-PLAN.md — Tech Debt & Bug Fixes (Wave 1): TD-1 through TD-6, BUG-1 through BUG-4
- [ ] 19-02-PLAN.md — Security Hardening (Wave 1): SEC-1 through SEC-4
- [ ] 19-03-PLAN.md — Performance & Fragile Area Improvements (Wave 2): PERF-1 through PERF-4, FRAG-1 through FRAG-4
- [ ] 19-04-PLAN.md — Missing Features & Test Coverage (Wave 2): FEAT-1 through FEAT-3, TEST-1 through TEST-7, DEP-1 through DEP-3
```

### Wave Structure

| Wave | Plans | Autonomous |
|------|-------|------------|
| 1 | 19-01, 19-02 | yes, yes |
| 2 | 19-03, 19-04 | yes, yes |

### Deliverables

- All 6 tech debt items resolved
- All 5 known bugs fixed
- All 4 security considerations mitigated
- All 4 performance bottlenecks improved
- Fragile areas decomposed or hardened
- Backup pruning, session auto-cleanup, config hot-reload implemented
- Test coverage gaps filled (permissions, commands, error paths)
- `go test -race ./...` passes
- `golangci-lint run ./...` clean

---

## Deferred to Future (V2.0+)

| Feature | Reason for Deferral | Notes |
|---------|-------------------|-------|
| **MCP Integration** | Explicitly excluded from this wave | Adaptation report item #5 — add in follow-up |
| **Plugin/Extensibility System** | Explicitly excluded from this wave | Adaptation report item #6 — add in follow-up |
| **Vision support** | No terminal UX for image input/output | `CapFlags.Vision` field exists but unused |
| **Voice interaction** | Terminal not suited for audio | Could explore TTS/STT integration later |
| **Multi-modal outputs** | Terminal cannot render images/video | ASCII art previews possible |
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
            → Phase 9 (V1.1 Ghost/PiP/Subagents)
            → Phase 10 (Provider & Message Layer Adaptations)
              → Phase 11 (Session & Config Adaptations)
                → Phase 12 (UX & Editor Experience)
                  → Phase 13 (Infrastructure & Sharing)
                    → Phase 14 (TUI ↔ Core Wiring Fixes)
```

**Parallel tracks:**
- Phase 4 (Tool System) runs parallel to Phase 5 (State & Config)
- Phase 8 starts once Phase 6 is 50% complete
- Phase 10 and Phase 11 share no file conflicts and could run in parallel with a 2-person team

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
| `internal/bus/` | Lightweight pub/sub event bus | 13 | ~250 |
| `internal/mcp/` | MCP client support | deferred | ~600 |

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
| 2026-06-01 | Added Phase 10-13 for OpenCode adaptation adoption (items 1-4, 7-16 excluding MCP and Plugin System) |
| 2026-06-01 | Phase 11 planned: 3 plans (Session Forking, Multi-Layer Config, Permission Ruleset Completion) |
| 2026-06-01 | Phase 11 executed and completed — all 3 plans implemented, code review passed with all fixes applied |
| 2026-06-04 | Phase 21 planned: 4 plans (TUI Logical Errors and Connectivity Fixes from rush/tui_logical_errors_and_connectivity_report.md) |

---

## Phase 20 — Internal Wiring and Logic Fixes

**Duration:** 1 week  
**Complexity:** 8/10  
**Status:** In Progress
**Milestone:** All 6 critical wiring and logical flaws from rush/internal_wiring_and_logic_report.md are resolved.
**Source:** rush/internal_wiring_and_logic_report.md
**Depends on:** Phase 19

### Deliverables
- Token Usage Tracking fixed
- TUI State Concurrency Violation resolved
- Ignored Per-Phase Configuration fixed
- TUI/Engine Synchronization Failure resolved
- Resumable Workflow Deadlock fixed
- Redundant Phase Transition Logic removed

---

## Phase 21 — TUI Logical Errors and Connectivity Fixes

**Duration:** 2 weeks  
**Complexity:** 8/10  
**Status:** Planned
**Milestone:** All 11 categories of TUI logical errors and connectivity issues from rush/tui_logical_errors_and_connectivity_report.md are resolved.
**Source:** rush/tui_logical_errors_and_connectivity_report.md
**Depends on:** Phase 19

### Background

The TUI logical errors and connectivity audit (`rush/tui_logical_errors_and_connectivity_report.md`) analyzed ~6,500+ lines of TUI code across 19 source files. It identified 11 categories of issues affecting state synchronization, message flow, component connectivity, error handling, timer/goroutine management, memory management, UI rendering, config hot-reload, input handling, and accessibility.

### Issues Fixed

| Wave | Issues | Plans |
|------|--------|-------|
| 1 (Critical) | State Synchronization, Message Flow, Component Connectivity, Phase Transitions | 21-01 |
| 2 (High) | Error Handling, Timer/Goroutine Management, Memory Management | 21-02 |
| 3 (Medium) | UI Rendering, Config Hot-Reload, Input Handling | 21-03 |
| 4 (Low) | Accessibility and Usability | 21-04 |

### Plans

```
Plans:
- [ ] 21-01-PLAN.md — Critical State & Component Fixes (Wave 1)
- [ ] 21-02-PLAN.md — Error Handling & Resource Management (Wave 2)
- [ ] 21-03-PLAN.md — UI Rendering & Input Handling (Wave 3)
- [ ] 21-04-PLAN.md — Accessibility Improvements (Wave 4)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 21-01 | yes | — |
| 2    | 21-02 | yes | Wave 1 |
| 3    | 21-03 | yes | Wave 2 |
| 4    | 21-04 | yes | Wave 3 |

### Deliverables

- workflowRunning and currentPhase never diverge
- sessionID propagates to all components on change
- model/provider changes update all dependent components
- StreamMsg handled correctly during all phases
- PermissionRequestMsg queued when modal active
- PlanModel always reflects current tasks
- ExecuteModel receives streaming updates
- VerifyModel contains actual verification results
- ShipModel summary has complete data
- Phase transitions clean up properly on error
- StreamErrorMsg properly updates workflow state
- Provider errors pause/resume workflow correctly
- Context exceeded triggers cleanup
- Discuss timeout timer properly stopped
- Health check ticker properly managed
- Stream goroutine lifecycle correct
- Message history bounded
- Thinking blocks cache properly managed
- Tool cards cache properly managed
- Header cache reflects current state
- Sidebar width propagates to message renderer
- Theme changes propagate to all components
- Config reload applies all changes
- Permission config changes take effect
- Slash during streaming handled gracefully
- Rapid key presses queued correctly
- Ctrl+C during permission modal works
- Color contrast meets WCAG standards
- Keyboard navigation discoverable
- Unicode characters render correctly with fallbacks

---

## Phase 22 — Hardcoded Values & Logical Bug Fixes

**Duration:** 2 weeks  
**Complexity:** 7/10  
**Status:** ✓ Complete
**Milestone:** All 36 hardcoded values, 11 logical bugs, and config wiring issues from rush/deep_logical_errors_and_hardcoded_values_report.md are fixed.
**Source:** rush/deep_logical_errors_and_hardcoded_values_report.md
**Depends on:** Phase 14 (TUI ↔ Core Wiring Fixes)

### Background

The deep audit (`rush/deep_logical_errors_and_hardcoded_values_report.md`) identified 36 hardcoded values, 11 logical bugs, 15 TUI screen issues, and 26 config/settings problems. The most critical issues involve hardcoded provider URLs that ignore user configuration, model capability maps that violate the "no hardcoded model lists" architecture rule, and several TUI state management bugs.

### Issues Fixed

| Wave | Issues | Plans |
|------|--------|-------|
| 1 (Critical) | BUG-01 (firstrun URLs), BUG-05 (self-heal), BUG-06 (new session), Config Wiring | 22-01 |
| 2 (High) | BUG-03 (workflowRunning), BUG-04 (theme auto), BUG-02 (bool merge), Model Maps, Health Defaults | 22-02 |
| 3 (Medium) | Tool Constants (18 values), Config Validation (7 fields), Duplicated Constants, WebFetch UA | 22-03 |
| 4 (Low) | TUI Minor Issues, Settings UX, Missing Config Fields | 22-04 |

### Plans

```
Plans:
- [x] 22-01-PLAN.md — Critical Bug Fixes & Config Wiring (Wave 1)
- [x] 22-02-PLAN.md — High Priority Bug Fixes & Model Map Removal (Wave 2)
- [x] 22-03-PLAN.md — Tool Constants & Config Validation (Wave 3)
- [x] 22-04-PLAN.md — Low Priority Polish & Config Fields (Wave 4)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 22-01 | yes | — |
| 2    | 22-02 | yes | Wave 1 |
| 3    | 22-03 | yes | Wave 2 |
| 4    | 22-04 | yes | Wave 3 |

### Deliverables

- First-run validation uses configurable base URLs (BUG-01)
- Self-heal confirmation actually triggers healing (BUG-05)
- New session creates fresh session, not first-run wizard (BUG-06)
- All 8 config fields wired to provider Options structs
- Ship phase included in workflowRunning check (BUG-03)
- Theme "auto" handled in runtime switch (BUG-04)
- Config bool merge preserves explicit false values (BUG-02)
- No hardcoded model capability maps in providers
- Health check defaults consistent between config and providers
- All 18 tool magic numbers extracted to named constants
- Config validation for all numeric fields
- WebFetch User-Agent uses Version variable
- No duplicate constants across packages
- Tab-completion standardized
- Settings unsaved changes warning
- Missing config fields added for tool limits

---

## Phase 23 — Hardcoded Values & Function Simplification

**Duration:** 2 weeks  
**Complexity:** 7/10  
**Status:** Planned
**Milestone:** All 94 findings from rush/hardcoded_values_and_simplification_report.md are fixed: duplicated functions extracted, constants centralized, config fields added, theme colors consolidated.
**Source:** rush/hardcoded_values_and_simplification_report.md
**Depends on:** Phase 22

### Background

The comprehensive codebase audit (`rush/hardcoded_values_and_simplification_report.md`) identified 94 findings across 5 categories: 12 duplicated functions between OpenRouter and Zen providers, 38 hardcoded values that should be user-configurable, 28 magic numbers missing named constants, 10 theme/color duplications, and 6 config system gaps. Two findings are bugs: Zen's unbounded body read (OOM risk) and missing ResponseHeaderTimeout.

### Issues Fixed

| Wave | Issues | Plans |
|------|--------|-------|
| 1 (Critical/High) | Provider duplication (S-1–S-8), Bugs (C-16, C-17), Health strings (S-12) | 23-01 |
| 2 (Medium-High) | Theme consolidation (S-9, T-1–T-3), Constants (S-10, S-11, C-18, C-30, C-31, C-35) | 23-02 |
| 3 (Medium) | Config fields for 30+ hardcoded values (C-3–C-14, C-19–C-29, C-32–C-34, C-36–C-37), Env vars (G-1), DefaultConfig (G-2) | 23-03 |
| 4 (Low) | Bool merge bug (G-4), WebFetch UA (C-32), Full verification of all 94 findings | 23-04 |

### Plans

```
Plans:
- [ ] 23-01-PLAN.md — Provider Function Extraction & Bug Fixes (Wave 1)
- [ ] 23-02-PLAN.md — Theme Consolidation & Constants Centralization (Wave 2)
- [ ] 23-03-PLAN.md — Config Fields for Hardcoded Values (Wave 3)
- [ ] 23-04-PLAN.md — Remaining Fixes, Config Bool Merge Bug & Verification (Wave 4)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 23-01 | yes | — |
| 2    | 23-02 | yes | Wave 1 |
| 3    | 23-03 | yes | Wave 1 |
| 4    | 23-04 | yes | Waves 1–3 |

### Deliverables

- `internal/provider/common.go` with 8 shared functions extracted from both clients
- `internal/provider/capabilities.go` with `ParseModelCapabilities`
- Zen body read bounded with `io.LimitReader` (C-16 bug fix)
- Zen HTTP transport has `ResponseHeaderTimeout` (C-17 bug fix)
- Theme struct has `BadgeForeground`, `BadgeTextLight`, `BadgeTextDark` fields
- Zero `#000000` or `#FFFFFF` hardcoded in component files
- All 12 duplicated functions eliminated
- 30+ named constants in `types/constants.go`
- 15+ new config fields with sensible defaults
- 4+ env var overrides added
- Config bool merge bug fixed
- All 94 findings verified as FIXED or DEFERRED

---

## Phase 24 — TUI Redesign

**Duration:** 3 weeks
**Complexity:** 8/10
**Status:** Planned
**Milestone:** Complete visual and interaction redesign of all M31A TUI screens based on the TUI Redesign Proposal
**Source:** rush/tui_redesign_proposal.md
**Depends on:** Phase 23

### Background

The TUI Redesign Proposal (`rush/tui_redesign_proposal.md`) analyzed ~350KB of Go source across 70+ TUI files and proposes a complete visual overhaul of all 10 existing screens plus 6 new proposed screens. The redesign maintains all architectural constraints: Bubble Tea single-threaded model, lipgloss styling, no CSS animations, charmbracelet component ecosystem, CGO_ENABLED=0.

### Requirement IDs

| ID | Description |
|----|-------------|
| TUI-01 | Theme Enhancement — new color tokens, border/drawing primitives, sparkline/starfield components |
| TUI-02 | Shared Chrome — Header (block anchors, phase breadcrumb), StatusBar (multi-segment), Sidebar (configurable width), PermissionModal (countdown bar) |
| TUI-03 | REPL Redesign — Mission Control with role gutters, timestamp bars, double-border tool cards, git status strip |
| TUI-04 | FirstRun Redesign — Launchpad with galaxy metaphor, 2x2 feature cards, provider constellation picker |
| TUI-05 | Plan Screen Redesign — Blueprint with Kanban layout, file impact, dependency graph |
| TUI-06 | Execute + Verify Redesign — Mission Live with live metrics, QA Gate with per-task result panels |
| TUI-07 | Ship + Diff Redesign — Launch Pad with commit review, enhanced diff with syntax highlighting |
| TUI-08 | ModelSelector + Settings + Resume — Observatory with sparklines, Control Tower with icon tabs, Vault with timeline view |

### Plans

```
Plans:
- [ ] 24-01-PLAN.md — Theme Enhancement + Shared Chrome (Wave 1)
- [ ] 24-02-PLAN.md — REPL + FirstRun Redesign (Wave 2)
- [ ] 24-03-PLAN.md — Plan + Execute + Verify Redesign (Wave 3)
- [ ] 24-04-PLAN.md — Ship + Diff Redesign (Wave 3)
- [ ] 24-05-PLAN.md — ModelSelector + Settings + Resume Redesign (Wave 4)
```

### Wave Structure

| Wave | Plans | Autonomous | Depends on |
|------|-------|------------|------------|
| 1    | 24-01 | yes | — |
| 2    | 24-02 | yes | Wave 1 |
| 3    | 24-03, 24-04 | yes, yes | Wave 1 |
| 4    | 24-05 | yes | Wave 1 |

### Deliverables

- Theme struct has new style fields: CardBorder, PhaseActive, PhasePast, PhaseFuture, TimelineDate, MetricValue, MetricLabel
- Dark() and Light() use new color palette: Brand=#7C3AED, Accent=#06B6D4, Success=#10B981, Warning=#F59E0B, Error=#EF4444, Surface=#1E1E2E
- Sparkline component renders braille/block character visualizations
- Starfield component renders deterministic scattered dots
- Header renders with block-character anchors and phase breadcrumb
- StatusBar adapts to idle/streaming/workflow states
- Sidebar width configurable (default 120)
- PermissionModal has double-border, tool card, and countdown bar
- REPL renders with role gutters, timestamp bars, double-border tool cards
- FirstRun shows galaxy starfield, 2x2 feature cards, provider constellation picker
- Plan screen shows Blueprint layout with detail box, file impact, dependency graph
- Execute screen shows live metrics, progress bar, running task panel
- Verify screen shows summary bar, per-task result panels, self-heal overlay
- Ship screen shows commit review, diff summary, ship action card
- Diff screen renders with syntax highlighting and line numbers
- ModelSelector shows sparklines, capability badges, detail pane with cost bars
- Settings shows icon tabs, two-column layout, description pane
- Resume shows timeline view, session cards with phase badges, preview pane
- All screens render gracefully at 80, 120, 160, 200 cols
- `go test -race ./internal/tui/...` passes
- `go vet ./internal/tui/...` exits 0
