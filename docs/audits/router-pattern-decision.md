# Router Pattern Decision

**Date:** 2026-07-12
**Scope:** Formalize the actual proven Router architecture vs. the originally specified one

---

## Context

The `screen-interface-pilot.md` (2026-07-11) introduced the Screenable interface + Router architecture with four methods on Router: `Register`, `SwitchTo` (returning `tea.Cmd`), `Update` (delegating to active screen), and `View` (delegating to active screen). The design implied a symmetric architecture where the Router would handle both Update and View dispatch.

After ~25 screens were migrated, the actual proven architecture turned out to be asymmetric. This document formalizes what is actually running.

---

## What We Originally Specified vs. What We Built

| Aspect | Specified (screen-interface-pilot.md) | Actual (proven across ~25 screens) |
|--------|---------------------------------------|-------------------------------------|
| Update dispatch | `Router.Update()` delegates to active screen | `screenUpdaters` map in `app_routing.go` calls `.Update()` directly on concrete model pointers |
| View dispatch | `Router.View()` delegates to active screen | `Router.View()` delegates to active screen (identical) |
| Screen switching | `Router.SwitchTo()` calls `Init()` | `ensureSubModel()` calls `Init()` during model creation; `SwitchTo()` only syncs dimensions/theme |
| Registration | `Router.Register()` stores screen | `Router.Register()` stores screen (identical) |

The Router was only ever doing real work for **View()**. The Update() dispatch was always handled by `screenUpdaters`, and Init() was always called by `ensureSubModel`.

---

## Evidence

### 1. `Router.Update()` has zero call sites

Exhaustive grep for `m.router.Update(` across the entire codebase returns 0 results. Every screen's Update path goes through the `screenUpdaters` map:

```go
// app_routing.go -- each screen has a closure like:
m.screenUpdaters[ScreenHelp] = func(msg tea.Msg) tea.Cmd {
    if m.helpModel == nil { ... }
    newModel, cmd := m.helpModel.Update(msg)  // direct call on concrete pointer
    ...
}
```

This pattern is used by all ~25 migrated screens. The Router was never consulted for Update dispatch.

### 2. `Router.SwitchTo()` Init() is redundant

`ensureSubModel()` (`app_nav.go:311`) calls `Init()` on screens that need it (Settings, Help, Discuss, GoalInput, etc.) during model creation, before `switchScreen()` is ever called. The `Init()` inside `SwitchTo()` was a redundant second call.

For screens where `ensureSubModel` returns `nil` (no Init needed), the Router's `Init()` was also a no-op. In neither case did the Router's `Init()` call do useful work.

### 3. `Router.ActiveScreen()` was never called

The method existed but had zero external callers.

---

## Decision: Remove Dead Code

Removed from `Router`:
- `Update(msg tea.Msg) tea.Cmd` method -- never called
- `ActiveScreen() Screenable` method -- never called
- `Init()` call inside `SwitchTo()` -- redundant with `ensureSubModel`
- `SwitchTo` return type changed from `tea.Cmd` to nothing (callers already discarded it)

Kept in `Router`:
- `Register(id ScreenID, s Screenable)` -- actively used by ~25 screens
- `SwitchTo(id ScreenID)` -- actively used by `switchScreen()`
- `View() string` -- the one method that actually does real work
- `SetTheme(t theme.Theme)` -- propagates theme to all registered screens
- `SetDimensions(w, h int)` -- propagates dimensions to all registered screens
- `ActiveID() ScreenID` -- used by callers to check current screen

Not changed:
- `Screenable` interface -- `Update()` remains in the interface because models still need it for the `screenUpdaters` pattern. The Router just doesn't call it.

---

## The Proven Architecture (Plain Terms)

The TUI uses a **split dispatch** pattern:

1. **Update path:** `AppState.Update()` -> `screenUpdaters[screen](msg)` -> calls `.Update()` directly on the concrete model pointer. Each screen has its own closure that knows the concrete type.

2. **View path:** `AppState.View()` -> `renderActiveScreen()` -> `m.router.View()` -> calls `.View()` on the active `Screenable`.

3. **Screen switching:** `switchScreen(id)` -> sets `m.screen = id` -> calls `m.router.SwitchTo(id)` to update the router's active screen and propagate dimensions/theme.

4. **Model creation + Init:** `ensureSubModel(screen)` -> creates model if nil, sets dimensions, calls `Init()` if needed. This happens before `switchScreen`.

The Router is essentially a **view dispatcher + dimension/theme propagator**. It does not handle Update dispatch or screen initialization -- those are handled by the existing `screenUpdaters` and `ensureSubModel` infrastructure respectively.

---

## Why This Works

The `screenUpdaters` pattern requires knowing the concrete type for each screen (to assign back to the typed field after Update). A generic `Router.Update()` would need type assertions or interface gymnastics that the closure-per-screen pattern avoids entirely. The split dispatch is actually simpler and more type-safe than a centralized Router Update would be.

For **REPL and Sidebar** (next planned migrations), the same pattern applies -- they are Bubble Tea models with `Update()` methods, no different from the ~25 screens already migrated. There is no architectural reason they would need centralized Update dispatch through the Router.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go vet ./...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |

---

## Files Changed

- `internal/tui/router.go` -- removed `Update()`, `ActiveScreen()`, `Init()` call in `SwitchTo()`, unused `tea` import. Updated doc comments.
