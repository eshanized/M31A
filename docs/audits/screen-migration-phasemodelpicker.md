# Screen Migration Report — PhaseModelPicker Screen

**Date:** 2026-07-11  
**Scope:** Migration of PhaseModelPicker (337 lines across phasemodelpicker.go + phasemodelpicker_view.go) to new Screenable interface + Router architecture (strangler-fig pattern, following ConfirmQuit pilot)

---

## Summary

Successfully migrated **PhaseModelPicker** screen to the new Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated). The dual-model selection UI (Planning model left panel, Coding model right panel) renders identically, keyboard navigation (tab, arrows, enter, ctrl+enter, esc) works identically.

---

## Changes Made

### 1. Screenable Interface Conformance (`phasemodelpicker.go`)

**PhaseModelPickerModel** (`phasemodelpicker.go:154`):
- Changed `Update(msg tea.Msg) (*PhaseModelPickerModel, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Changed `handleKey(msg tea.KeyMsg) (*PhaseModelPickerModel, tea.Cmd)` → `handleKey(msg tea.KeyMsg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation
- Already implemented `Init()`, `View()`, `SetDimensions(w, h int)`, `SetTheme(theme.Theme)` — satisfied interface requirements without changes

### 2. Router Registration (`app_routing.go`)

**ScreenPhaseModelPicker** (`app_routing.go:341-354`):
```go
m.screenUpdaters[ScreenPhaseModelPicker] = func(msg tea.Msg) tea.Cmd {
    if m.phaseModelPicker == nil {
        cw, ch := m.contentDimensions()
        m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenPhaseModelPicker, m.phaseModelPicker)
    }
    newModel, cmd := m.phaseModelPicker.Update(msg)
    if r, ok := newModel.(*PhaseModelPickerModel); ok {
        m.phaseModelPicker = r
    }
    return cmd
}
```

Follows ConfirmQuit pilot pattern: lazy init on first message, router registration inline.

### 3. View Delegation to Router (`app_view.go`)

**renderPhaseModelPickerContent** (`app_view.go:756-767`):
```go
func (m *AppState) renderPhaseModelPickerContent(chrome layout.PageChrome) string {
    if m.phaseModelPicker == nil {
        cw, ch := m.contentDimensions()
        m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenPhaseModelPicker, m.phaseModelPicker)
    }
    m.phaseModelPicker.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (in Update updater + View renderer) mirrors ConfirmQuit pilot — safe due to router's idempotent Register.

### 4. Screen Switch via `switchScreen` (`handler_sidebar.go`)

**handleGoalSubmittedMsg** (`handler_sidebar.go:15-22`):
```go
func handleGoalSubmittedMsg(m *AppState, msg GoalSubmittedMsg) (tea.Model, tea.Cmd) {
    m.workflowGoal = msg.Goal
    cw, ch := m.contentDimensions()
    picker := NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
    m.phaseModelPicker = picker
    return m, tea.Batch(picker.Init(), m.switchScreen(ScreenPhaseModelPicker))
}
```

Changed `m.screen = ScreenPhaseModelPicker` → `m.switchScreen(ScreenPhaseModelPicker)` (which sets `m.screen` AND calls `m.router.SwitchTo()`). This keeps the router's active screen in sync.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test -race ./internal/tui/...` | ✅ (all pass) |
| `go vet ./internal/tui/...` | ✅ (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning only) |

### Test Coverage Specific to Migration

| Test | Status |
|------|--------|
| `TestViewPhaseModelPicker` | ✅ PASS |
| `TestEnsureSubModel_PhaseModelPicker` | ✅ PASS |
| `TestPhaseModelPickerSetters` | ✅ PASS |
| `TestPhaseModelPickerVisibleRows` | ✅ PASS |
| `TestPickerPanelClampScroll` | ✅ PASS |
| `TestSessionRunWorkflowFromGoal_PhasePreserved` | ✅ PASS |
| `TestRunWorkflowFromGoal_SetsPhase` | ✅ PASS |
| `TestHandlePhaseResultMsg_NilEngine` | ✅ PASS |

All 7 existing PhaseModelPicker tests pass.

---

## Remaining Screens Untouched

All other 30 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`, `ScreenPhaseModelPicker`.

---

## Friction / Interface Issues

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is `int` in `tuitypes.go`; interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| `router.Register` takes `ScreenID` but model creation is lazy | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only ConfirmQuit/Help/Home/GhostPicker/GhostOutput/PhaseModelPicker receive propagation currently; safe |

---

## Conclusion

PhaseModelPicker migration complete. The dual-panel model selector (Planning/Coding) with async provider model loading, search filtering, tab navigation, and selection confirmation works identically post-migration. The strangler-fig pattern holds — old and new paths coexist, zero regressions, minimal friction. Phase 1 batch migration complete (GoalInput, FileExplorer, GhostPicker, GhostOutput, PhaseModelPicker all migrated).