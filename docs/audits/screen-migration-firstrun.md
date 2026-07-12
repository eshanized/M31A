# Screen Migration Report — FirstRun (ScreenFirstRun)

**Date:** 2026-07-12
**Scope:** Migration of FirstRunModel (780 lines, 12 existing tests) to Screenable + Router architecture

---

## Summary

Successfully migrated FirstRunModel to the Screenable interface + Router architecture. This is the largest screen migrated so far and the first with real filesystem/keychain side effects. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go warning only).

---

## Changes Made

### 1. Screenable Interface Conformance

**FirstRunModel** (`firstrun_model.go:245`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` to `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Changed `handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd)` to `handleKey(msg tea.KeyMsg) (Screenable, tea.Cmd)`
- Changed `handleMouse(msg tea.MouseMsg) (tea.Model, tea.Cmd)` to `handleMouse(msg tea.MouseMsg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions()`, `SetTheme()`, `SetContentWidth()`
- No internal logic rewritten — pure boundary adaptation

### 2. Router Registration (`app_routing.go`)

`screenUpdaters[ScreenFirstRun]` (lines 336-350) now registers with the router on the first message:

```go
m.screenUpdaters[ScreenFirstRun] = func(msg tea.Msg) tea.Cmd {
    if m.firstRunModel == nil {
        return nil
    }
    if m.router != nil {
        m.router.Register(ScreenFirstRun, m.firstRunModel)
    }
    newModel, cmd := m.firstRunModel.Update(msg)
    if r, ok := newModel.(*FirstRunModel); ok {
        m.firstRunModel = r
    }
    return cmd
}
```

Note: The nil guard on `m.router` is needed because some tests create AppState without a router.

### 3. View Delegation to Router (`app_view.go`)

`renderFirstRunContent` (lines 650-663) now registers with router and delegates:

```go
func (m *AppState) renderFirstRunContent(chrome layout.PageChrome) string {
    if m.firstRunModel == nil {
        return renderLoading("Loading first-run wizard…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
    }
    m.firstRunModel.SetContentWidth(chrome.ContentWidth())
    if m.router != nil {
        m.router.Register(ScreenFirstRun, m.firstRunModel)
        if m.router.ActiveID() != ScreenFirstRun {
            m.router.SwitchTo(ScreenFirstRun)
        }
        return m.router.View()
    }
    return m.firstRunModel.View()
}
```

Key design decisions:
- **Kept the nil-model guard** returning `renderLoading()`. The model is created in `routeToScreen()`/`ensureSubModel()` (which have access to `config`, `registry`, `ctx`). Creating it in the render function would require passing these dependencies, which the render function doesn't have.
- **Added `m.router != nil` guard** because some tests create AppState without a router. This matches the pattern used by `renderSettingsContent`.
- **Added `ActiveID() != ScreenFirstRun` check** before `SwitchTo` to avoid calling `Init()` (which returns nil but still does setup work) on every render frame.
- **Falls back to `m.firstRunModel.View()`** when router is nil (test environments).

### 4. No Changes to Navigation

`routeToScreen()` and `ensureSubModel()` in `app_nav.go` were NOT modified. They continue to:
- Create the model with full dependencies (theme, registry, config, version, ctx)
- Set dimensions
- Call Init()

This follows the same pattern as GoalInput and other migrated screens where model creation stays in the navigation layer.

### 5. No Changes to Resize/Theme Handlers

`app_input.go` resize handler (line 94-96) and theme handler (line 429-431) continue to call `m.firstRunModel.SetDimensions()` and `m.firstRunModel.SetTheme()` directly on the concrete pointer. These are idempotent and work correctly with the router registration.

---

## Navigation Flow

1. **First navigation:** `navigateToScreen(ScreenFirstRun)` → `routeToScreen()` creates `NewFirstRunModel(m.themeManager.Current(), m.registry, m.config, m.version, m.shutdownCtx)`, sets dimensions, calls `Init()`
2. **Subsequent navigation:** `ensureSubModel(ScreenFirstRun)` always recreates the model (no nil check — intentional for wizard reset)
3. **Update path:** `screenUpdaters[ScreenFirstRun]` closure → model.Update(msg) → type-assert back
4. **View path:** `renderFirstRunContent` → `m.router.Register` + `m.router.View()`
5. **Screen switch:** `navigateToScreen` → `switchScreen` → `m.router.SwitchTo(s)`

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go vet ./internal/tui/...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |
| `firstrun_extra_test.go` (12 tests) | PASS (all 12) |
| `TestViewFirstRun` | PASS |
| `TestEnsureSubModel_FirstRun` | PASS |
| `TestHandleFirstRunCompleteMsg_NilFirstRunModel` | PASS |

---

## Manual Verification: Wizard Flow & Keychain/Config Side Effects

### Wizard Flow Traced

The first-run wizard is a 5-step flow:
1. **Welcome** (stepWelcome) — display only, no side effects
2. **Provider Select** (stepProviderSelect) — multi-select UI, no side effects
3. **API Key Entry** (stepAPIKey) — validates key format, stores in `opts.Providers`
4. **Model Pick** (stepModelPick) — fetches models, user selects, stores in `opts.ModelID`
5. **Done** (stepDone) — emits `FirstRunCompleteMsg`

### Keychain/Config Write Path

The critical side effects happen in `handleFirstRunComplete()` (in `app_handlers_config.go`), which is called when `FirstRunCompleteMsg` is received:

```go
func (m *AppState) handleFirstRunComplete(msg FirstRunCompleteMsg) tea.Cmd {
    // 1. Registers each provider with its API key
    for _, entry := range msg.Providers {
        if err := RegisterProvider(m.config, m.registry, entry.ID, entry.APIKey, m.version); err != nil {
            // error handling
        }
    }
    // 2. Saves to keychain (if SaveKeychain=true) or config file
    // 3. Navigates to ScreenREPL
}
```

`RegisterProvider` (in `provider.go`) follows the established fallback chain:
1. **OS keychain** (`pkg/keychain/`) — tried first when `SaveKeychain=true`
2. **Config file** (`config.toml`) — written as plaintext only when keychain unavailable
3. **Environment variable** — checked at runtime, never written

**This path is completely unchanged by the migration.** The `FirstRunCompleteMsg` is emitted by `FirstRunModel.completeSetup()` as a `tea.Msg`, which is handled by `AppState.Update()` → `handleFirstRunCompleteMsg()` → `handleFirstRunComplete()`. The migration only changed the Update/View boundary — the message handling pipeline is identical.

### What Changed vs. What Didn't

| Aspect | Changed? | Notes |
|--------|----------|-------|
| `Update()` return type | Yes | `(tea.Model, tea.Cmd)` → `(Screenable, tea.Cmd)` |
| `handleKey()` return type | Yes | Same change |
| `handleMouse()` return type | Yes | Same change |
| Router registration | Yes | Added in screenUpdater and renderFirstRunContent |
| View delegation | Yes | Now goes through `m.router.View()` |
| `completeSetup()` | No | Still emits `FirstRunCompleteMsg` |
| `handleFirstRunComplete()` | No | Still calls `RegisterProvider` and saves to keychain/config |
| `RegisterProvider()` | No | Still uses keychain → config fallback chain |
| Provider registration flow | No | `FirstRunCompleteMsg` → `handleFirstRunComplete` → `RegisterProvider` |
| Config file writes | No | Still written through same path |
| Keychain writes | No | Still written through same path |
| Model fetching | No | `fetchModelsCmd()` unchanged |

---

## Files Changed

| File | Change |
|------|--------|
| `internal/tui/firstrun_model.go` | `Update`, `handleKey`, `handleMouse` return types: `tea.Model` → `Screenable` |
| `internal/tui/app_routing.go` | Added `m.router.Register(ScreenFirstRun, m.firstRunModel)` in screen updater |
| `internal/tui/app_view.go` | `renderFirstRunContent`: added router registration + `m.router.View()` delegation |

---

## Conclusion

FirstRunModel migrated successfully. The strangler-fig pattern holds — the Screenable boundary adaptation is clean, the router integration follows the established pattern, and all keychain/config side effects flow through the unchanged `FirstRunCompleteMsg` → `handleFirstRunComplete()` → `RegisterProvider()` pipeline. The 780-line wizard with 12 existing tests passes without regressions.
