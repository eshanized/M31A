# Screen Migration Report — ShipModel (ScreenShip)

**Date:** 2026-07-13
**Scope:** Migration of Ship screen to Screenable interface + Router architecture (Phase 3, screen 1 of 6)

---

## Summary

Migrated **ShipModel** (204 lines) to the new Screenable interface + Router architecture, following the strangler-fig pattern established by the ConfirmQuit pilot. Ship was chosen as the first Phase 3 screen because it has the simplest data flow: `SetDemonstration()` is called from exactly two Update()-path handlers with a value passed in from phase results, no fan-out mutation across multiple handlers.

---

## SetDemonstration Verification

Confirmed that `SetDemonstration()` is **not** reading live engine state. It is called from:

| Call site | File:Line | Context |
|-----------|-----------|---------|
| `handleDemonstrationReadyMsg` | `handler_workflow.go:68` | Receives `DemonstrationReadyMsg` from workflow engine — value passed via message |
| `handlePhaseResult` (PhaseShip case) | `app_update_phase.go:318` | Receives `msg.Demonstration` from `PhaseResultMsg` — value passed via message |

Both are Update()-path handlers. The demonstration content arrives as a string value in a message, not by reading `workflowEngine.Demonstration()` or any live engine field. No change needed.

---

## Changes Made

### 1. ShipModel adapts to Screenable interface (`ship_model.go`)

Added three new methods and changed the `Update` return type:

```go
func (sm *ShipModel) Init() tea.Cmd { return nil }

func (sm *ShipModel) SetDimensions(w, h int) {
    sm.width = w
    sm.height = h
    sm.demoViewport = viewport.New(w-4, h-8)
    if sm.demonstration != "" {
        sm.demoViewport.SetContent(sm.demonstration)
    }
}

func (sm *ShipModel) SetTheme(t theme.Theme) { sm.theme = t }
```

Changed `Update` signature from `(tea.Msg) (*ShipModel, tea.Cmd)` to `(tea.Msg) (Screenable, tea.Cmd)`. Internal logic unchanged — all `return sm` statements are compatible because `*ShipModel` satisfies `Screenable`.

### 2. Router registration in screenUpdaters (`app_routing.go:118-129`)

Before:
```go
m.screenUpdaters[ScreenShip] = func(msg tea.Msg) tea.Cmd {
    if m.shipModel == nil {
        return nil
    }
    newModel, cmd := m.shipModel.Update(msg)
    m.shipModel = newModel
    return cmd
}
```

After:
```go
m.screenUpdaters[ScreenShip] = func(msg tea.Msg) tea.Cmd {
    if m.shipModel == nil {
        cw, ch := m.contentDimensions()
        m.shipModel = NewShipModel(ShipSummary{}, m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenShip, m.shipModel)
    }
    newModel, cmd := m.shipModel.Update(msg)
    if r, ok := newModel.(*ShipModel); ok {
        m.shipModel = r
    }
    return cmd
}
```

### 3. View delegation through router (`app_view.go:620-628`)

Before:
```go
func (m *AppState) renderShipContent(chrome layout.PageChrome) string {
    if m.shipModel == nil {
        return renderEmptyState(...)
    }
    m.shipModel.width = chrome.ContentWidth()
    m.shipModel.height = chrome.ContentHeight()
    return m.shipModel.View()
}
```

After:
```go
func (m *AppState) renderShipContent(chrome layout.PageChrome) string {
    if m.shipModel == nil {
        m.shipModel = NewShipModel(ShipSummary{}, m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        m.router.Register(ScreenShip, m.shipModel)
    }
    m.shipModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

### 4. Screen switching via switchScreen (`app_update_phase.go:316`, `app_input.go:338`)

Replaced `m.screen = ScreenShip` with `m.switchScreen(ScreenShip)` in two locations:

- `app_update_phase.go:316` — PhaseShip result handler
- `app_input.go:338` — `runtime_continue` action handler

### 5. Tick handler type assertion (`app_handlers_tick.go:82-86`)

Added type assertion to match the return type change:
```go
if m.screen == ScreenShip && m.shipModel != nil {
    newShip, cmd := m.shipModel.Update(msg)
    if r, ok := newShip.(*ShipModel); ok {
        m.shipModel = r
    }
    cmds = append(cmds, cmd)
}
```

---

## Files Changed

| File | Change |
|------|--------|
| `internal/tui/ship_model.go` | Added `Init`, `SetDimensions`, `SetTheme`; changed `Update` return type |
| `internal/tui/app_routing.go` | Lazy-create + register with router in screenUpdaters closure |
| `internal/tui/app_view.go` | `renderShipContent` delegates to `m.router.View()` |
| `internal/tui/app_update_phase.go:316` | `m.screen =` → `m.switchScreen()` |
| `internal/tui/app_input.go:338` | `m.screen =` → `m.switchScreen()` |
| `internal/tui/app_handlers_tick.go:82-86` | Added type assertion for `Screenable` → `*ShipModel` |

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go vet ./...` | PASS (pre-existing emitter_stress_test.go copylocks warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |
| `go test -race ./internal/tui/... -count=1` | PASS (all 8 sub-packages) |

---

## Data Flow After Migration

```
Workflow engine completes Ship phase
  → PhaseResultMsg{Phase: PhaseShip, Demonstration: "..."}
    → handlePhaseResult (Update path)
      → m.switchScreen(ScreenShip)  [sets m.screen + router.SwitchTo]
      → m.shipModel.SetDemonstration(...)  [passes value to model]
  → View()
    → renderShipContent()
      → m.router.View()  [delegates to registered ShipModel.View()]
```

The demonstration content flows: Engine → PhaseResultMsg → Update() handler → SetDemonstration() → View() reads cached field. No View()-boundary violations.

---

## Remaining Phase 3 Screens

| Screen | Lines | Complexity | Notes |
|--------|-------|------------|-------|
| **Verify** | 337 | Medium | Healing integration, spinner state |
| **Execute** | 336 | High | Task list, live output, pause/resume |
| **Plan** | 385 | High | Task list, refinement, phase coupling |
| **REPL** | ~1300+ | Highest | Streaming, tool calls, sidebar coupling |
| **Sidebar** | 1621 | Highest | Central event hub, touches everything |
