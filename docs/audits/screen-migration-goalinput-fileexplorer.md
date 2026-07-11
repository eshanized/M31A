# Screen Migration Report — GoalInput & FileExplorer Screens

**Date:** 2026-07-11  
**Scope:** Migration of GoalInput (ScreenGoalInput, 159 lines) and FileExplorer (ScreenFileExplorer, 142 lines) screens to new Screenable interface + Router architecture (strangler-fig pattern, following Help/Home batch)

---

## Summary

Successfully migrated **GoalInput** and **FileExplorer** screens to the new Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated). The old and new dispatch paths coexist — GoalInput and FileExplorer now route through the router, while all other 29 screens still use the original `app_routing.go` / `app_view.go` switch statements.

---

## Changes Made

### 1. Screenable Interface Conformance (`goalinput_model.go`, `fileexplorer_model.go`)

**GoalInputModel** (`goalinput_model.go:62`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

**FileExplorerModel** (`fileexplorer_model.go:56`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

Both models already implemented `Init()`, `View()`, `SetDimensions(w, h int)`, and `SetTheme(theme.Theme)` — satisfied interface requirements without changes.

### 2. Router Registration (`app_routing.go`)

**ScreenGoalInput** (`app_routing.go:272-282`):
```go
m.screenUpdaters[ScreenGoalInput] = func(msg tea.Msg) tea.Cmd {
    if m.goalInput == nil {
        m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
        // Register with router
        m.router.Register(ScreenGoalInput, m.goalInput)
    }
    newModel, cmd := m.goalInput.Update(msg)
    if r, ok := newModel.(*GoalInputModel); ok {
        m.goalInput = r
    }
    return cmd
}
```

**ScreenFileExplorer** (`app_routing.go:184-202`):
```go
m.screenUpdaters[ScreenFileExplorer] = func(msg tea.Msg) tea.Cmd {
    if m.fileExplorerModel == nil {
        cw, ch := m.contentDimensions()
        m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
        if m.cwd != "" {
            root := buildFileTree(m.cwd, 0, 3)
            if root != nil {
                m.fileExplorerModel.SetRoot(root)
            }
        }
        // Register with router
        m.router.Register(ScreenFileExplorer, m.fileExplorerModel)
    }
    newModel, cmd := m.fileExplorerModel.Update(msg)
    if r, ok := newModel.(*FileExplorerModel); ok {
        m.fileExplorerModel = r
    }
    return cmd
}
```

Follows established pattern: lazy registration on first message, router registration inline. FileExplorer creation mirrors `ensureSubModel` — builds file tree from `m.cwd` when available.

### 3. View Delegation to Router (`app_view.go`)

**renderGoalInputContent** (`app_view.go:619-628`):
```go
func (m *AppState) renderGoalInputContent(chrome layout.PageChrome) string {
    // Ensure GoalInput is registered with router
    if m.goalInput == nil {
        m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
        m.router.Register(ScreenGoalInput, m.goalInput)
    }
    m.goalInput.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

**renderFileExplorerContent** (`app_view.go:730-744`):
```go
func (m *AppState) renderFileExplorerContent(chrome layout.PageChrome) string {
    // Ensure FileExplorer is registered with router
    if m.fileExplorerModel == nil {
        m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        if m.cwd != "" {
            root := buildFileTree(m.cwd, 0, 3)
            if root != nil {
                m.fileExplorerModel.SetRoot(root)
            }
        }
        m.router.Register(ScreenFileExplorer, m.fileExplorerModel)
    }
    m.fileExplorerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (in Update updater + View renderer) follows established pattern — safe due to router's idempotent Register.

### 4. Navigation Flow (no changes needed)

Both screens already use `navigateToScreen()` which calls `switchScreen()` (refactored in Help/Home batch). Screen switching correctly syncs router for all registered screens:
- `navigateToScreen` → `m.switchScreen(screen)` → `m.router.SwitchTo(s)`
- `popScreen` → `m.switchScreen(prev)` → `m.router.SwitchTo(prev)`

GoalInput `esc` returns `AppMsg{Screen: ScreenREPL}` → `handleAppMsg` → `navigateToScreen(ScreenREPL)` → `switchScreen(ScreenREPL)`.  
FileExplorer `esc`/`q` returns `PopScreenMsg{}` → `popScreen()` → `switchScreen(prev)`.

No direct `m.screen = ScreenGoalInput` or `m.screen = ScreenFileExplorer` assignments in runtime code (only in test setup).

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test -race ./internal/tui/...` | ✅ (all pass) |
| `go vet ./internal/tui/...` | ✅ (pre-existing emitter_stress_test.go govet warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning only) |

### Test Coverage Specific to Migration

| Test | Status |
|------|--------|
| `TestViewGoalInput` | ✅ PASS |
| `TestViewFileExplorer` | ✅ PASS |
| `TestEnsureSubModel_GoalInput` | ✅ PASS |
| `TestEnsureSubModel_FileExplorer` | ✅ PASS |
| `TestUpdate_ScreenRouting` | ✅ PASS |
| `TestUpdate_KeyboardRouting` | ✅ PASS |
| `TestE2E_RoutingInit` | ✅ PASS |
| `TestNavigateToScreen` (all variants) | ✅ PASS |

---

## Manual Verification (Behavioral Equivalence)

### GoalInput Screen (opens via `/goal` slash command)

**Expected (pre-migration):**
1. Full-screen textarea with placeholder "Describe the goal for this coding session..."
2. `ctrl+enter` submits `GoalSubmittedMsg{Goal: text}` → workflow or chat
3. `esc` returns to REPL via `AppMsg{Screen: ScreenREPL}`
4. `ctrl+r` toggles recent goals panel (if any)
5. `up/down` navigates recent goals, `enter` selects
6. Cursor blinks via `textarea.Blink` on Init

**Verified (post-migration):**
- Textarea renders correctly with proper dimensions
- `ctrl+enter` emits `GoalSubmittedMsg` → `handleGoalSubmittedMsg` processes it
- `esc` returns `AppMsg{Screen: ScreenREPL}` → `handleAppMsg` → `navigateToScreen(ScreenREPL)` → `switchScreen`
- Recent goals panel toggle works identically
- `SetDimensions`/`SetTheme` propagate via router and direct calls
- Footer hints: "ctrl+enter submit", "esc back", "ctrl+r recent"

### FileExplorer Screen (opens via `/files` or `ctrl+x f`)

**Expected (pre-migration):**
1. Shows file tree browser with title "File Explorer"
2. `SetRoot()` populates tree from working directory (max depth 3)
3. `SetDimensions()` sizes tree correctly (width-4, height-6)
4. `j/k` or `up/down` navigates tree
5. `enter`/`space` toggles directory expand/collapse
6. `esc`/`q` returns to previous screen via `PopScreenMsg`
7. Hidden files and vendor/node_modules directories excluded

**Verified (post-migration):**
- Tree renders correctly with title and footer
- `SetRoot()` with real file tree from `buildFileTree(m.cwd, 0, 3)` populates correctly
- `SetDimensions()` sizes tree (w-4, h-6) as before
- Navigation (j/k/up/down) and toggle (enter/space) work identically
- `esc`/`q` returns `PopScreenMsg` → `popScreen()` → `switchScreen(prev)`
- Footer hints: "j/k Navigate", "enter Toggle dir", "esc Back"

---

## Remaining Screens Untouched

All other 29 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGoalInput`, `ScreenFileExplorer`. Strangler-fig pattern confirmed viable.

---

## Friction / Interface Issues for Next Batch

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is `int` in `tuitypes.go`; interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| `router.Register` takes `ScreenID` but model creation is lazy | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only ConfirmQuit/Help/Home/GoalInput/FileExplorer receive propagation currently; safe |
| `navigateToScreen` now calls `switchScreen` globally | All screen transitions now sync router — no-op for unregistered screens | Correct behavior; router.SwitchTo returns nil for unregistered IDs |
| GoalInput `esc` uses `AppMsg{Screen: ScreenREPL}` not `PopScreenMsg` | Inconsistent with other screens (FileExplorer uses PopScreenMsg) | Consider standardizing to PopScreenMsg in future cleanup |

---

## Next Candidates (Phase 1 from inventory)

Per `docs/audits/screen-inventory.md`:
1. **GhostPicker / GhostOutput** (193+178 lines, isolated feature)
2. **PhaseModelPicker** (337 lines, 7 tests, clean dual-model selection UI)

---

## Conclusion

The Screenable + Router pattern works end-to-end for GoalInput and FileExplorer screens. The strangler-fig migration is viable: old and new paths coexist, zero regressions, minimal friction. Ready to proceed with Phase 1 batch migration.