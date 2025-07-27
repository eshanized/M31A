---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: Release
status: Phase 19 in progress
last_updated: "2026-06-04T03:15:00.000Z"
progress:
  total_phases: 19
  completed_phases: 14
  total_plans: 80
  completed_plans: 70
  percent: 88
---

# M31A — Current State

## Active Phase

Phase 19 — Comprehensive Codebase Concerns Fixes (4 plans, 2 waves)

## Status

Phase 17 (Post-Phase-16 Audit Fixes) is complete. 4 plans verified/committed: 17-01 (Critical build/test — all pre-existing), 17-02 (High severity — 8 commits), 17-03 (Medium severity — 9 commits), 17-04 (Low severity — 5 commits + 3 pre-existing). 36 issues from comprehensive codebase audit addressed.

## Last Session

- **2026-06-03** — Phase 17 executed. All 36 audit findings resolved across 4 plans. go.mod bumped to 1.24, atomic.Bool for cache refresh, crypto/rand temp files, Zen tests fixed, isContextExceeded patterns verified, Grep truncation fixed, WebFetch shared client, SSE whitespace trim, Glob sorted, Engine error handling, async SetProvider, Edit fsync, SSRF DNS pinning, config merge, permission timeouts, API key preservation, HTML strip optimization, relative gitignore paths, secure file permissions.

## Completed Phases

- Phase 0 — Foundation & Documentation
- Phase 1 — Provider Abstraction Layer
- Phase 2 — TUI Foundation
- Phase 3 — Message Rendering Pipeline
- Phase 4 — Tool System
- Phase 5 — Session State & Configuration
- Phase 6 — Workflow Engine
- Phase 7 — Signature Features
- Phase 10 — Provider & Message Layer Adaptations
- Phase 11 — Session & Config Adaptations
- Phase 12 — UX & Editor Experience Adaptations
- Phase 16 — UX Polish
- Phase 17 — Post-Phase-16 Audit Fixes
- Phase 18 — Welcome Page Rebuild

## In Progress

- Phase 19 — Comprehensive Codebase Concerns Fixes (2/4 plans complete)
  - 19-01: ✅ Nil-Safety & Concurrency Fixes
  - 19-02: ✅ Provider Robustness & Config Fixes
  - 19-03: ✅ Post-Phase-18 Codebase Concerns Fixes
  - 19-04: ✅ Session Cleanup, Config Hot-Reload & Test Coverage
  - 19-05: ⏳ (pending)
  - 19-06: ⏳ (pending)
  - 19-07: ⏳ (pending)
  - 19-08: ⏳ (pending)

## Phase 14 Plans

- 14-01: Discuss Phase Q&A Wiring (Wave 1, CRITICAL — D-01)
- 14-02: Workflow Screen Wiring (Wave 1, HIGH — D-02/D-03/D-05)
- 14-03: msgChan Drainer Synchronization (Wave 1, HIGH — D-04)
- 14-04: Workflow State Persistence (Wave 2, MEDIUM — D-06)
- 14-05: Discuss Streaming + Low Severity (Wave 2, MEDIUM/LOW — D-07/D-08/D-09/D-10)
- 14-06: AppState Refactor (Wave 3, MEDIUM — D-11, drafted, optional/stretch)

## Completed Plans (Phase 10)

- 10-01: Structured Tool Calls from SSE
- 10-02: Undo/Redo Session Management
- 10-03: Auto Context Compaction

## Completed Plans (Phase 11)

- 11-01: Session Forking — ParentID/ChildrenIDs, ForkSession, /fork /prev /next commands
- 11-02: Multi-Layer Configuration — project-level m31a.toml, validation, var substitution
- 11-03: Permission Ruleset Completion — glob matching, per-agent profiles

## Completed Plans (Phase 12)

- 12-01: Model Variants & Favorites System
- 12-02: Shell Mode — ! prefix REPL command execution
- 12-03: Prompt History with Frecency Ranking
- 12-04: Diff Viewer Screen — lipgloss-styled git diff
- 12-05: Editor Context Auto-Include — @filepath syntax

## Key Decisions Made

### Phase 17 Decisions (17-01 through 17-04)

