# Phase 14: TUI ↔ Core Wiring Fixes — Context

**Gathered:** 2026-06-02
**Status:** Ready for planning
**Source:** `rush/tui_core_wiring_report.md` (11 wiring issues, D-01 through D-11)

<domain>
## Phase Boundary

Phase 14 resolves the 11 wiring issues identified by the TUI ↔ Core wiring audit
(`rush/tui_core_wiring_report.md`). The audit followed a previous walkthrough
(`walkthrough_wiring_0_8.md`) that fixed 13 issues (W-01 through W-13) but missed
workflow-phase wiring — specifically the Discuss/Plan/Execute/Verify phase
transitions and the `msgChan` synchronization that backstops them.

**Depends on:** Phase 13 (Infrastructure & Sharing Adaptations)
**Duration:** ~3 weeks | **Complexity:** 8/10 | **Milestone:** Multi-phase
workflow `Initialize → Discuss → Plan → Execute → Verify → Ship` runs end-to-end
without dead-ends, screen flashes, or message drops.

### Issues in scope

| ID | Severity | Component | Type |
|----|----------|-----------|------|
| D-01 | CRITICAL | Discuss phase Q&A | `dead_end` |
| D-02 | HIGH | Plan screen | `unreachable_screen` |
| D-03 | HIGH | PlanModel dimensions | `missing_init` |
| D-04 | HIGH | msgChan race | `data_race` |
| D-05 | MEDIUM | Execute/Verify flash | `unreachable_screen` |
| D-06 | MEDIUM | Workflow state non-persistent | `missing_persistence` |
| D-07 | MEDIUM | Discuss not streamed | `inconsistent_patterns` |
| D-08 | LOW | PlanModel field access | `tight_coupling` |
| D-09 | LOW | Sidebar threshold | `magic_number` |
| D-10 | LOW | streamCh not closed | `resource_leak` |
| D-11 | MEDIUM | AppState god object | `architectural_smell` |

### Out of scope (deferred or out-of-band)

- **Shell mode permission bypass** (audit §6.1, loophole report H1) — already
  documented as intentional at `repl.go:1003-1004`. The `!cmd` shell mode
  bypasses the permission modal by design; `PermissionRule` deny rules still
  apply. This is not a Phase 14 fix.
- **Vision / multi-modal support** — explicitly out of M31A V1 scope.
- **TUI <-> provider API changes** — provider layer is correct; only consumer
  wiring is in scope.
</domain>

<decisions>
## Implementation Decisions

### Discuss phase Q&A (D-01) — CRITICAL

- The Discuss phase returns `PhaseResult{NeedsAnswers: true}` with the parsed
  questions in `e.discussState.Questions` and the assistant content in
  `Messages[]`. The engine primitives `SubmitDiscussAnswer(index, answer)`,
  `SkipDiscuss()`, and `FinalizeDiscuss()` already exist (`engine.go:231-265`)
  but the TUI never calls them.
- The TUI must collect answers for each question by emitting
  `QuestionRequestMsg` (which is the same flow used by the `AskUserQuestion`
  tool — `app.go:971-985`). The existing `questionListenerCmd` mechanism
  already wires the request/response channels to `m.dispatcher`.
- The Discuss phase will use the **inline REPL question display** (via
  `ReplModel.ShowQuestion`, `repl.go:1422`) rather than a dedicated screen,
  to keep the user in the same context as their original goal. This matches
  the existing `AskUserQuestion` tool UX.
- New state on `AppState`:
  ```go
  pendingDiscussAnswers map[int]string  // index -> answer
  currentDiscussIndex   int              // next question to ask
  ```
- Answer timeout: 5 minutes. On timeout, `engine.SkipDiscuss()` is called
  (which fills remaining answers with `""` and finalizes). Implemented via a
  `tea.Cmd` that returns a `time.AfterFunc(5*time.Minute, ...)` and a
  `DiscussAnswerTimeoutMsg` that triggers the skip path.
- The Discuss flow is sequential: one question at a time, advancing
  `currentDiscussIndex` on each `QuestionResponseMsg`. After all answers
  collected (or timeout), call `engine.FinalizeDiscuss()` then
  `RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)`.

### Plan screen reachability (D-02, D-03) — HIGH

- In `app.go:1059-1072` (`case types.PhasePlan`), set `m.screen = ScreenPlan`
  after creating `m.planModel`. **Stop the auto-advance to Execute** — the
  plan must require the user to press `a`/`A` to accept (the
  `PlanModel.Update()` already emits `AppMsg{Screen: ScreenExecute}` on
  accept — `plan.go:57-58`).
- New handler for `AppMsg{Screen: ScreenExecute}` from plan acceptance:
  call `RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)`.
