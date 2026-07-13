# Screen Migration Audit: VerifyModel

**Date:** 2026-07-13
**Screen:** ScreenVerify
**Status:** MIGRATED

## Summary

VerifyModel has been successfully migrated to the Screenable + Router pattern, following the strangler-fig migration approach established in Phase 3.

## Data Flow Analysis

### Question: Is VerifyModel SAFE or UNSAFE to migrate?

**Answer: SAFE** - All data flows through messages.

### Evidence

1. **StartHealing() and StopHealing()** are called from message handlers in `app_handlers_workflow.go`:
   - `handleWorkflowSelfHealStart()` at line 145 calls `m.verifyModel.StartHealing(msg.TaskID, msg.Attempt)`
   - `handleWorkflowSelfHealComplete()` at line 162 calls `m.verifyModel.StopHealing()`
   - Both receive values from `workflow.SelfHealStartMsg` and `workflow.SelfHealCompleteMsg` messages

2. **SetManualSteps()** is called from `app_update_phase.go` at line 273:
   - Receives `msg.ManualVerificationSteps` from `PhaseExecuteCompleteMsg`
   - No direct access to live engine or goroutine state

3. **HealResultMsg** is handled in the Update() method at line 157-166:
   - Updates task status based on message content
   - Calls `StopHealing()` to clear healing state

4. **No direct access to live engine state**:
   - VerifyModel has no field referencing workflow engine
   - All data (tasks, results, manual steps) passed through messages
   - Healing state managed entirely through message-driven callbacks

## Changes Made

### 1. VerifyModel Screenable Implementation (`verify_model.go`)

**Added methods:**
- `Init() tea.Cmd` - Returns nil (no initialization needed)
- `SetDimensions(w, h int)` - Updates width/height and reinitializes viewport
- `SetTheme(theme.Theme)` - Updates theme and refreshes content

**Modified methods:**
- `Update(msg tea.Msg) (Screenable, tea.Cmd)` - Changed return type from `*VerifyModel` to `(Screenable, tea.Cmd)`

### 2. ScreenUpdaters Registration (`app_routing.go`)

**Before:**
```go
m.screenUpdaters[ScreenVerify] = func(msg tea.Msg) tea.Cmd {
    if m.verifyModel == nil {
        return nil
    }
    newModel, cmd := m.verifyModel.Update(msg)
    m.verifyModel = newModel
    return cmd
}
```

**After:**
```go
m.screenUpdaters[ScreenVerify] = func(msg tea.Msg) tea.Cmd {
    if m.verifyModel == nil {
        cw, ch := m.contentDimensions()
        m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenVerify, m.verifyModel)
    }
    newModel, cmd := m.verifyModel.Update(msg)
    if r, ok := newModel.(*VerifyModel); ok {
        m.verifyModel = r
    }
    return cmd
}
```

### 3. View Rendering (`app_view.go`)

**Before:**
```go
func (m *AppState) renderVerifyContent(chrome layout.PageChrome) string {
    if m.verifyModel == nil {
        return renderEmptyState("No verification results", "Run /verify after executing tasks", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
    }
    m.verifyModel.width = chrome.ContentWidth()
    m.verifyModel.height = chrome.ContentHeight()
    return m.verifyModel.View()
}
```

**After:**
```go
func (m *AppState) renderVerifyContent(chrome layout.PageChrome) string {
    // Ensure Verify is registered with router
    if m.verifyModel == nil {
        cw, ch := m.contentDimensions()
        m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenVerify, m.verifyModel)
    }
    m.verifyModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

### 4. Screen Assignment (`app_update_phase.go`)

Converted `m.screen = ScreenVerify` to `m.switchScreen(ScreenVerify)` at line 245.

Added `m.router.Register(ScreenVerify, m.verifyModel)` after model creation at line 271.

### 5. Handler Type Assertion (`handler_workflow.go`)

Updated `handleHealResultMsg()` to add type assertion for VerifyModel Update result (lines 84-86).

## Manual Verification

### Healing Spinner State
- **StartHealing()** is called when `SelfHealStartMsg` is received
- Sets `healingTaskID` and `healAttempt`, resets spinner
- **TickSpinner()** is called from tick handler to animate spinner
- **StopHealing()** is called when `SelfHealCompleteMsg` is received
- Clears healing state and refreshes viewport

**Flow confirmed:**
```
Workflow engine → SelfHealStartMsg → handleWorkflowSelfHealStart()
→ m.verifyModel.StartHealing(msg.TaskID, msg.Attempt)
→ renderResults() shows spinner
```

### Manual Step Display
- **SetManualSteps()** is called from `PhaseExecuteCompleteMsg` handler
- Stores steps in `vm.manualSteps` slice
- Displayed in `renderResults()` with brand-colored header

**Flow confirmed:**
```
Workflow engine → PhaseExecuteCompleteMsg → app_update_phase.go
→ m.verifyModel.SetManualSteps(msg.ManualVerificationSteps)
→ renderResults() displays manual steps
```

## Verification

- ✅ Build: `go build ./...` passes
- ✅ Vet: `go vet ./...` passes (pre-existing test issue only)
- ✅ Lint: `golangci-lint run ./internal/tui/...` passes (pre-existing test issue only)
- ✅ Tests: `go test -race ./internal/tui/...` all pass

## Architecture Compliance

- VerifyModel now fully implements Screenable interface
- Router registration follows established pattern
- Update flow uses screenUpdaters map
- View flow uses router.View()
- No direct engine access - all data via messages

## Migration Pattern

This migration followed the exact same pattern as the previous screen (RuntimeCheckModel):

1. Added Screenable methods (Init, SetDimensions, SetTheme)
2. Changed Update return type to (Screenable, tea.Cmd)
3. Updated screenUpdaters to create model + register if nil
4. Updated view renderer to use router.View()
5. Converted direct screen assignments to switchScreen()
6. Added type assertions where needed

## Notes

- VerifyModel is more complex than RuntimeCheckModel due to healing state and manual steps
- The `healingTaskID` field controls spinner visibility per-task
- `SetHealFunc()` provides the callback for triggering heal actions
- Model is created in `app_update_phase.go` when `PhaseExecuteCompleteMsg` is received
- Test file (`app_view_extra_test.go`) uses direct `m.screen = ScreenVerify` assignment, which is acceptable for tests

## Related Files

- `internal/tui/verify_model.go` - Main model implementation
- `internal/tui/verify_view.go` - View rendering (not modified, uses model's View())
- `internal/tui/app_routing.go` - screenUpdaters registration
- `internal/tui/app_view.go` - renderVerifyContent
- `internal/tui/app_update_phase.go` - Screen assignment and model creation
- `internal/tui/handler_workflow.go` - HealResultMsg handler
- `internal/tui/app_handlers_workflow.go` - SelfHealStart/Complete handlers