- **17-01:** All 4 Critical issues (C-1 through C-4) were pre-existing fixes from earlier phases. No new code changes needed — verified go.mod Go 1.24, atomic.Bool cache refresh, crypto/rand temp files, Zen test patterns.
- **17-02 (H-1):** isContextExceeded already had "context window exceeded" pattern; added test coverage to confirm.
- **17-02 (H-2/H-3):** Grep truncation simplified to generic "[... more matches (limit: N)]" — counting exact remaining matches requires full scan.
- **17-02 (H-4):** WebFetch shares a single http.Client across calls for connection reuse; per-request timeout via context.WithTimeout.
- **17-02 (H-5):** SSEParser uses strings.TrimSpace before [DONE] comparison.
- **17-02 (H-7):** consumeStream checks errors before appending delta to preserve partial content on non-EOF errors.
- **17-02 (H-8):** verifyTask uses exec.CommandContext with 5-minute timeout for go test commands.
- **17-02 (H-9):** SetProvider returns tea.Cmd for async model validation — prevents TUI event loop blocking.
- **17-03 (M-4):** WebFetch SSRF DNS pinning resolves once, checks IP, connects with pinned IP to prevent TOCTOU races.
- **17-03 (M-10):** Config.Save copies struct before clearing API keys to prevent receiver mutation.
- **17-04 (L-1):** HTML stripping uses single-pass scanner instead of O(n²) nested loops.
- **17-04 (L-5):** config.atomicWrite uses os.OpenFile with 0600 permissions for security-sensitive config files.
- **17-04 (L-6):** Registry.SetActive returns ErrProviderNotFound sentinel error for unregistered providers.

### Phase 14 Decisions (14-01 through 14-04)