- The `NewPlanModel`, `NewExecuteModel`, `NewVerifyModel`, `NewShipModel`,
  and `NewDiffModel` constructors all take `width, height int` parameters
  starting in Phase 14. The TUI passes `m.width, m.height` at creation.
  Internally, these models also continue to handle `WindowSizeMsg` for
  subsequent resizes, but the initial size is now non-zero.
- Internal change to `plan.go:74-110` (`PlanModel.View()`): the
  `"Loading plan..."` placeholder is only shown when `m.width == 0` and
  `m.tasks == nil`. With Phase 14 changes, `m.width` is set at construction
  so the placeholder is only shown for empty task lists.

### Execute/Verify screen flash (D-05) — MEDIUM

- After `PhaseExecute` completes, the auto-advance to `PhaseVerify` is
  suspended. The `ExecuteModel.Update()` already checks `allDone` and emits
  `AppMsg{Screen: ScreenVerify}` (`execute.go:73-75`). The TUI handler for
  `AppMsg{Screen: ScreenVerify}` from execute calls
  `RunPhaseCmd(m, types.PhaseVerify, m.workflowGoal)`.
- Same pattern for Verify → Ship: stop auto-advance from `PhaseVerify` to
  `PhaseShip`; require user to confirm via `VerifyModel` or
  `ShipModel.Update()`.

### msgChan drainer synchronization (D-04) — HIGH

- Replace the `app.msgChan` field with a per-phase `done chan struct{}`
  channel created in `RunPhaseCmd`. The drainer selects on `app.msgCh`,
  `app.msgDone`, and a 100ms poll timer.
- The `runner` goroutine in `RunPhaseCmd` calls `close(msgDone)` after
  emitting the final `PhaseResultMsg`. The `Update` handler does **not**
  close `msgDone` — only the runner does.
- New `phaseGen` counter on `AppState`: incremented on every `RunPhaseCmd`
  call. The drainer captures the gen at spawn time; if `app.phaseGen`
  changes (a new phase started), the drainer returns `nil` and stops.
- `eng.SetMsgEmitter(&channelEmitter{ch: msgCh})` is called once at engine
  construction; the emitter is replaced per-phase to use the new channel.

### Workflow state persistence (D-06) — MEDIUM

- New fields on `pkg/session.Session`:
  ```go
  WorkflowGoal     string        `json:"workflow_goal,omitempty"`
  WorkflowPhase    WorkflowPhase `json:"workflow_phase,omitempty"`
  DiscussQuestions []string      `json:"discuss_questions,omitempty"`
  ```
- In the `PhaseResultMsg` handler, after `m.currentPhase` is updated, call
  `m.sessionManager.UpdateWorkflowState(m.sessionID, m.workflowGoal, m.currentPhase, m.discussQuestions)`.
- In `NewApp` (`app.go:225`), after `initWorkflowEngine()`, check
  `m.sessionManager.LoadWorkflowState(m.sessionID)`. If the loaded phase is
  not `idle` and not `complete`, emit a toast: `"Resuming workflow at
  {phase}..."` and the user can invoke `/workflow resume` to restart from
  the saved phase (manual restart, not auto-resume to avoid surprising the
  user with mid-workflow jumps).

### Discuss phase streaming (D-07) — MEDIUM

- `runDiscuss` in `workflow/discuss.go:13-57` currently uses `e.streamLLM`
  synchronously — the user sees no progress. Refactor to write tokens to
  the engine's `MsgEmitter` (already exists) and have `streamLLM` return a
  `*StreamIterator` instead of a final string.
- The TUI's REPL (or a new dedicated discuss screen) renders the streamed
  content token-by-token, matching the main REPL streaming UX.
- Decision: do **not** introduce a new `ScreenDiscuss`; reuse the existing
  REPL streaming. The Discuss phase already routes the user back to REPL
  in the current code (`app.go:1052`); the only change is to stream the
  LLM response into the REPL instead of waiting for the final result.

### ModelSelector field access (D-08) — LOW

- `m.modelSelector.registry` and `m.modelSelector.theme` accessed directly
  at `app.go:1195-1199`. Add setter methods:
  `ModelSelector.SetRegistry(registry)`, `ModelSelector.SetTheme(theme)`.
  Replace direct field writes with these calls.
- The `ModelSelector` type remains a value (not pointer) — setters mutate
  through the value's address.

### Sidebar threshold (D-09) — LOW

- Move the `120` magic number to `internal/config/types.go`:
  ```go
  type UIConfig struct {
      // ... existing fields ...
      SidebarWidthThreshold int `toml:"sidebar_width_threshold"` // default 120
  }
  ```
- `app.go:1171` reads `m.cfg.UI.SidebarWidthThreshold` instead of `120`.
- Default value in `DefaultConfig()`: `120`.

### streamCh close (D-10) — LOW

