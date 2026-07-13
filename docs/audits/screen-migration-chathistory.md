# Screen Migration Report: ChatHistoryModel

**Date:** 2026-07-13
**Screen:** ScreenChatHistory
**Status:** MIGRATED

---

## Step 1: Coupling Fix Verification

**Confirmed:** The View()-path coupling to REPL was already fixed.

- `SetMessages()` is called from `ensureSubModel(ScreenChatHistory)` in `app_nav.go:536` (Update-path navigation flow)
- `renderChatHistoryContent()` does NOT call `SetMessages()` - it only calls `SetDimensions()` and `View()`
- This is the correct pattern - messages are loaded when navigating to the screen, not at render time

---

## Step 2: Migration Changes

### 1. ChatHistoryModel Screenable Implementation (`chathistory_model.go`)

**Already implemented:**
- `Init() tea.Cmd` - Returns nil (line 64)
- `SetDimensions(w, h int)` - Updates width/height and reinitializes viewport (lines 41-52)
- `SetTheme(theme.Theme)` - Updates theme (lines 36-38)
- `View() string` - Returns rendered content (lines 125-127)

**Modified:**
- `Update(msg tea.Msg) (Screenable, tea.Cmd)` - Changed return type from `tea.Model` to `(Screenable, tea.Cmd)` (lines 67-122)

### 2. ScreenUpdaters Registration (`app_routing.go:433-443`)

**Before:**
```go
m.screenUpdaters[ScreenChatHistory] = func(msg tea.Msg) tea.Cmd {
    if m.chatHistoryModel == nil {
        return nil
    }
    newModel, cmd := m.chatHistoryModel.Update(msg)
    if r, ok := newModel.(*ChatHistoryModel); ok {
        m.chatHistoryModel = r
    }
    return cmd
}
```

**After:**
```go
m.screenUpdaters[ScreenChatHistory] = func(msg tea.Msg) tea.Cmd {
    if m.chatHistoryModel == nil {
        cw, ch := m.contentDimensions()
        m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenChatHistory, m.chatHistoryModel)
    }
    newModel, cmd := m.chatHistoryModel.Update(msg)
    if r, ok := newModel.(*ChatHistoryModel); ok {
        m.chatHistoryModel = r
    }
    return cmd
}
```

### 3. View Rendering (`app_view.go:877-884`)

**Before:**
```go
func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
    if m.chatHistoryModel == nil {
        m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
    }
    m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.chatHistoryModel.View()
}
```

**After:**
```go
func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
    // Ensure ChatHistory is registered with router
    if m.chatHistoryModel == nil {
        m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        m.router.Register(ScreenChatHistory, m.chatHistoryModel)
    }
    m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

### 4. Screen Assignments

**No conversions needed** - ChatHistory has no `m.screen = ScreenChatHistory` assignments. Navigation is handled via `ensureSubModel(ScreenChatHistory)` which creates the model and loads messages in the Update-path.

### 5. Tick Handler (`app_handlers_tick.go:100-106`)

**No changes needed** - Already has proper type assertion for ChatHistoryModel.

---

## Manual Verification

### Message Loading

**Flow confirmed:**
```
User navigates to ChatHistory → ensureSubModel(ScreenChatHistory)
→ Creates model if nil, sets dimensions
→ Calls m.chatHistoryModel.SetMessages(m.replModel.Messages())
→ Messages loaded from REPL session
```

### Scrolling/Viewing Behavior

**Flow confirmed:**
```
User presses j/k/G/g → Update() updates cursor position
→ clampScroll() ensures cursor stays in viewport
→ View() renders messages with cursor indicator
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

- ChatHistoryModel now fully implements Screenable interface
- Router registration follows established pattern
- Update flow uses screenUpdaters map
- View flow uses router.View()
- No direct REPL access at render time - messages loaded via Update-path navigation

---

## Migration Pattern

This migration followed the exact same pattern as previous screens:

1. Changed Update return type to (Screenable, tea.Cmd)
2. Updated screenUpdaters to create model + register if nil
3. Updated view renderer to use router.View()
4. No screen assignment conversions needed (none existed)

---

## Notes

- ChatHistoryModel is a simple screen (147 lines) with minimal coupling
- The View()-path coupling to REPL was already fixed before this migration
- Messages are loaded once when navigating to the screen, not at render time
- The model supports keyboard navigation (j/k/G/g) and mouse wheel scrolling

---

## Related Files

- `internal/tui/chathistory_model.go` - Model implementation (147 lines)
- `internal/tui/app_routing.go` - screenUpdaters registration
- `internal/tui/app_view.go` - renderChatHistoryContent
- `internal/tui/app_nav.go` - ensureSubModel (Update-path navigation)
- `internal/tui/app_handlers_tick.go` - Tick forwarding (no changes needed)

---

## Router Migration Status

For current status of all screen migrations, see `docs/audits/router-migration-status.md`.
