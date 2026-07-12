# Screen Migration Report — CommandPalette, ModelSelector, Diff

**Date:** 2026-07-12
**Scope:** Migration of CommandPaletteScreenModel (524 lines), ModelSelector (293 lines), and DiffModel (127 lines) to new Screenable interface + Router architecture

---

## Summary

Successfully migrated all three low-coupling screens to the Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go warning only).

---

## CommandPalette Overlay Finding

**`CommandPaletteModel` in `cmdpalette.go` (476 lines) is an overlay popup, NOT a routed screen.**

Evidence:
- Has `Open()`/`Close()`/`IsOpen()` methods with a `visible` field (lines 141-157)
- Intercepted early in `renderFrameWithTheme()` at line 215: `if m.cmdPalette != nil && m.cmdPalette.IsOpen() { return m.cmdPalette.View() }` — returns before normal screen rendering
- Keyboard input intercepted at `app_input.go:160`: `if m.cmdPalette != nil && m.cmdPalette.IsOpen()`
- Not routed through `ScreenCommandPalette` — lives entirely outside the router

**This is architecturally identical to the Permission modal** (`ScreenPermission` in `routeKeyToScreen`, `renderFrameWithTheme`). Both are overlay popups that intercept input and render on top of the current screen without participating in the router's screen lifecycle.

**Not migrated.** `CommandPaletteModel` correctly stays as an overlay. The routed full-screen model (`CommandPaletteScreenModel` in `commandpalette_model.go`) is what was migrated.

---

## Changes Made

### 1. Screenable Interface Conformance

**CommandPaletteScreenModel** (`commandpalette_model.go:77`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

**ModelSelector** (`modelselector_model.go:117`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

**DiffModel** (`diff_model.go:102`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`
- No internal logic rewritten — pure boundary adaptation

### 2. Router Registration (`app_routing.go`)

All three screens follow the same lazy-init + router.Register pattern:

**ScreenDiff** (lines 163-174): Lazy init with `NewDiffModel` + `router.Register`
**ScreenCommandPalette** (lines 198-210): Lazy init with `NewCommandPaletteScreenModel` + `router.Register`
**ScreenModelSelector** (lines 293-305): Lazy init with `NewModelSelector` + `router.Register`

### 3. View Delegation to Router (`app_view.go`)

All three render functions updated to use router.View():

- `renderDiffContent` — lazy init + `m.router.Register` + `m.router.View()`
- `renderCommandPaletteContent` — lazy init + `m.router.Register` + `m.router.View()`
- `renderModelSelectorContent` — lazy init + `m.router.Register` + `m.router.View()`

Dual registration (Update updater + View renderer) — safe due to router's idempotent `Register`.

### 4. ModelSelector Overlay Rendering Preserved

`renderFrameWithTheme()` (lines 229-253) still renders ModelSelector as a centered dialog overlay on dimmed REPL background. This code path returns early before `renderScreenContent`, so the router.View() change in `renderModelSelectorContent` has no effect on the overlay path. The overlay calls `m.msModel.View()` directly on the concrete pointer.

### 5. ForwardTickToScreen Type Assertion

The `forwardTickToScreen` function in `app_handlers_tick.go:60-66` type-asserts the result of `msModel.Update()`. The assertion `newMS.(*ModelSelector)` remains valid — it's a standard interface type assertion on `Screenable` that succeeds when the underlying concrete type is `*ModelSelector`.

---

## Navigation Flow

### CommandPalette
1. **First navigation:** `routeToScreen()` creates `NewCommandPaletteScreenModel`, calls `Init()`
2. **Subsequent navigation:** `ensureSubModel()` resizes dimensions, calls `Init()`
3. **Update path:** `initScreenUpdaters` closure → model.Update(msg) → type-assert back
4. **View path:** `renderCommandPaletteContent` → router.View()
5. **Screen switch:** `navigateToScreen` → `switchScreen` → `m.router.SwitchTo(s)`

### ModelSelector
1. **First navigation:** `routeToScreen()` creates `NewModelSelector`, calls `Init()` (starts async model fetch)
2. **Subsequent navigation:** `ensureSubModel()` resizes dimensions, calls `Init()`
3. **Update path:** `initScreenUpdaters` closure → model.Update(msg) → type-assert back
4. **View path:** `renderModelSelectorContent` → router.View() (but `renderFrameWithTheme` returns early with overlay)
5. **Screen switch:** `navigateToScreen` → `switchScreen` → `m.router.SwitchTo(s)`
6. **Tick path:** `forwardTickToScreen` → model.Update(TickMsg) → type-assert back

### Diff
1. **First navigation:** `routeToScreen()` creates `NewDiffModel`, sets dimensions
2. **Subsequent navigation:** `ensureSubModel()` resizes dimensions
3. **Update path:** `initScreenUpdaters` closure → model.Update(msg) → type-assert back
4. **View path:** `renderDiffContent` → router.View()
5. **Screen switch:** `navigateToScreen` → `switchScreen` → `m.router.SwitchTo(s)`

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |
| `go vet ./internal/tui/...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |

---

## Files Changed

| File | Change |
|------|--------|
| `internal/tui/commandpalette_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/modelselector_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/diff_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/app_routing.go` | Lazy init + `router.Register` for ScreenDiff, ScreenCommandPalette, ScreenModelSelector |
| `internal/tui/app_view.go` | Lazy init + `router.Register` + `router.View()` for renderDiffContent, renderCommandPaletteContent, renderModelSelectorContent |

---

## Remaining Screens Untouched

All other screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`, `ScreenPhaseModelPicker`, `ScreenDiscuss`, `ScreenBisect`, `ScreenDashboard`, `ScreenPlan`, `ScreenConfig`, `ScreenGoalInput`, `ScreenFileExplorer`, `ScreenLedger`, `ScreenRollback`, `ScreenMetrics`, `ScreenToolDetail`, **ScreenDiff**, **ScreenCommandPalette**, **ScreenModelSelector** (20 of ~33 screens).

---

## Conclusion

All three low-coupling screens migrated successfully. The strangler-fig pattern holds — zero regressions, all data flows intact. The CommandPalette overlay model (`CommandPaletteModel` in `cmdpalette.go`) was correctly identified as an overlay (like Permission modal) and left untouched — only the routed full-screen model (`CommandPaletteScreenModel`) was migrated.

Total migrated screens: 20 of ~33 (61%).