- In `internal/tui/streaming.go`, `StartStreamCmd` already closes
  `streamDone` via `defer`. Add `defer close(streamCh)` so the data channel
  is also closed when the stream ends (success or error).
- The REPL continues to read from `streamCh` until close; this prevents GC
  delays and signals to any consumer that no more data is coming.

### AppState refactor (D-11) — MEDIUM

- The 1694-line `app.go` with 35+ fields and a 950-line `Update()` is
  unsupportable. Refactor into three coordinator types in new files:
  - `internal/tui/workflow_coordinator.go` — owns `workflowEngine`,
    `currentPhase`, `workflowGoal`, `planModel`, `executeModel`,
    `verifyModel`, `shipModel`, `msgChan`, `msgDone`, `phaseGen`, and the
    `PhaseResultMsg` / `PlanReadyMsg` / `TaskStartMsg` / `TaskUpdateMsg`
    handlers.
  - `internal/tui/screen_router.go` — owns `screen`, `prevScreen`,
    `discussQuestions`, `pendingDiscussAnswers`, and screen-transition
    dispatch.
  - `internal/tui/stream_coordinator.go` — owns `replModel` streaming
    state and the `StreamMsg` / `StreamDoneMsg` / `StreamErrorMsg` handlers.
- `AppState` retains the cross-cutting state (config, keyRegistry,
  cmdRegistry, themeManager, dispatcher, sidebar, cmdPalette) and embeds
  the three coordinators. The `Update()` method becomes a thin router:
  delegate `PhaseResultMsg`-class messages to `workflowCoord`,
  `StreamMsg`-class to `streamCoord`, screen-specific keys to
  `screenRouter`.
- Refactor must be **behavior-preserving** — all existing tests pass
  without modification. New behavior (D-01 through D-07) is added in
  follow-up plans on top of the refactored structure.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Files to read (current state)

- `internal/tui/app.go` — `AppState` struct (lines 38-95), `NewApp()`
  (lines 175-330), `RunPhaseCmd` (lines 325-385), `workflowMsgDrainer`
  (lines 387-402), `PhaseResultMsg` handler (lines 1022-1116),
  `QuestionRequestMsg`/`QuestionResponseMsg` handlers (lines 971-985),
  `PlanReadyMsg` handler (lines 987-999).
- `internal/workflow/discuss.go` — `runDiscuss` (lines 13-57),
  `buildDiscussContext` (lines 60+).
- `internal/workflow/engine.go` — `DiscussState` struct (lines 225-229),
  `SubmitDiscussAnswer` (lines 232-244), `SkipDiscuss` (lines 247-260),
  `FinalizeDiscuss` (lines 262+).
- `internal/tui/plan.go` — `NewPlanModel` (lines 24-50), `PlanModel.Update`
  (lines 40-72), `PlanModel.View` (lines 74-110).
- `internal/tui/execute.go` — `NewExecuteModel`, `ExecuteModel.Update`,
  `ExecuteModel.allDone` flag.
- `internal/tui/streaming.go` — `StartStreamCmd` (lines 36-150).
- `pkg/session/manager.go` — `SaveSession`, `LoadSession` (existing
  patterns to extend for `UpdateWorkflowState`).
- `internal/tui/repl.go` — `ReplModel.ShowQuestion` (lines 1422-1465),
  the inline question rendering pattern.

### Files to create/modify

- `internal/tui/app.go` — `AppState` struct (split into coordinators),
  `NewApp()` (delegate to coordinators), `Update()` (thin router).
- `internal/tui/workflow_coordinator.go` — NEW. Workflow state + handlers.
- `internal/tui/screen_router.go` — NEW. Screen routing.
- `internal/tui/stream_coordinator.go` — NEW. Streaming state + handlers.
- `internal/tui/plan.go` — `NewPlanModel` signature change (add
  `width, height int`).
- `internal/tui/execute.go` — `NewExecuteModel` signature change.
- `internal/tui/verify.go` — `NewVerifyModel` signature change (if exists).
- `internal/tui/ship.go` — `NewShipModel` signature change.
- `internal/tui/diff.go` — `NewDiffModel` signature change.
- `internal/tui/streaming.go` — `defer close(streamCh)`.
- `internal/tui/modelselector.go` — `SetRegistry`, `SetTheme` setters.
- `internal/config/types.go` — `UIConfig.SidebarWidthThreshold`.
- `internal/config/loader.go` — `DefaultConfig` adds
  `SidebarWidthThreshold: 120`.
- `internal/workflow/discuss.go` — `runDiscuss` streams via
  `MsgEmitter` rather than `streamLLM` blocking.
- `pkg/session/manager.go` — `UpdateWorkflowState`,
  `LoadWorkflowState` methods.
- `pkg/session/session.go` (or equivalent) — `Session` struct gets
  `WorkflowGoal`, `WorkflowPhase`, `DiscussQuestions` fields.
