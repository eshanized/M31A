## Diagnostic Report: Main Content Area Not Rendering

### 1. Top-level View() composition

**File:** `internal/ui/tui/app_view.go:217-303` (`renderFrameWithTheme`)

The composition is structurally correct. The flow is:
1. `buildSidebarAndChrome(m.screen)` → produces `PageChrome` + sidebar string (line 280)
2. `renderActiveScreen(chrome)` → dispatches to per-screen renderer (line 283)
3. `layout.RenderPage(chrome, content, headerInfo, footerInfo, ...)` → composes header + content + footer (line 300)
4. `applySidebar(main, sidebarStr, m.screen)` → `lipgloss.JoinHorizontal(sidebarStr, main)` (line 302)

The main pane IS included in the composition. However, **the content string returned by `renderScreenContent` is empty for every screen that delegates through the Router** (see finding #3 below).

### 2. Sizing / WindowSizeMsg propagation

**File:** `internal/ui/tui/app_input_resize.go:12-148`

`handleWindowResize` correctly forwards `WindowSizeMsg` to all sub-models including the REPL. Width/height math is correct with `contentW -= sw` guarded by `contentW < 1` clamp. No zero/negative dimension paths found. The viewport is resized correctly in `renderREPLContent` via `syncReplSize`. **No issue here.**

### 3. Screen/phase state routing — **ROOT CAUSE FOUND**

**File:** `internal/ui/tui/router.go:46-58` (`SwitchTo`) and `router.go:30-42` (`Register`)

The `Router.SwitchTo` method has a critical ordering bug. When called before a screen is registered, `activeID` is never set, causing `Register`'s auto-activate guard to permanently fail.

**The exact failure chain:**

1. `Init()` (`app.go:44`) calls `m.switchScreen(ScreenHome)` which calls `m.router.SwitchTo(ScreenHome)`
2. `SwitchTo` (`router.go:50`) checks `r.screens[id]` — ScreenHome is **not yet registered** (homeModel doesn't exist), so the entire body is skipped. **`r.activeID` stays at 0** (zero value = `ScreenFirstRun`).
3. Later, `renderHomeContent` (`app_screens.go:393-401`) creates HomeModel and calls `m.router.Register(ScreenHome, m.homeModel)`
4. `Register` (`router.go:39`) checks the auto-activate guard: `r.activeID == id` → `0 == 32` → **FALSE**. `r.active` is never set.
5. `Router.View()` (`router.go:62`) returns `""` because `r.active == nil`
6. **Result:** `renderHomeContent` returns `""`, producing an empty content area

The comment on `Register` (line 37-38) explicitly acknowledges this race: *"Auto-activate if this screen was SwitchTo'd before Register was called (e.g. during Init where switchScreen runs before routeToScreen)."* But the implementation is flawed — `SwitchTo` only updates `activeID` inside the `if r.screens[id]` guard, which fails when the screen isn't registered yet.

**This affects ALL router-delegated screens:** Home, Plan, Execute, Verify, Ship, Discuss, Settings, Help, Resume, GoalInput, Ledger, Rollback, Metrics, Config, Diff, Bisect, Notifications, Dashboard, SessionDetail, FileExplorer, ToolDetail, PhaseModelPicker, GhostPicker, GhostOutput, ConfirmQuit, ChatHistory, CommandPalette, Decisions, PhaseTransition.

The only screen unaffected is **ScreenREPL** — `renderREPLContent` (`app_screens.go:19-50`) calls `m.replModel.ViewContent()` directly, bypassing the Router entirely.

**Confidence: HIGH** — This is a deterministic bug in the initialization sequence. The `activeID` is never updated for screens that aren't yet registered, so `Register`'s auto-activate guard permanently fails.

### 4. Message/command flow

The `routeToScreen()` Cmd (`app_nav.go:21-326`) creates models and registers them with the router, but does NOT re-invoke `SwitchTo` after registration. By the time `Register` runs, the `activeID` is stale (still 0). The Cmd executes asynchronously, after the first `View()` call, so the initial render already produces empty content.

**Confidence: HIGH** — This is a consequence of finding #3.

### 5. Component registration

All screen components are registered lazily (on first render or first navigation). The registration paths in `renderScreenContent` → per-screen renderers and `ensureSubModel` are complete. Every `Screen` constant in the `renderScreenContent` switch (`app_view.go:122-192`) has a corresponding branch. **No missing wire-ups.**

### 6. Recent regression check

```
546f3b64 fix(01-02): engine planMu races (B12/B13), repl program wiring (B23), dispatcher collector lock (B24)
18a823a3 refactor(03-03): consolidate test files into tests/ directory per D-11/D-12
5a4c64eb refactor(core): reorder and group import statements across multiple packages
42d84f82 feat(01-05): update all remaining import paths and fix pre-existing test issues
8ca46e1b feat(01-04): move tui package to internal/ui/tui/
```

The router was introduced in `8ca46e1b` (the package move commit). The bug has been present since the router was introduced — it's not a regression from a specific commit, but a latent design flaw in the Router's initialization ordering.

---

## Summary

| # | Finding | File:Line | Confidence |
|---|---------|-----------|------------|
| 1 | `Router.SwitchTo` doesn't set `activeID` when screen isn't registered | `router.go:50` | **HIGH** |
| 2 | `Router.Register` auto-activate guard fails because `activeID` is stale | `router.go:39` | **HIGH** |
| 3 | `Router.View()` returns `""` because `r.active` is permanently nil | `router.go:62-65` | **HIGH** |
| 4 | All router-delegated screens produce empty content | `app_screens.go` (all render functions calling `m.router.View()`) | **HIGH** |
| 5 | ScreenREPL is unaffected (bypasses router) | `app_screens.go:19-50` | MEDIUM |

## Most Likely Root Cause

**`Router.SwitchTo()` (`router.go:50`) fails to set `activeID` when the target screen hasn't been registered yet.** The `activeID` stays at 0, so `Router.Register()`'s auto-activate guard (`router.go:39: r.activeID == id`) permanently evaluates to false. `r.active` is never set, and `Router.View()` returns `""` for every screen.

The fix is a one-line change: `SwitchTo` should **always** set `r.activeID = id` (outside the `if s, ok` guard), so that the subsequent `Register` call's auto-activate condition succeeds.
