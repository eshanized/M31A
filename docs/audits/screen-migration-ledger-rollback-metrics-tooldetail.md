# Screen Migration Report — Ledger, Rollback, Metrics, ToolDetail

**Date:** 2026-07-12  
**Scope:** Migration of LedgerModel (156 lines), RollbackModel (288 lines), MetricsModel (248 lines), and ToolDetailModel (92 lines) to new Screenable interface + Router architecture (strangler-fig pattern, following ConfirmQuit pilot and Plan migration)

---

## Summary

Successfully migrated all four low-coupling screens to the Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go warning only).

---

## Changes Made

### 1. Screenable Interface Conformance

**LedgerModel** (`ledger_model.go:69`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

**RollbackModel** (`rollback_model.go:74`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

**MetricsModel** (`metrics_model.go:124`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Updated doc comments from `tea.Model` to `Screenable`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

**ToolDetailModel** (`tooldetail_model.go:59`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

### 2. Router Registration (`app_routing.go`)

All four screens follow the same lazy-init + router.Register pattern established by ConfirmQuit:

**ScreenLedger** (lines 126-138):
```go
m.screenUpdaters[ScreenLedger] = func(msg tea.Msg) tea.Cmd {
    if m.ledgerModel == nil {
        m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
        m.ledgerModel.LoadEntries()
        m.router.Register(ScreenLedger, m.ledgerModel)
    }
    newModel, cmd := m.ledgerModel.Update(msg)
    if r, ok := newModel.(*LedgerModel); ok {
        m.ledgerModel = r
    }
    return cmd
}
```

**ScreenRollback** (lines 139-151): Same pattern with `NewRollbackModel` + `LoadCommits()`

**ScreenMetrics** (lines 152-163): Same pattern with `NewMetricsModel` (async load via `LoadStatsCmd` handled in nav)

**ScreenToolDetail** (lines 181-193): Same pattern with `NewToolDetailModel`

### 3. View Delegation to Router (`app_view.go`)

All four render functions updated to use router.View():

- `renderLedgerContent` — lazy init + `m.router.Register` + `m.router.View()`
- `renderRollbackContent` — lazy init + `m.router.Register` + `m.router.View()`
- `renderMetricsContent` — lazy init + `m.router.Register` + `m.router.View()`
- `renderToolDetailContent` — lazy init + `m.router.Register` + `m.router.View()`

Dual registration (Update updater + View renderer) — safe due to router's idempotent `Register`.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |
| `go vet ./internal/tui/...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |

---

## ToolDetail Sidebar Handoff Verification

**Location:** `handler_modal.go:79-83`

```go
m.ensureToolDetailModel()
title, body := m.extractToolDetail(msg.MessageIndex, msg.ToolName)
if title != "" {
    m.toolDetailModel.SetContent(title, body)
    return m, m.navigateToScreen(ScreenToolDetail)
}
```

**Verification:**
- `m.toolDetailModel` is a concrete `*ToolDetailModel` reference in `AppState` — unchanged by migration
- `SetContent()` is called directly on the concrete pointer before navigation — identical to the Plan→Execute handoff pattern documented in `screen-migration-plan.md`
- `navigateToScreen(ScreenToolDetail)` triggers `switchScreen` which calls `m.router.SwitchTo(ScreenToolDetail)` — router delegates to the registered ToolDetailModel
- **Handoff is identical** — data is set on the concrete pointer, then screen switches via router

---

## Navigation Flow (All Four Screens)

All four screens follow the same navigation lifecycle:

1. **First navigation:** `routeToScreen()` / `ensureSubModel()` creates the model, calls data loaders (`LoadEntries`, `LoadCommits`, `LoadStatsCmd`), and returns `Init()` or nil
2. **Subsequent navigation:** `ensureSubModel()` resizes dimensions only
3. **Update path:** `initScreenUpdaters` closure → model.Update(msg) → type-assert back to concrete pointer
4. **View path:** `renderXContent` → router.View() (delegates to registered screen)
5. **Screen switch:** `navigateToScreen` → `switchScreen` → `m.router.SwitchTo(s)`

---

## Files Changed

| File | Change |
|------|--------|
| `internal/tui/ledger_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/rollback_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/metrics_model.go` | `Update` return type: `tea.Model` → `Screenable`, doc comments updated |
| `internal/tui/tooldetail_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/app_routing.go` | Lazy init + `router.Register` for all 4 screens |
| `internal/tui/app_view.go` | Lazy init + `router.Register` + `router.View()` for all 4 render functions |

---

## Remaining Screens Untouched

All other screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`, `ScreenPhaseModelPicker`, `ScreenDiscuss`, `ScreenBisect`, `ScreenDashboard`, `ScreenPlan`, `ScreenConfig`, `ScreenGoalInput`, `ScreenFileExplorer`, **ScreenLedger**, **ScreenRollback**, **ScreenMetrics**, **ScreenToolDetail** (17 of ~33 screens).

---

## Conclusion

All four low-coupling screens migrated successfully. The strangler-fig pattern holds — zero regressions, all data flows intact. The ToolDetail sidebar handoff works identically to the established Plan→Execute pattern: concrete model pointer in AppState, data set before navigation, router handles rendering.

Total migrated screens: 17 of ~33 (52%).
