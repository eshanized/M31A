---
phase: 24
reviewed: 2026-06-06T00:00:00Z
depth: deep
files_reviewed: 24
files_reviewed_list:
  - internal/tui/types.go
  - internal/tui/app.go
  - internal/tui/app_view.go
  - internal/tui/app_update.go
  - internal/tui/app_workflow.go
  - internal/tui/app_update_workflow.go
  - internal/tui/streaming.go
  - internal/tui/repl.go
  - internal/tui/repl_stream.go
  - internal/tui/commands.go
  - internal/tui/commands_workflow.go
  - internal/workflow/engine.go
  - internal/workflow/engine_messages.go
  - internal/workflow/engine_parse.go
  - internal/workflow/engine_verify.go
  - internal/workflow/initialize.go
  - internal/workflow/discuss.go
  - internal/workflow/plan.go
  - internal/workflow/execute.go
  - internal/workflow/verify.go
  - internal/workflow/ship.go
  - internal/tui/ship.go
  - internal/tui/plan.go
  - internal/tui/execute.go
findings:
  critical: 3
  warning: 12
  info: 6
  total: 21
status: issues_found
---

# Phase 24: Code Review Report

**Reviewed:** 2026-06-06T00:00:00Z
**Depth:** deep
**Files Reviewed:** 24
**Status:** issues_found

## Summary

Deep review of TUI↔Workflow wiring across 9 focus areas (screen routing, message types, workflow phase transitions, context pruning, tool call cycle, stream handling, state persistence, nil pointer risks, git operations). Found 3 critical bugs, 12 warnings, and 6 informational items. Most critical issues involve goroutine leaks, incorrect message history construction, and skipped phase-transition guards.

---

## Critical Issues

### CR-01: `config.WatchConfig` goroutine leaks on app shutdown

**File:** `internal/tui/app.go:356`
**Issue:** `go config.WatchConfig(app.configWatchCtx, configPath, app.configReloadCh)` launches a goroutine with a context derived from `context.Background()`. The cancel function `configWatchCancel` is stored on `AppState`, but no shutdown path calls it. The `tea.Quit` path in `app_update.go:205` does not call `configWatchCancel()`. The goroutine leaks until process exit, holding an open inotify/file watcher handle. On systems with limited inotify instances, this can prevent other watchers from being created.
**Fix:** Call `app.configWatchCancel()` before `tea.Quit` in the shutdown path at `app_update.go:205`:
```go
// Graceful shutdown: cancel config watcher, then save and quit
if m.configWatchCancel != nil {
    m.configWatchCancel()
}
return m, tea.Quit
```

### CR-02: `View()` mutates state — violates Bubble Tea rendering contract

**File:** `internal/tui/app_view.go:40-41`
**Issue:** `View()` calls `m.replModel.SetKeyRegistry(m.keyRegistry)` and `m.replModel.SetLastActivity(m.lastActivity)`, which mutate `replModel` fields. Bubble Tea requires `View()` to be a pure function — state mutations in `View()` cause non-deterministic rendering. On every render cycle, these calls modify the REPL model, which can trigger another `Update()` if the framework detects state changes, creating an indirect update loop. This also causes data races if `View()` is ever called concurrently.
**Fix:** Move these state syncs into `Update()` — set `replModel.keyRegistry` and `replModel.lastActivity` in the `Update()` method before returning to `View()`. Alternatively, pass them as parameters to `replModel.View(keyRegistry, lastActivity)`.

### CR-03: `executeTaskWithTools` reuses LLM response content for all tool result messages

**File:** `internal/workflow/execute.go:223-231`
**Issue:** When the LLM returns multiple tool calls in one response, lines 223-231 append the same `content` string (the full LLM response) as the assistant message content for *each* tool result. This means the message history receives N copies of the full LLM response text, each paired with one tool result. On the next LLM call within the same task, the model receives duplicated assistant messages, inflating context and potentially causing confused/incorrect behavior. The correct pattern is one assistant message with all tool calls, followed by all tool results.
**Fix:** Consolidate into a single assistant message with all tool calls, then append all tool results:
```go
// After all tool calls are dispatched, build the message history correctly
assistantMsg := m31types.Message{
    Role:      "assistant",
    Content:   content,
    ToolCalls: toolCalls,
}
messages = append(messages, assistantMsg)
for i, result := range toolResults {
    messages = append(messages, m31types.Message{
        Role:    "tool",
        Content: result.Output,
    })
}
```