- `internal/tui/app_test.go` — update for new constructors (signature
  changes), add new tests for Discuss Q&A flow, Plan acceptance flow,
  drainer sync, and persistence.

### Existing Code to Study (DO NOT modify)

- `internal/tools/question.go` — `AskUserQuestion` tool. The
  `QuestionRequestMsg` / `QuestionResponseMsg` cycle is already correctly
  wired for this tool (lines 49+). Phase 14 reuses the **same** cycle for
  the Discuss phase — no changes to `question.go`.
- `internal/tui/keybindings.go` — `KeyActionMsg` is the existing
  abstraction for high-level user actions; the discuss timeout uses a
  new `DiscussAnswerTimeoutMsg` instead, not a `KeyActionMsg`.
</canonical_refs>

<specifics>
## Wave Structure

**Wave 1** (critical and high priority, no inter-dependencies — fully parallel):
- Plan 01: Discuss Phase Q&A Wiring (D-01)
- Plan 02: Workflow Screen Wiring (D-02, D-03, D-05)
- Plan 03: msgChan Drainer Synchronization (D-04)

**Wave 2** (depends on Wave 1 to verify the workflow loop is functional):
- Plan 04: Workflow State Persistence (D-06)
- Plan 05: Discuss Streaming + Low Severity Fixes (D-07, D-08, D-09, D-10)

**Wave 3** (refactor, must run after all D-01 through D-10 are done):
- Plan 06: AppState God-Object Refactor (D-11) — *This plan is a stretch;
  the project may decide to defer it if the refactor proves too risky for
  the v1.0 release window.*

### Plan numbering

```
Plans:
- [ ] 14-01-PLAN.md — Discuss Phase Q&A Wiring (Wave 1, D-01)
- [ ] 14-02-PLAN.md — Workflow Screen Wiring (Wave 1, D-02/D-03/D-05)
- [ ] 14-03-PLAN.md — msgChan Drainer Synchronization (Wave 1, D-04)
- [ ] 14-04-PLAN.md — Workflow State Persistence (Wave 2, D-06)
- [ ] 14-05-PLAN.md — Discuss Streaming + Low Severity (Wave 2, D-07/D-08/D-09/D-10)
- [ ] 14-06-PLAN.md — AppState Refactor (Wave 3, D-11, optional)
```

### Key Patterns to Follow

- The existing `QuestionRequestMsg` / `QuestionResponseMsg` flow used by the
  `AskUserQuestion` tool is the canonical pattern for collecting user input.
  Phase 14 reuses this for the Discuss phase — no new message types are
  needed for the Q&A itself (only a new `DiscussAnswerTimeoutMsg` for the
  5-minute timeout).
- Screen transitions: `PlanModel.Update()` already returns
  `AppMsg{Screen: ScreenExecute}` on accept (`plan.go:57-58`). The TUI's
  `Update()` handles this message to call `RunPhaseCmd`. No new screen
  transition primitive is needed.
- Concurrency: the Bubble Tea Update() model is single-threaded. All
  state mutations go through `Update()`. The drainer pattern
  (`workflowMsgDrainer`) is a `tea.Cmd` that returns a single
  `tea.Msg` per call and is re-armed by the `Update()` loop.
- Testing: per-test `m.workflowEngine = nil` patterns in `app_test.go` are
  brittle; new tests should use a mock engine that records the phases
  triggered. Plans 01-03 add a `mockWorkflowEngine` test helper.
</specifics>

<deferred>
## Deferred Ideas

- **Question screen with rich answers** — currently the Discuss phase uses
  inline REPL questions (matching the `AskUserQuestion` tool UX). A
  dedicated `ScreenDiscuss` with multi-line textareas and form validation
  could be added in a future phase, but is not required for V1.
- **Workflow checkpoints for crash recovery** — partial persistence
  (D-06) covers the high-level workflow phase. Fine-grained task-level
  checkpoints for crash recovery are deferred to V1.1+.
- **Per-question timeout customization** — current 5-minute timeout is
  hardcoded. Could be made configurable via `Config` in a follow-up.
- **Parallel sub-questions** — currently questions are asked sequentially.
  Parallel asking (show all questions in a list, collect all answers in
  one form) is a UX improvement deferred to V1.1+.
- **Discuss phase retry** — currently if all answers are auto-skipped on
  timeout, the workflow proceeds to Plan with empty answers. A "retry
  discuss" mechanism that resets `discussState` is deferred.
- **AppState → MVU split** (D-11) — the coordinator refactor in Plan 06
  reduces coupling but keeps the Update() router in AppState. A full
  Bubble Tea MVU decomposition with sub-models owning their own state
  is deferred to V2.0+.

---

*Phase: 14-tui-core-wiring-fixes*
*Context gathered: 2026-06-02 via TUI ↔ Core Wiring Audit Report*
</deferred>
