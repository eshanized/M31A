# Screen Migration Report — GhostPicker & GhostOutput Screens

**Date:** 2026-07-11  
**Scope:** Migration of GhostPicker (193 lines) and GhostOutput (178 lines) screens to new Screenable interface + Router architecture (strangler-fig pattern, following ConfirmQuit pilot)

---

## Summary

Successfully migrated **GhostPicker** and **GhostOutput** screens to the new Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated). These are a functional pair — GhostPicker feeds GhostOutput via `SetResult()` called from the sidebar handler — and the handoff path is confirmed intact.

---

## Changes Made

### 1. Screenable Interface Conformance (`ghostpicker_model.go`, `ghostoutput_model.go`)

**GhostPickerModel** (`ghostpicker_model.go:71`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

**GhostOutputModel** (`ghostoutput_model.go:72`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

Both models already implemented `Init()`, `View()`, `SetDimensions(w, h int)`, and `SetTheme(theme.Theme)` — satisfied interface requirements without changes.

### 2. Router Registration (`app_routing.go`)

**ScreenGhostPicker** (`app_routing.go:303-317`):
```go
m.screenUpdaters[ScreenGhostPicker] = func(msg tea.Msg) tea.Cmd {
    if m.ghostPickerModel == nil {
        cw, ch := m.contentDimensions()
        m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenGhostPicker, m.ghostPickerModel)
    }
    newModel, cmd := m.ghostPickerModel.Update(msg)
    if r, ok := newModel.(*GhostPickerModel); ok {
        m.ghostPickerModel = r
    }
    return cmd
}
```

**ScreenGhostOutput** (`app_routing.go:318-332`):
```go
m.screenUpdaters[ScreenGhostOutput] = func(msg tea.Msg) tea.Cmd {
    if m.ghostOutputModel == nil {
        cw, ch := m.contentDimensions()
        m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenGhostOutput, m.ghostOutputModel)
    }
    newModel, cmd := m.ghostOutputModel.Update(msg)
    if r, ok := newModel.(*GhostOutputModel); ok {
        m.ghostOutputModel = r
    }
    return cmd
}
```

Follows ConfirmQuit pilot pattern: lazy init on first message, router registration inline.

### 3. View Delegation to Router (`app_view.go`)

**renderGhostPickerContent** (`app_view.go:764-772`):
```go
func (m *AppState) renderGhostPickerContent(chrome layout.PageChrome) string {
    if m.ghostPickerModel == nil {
        m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        m.router.Register(ScreenGhostPicker, m.ghostPickerModel)
    }
    m.ghostPickerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

**renderGhostOutputContent** (`app_view.go:774-782`):
```go
func (m *AppState) renderGhostOutputContent(chrome layout.PageChrome) string {
    if m.ghostOutputModel == nil {
        m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        m.router.Register(ScreenGhostOutput, m.ghostOutputModel)
    }
    m.ghostOutputModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (in Update updater + View renderer) mirrors ConfirmQuit pilot — safe due to router's idempotent Register.

---

## Handoff Verification (GhostPicker → GhostOutput)

The critical functional path for this pair:

1. **GhostPicker** (`ghostpicker_model.go:104-110`): User presses Enter → emits `GhostWriteRequestMsg{Files: selected}`
2. **Handler** (`handler_sidebar.go:49-58`): `handleGhostWriteRequestMsg` → toast + `m.navigateToScreen(ScreenGhostOutput)`
3. **Navigation** (`app_nav.go`): `navigateToScreen` → `switchScreen` → `router.SwitchTo(ScreenGhostOutput)` — router stays in sync
4. **Model creation** (`app_routing.go:318-322`): GhostOutput lazy-init + router registration in updater (or `app_view.go:774-782` in render path)
5. **Result delivery** (`handler_sidebar.go:62-67`): `handleGhostWriteResultMsg` → `m.ghostOutputModel.SetResult(msg.Result)` — sets the result directly on the model
6. **Render** (`app_view.go:774-782`): `renderGhostOutputContent` → `m.router.View()` — router delegates to GhostOutputModel.View()

No changes to the handoff path. `SetResult()` is a direct method call on the concrete `*GhostOutputModel`, unaffected by the interface boundary change.

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
| `TestHandleGhostWriteResultMsg_NilResult` | ✅ PASS |
| `TestHandleGhostWriteResultMsg_Fields` | ✅ PASS |
| `TestEnsureSubModel_GhostPicker` | ✅ PASS |
| `TestEnsureSubModel_GhostOutput` | ✅ PASS |
| `TestRouteToScreen_GhostPicker` | ✅ PASS |
| `TestRouteToScreen_GhostOutput` | ✅ PASS |
| `TestRegression_ScreensRouteThroughSwitchScreen` (GhostPicker/GhostOutput) | ✅ PASS |
| `TestHandleCommand_Ghost` (commands) | ✅ PASS |

---

## Remaining Screens Untouched

All other 31 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`.

---

## Friction / Interface Issues

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| Same as prior batches | — | No new friction points |

---

## Next Candidates (Phase 1 remaining)

Per `docs/audits/screen-inventory.md`:
1. **PhaseModelPicker** (337 lines, 7 tests, clean dual-model selection UI)

---

## Conclusion

GhostPicker + GhostOutput migration complete. The functional pair's handoff path (`GhostWriteRequestMsg` → navigation → `SetResult` → render) works identically after migration. The strangler-fig pattern continues to hold — old and new paths coexist, zero regressions, minimal friction. Ready to proceed with remaining Phase 1 screens.
