# Screen Migration Report: ExecuteModel

**Date:** 2026-07-13
**Screen:** ScreenExecute
**Status:** MIGRATED

---

## Step 1: Mutation Trace Summary

Full mutation trace written to `docs/audits/execute-mutation-trace.md` before migration.

### Key Findings

| Classification | Count |
|----------------|-------|
| **SAFE** | 25 |
| **UNSAFE** | 0 |
| **UNCLEAR** | 0 |

**All 25 mutation sites are SAFE.** Every mutation is called from an Update()-path handler with values passed in via messages, or from view/nav handlers with computed values. No mutation reads live engine or goroutine state directly.

### Mutation Categories

1. **Workflow handler mutations** (10+ sites in `app_handlers_workflow.go`):
   - `SetCurrentTask()` - called from `handleWorkflowTaskStart()` with index from message
   - `UpdateTaskStatus()` - called from `handleWorkflowTaskStart()` and `handleWorkflowTaskUpdate()` with values from messages
   - `AppendLiveOutput()` - called from 7 different workflow handlers with values from messages

2. **Phase transition mutations** (3 sites):
   - `tasks = tasks` in `app_update_phase.go:167,361` - from messages or planModel
   - `tasks = tasks` in `app_session.go:327` - from session restore

3. **View/nav mutations** (8 sites):
   - `width`, `height`, `theme` assignments from computed values

4. **Other mutations** (4 sites):
   - `sessionID` from `setSessionID()`
   - `paused` from `ExecutePauseMsg`

### Cross-Screen Coupling

- **Execute reads Plan**: `handlePlanApprove()` copies `m.planModel.tasks` to `m.executeModel.tasks`
- **One-way coupling only**: Plan does NOT read Execute's tasks
- **Confirmed**: This is the only place Execute reads from another screen's concrete state

---

## Step 2: Migration Changes

### 1. ExecuteModel Screenable Implementation (`execute_model.go`)

**Added methods:**
- `Init() tea.Cmd` - Returns nil (no initialization needed)
- `SetDimensions(w, h int)` - Updates width/height and reinitializes viewport
- `SetTheme(theme.Theme)` - Updates theme, animated progress bar theme, and refreshes content

**Modified methods:**
- `Update(msg tea.Msg) (Screenable, tea.Cmd)` - Changed return type from `*ExecuteModel` to `(Screenable, tea.Cmd)`

### 2. ScreenUpdaters Registration (`app_routing.go:95-105`)

**Before:**
```go
m.screenUpdaters[ScreenExecute] = func(msg tea.Msg) tea.Cmd {
    if m.executeModel == nil {
        return nil
    }
    newModel, cmd := m.executeModel.Update(msg)
    m.executeModel = newModel
    return cmd
}
```

**After:**
```go
m.screenUpdaters[ScreenExecute] = func(msg tea.Msg) tea.Cmd {
    if m.executeModel == nil {
        cw, ch := m.contentDimensions()
        m.executeModel = NewExecuteModel([]types.Task{}, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenExecute, m.executeModel)
    }
    newModel, cmd := m.executeModel.Update(msg)
    if r, ok := newModel.(*ExecuteModel); ok {
        m.executeModel = r
    }
    return cmd
}
```

### 3. View Rendering (`app_view.go:608-617`)

**Before:**
```go
func (m *AppState) renderExecuteContent(chrome layout.PageChrome) string {
    if m.executeModel == nil {
        return renderEmptyState("No tasks to execute", "Run /new to start a workflow", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
    }
    m.executeModel.width = chrome.ContentWidth()
    m.executeModel.height = chrome.ContentHeight()
    return m.executeModel.View()
}
```