---

## Warnings

### WR-01: `setWorkflowPhase` only flushes pending chunks on PhaseIdle — intermediate transitions lose buffered chunks

**File:** `internal/tui/app.go:141-148`
**Issue:** `setWorkflowPhase` flushes `pendingStreamChunks` only when transitioning to `PhaseIdle`. When transitioning between active phases (e.g., PhasePlan→PhaseExecute, PhaseExecute→PhaseVerify), buffered stream chunks remain stuck in `pendingStreamChunks`. The `flushPendingStreamChunks()` method exists and is called in error paths (`app_update_workflow.go:70,87`), but the normal phase-transition handlers in `handlePhasePlan`/`handlePhaseExecute` do not call it. Chunks from one phase's streaming are delivered only when the workflow ends, which can be much later.
**Fix:** Flush in `setWorkflowPhase` for all transitions:
```go
func (m *AppState) setWorkflowPhase(phase types.WorkflowPhase) {
    // Flush on any transition, not just to idle
    if len(m.pendingStreamChunks) > 0 && phase != m.currentPhase {
        m.flushPendingStreamChunks()
    }
    m.currentPhase = phase
    m.workflowRunning = (phase != types.PhaseIdle)
    m.headerCacheValid = false
}
```

### WR-02: `handlePhaseDiscuss` bypasses engine Transition guard for Discuss→Plan

**File:** `internal/tui/app_update_workflow.go:160-163`
**Issue:** `handlePhaseDiscuss` calls `m.setWorkflowPhase(types.PhasePlan)` and `RunPhaseCmd(m, types.PhasePlan, ...)` without calling `m.workflowEngine.Transition(ctx, PhaseDiscuss, PhasePlan)` first. The engine's `Transition()` validates the transition via `validPhaseTransitions` and saves checkpoints/STATE.md. Skipping it means no checkpoint is saved for Discuss→Plan, and the transition guard is bypassed. Compare with `finalizeDiscussAndAdvance` in `app_workflow.go:203` which correctly calls `Transition()`. This inconsistency means the Discuss→Plan path has no state persistence or validation.
**Fix:** Add the `Transition()` call before setting the phase:
```go
if m.workflowEngine != nil {
    _ = m.workflowEngine.Transition(context.Background(), types.PhaseDiscuss, types.PhasePlan)
}
m.setWorkflowPhase(types.PhasePlan)
```

### WR-03: `handlePhasePlan` shows ScreenPlan but workflow stalls — no auto-advance or user guidance

**File:** `internal/tui/app_update_workflow.go:193-199`
**Issue:** When `handlePhasePlan` runs after the plan phase completes, it creates/updates the plan model and switches to `ScreenPlan`, but does not auto-advance to the Execute phase. In the `/workflow` automated flow, this creates a dead-end — the user must manually press a key or run `/execute`. There is no status message or toast explaining what to do next. The workflow simply pauses silently.
**Fix:** Add a toast or status message like "Plan ready: N tasks. Press 'A' to accept and execute, or run /execute <goal>."

### WR-04: `finalizeDiscussAndAdvance` silently ignores Transition error

**File:** `internal/tui/app_workflow.go:203`
**Issue:** `_ = m.workflowEngine.Transition(context.Background(), types.PhaseDiscuss, types.PhasePlan)` discards the error from `Transition()`. If the transition is rejected (e.g., due to invalid phase ordering from a corrupted state), the engine's phase is not updated, but `m.setWorkflowPhase(types.PhasePlan)` on line 206 still updates the TUI state, creating a desync between the engine's `activePhase` and the TUI's `currentPhase`.
**Fix:** Check the error and handle it:
```go
if err := m.workflowEngine.Transition(context.Background(), types.PhaseDiscuss, types.PhasePlan); err != nil {
    slog.Warn("Discuss→Plan transition failed", "err", err)
    m.setWorkflowPhase(types.PhaseIdle)
    return nil
}
```

### WR-05: `permissionListenerCmd` blocks indefinitely — goroutine leak if dispatcher closes

