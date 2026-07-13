# Screen Migration Audit: RuntimeCheckModel

**Date:** 2026-07-13
**Screen:** ScreenRuntimeCheck
**Status:** MIGRATED

## Summary

RuntimeCheckModel has been successfully migrated to the Screenable + Router pattern, following the strangler-fig migration approach established in Phase 3.

## Data Flow Analysis

### Question: Is RuntimeCheckModel SAFE or UNSAFE to migrate?

**Answer: SAFE** - All data flows through messages.

### Evidence

1. **RuntimeModel receives data via `SetSummary()` method** (`runtime_model.go:52`)
   - Called from `app_update.go:203` and `app_update_phase.go:299-301`
   - Data source: `workflow.RuntimeSummary` structs passed via messages

2. **No direct access to DevServer live state**
   - RuntimeModel has no field referencing `DevServer` or `devServerEntry`
   - No imports of `internal/tools` package
   - `workflow.RuntimeSummary` is constructed in `internal/workflow/runtime.go` from message-driven data

3. **Message flow path:**
   ```
   Engine.runtime() → constructs RuntimeSummary → sends via message
   → TUI receives in app_update_phase.go → calls SetSummary()
   → RuntimeModel displays results
   ```

## Changes Made

### 1. RuntimeModel Screenable Implementation (`runtime_model.go`)

**Added methods:**
- `Init() tea.Cmd` - Returns nil (no initialization needed)
- `SetDimensions(w, h int)` - Updates width/height and reinitializes viewport
- `SetTheme(theme.Theme)` - Updates theme and refreshes content
- `View() string` - Returns rendered content (delegates to `renderContent()`)

**Modified methods:**
- `Update(msg tea.Msg) (Screenable, tea.Cmd)` - Changed return type from `*RuntimeModel` to `(Screenable, tea.Cmd)`

### 2. ScreenUpdaters Registration (`app_routing.go`)

**Before:**
```go
m.screenUpdaters[ScreenRuntimeCheck] = func(msg tea.Msg) tea.Cmd {
    if m.runtimeModel == nil {
        return nil
    }
    newModel, cmd := m.runtimeModel.Update(msg)
    m.runtimeModel = newModel
    return cmd
}
```

**After:**
```go
m.screenUpdaters[ScreenRuntimeCheck] = func(msg tea.Msg) tea.Cmd {
    if m.runtimeModel == nil {
        cw, ch := m.contentDimensions()
        m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
    }
    newModel, cmd := m.runtimeModel.Update(msg)
    if r, ok := newModel.(*RuntimeModel); ok {
        m.runtimeModel = r
    }
    return cmd
}
```

### 3. View Rendering (`runtime_view.go`)

**Before:**
```go
func (m *AppState) renderRuntimeContent(chrome layout.PageChrome) string {
    if m.runtimeModel == nil {
        return ""
    }
    m.runtimeModel.width = chrome.ContentWidth()
    m.runtimeModel.height = chrome.ContentHeight()
    return m.runtimeModel.renderContent()
}
```

**After:**
```go
func (m *AppState) renderRuntimeContent(chrome layout.PageChrome) string {
    if m.runtimeModel == nil {
        cw, ch := m.contentDimensions()
        m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
    }
    m.runtimeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

### 4. Screen Assignment (`app_update_phase.go`)

Converted 2 occurrences of `m.screen = ScreenRuntimeCheck` to `m.switchScreen(ScreenRuntimeCheck)`.

### 5. Tick Handler (`app_handlers_tick.go`)

Added type assertion for RuntimeModel Update result (lines 70-74).

## Verification

- ✅ Build: `go build ./...` passes
- ✅ Vet: `go vet ./...` passes (pre-existing test issue only)
- ✅ Lint: `golangci-lint run ./internal/tui/...` passes (pre-existing test issue only)
- ✅ Tests: `go test -race ./internal/tui/...` all pass

## Architecture Compliance

- RuntimeModel now fully implements Screenable interface
- Router registration follows established pattern
- Update flow uses screenUpdaters map
- View flow uses router.View()
- No direct DevServer access - all data via messages

## Migration Pattern

This migration followed the exact same pattern as the first screen (likely ShipModel or LedgerModel):

1. Added Screenable methods (Init, SetDimensions, SetTheme, View)
2. Changed Update return type to (Screenable, tea.Cmd)
3. Updated screenUpdaters to create model + register if nil
4. Updated view renderer to use router.View()
5. Converted direct screen assignments to switchScreen()

## Notes

- RuntimeModel is simpler than most screens (no complex state management)
- The `testing` flag controls spinner animation during dev server startup
- `SetSummary()` is the primary data injection point
- Model is created lazily in both screenUpdaters and view renderer