**After:**
```go
func (m *AppState) renderExecuteContent(chrome layout.PageChrome) string {
    // Ensure Execute is registered with router
    if m.executeModel == nil {
        cw, ch := m.contentDimensions()
        m.executeModel = NewExecuteModel([]types.Task{}, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenExecute, m.executeModel)
    }
    m.executeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

### 4. Screen Assignments (`app_update_phase.go`)

Converted 2 occurrences of `m.screen = ScreenExecute` to `m.switchScreen(ScreenExecute)`:
- Line 161: Fast/direct mode path
- Line 348: `handlePlanApprove()`

Added `m.router.Register(ScreenExecute, m.executeModel)` after model creation in both locations.

### 5. Tick Handler (`app_handlers_tick.go:48-52`)

Added type assertion for ExecuteModel Update result:
```go
if m.screen == ScreenExecute && m.executeModel != nil {
    execM, cmd := m.executeModel.Update(msg)
    if r, ok := execM.(*ExecuteModel); ok {
        m.executeModel = r
    }
    cmds = append(cmds, cmd)
}
```

### 6. Test Fix (`handler_workflow_test.go:10-16`)

Added `router: NewRouter()` to `newTestAppStateForWorkflow()` to prevent nil pointer dereference when `handlePlanApprove()` calls `m.router.Register()`.

---

## Manual Verification

### Live Task Output Streaming

**Flow confirmed:**
```
Workflow engine → ToolStartMsg/ToolCompleteMsg → handleWorkflowToolStart/Complete()
→ m.executeModel.AppendLiveOutput(lines)
→ renderTasks() displays live output in viewport
```

The `AppendLiveOutput()` method:
- Appends lines to `em.liveOutput` slice
- Keeps only last 50 lines (ring buffer behavior)
- Calls `refreshContent()` to update viewport
- View renders live output for the currently running task, capped to 1/3 of viewport height

### Pause/Resume

**Flow confirmed:**
```
User presses 'p' → Update() sets em.paused = true
→ Returns ExecutePauseMsg{Paused: true}
→ handleExecutePauseMsg() sets m.executeModel.paused = msg.Paused
→ View() shows "PAUSED" indicator
→ TickMsg handling skipped when paused (spinner/animation frozen)
```

### Task Status Updates

**Flow confirmed:**
```
Workflow engine → TaskUpdateMsg → handleWorkflowTaskUpdate()
→ m.executeModel.UpdateTaskStatus(msg.Task.ID, status)
→ Updates task status in em.tasks slice
→ Triggers progress animation (flash green on completion, red on failure)
→ refreshContent() updates viewport
```

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test -race ./internal/tui/...` | ✅ (all pass) |
| `go vet ./...` | ✅ (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning only) |

---

## Architecture Compliance

- ExecuteModel now fully implements Screenable interface
- Router registration follows established pattern
- Update flow uses screenUpdaters map
- View flow uses router.View()
- No direct engine access - all data via messages
- Plan->Execute task-copy handoff unchanged (one-way coupling preserved)

---

## Migration Pattern

This migration followed the exact same pattern as previous screens:

1. Added Screenable methods (Init, SetDimensions, SetTheme)
2. Changed Update return type to (Screenable, tea.Cmd)
3. Updated screenUpdaters to create model + register if nil
4. Updated view renderer to use router.View()
5. Converted direct screen assignments to switchScreen()
6. Added type assertions where needed
7. Fixed test helper to include router

---

## Notes

- ExecuteModel is the highest-risk screen migrated so far due to 10+ workflow handler mutations
- All mutations are message-driven - no live engine state access
- Live output streaming works through AppendLiveOutput() with 50-line ring buffer
- Progress animation triggers on task status changes (green flash on completion, red on failure)
- Pause/resume freezes spinner and animation, dims viewport content

---

## Related Files

- `internal/tui/execute_model.go` - Model implementation (336 lines)
- `internal/tui/app_routing.go` - screenUpdaters registration
- `internal/tui/app_view.go` - renderExecuteContent
- `internal/tui/app_update_phase.go` - Screen assignment and model creation
- `internal/tui/app_handlers_workflow.go` - Workflow handler mutations
- `internal/tui/app_handlers_tick.go` - Tick forwarding
- `internal/tui/handler_workflow.go` - Pause/resume handler
- `internal/tui/handler_workflow_test.go` - Test helper fix
- `docs/audits/execute-mutation-trace.md` - Pre-migration mutation trace

---

## Router Migration Status

For current status of all screen migrations, see `docs/audits/router-migration-status.md`.