**File:** `internal/tui/app.go:612-617`
**Issue:** `permissionListenerCmd` reads from `dispatcher.RequestCh()` in a blocking goroutine. If the dispatcher is closed or its channel is never written to again (e.g., during shutdown after all tools complete), the goroutine blocks forever. There is no timeout, context cancellation, or done channel to allow the listener to exit. Over multiple `/clear` commands or session switches, leaked goroutines accumulate.
**Fix:** Add a timeout or done channel:
```go
func permissionListenerCmd(dispatcher *tools.Dispatcher) tea.Cmd {
    return func() tea.Msg {
        select {
        case req := <-dispatcher.RequestCh():
            return PermissionRequestMsg{Request: req}
        case <-time.After(5 * time.Minute):
            return nil // stop the listener after inactivity
        }
    }
}
```

### WR-06: `questionListenerCmd` same goroutine leak risk

**File:** `internal/tui/app.go:621-633`
**Issue:** Same pattern as WR-05 — `questionListenerCmd` blocks on `dispatcher.QuestionResponseCh()` with no timeout or done channel.
**Fix:** Same approach as WR-05.

### WR-07: `handleStreamChunkMsg` appends chunk without nil guard

**File:** `internal/tui/app_update.go:516-518`
**Issue:** `m.pendingStreamChunks = append(m.pendingStreamChunks, msg.Chunk)` does not check if `msg.Chunk` is nil. The streaming goroutine in `streaming.go:134` does check `chunk == nil` and skips, but if the `StreamChunkMsg` is constructed by other code paths (e.g., the discuss phase emitter at `discuss.go:61`), a nil chunk could slip through. Downstream, `flushPendingStreamChunks` at `app_update_workflow.go:130-134` passes chunks to `replModel.AppendStreamChunk` which does nil-check, so this is not a crash — but it's an unnecessary nil entry in the slice.
**Fix:** Add nil guard before append.

### WR-08: `validateTasks` doesn't remove tasks with ID=0 — leaves phantom tasks in dependency graph

**File:** `internal/workflow/engine_parse.go:103-107`
**Issue:** When `t.ID == 0`, the validator appends an error and `continue`s, but the task remains in the slice with ID=0. At line 111, `idSet[t.ID]` is never set to `true` for this task. Any subsequent task with dependency `[0]` will get a "non-existent dependency" error, masking the real issue. More importantly, the task will be executed by the task runner with ID=0, which may conflict with other tasks.
**Fix:** Either filter out tasks with ID=0 before returning from `parseTasksFromJSON`, or assign sequential IDs to fill gaps.

### WR-09: `extractJSONObject` calls `stripJSONComments` but continues scanning original string indices

**File:** `internal/workflow/engine_parse.go:540-574`
**Issue:** `extractJSONObject` calls `stripJSONComments(s)` to produce a cleaned string, but then runs the depth-tracking scan on the original `s` parameter (the scan loop at lines 549-573 uses `s` not the stripped result). If `stripJSONComments` removed characters (comments), the indices no longer correspond. The function may return incorrect substrings or fail to find the closing brace. This is masked because most LLM responses don't contain comments, but when they do, tool call parsing silently fails.
**Fix:** Run the depth scan on the stripped string:
```go
s = stripJSONComments(s)
// Then scan s (not the original)
for i, c := range s { ... }
```

### WR-10: `collectDiffStats` misidentifies added files — treats additions-only as "added"

**File:** `internal/workflow/ship.go:196`
**Issue:** The heuristic `if adds > 0 && dels == 0 && parts[0] != "0"` treats any file with only additions as "added". But a file that was previously committed and receives only additions in this diff is actually modified. The correct way to identify new files in `git diff --numstat` is to check if the file existed before the session started, or use `git diff --diff-filter=A`.
**Fix:** Use `--diff-filter=A` for accurate added-file counts, or check existence against `e.sessionStartHash`.

### WR-11: `handleDiscussAnswerTimeout` doesn't validate question index — stale timeout can skip active questions

**File:** `internal/tui/app_update_workflow.go:432-438`
**Issue:** When the discuss timeout fires, `handleDiscussAnswerTimeout` calls `m.skipDiscussAndAdvance()` unconditionally. The timeout was set for a specific question index (`askNextDiscussQuestion` at `app_workflow.go:189` emits `DiscussAnswerTimeoutMsg{QuestionIndex: m.currentDiscussIndex}`), but the handler at `app_update_workflow.go:432` does not check if the timeout's `QuestionIndex` matches `m.currentDiscussIndex`. If the user answers quickly and the old timer fires later, it can skip the remaining questions even though the user already answered.
**Fix:** Add index validation in the handler. However, since `DiscussAnswerTimeoutMsg` currently doesn't carry the index (it's checked in `handleDiscussAnswerTimeout` which reads the struct), update `handleDiscussAnswerTimeout` to check the index.

