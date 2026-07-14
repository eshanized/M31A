---
phase: 02-code-review
reviewed: 2026-07-14T00:00:00Z
depth: deep
files_reviewed: 4
files_reviewed_list:
  - internal/tui/phase_transition_model.go
  - internal/tui/app_update_phase.go
  - internal/workflow/execute.go
  - internal/tui/execute_model.go
findings:
  critical: 2
  warning: 3
  info: 1
  total: 6
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-07-14
**Depth:** deep
**Files Reviewed:** 4
**Status:** issues_found

## Summary

Reviewed the phase transition confirmation screens, pause/resume execution logic, and TUI integration across 4 files. Found a data race in the engine's pause/resume channel handling that can deadlock the execute phase, and a duplicate rendering bug in the execute screen. The phase transition screens are correctly wired for Discuss->Plan, Execute->Verify, and Verify->Runtime transitions.

## Critical Issues

### CR-01: Data race on `e.resumeCh` in `consumeSkipOrCancel` can deadlock execute phase

**File:** `internal/workflow/engine.go:270-278`
**Issue:** `consumeSkipOrCancel` captures `skipCh`, `cancelCh`, and `groupCh` under the mutex, but reads `e.resumeCh` directly in the select WITHOUT holding the lock. Meanwhile, `ResumeExecution()` (line 200) closes and nils `e.resumeCh` under the lock. If `ResumeExecution` runs between the unlock at line 275 and the select evaluation at line 278, `e.resumeCh` becomes a nil channel. A nil channel in a select never fires, and the captured local channel variables (`skipCh`, `cancelCh`, `groupCh`) are never signaled after resume, causing the goroutine to block forever in the select -- a deadlock.

**Reproduction scenario:** User presses 'p' to pause, then immediately presses 'p' again to resume. The resume runs in the TUI goroutine while the engine goroutine is between the mutex unlock and the select statement.

**Fix:**
```go
func (e *Engine) consumeSkipOrCancel(ctx context.Context) (skipID int, cancelID int, cancelledGroup bool, ok bool) {
	e.pauseMu.Lock()
	skipCh := e.skipTaskCh
	cancelCh := e.cancelTaskCh
	groupCh := e.cancelGroupCh
	resumeCh := e.resumeCh  // capture under lock
	e.pauseMu.Unlock()

	select {
	case <-resumeCh:  // use captured local, not field
		return 0, 0, false, true
	case id := <-skipCh:
		return id, 0, false, true
	case id := <-cancelCh:
		return 0, id, false, true
	case <-groupCh:
		return 0, 0, true, true
	case <-ctx.Done():
		return 0, 0, false, false
	}
}
```

### CR-02: Pending tasks rendered twice in execute viewport

**File:** `internal/tui/execute_model.go:289-354`
**Issue:** `renderTasks()` has two loops. The first loop (lines 289-345) iterates all tasks and only `continue`s for `StatusDone` and `StatusFailed`. All other statuses -- including `StatusPending` -- fall through to the badge rendering at lines 317-324, producing a line like `"  3. ○ pending implement_foo"`. Then the second loop (lines 348-354) explicitly renders tasks with `StatusPending` or empty status again with a different style (`"  3. ○ implement_foo"`). This causes every pending task to appear twice in the viewport, confusing the user and inflating the visual task count.

**Fix:** Add a `continue` for pending and skipped statuses in the first loop:
```go
// After the StatusFailed check at line 314, add:
if task.Status == types.StatusPending || task.Status == types.StatusSkipped || task.Status == "" {
    // Will be rendered in the pending section below (or skipped for Skipped)
    continue
}
```
Then in the second pending loop, also handle `StatusSkipped` with a distinct visual if needed, or skip it entirely.

## Warnings

### WR-01: Variable `w` assigned but never used in `PhaseTransitionModel.View()` -- triggers ineffassign

**File:** `internal/tui/phase_transition_model.go:125-128`
**Issue:** The local variable `w` is assigned from `m.width` on line 125, conditionally reassigned to `80` on line 127, but never read anywhere in the rest of the `View()` method. This triggers the `ineffassign` linter (enabled in golangci-lint config). It also means the fallback width of 80 is dead code -- if `m.width` is 0 or negative, the viewport will use that invalid width rather than the intended 80-column fallback.

**Fix:** Remove the unused variable, or use it to constrain rendering width (e.g., set `MaxWidth` on styles or the content builder):
```go
func (m *PhaseTransitionModel) View() string {
	t := m.theme
	w := m.width
	if w <= 0 {
		w = 80
	}
	// Use w for width-constrained rendering, e.g.:
	contentStyle := lipgloss.NewStyle().Width(w).MaxWidth(w)
	// ... rest of rendering
}
```

### WR-02: Skip/cancel only works for the currently executing task, not pending tasks

**File:** `internal/tui/execute_model.go:178-198`
**Issue:** When paused, the 's' (skip) and 'c' (cancel) keys only operate on `em.tasks[em.currentTask]` -- the single task that was running when pause was triggered. There is no way to skip or cancel other pending tasks in the group. This is an incomplete implementation of the pause-control feature: users can see pending tasks in the viewport but cannot interact with them. The 'x' key cancels the entire group, but there is no way to selectively skip individual pending tasks.

**Fix:** Either:
1. Add cursor navigation in paused mode so users can select which pending task to skip/cancel, or
2. Document that skip/cancel only applies to the current task (and remove the misleading "Skip task" / "Cancel task" labels that suggest per-task granularity)

### WR-03: Pressing `esc`/`q` during active execution navigates away without pausing or confirmation

**File:** `internal/tui/execute_model.go:199-202`
**Issue:** Pressing `esc` or `q` on the execute screen emits `PopScreenMsg{}`, which pops back to the previous screen (usually REPL). Execution continues in the background with no visual feedback on the REPL screen. The user has no indication that execution is still running, and no way to return to the execute screen to monitor progress (the screen stack is popped). Per AGENTS.md: "Bubble Tea is strictly single-threaded -- use channels, never shared mutable state from goroutines." The execution goroutine continues independently of the TUI state.

**Fix:** Either:
1. Auto-pause execution before popping the screen, and show a persistent notification/indicator on the REPL, or
2. Add a confirmation dialog ("Execution is in progress. Pause and go back?"), or
3. Prevent esc/q from working while execution is actively running (only allow when paused or after completion)

## Info

### IN-01: Phase transition screens correctly integrated for Discuss->Plan, Execute->Verify, Verify->Runtime

**Files:** `internal/tui/app_update_phase.go:153-158, 220-226, 229-235`
**Issue (positive confirmation):** Transition screens ARE shown for the three requested transitions:
- Discuss->Plan: `app_update_phase.go:153-158` (when discuss has no questions)
- Execute->Verify: `app_update_phase.go:220-226`
- Verify->Runtime: `app_update_phase.go:229-235`

The router integration at `app_routing.go:410-421` correctly lazily creates and registers the `PhaseTransitionModel`, and the view at `app_view.go:186-187` correctly dispatches to `renderPhaseTransitionContent`. The model is properly stored on `AppState.phaseTransitionModel` (line 177 of `app_state.go`). The `PhaseTransitionMsg` is correctly handled in `app_update.go:164-166` and dispatched to `handlePhaseTransitionDecision`. No issues found with the integration.

Note: Initialize->Discuss and Plan->Execute transitions do NOT show transition screens. Plan->Execute is user-initiated via the plan approve button, so the omission is intentional. Initialize->Discuss goes directly without user confirmation, which is reasonable since it is the first phase.

---

_Reviewed: 2026-07-14_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
