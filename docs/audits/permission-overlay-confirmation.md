# Permission Overlay Confirmation

**Date:** 2026-07-13
**Scope:** Confirm ScreenPermission is overlay-only (same shape as ModelSelector) and document it accordingly

---

## Finding: ScreenPermission Is Purely Overlay-Shaped

ScreenPermission is **never rendered as a routed full-screen view**. It is always shown as a modal overlay (permission prompt or question modal) on a dimmed REPL background. It has never had `router.Register()` — it has been legacy from the start.

---

## Render Intercept Point

In `app_view.go:renderFrameWithTheme()` (lines 218-225):

```go
// Permission/question modal (top priority overlay).
// This overlay path returns before renderScreenContent, so the router never
// controls Permission's View(). The modal is rendered directly on the
// concrete pointer. This is intentional — Permission is a modal overlay,
// not a full-screen routed view. It has no router.Register() and never has.
// Do not "fix" this by removing the early return or adding router registration.
if m.screen == ScreenPermission {
    modalContent := m.renderPermissionModalContent()
    if result := m.renderDimmedModal(modalContent, t); result != "" {
        return result  // <-- returns complete frame; renderScreenContent never reached
    }
    return m.renderPermissionModal()
}
```

This is the **second-highest priority** overlay (after CommandPalette at line 214). The overlay renders the permission modal or question modal centered on a dimmed REPL background via `renderDimmedModal`, then returns a complete frame. `renderScreenContent` is never reached.

---

## Input Handling Path

In `app_routing.go:routeKeyToScreen()` (lines 487-499):

```go
func (m *AppState) routeKeyToScreen(msg tea.KeyMsg) tea.Cmd {
    switch m.screen {
    case ScreenPermission:
        return m.handlePermissionKey(msg)
    default:
        if fn, ok := m.screenUpdaters[m.screen]; ok {
            return fn(msg)
        }
    }
    return nil
}
```

This is the **only remaining `case Screen*`** in the Update switch in `app_routing.go`. All other screens route through `screenUpdaters`. ScreenPermission has a dedicated case because it intercepts keyboard input before the normal screen dispatch — consistent with its overlay nature.

---

## Router Registration: Never Has, Never Needed

- **Grep for `router.Register(ScreenPermission`** across all `.go` files: **0 results**
- **Git history search** (`git log --all -p -S 'router.Register(ScreenPermission'`): **0 results**
- ScreenPermission has never had `router.Register()` in the codebase. This is not a migration gap — it is the correct state for an overlay.

---

## Entry Points

Two places set `m.screen = ScreenPermission`:

| File:Line | Context | Handler |
|-----------|---------|---------|
| `handler_tool.go:24` | Permission request from tool execution | `handlePermissionRequestMsg` (Update-path) |
| `handler_tool.go:51` | Question request from tools | `handleQuestionRequestMsg` (Update-path) |

Both are Update()-path handlers that receive a message, set up the modal model, and switch the screen. The modal content is set via `m.permModal` and `m.questionModel`, not read from any live engine state.

---

## Navigation Behavior

`app_nav.go:254` and `app_nav.go:267` treat ScreenPermission as an overlay:
- ScreenPermission is excluded from the screen back-stack (line 254)
- Screen transitions are skipped for ScreenPermission (line 267)

This confirms Permission is not a navigable screen — it is a transient overlay.

---

## Comparison: Permission vs ModelSelector

| Aspect | ModelSelector (overlay) | Permission (overlay) |
|--------|------------------------|---------------------|
| Render intercept | `renderFrameWithTheme:232` | `renderFrameWithTheme:219` (higher priority) |
| Router registration | Removed (was dead code) | Never had any |
| Update dispatch | `screenUpdaters` map | `routeKeyToScreen` switch (only remaining case) |
| Entry points | "cycle_model" actions | Permission/question request messages |
| Navigation | `navigateToScreen` | Direct `m.screen =` assignment (no stack push) |
| Modal rendering | `m.msModel.View()` direct | `m.renderPermissionModalContent()` / `m.renderPermissionModal()` |

Both are overlays. Permission is the simpler case — it has no router registration history and its input path is a dedicated switch case rather than a `screenUpdaters` entry.

---

## Changes Made

### 1. `internal/tui/app_view.go:218-225` — Explanatory comment

Added a comment at the overlay intercept site explaining this is intentional and should not be "fixed." Matches the comment style added for ModelSelector at lines 228-231.

### 2. `docs/audits/router-migration-status.md` — ScreenPermission annotation

Updated the "Why Not Yet Migrated" column for ScreenPermission (enum value 5) from the generic "Modal overlay; not a full screen" to a precise explanation: "Overlay-only — same shape as ModelSelector. Render intercept at app_view.go:221 returns before renderScreenContent. Never had router.Register(). See permission-overlay-confirmation.md."

This was a manual prose edit to a mechanically-generated file. This is acceptable because:
- The registered/legacy determination itself stays mechanically grepped (unchanged)
- Only the "why" column is prose, and the mechanical truth (no router.Register) is preserved
- The new text references the confirmation report for full evidence

---

## Implication for Phase 3 Migration

ScreenPermission should **not** be migrated to Screenable + Router. It is permanently classified as an overlay, same as ModelSelector. The `routeKeyToScreen` switch case in `app_routing.go` is the correct input dispatch mechanism for this overlay — it is not a migration gap.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go test -run TestViewPermission ./internal/tui/...` | PASS |