### WR-12: `handlePhaseShip` calls `persistWorkflowState()` then immediately overwrites with explicit reset

**File:** `internal/tui/app_update_workflow.go:314-324`
**Issue:** Line 316 calls `m.persistWorkflowState()` which writes `m.workflowGoal` + `PhaseIdle` to session.json. Lines 317-322 then call `m.sessionManager.UpdateWorkflowState(m.sessionID, "", PhaseIdle, nil)` which overwrites the first write with empty goal and nil questions. Line 324 then sets `m.workflowGoal = ""`. The first write is wasted work, and the double-write to the same file is confusing and error-prone.
**Fix:** Remove the `m.persistWorkflowState()` call at line 316, and only keep the explicit reset at lines 317-322. Set `m.workflowGoal = ""` before the explicit call:
```go
m.workflowGoal = ""
m.setWorkflowPhase(types.PhaseIdle)
m.flushPendingStreamChunks()
if m.sessionManager != nil && m.sessionID != "" {
    if err := m.sessionManager.UpdateWorkflowState(
        m.sessionID, "", types.PhaseIdle, nil,
    ); err != nil {
        slog.Warn("failed to reset workflow state after ship", "err", err)
    }
}
```

---

## Info

### IN-01: `currentKeyContext` missing cases for workflow screens

**File:** `internal/tui/app.go:722-736`
**Issue:** `currentKeyContext` handles `ScreenREPL`, `ScreenSettings`, `ScreenModelSelector`, `ScreenResume`, `ScreenFirstRun` but returns `CtxGlobal` for `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, `ScreenShip`, `ScreenDiff`, `ScreenPermission`. This means workflow screens always use global key bindings, limiting customization for phase-specific shortcuts.
**Fix:** Add cases for workflow screens: `CtxPlan`, `CtxExecute`, `CtxVerify`, `CtxShip`.

### IN-02: Magic number `256` in workflow message channel buffer

**File:** `internal/tui/app.go:459`
**Issue:** `msgCh := make(chan tea.Msg, 256)` uses a magic number. The streaming pipeline uses `64` at `streaming.go:75`. These should be named constants with documentation.
**Fix:** Define `const WorkflowMsgChBuf = 256` in `types.go`.

### IN-03: `ReplModel` has 40+ fields — high structural complexity

**File:** `internal/tui/repl.go:36-118`
**Issue:** The `ReplModel` struct has over 40 fields spanning streaming, thinking blocks, tool cards, fallback banners, questions, slash commands, history, and rendering. This makes state ownership reasoning difficult and increases bug surface area.
**Fix:** Consider extracting sub-structs: `StreamState`, `ThinkingState`, `SlashState`, `BannerState`.

### IN-04: `ReplModel.Messages()` returns internal slice without defensive copy

**File:** `internal/tui/repl.go:891-893`
**Issue:** `func (m *ReplModel) Messages() []types.Message { return m.messages }` returns the internal slice directly. Callers (e.g., `app_update.go:319` passing to `autoDream.SetMessages`) can mutate the message history.
**Fix:** Return a copy: `result := make([]types.Message, len(m.messages)); copy(result, m.messages); return result`.

### IN-05: `stripCodeBlocks` and `stripJSONComments` perform overlapping regex work

**File:** `internal/workflow/engine_parse.go:20, 540`
**Issue:** `parseTasksFromJSON` calls `stripCodeBlocks` (regex for ``` fences), then `extractJSONArray` which calls `extractJSONObject` which calls `stripJSONComments`. Both strip different things but process the same content. Consolidating into a single preprocessing pass would be cleaner.
**Fix:** Consider a unified `preprocessLLMResponse` function.

### IN-06: `healTask` does not return updated `HealsAttempted` — caller must track separately

**File:** `internal/workflow/execute.go:330-401`
**Issue:** `healTask` receives `task` by value and doesn't return the updated heal count. The caller at lines 318, 138, 166, 243 independently increments `task.HealsAttempted` and tracks the count. This split ownership is fragile — if `healTask` is modified to internally track attempts, the caller's count will desync.
**Fix:** Either have `healTask` return the updated count, or document the ownership model clearly in comments.

---

_Reviewed: 2026-06-06T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