- **14-04 (D-06):** Workflow state (goal, phase, discuss questions) persists to `session.json` on every phase transition via `UpdateWorkflowState`. Resume is MANUAL (`/workflow resume` slash command) not AUTO — avoids surprising the user with mid-workflow jumps. AppState shows a 10-second resume toast on startup when a non-idle workflow is detected.
- **14-04:** Toast uses existing `toastText`/`toastType`/`toastExpires` fields (not a `toasts` slice) and string `Type: "info"` (no `ToastInfo` constant). Follows convention from `cycleRecentModel` and `renderToast`.
- **14-04:** `Session` struct gained `WorkflowGoal string` and `DiscussQuestions []string` fields (both `omitempty` for clean JSON). `DiscussQuestions` initialized to an empty slice (not nil) in `NewSession`.
- **14-04:** `LoadWorkflowState` returns zero values (not error) for `ErrSessionCorrupted` (missing session) — matches the "session doesn't exist" handling contract.
- **14-04:** `saveSessionAtomic` is a partial save (only `session.json`, not `messages.json`) — splits partial save for `UpdateWorkflowState` from full save in `SaveSession` to avoid clobbering in-flight message updates from the REPL.
- **14-04:** `AppState` gained a `sessionID string` field set by `initWorkflowEngine` (the plan's code referenced `m.sessionID` throughout — field had to exist).
- **14-04:** `/workflow resume` is intercepted in the command registry's `handleWorkflow` (not the `/workflow <goal>` prefix match in app.go). The prefix match was modified to skip when goal is `resume`.
- **14-04:** PhaseShip case calls `UpdateWorkflowState` with empty values (post-Ship reset) after persisting the Ship phase, then clears in-memory `workflowGoal`/`currentPhase` — both persist AND in-memory reset.
- **14-03 (D-04):** msgChan drainer pattern: `AppState` owns a `drainer` goroutine that reads `engine.msgChan` and dispatches `tea.Msg` via the program. All message-emitting goroutines go through the same channel, ensuring Bubble Tea's single-thread invariant.
- **14-02 (D-02/D-03/D-05):** Workflow screens (Plan, Execute, Verify, Ship) wired to PhaseResultMsg transitions. Status indicators: `[x]` done, `[>]` running, `[ ]` queued, `[ ]` blocked.
- **14-01 (D-01):** WorkflowEngine interface introduced in tui package for mock injection. Discuss phase Q&A flow uses tea.Batch for question + 5-minute timer emission per question.

### Phase 12 Decisions

- ModelInfo.Variant is `*string` — nil by default, values: "thinking", "fast", "extended", "vision"
- RecentModelsData stored at `~/.m31a/recent_models.json` with atomic writes; max 10 recent entries
- Ctrl+M/Ctrl+Shift+M bindings registered in CtxREPL (not global) — global fallback keeps ctrl+x m chord
- Shell mode (!) bypasses LLM entirely — commands execute via dispatcher without permission modal (PermissionRule globs still apply)
- Shell results display as assistant messages but don't count as conversation turns for LLM context
- FrecentHistory: score = frequency / (hours_since_last_use + 1); max 1000 entries with frecency-based eviction on save
- Arrow-up frecency navigation activates when textarea has content; falls back to in-memory session history when empty
- DiffModel uses lipgloss styling: green (#81C995) for additions, red (#F28B82) for deletions, brand (#D77757) for hunk headers
- /diff routes to full-screen diff viewer by default; --stat outputs inline; <commit> accepts any git ref
- @filepath regex requires a path separator (`/`) — prevents matching @user, @mention, email addresses
- File expansion limited to 100KB; binary detection via 512-byte mime sniff

### Phase 11 Decisions

- SiblingSessions includes the current session in its return list for consistent index-based prev/next navigation
- ForkSession uses retry loop (max 10) for ID collision avoidance
- Corrupt children are silently skipped in ListChildren for resilience
- Session switching clears REPL messages and reloads from the loaded session
- Deep copy on fork: parent messages and project state are copied via marshal/unmarshal to prevent aliasing bugs

### Phase 7 Decisions

- Slash commands implemented as pure functions returning CommandResult (not direct AppState mutation)
- CommandContext carries Registry, SessionManager, Git, Ledger, Rollback, AutoDream references
- AutoDream preserves: system messages, last 5 messages, tool call messages, first message (goal)
- Summary prefix: [AutoDream Context Summary], token estimation: words × 1.3
- LEDGER.md uses Markdown table format with 9 columns (user-approved deviation from original spec)
- Model selector uses bubbles/list with textinput for search; P cycles provider filter
- Settings uses *config.Config + path string for save (not config.Loader — no such type exists)
- Fallback banner dismisses on 'x' key, auto-dismiss after 15s, or on user input
- Model cost display uses standard usage estimate (100K in + 50K out, not raw per-M rates)
- Model item FilterValue includes name, ID, description, provider
- Settings tabs: General, Provider, Model, Permissions, Features, Ledger (6 total)

### Earlier Phases (preserved)

- Sequential task execution in V1 (no concurrency)
- Only OpenRouter and Zen as LLM gateways
- CGO_ENABLED=0 static binary
- API keys always masked as "••••••••" in settings View(), never displayed in plaintext
- SSE parser uses line-by-line bufio.Scanner (not custom split on \n\n)

## Blockers

None

## Phase 15 Plans

- 15-01: ✅ Nil-Safety Guards (C-1 — CRITICAL)
- 15-02: ✅ Stream Pipeline Channel Ownership Refactor (C-3, H-9, H-14, M-21)
- 15-03: ⏳ (pending)
- 16-02: ✅ Error UX Fixes (15 tasks)
- 16-03: ✅ Feedback & Loading (Tasks 11-12, timeout warnings)
- 16-04: ✅ Theme & Visual Fixes (theme injection, color fixes)
- 16-05: ✅ Navigation & Help (command palette Esc, categorized /help)
- 16-06: ✅ Screen-Specific (ship/verify improvements, first-run fixes)
- 16-07: ✅ Tool & Provider (tool card improvements, permission syntax)
- 16-08: ✅ Config & Polish (config validation, autodream messages)

## Key Decisions Made

### Phase 15 Decisions (15-01)

- **15-01 (C-1):** Guard route chosen over lazy-init — minimal surface change, 3 one-line `if m.replModel == nil { return nil, nil }` additions. Regression test exercises 13 representative tea.Msg types with nil replModel.

### Phase 15 Decisions (15-02)

- **15-02 (C-3/H-9/H-14/M-21):** Kept streamCh as a return value from StartStreamCmd for the continuation pattern (BT only calls a cmd once, so the REPL needs the channel reference). The goroutine owns the channel; the REPL stores a read-only reference. safeCloseOnce uses sync.Map of sync.Once per channel-pointer. Eliminated separate streamDone channel entirely.

## Last Session

- **2026-06-03** — Phase 17 executed. 4 plans across 4 waves: Critical build/test (17-01, pre-existing), High severity (17-02, 8 commits), Medium severity (17-03, 9 commits), Low severity (17-04, 5 commits + 3 pre-existing). 36 audit findings from comprehensive codebase audit resolved. All tests pass, build clean.

### Phase 16 Decisions (16-01)

- **16-01:** `/clear` uses callback pattern (`ClearMessages func()` on `CommandContext`) — clean separation of concerns without direct model reference.
- **16-01:** Phase aliases (`/plan`, `/execute`, etc.) route through `handlePhase` with phase name detection — reuses existing infrastructure instead of duplicating.
- **16-01:** `--confirm` flag required for `/reset` and `/rollback --hard` — prevents accidental data loss.
- **16-01:** RiskDangerous uses `theme.Warning` (yellow), RiskDestructive uses `theme.Error` (red) — clear visual hierarchy.
- **16-01:** Permission modal exit hint uses `Faint(true)` for subordinate appearance — reduces user anxiety.
- **16-01:** Execute screen completion summary shows ✓ (green) or ⚠ (yellow) based on actual task status.

## Phase 17 Plans

- 17-01: ✅ Critical Build & Test Fixes (C-1 through C-4, all pre-existing)
- 17-02: ✅ High Severity Correctness Fixes (H-1 through H-9)
- 17-03: ✅ Medium Severity Fixes (M-1 through M-11)
- 17-04: ✅ Low Severity Polish (L-1 through L-11)
