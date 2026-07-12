# ModelSelector Overlay Resolution

**Date:** 2026-07-12
**Scope:** Resolve inconsistent overlay classification for ModelSelector after CommandPalette was correctly excluded from Screenable/Router migration

---

## Finding: ModelSelector Is Purely Overlay-Shaped

ModelSelector is **never rendered as a routed full-screen view**. It is always shown as a centered dialog overlay on a dimmed REPL background. The Screenable/Router registration applied to it was dead code for View() and misleading to future readers.

### Evidence

**1. The overlay intercepts rendering before `renderScreenContent` is called.**

In `app_view.go:renderFrameWithTheme()` (lines 229-253):

```go
if m.screen == ScreenModelSelector && m.msModel != nil {
    // ... sizing code ...
    modalContent := m.msModel.View()
    modal := lipgloss.NewStyle().Border(...).Render(modalContent)
    if result := m.renderDimmedModal(modal, t); result != "" {
        return result  // <-- returns complete frame; renderScreenContent never reached
    }
}
```

`renderDimmedModal` renders the REPL as a background via `m.replModel.ViewContent()`, then overlays the ModelSelector modal on top via `layout.RenderModalOverlay()`. It returns a complete frame. The `return result` exits `renderFrameWithTheme` entirely.

**2. `renderModelSelectorContent` is unreachable in normal operation.**

This function (`app_view.go:569-577`) was only reached via `renderScreenContent` -> `renderActiveScreen`, which is called at `renderFrameWithTheme:271`. But the overlay block returns at line 251 before line 271 is ever reached. The function was dead code.

**3. All three entry points route through the overlay.**

| Entry Point | Code Path | Overlay? |
|-------------|-----------|----------|
| `app_input.go:318` "cycle_model" | `navigateToScreen(ScreenModelSelector)` -> `switchScreen` -> sets `m.screen` -> `View()` -> overlay | Yes |
| `app_input.go:320` "cycle_model_backward" | Same as above | Yes |
| `commands/commands_ai.go:170` /model | Returns `ScreenModelSelector` screen constant -> same nav path | Yes |

There is no code path that shows ModelSelector without the overlay.

**4. The overlay path calls `m.msModel.View()` directly on the concrete pointer.**

The overlay at line 242 calls `m.msModel.View()` — not `m.router.View()`. The router was never consulted for rendering. This is the same pattern that got CommandPalette excluded: the overlay renders directly on the concrete model, bypassing the router entirely.

**5. `router.Update()` is never called.**

The router's `Update()` method (`router.go:59`) is never invoked anywhere in the codebase. All message dispatch goes through the `screenUpdaters` map in `app_routing.go`, which calls `m.msModel.Update(msg)` directly on the concrete pointer.

**6. `router.SwitchTo()` Init() call is redundant.**

`switchScreen` (`app_state.go:473-477`) calls `m.router.SwitchTo(s)`, which calls `r.active.Init()`. But `Init()` was already called by `ensureSubModel` (`app_nav.go:314`). The router's Init() call is a no-op.

---

## Decision: Option A — Back Out Router Registration

**Option A was chosen** because:
- The router registration did nothing for View() (overlay intercepts first)
- The router registration did nothing for Update() (screenUpdaters bypass the router)
- Keeping it would mislead future readers into thinking the router controls ModelSelector rendering
- The overlay pattern is architecturally identical to CommandPalette, which was correctly excluded

---

## Changes Made

### 1. `internal/tui/app_view.go` — `renderModelSelectorContent`

Removed `m.router.Register(ScreenModelSelector, m.msModel)`. Changed `return m.router.View()` to `return m.msModel.View()`. Added comment explaining the overlay intercepts before this function is called.

### 2. `internal/tui/app_routing.go` — `screenUpdaters[ScreenModelSelector]`

Removed `m.router.Register(ScreenModelSelector, m.msModel)` from the lazy-init block. Added comment explaining ModelSelector is an overlay and the router registration is unnecessary.

### 3. `internal/tui/app_view.go` — overlay render site (lines 228-233)

Added comment explaining that this overlay path returns before `renderScreenContent`, so the router never controls ModelSelector's View(). Explicitly states this is intentional and should not be "fixed."

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go vet ./...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |

---

## Comparison: CommandPalette vs ModelSelector

| Aspect | CommandPalette (overlay) | ModelSelector (overlay) |
|--------|--------------------------|------------------------|
| Overlay model | `CommandPaletteModel` (cmdpalette.go) | `ModelSelector` (modelselector_model.go) |
| Routed model | `CommandPaletteScreenModel` (commandpalette_model.go) | None (no separate routed model) |
| Overlay intercept | `renderFrameWithTheme:215` `cmdPalette.IsOpen()` | `renderFrameWithTheme:229` `m.screen == ScreenModelSelector` |
| Router registration | Kept for routed model (dead code too, but separate model exists) | **Removed** (no separate routed model) |
| Screen constant | `ScreenCommandPalette` — maps to routed model | `ScreenModelSelector` — maps to overlay model |

The inconsistency is now resolved: both overlays are handled the same way, with the overlay rendering directly on the concrete pointer and the router not controlling View().

---

## Note on ScreenCommandPalette

`ScreenCommandPalette` still has router registration for `CommandPaletteScreenModel` in `app_routing.go` and `app_view.go`. This is also effectively dead code (the overlay intercepts first), but it was left alone per the task scope ("Do not touch CommandPalette"). A follow-up could apply the same cleanup.
