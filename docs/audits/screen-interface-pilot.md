# Screen Interface Pilot Report — ConfirmQuit Migration

**Date:** 2026-07-11  
**Scope:** Pilot migration of ConfirmQuit screen to new Screenable interface + Router architecture (strangler-fig pattern)

---

## Summary

Successfully migrated **ConfirmQuit** (85 lines) as the pilot screen for the new Screenable interface + Router architecture. All existing tests pass, vet and lint are clean. The old and new dispatch paths coexist (strangler-fig migration) — all other 32 screens still route through the original `app_routing.go` / `app_view.go` switches.

---

## Changes Made

### 1. New Interface: `internal/tui/screen.go`
```go
type Screenable interface {
    Init() tea.Cmd
    Update(msg tea.Msg) (Screenable, tea.Cmd)
    View() string
    SetDimensions(w, h int)
    SetTheme(theme.Theme)
}
```

### 2. New Router: `internal/tui/router.go`
- `Register(id ScreenID, s Screenable)` — registers a screen
- `SwitchTo(id ScreenID) tea.Cmd` — switches active screen, calls `Init()`
- `Update(msg tea.Msg) tea.Cmd` — delegates to active screen
- `View() string` — delegates to active screen
- `SetTheme`, `SetDimensions` — propagates to all registered screens

### 3. ConfirmQuitModel Adaptation (`confirmquit_model.go`)
- Changed `Update` return type from `tea.Model` to `Screenable` (1 line)
- All other methods (`Init`, `View`, `SetDimensions`, `SetTheme`) already satisfied the interface
- **No internal logic rewritten** — pure boundary adaptation

### 4. AppState Integration (`app_state.go`)
- Added `router *Router` field
- Initialized in `NewApp()` via `a.router = NewRouter()`
- Added `switchScreen(s Screen)` helper: updates `m.screen` and calls `router.SwitchTo(s)`
- Updated `popScreen()` to call `switchScreen(prev)` so router stays in sync

### 5. Routing Integration (`app_routing.go`)
- ConfirmQuit case in `initScreenUpdaters` now registers the model with the router on first creation:
  ```go
  if m.confirmQuitModel == nil {
      m.confirmQuitModel = NewConfirmQuitModel(...)
      m.router.Register(ScreenConfirmQuit, m.confirmQuitModel)
  }
  ```

### 6. View Integration (`app_view.go`)
- `renderConfirmQuitContent` now delegates to `m.router.View()` instead of calling model directly
- Registration happens lazily on first render if not already registered

### 7. Screen Switch (`app_update.go`)
- Ctrl+C double-tap handler now uses `m.switchScreen(ScreenConfirmQuit)` instead of direct `m.screen =` assignment

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test ./internal/tui/... ./internal/workflow/...` | ✅ (all pass) |
| `go test -race ./internal/tui/...` | ✅ |
| `go vet ./...` | ✅ |
| `golangci-lint run ./...` | ✅ (0 issues) |

---

## Manual Verification

The quit-confirmation flow was verified to behave identically:
1. Start the TUI (`go run ./cmd/m31a`)
2. Press `Ctrl+C` once → toast "Press ctrl+c again to exit (2s window)"
2. Press `Ctrl+C` again within 2s → ConfirmQuit dialog appears with "An operation is in progress. Are you sure you want to quit?"
3. Press `y` → application quits cleanly
4. Press `n` / `Esc` → returns to previous screen (REPL)

All behaviors match the pre-migration implementation.

---

## Remaining Screens Untouched

All other 32 screens still route through the original `app_routing.go` / `app_view.go` switch statements. The router only handles `ScreenConfirmQuit`. This is the intended strangler-fig pattern — no other screens were modified.

---

## Friction / Interface Issues for Next Batch

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is an `int` in `tuitypes.go`; our interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses the int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| `router.Register` takes `ScreenID` but model creation is lazy | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only ConfirmQuit receives propagation currently; safe |
| `popScreen` in `app_nav.go` | Still uses `m.screen = prev` directly | Already refactored to use `switchScreen` in this PR; good |

---

## Next Candidates (Phase 1 from inventory)

Per `docs/audits/screen-inventory.md`, the next low-risk screens to migrate:
1. **Help** (404 lines, only needs `keyRegistry` at init)
2. **Home** (248 lines, only needs `cmdRegistry` + `config`)
3. **GoalInput** (159 lines, standalone)
4. **FileExplorer** (142 lines, only `SetRoot()` + `SetDimensions()`)
5. **GhostPicker / GhostOutput** (193+178 lines, isolated feature)
6. **PhaseModelPicker** (337 lines, 7 tests, clean dual-model selection UI)

---

## Conclusion

The Screenable + Router pattern works end-to-end for ConfirmQuit. The strangler-fig migration is viable: old and new paths coexist, zero regressions, minimal friction. Ready to proceed with Phase 1 batch migration.