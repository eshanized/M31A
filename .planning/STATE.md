---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: Release
status: Phase 14 — Plan 04 complete
last_updated: "2026-06-02T05:00:00.000Z"
progress:
  total_phases: 10
  completed_phases: 6
  total_plans: 48
  completed_plans: 39
  percent: 61
---

# M31A — Current State

## Active Phase

Phase 14 — TUI ↔ Core Wiring Fixes (Plan 14-04 complete, 14-05/06 pending)

## Status

Phase 14 — fixing 11 wiring issues identified by the TUI ↔ Core
wiring audit (`rush/tui_core_wiring_report.md`): 1 critical, 3 high, 4 medium,
3 low. Plans 14-01, 14-02, 14-03, and 14-04 are complete; 14-05 and 14-06 remain.

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

## In Progress

- Phase 14 — TUI ↔ Core Wiring Fixes (4/6 plans complete)
  - 14-01: ✅ Discuss Phase Q&A Wiring
  - 14-02: ✅ Workflow Screen Wiring
  - 14-03: ✅ msgChan Drainer Synchronization
  - 14-04: ✅ Workflow State Persistence (D-06)
  - 14-05: ⏳ Discuss Streaming + Low Severity (D-07/D-08/D-09/D-10)
  - 14-06: ⏳ AppState Refactor (D-11, drafted, optional/stretch)

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

## Last Session

- **2026-06-02** — Phase 14 Plan 14-04 (Workflow State Persistence) executed atomically. 5 tasks, 5 commits (`d1971f3`, `e95f599`, `5e4906e`, `940a123`, `c1f1f84`). All tests pass (`go test -count=1 -race ./...`). 14-05 next.
