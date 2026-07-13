# ChatHistory REPL Coupling Fix Report

**Date:** 2026-07-13
**Issue:** `renderChatHistoryContent()` (called from `View()`) called `m.chatHistoryModel.SetMessages(m.replModel.Messages())` — reading live state from the REPL model during render instead of receiving it via Update().

---

## Actual Call Site

**File:** `internal/tui/app_view.go:866-872` (before fix)
**Function:** `renderChatHistoryContent(chrome layout.PageChrome) string`
**Called from:** `AppState.View()` at line 177 via a `case ScreenChatHistory:` branch

The call was guarded by `if m.chatHistoryModel == nil` — a lazy-init that only ran on first render. But it was still in the View() path, reading `m.replModel.Messages()` (a live slice from another screen model) and passing it to `chatHistoryModel.SetMessages()`.

A second, correct call site already existed at `app_nav.go:536` in `routeToScreen()` → `ensureSubModel(ScreenChatHistory)`, which loads messages on every navigation to the screen from the Update() path.

---

## Why This Fix Shape (Not Message-Driven)

ChatHistory is **only ever shown after explicit navigation** — it is never continuously re-rendered while REPL is also visible. The user presses a key or selects "Chat History" from the sidebar, `navigateToScreen(ScreenChatHistory)` fires (Update path), and `ensureSubModel` creates the model and calls `SetMessages(m.replModel.Messages())`. After that, View() just renders.

This means the fix is a simple **move from View() to Update()** — no message type needed, no cached field on AppState, no DecisionsSnapshotMsg-style infrastructure. The nav path already did the right thing; the View() path was redundant.

By contrast, the Decisions fix required a message-driven flow because decisions are logged asynchronously by the workflow engine and must be reflected in the Decisions screen without explicit navigation. ChatHistory has no such requirement.

---

## Change Made

**File:** `internal/tui/app_view.go`

Removed the `SetMessages` call from `renderChatHistoryContent()`. The lazy model creation was kept as a safety net (so the model exists if View() is ever called before navigation, which should not happen in normal flow but is defensive).

Before:
```go
func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
    if m.chatHistoryModel == nil {
        m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        if m.replModel != nil {
            m.chatHistoryModel.SetMessages(m.replModel.Messages())
        }
    }
    m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.chatHistoryModel.View()
}
```

After:
```go
func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
    if m.chatHistoryModel == nil {
        m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
    }
    m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.chatHistoryModel.View()
}
```

---

## Verification

```bash
$ go build ./...
# success (no output)

$ go vet ./...
# success (pre-existing copylocks warning in emitter_stress_test.go:61, unrelated)

$ golangci-lint run ./internal/tui/... --timeout 5m
# 1 issue: pre-existing copylocks in emitter_stress_test.go:61 (not related to this change)

$ go test -race ./internal/tui/... -count=1
ok  github.com/eshanized/M31A/internal/tui      14.456s
ok  github.com/eshanized/M31A/internal/tui/a11y  1.020s
ok  github.com/eshanized/M31A/internal/tui/commands 1.054s
ok  github.com/eshanized/M31A/internal/tui/components 1.897s
ok  github.com/eshanized/M31A/internal/tui/layout 1.152s
ok  github.com/eshanized/M31A/internal/tui/streaming 1.043s
ok  github.com/eshanized/M31A/internal/tui/theme 1.076s
ok  github.com/eshanized/M31A/internal/tui/tuitypes 1.048s
```

---

## Remaining View()-Path Cross-Model Reads (Separate Issues)

The broader audit of `app_view.go` found other View()-path reads of live state from other screen models. These are NOT related to ChatHistory but are the same category of anti-pattern. They should be addressed before or during REPL migration.

### High Priority (reads live state from another model in View)

| Location | Code | What's read |
|----------|------|-------------|
| `app_view.go:333-336` `buildHeaderInfo()` | `m.replModel.lastUsage.TotalTokens`, `m.replModel.activeModel.ContextLength` | Token usage, model context length |
| `app_view.go:369-393` `buildFooterInfo()` | `m.replModel.thinking`, `.streaming`, `.thinkingStartAt`, `.spinner`, `.cwd` | REPL state for footer rendering |
| `app_view.go:374-375` | `m.sidebarModel.branch` | Git branch for header/footer |
| `app_view.go:464-467` | `m.replModel.lastUsage`, `.cfg.UI.ShowCostEstimate`, `.lastCost` | Cost/usage for footer |
| `app_view.go:780` `renderDashboardContent()` | `m.dashboardModel.SetWorkflowState(...)` | Pushes AppState data into sub-model during render |

### Medium Priority (mutations of other models in View)

| Location | Code | What's mutated |
|----------|------|----------------|
| `app_view.go:86-87` `buildSidebarAndChrome()` | `m.sidebarModel.SetHeight()`, `.SetCurrentScreen()` | Sidebar dimensions/label |
| `app_view.go:526-527` `renderREPLContent()` | `m.subagentsModel.SetSize()`, `.SetTheme()` | Subagent panel |
| `app_view.go:979-983` `syncReplSize()` | `m.replModel.width`, `.height`, `.SetSidebarWidth()` | REPL dimensions |

### Low Priority (dimension-setting, systematic pattern)

27+ models have `SetDimensions()` called in their render functions — a common Bubble Tea compromise for propagating window size. Not a data race but not pure Elm architecture.

---

## Data Flow After Fix

```
User navigates to ChatHistory
  → navigateToScreen(ScreenChatHistory)     [Update path]
    → ensureSubModel(ScreenChatHistory)     [Update path]
      → m.chatHistoryModel.SetMessages(m.replModel.Messages())  [Update path]
  → View()
    → renderChatHistoryContent()
      → m.chatHistoryModel.View()           [reads cached messages only]
```

The SetMessages call lives entirely in the Update() path. View() only reads what was already set.
