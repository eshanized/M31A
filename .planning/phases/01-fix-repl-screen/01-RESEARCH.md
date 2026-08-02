# Phase 1: Fix REPL Screen - Research

**Researched:** 2026-08-02
**Domain:** TUI Router Bug Fix
**Confidence:** HIGH

## Summary

The REPL screen is broken due to a timing bug in `Router.SwitchTo()`. When `SwitchTo()` is called before the target screen is registered (e.g., during Init), `activeID` silently remains at its zero value (0 == ScreenFirstRun). This permanently breaks `Register()`'s auto-activate guard for every other screen, preventing proper screen routing.

**Primary recommendation:** Fix `Router.SwitchTo()` to set `r.activeID = id` unconditionally before the screens map guard.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Screen routing | Router | AppState | Router owns screen registration and activation |
| Screen lifecycle | AppState | Router | AppState creates models and calls Register/SwitchTo |
| View delegation | Router | Screen | Router delegates View() to active screen |
| Navigation state | AppState | — | AppState tracks prevScreen, screenStack |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Bubble Tea | v0.25+ | TUI framework | Elm-architecture for Go TUIs |
| Lipgloss | v0.9+ | Styling | Standard companion to Bubble Tea |

**Installation:** Already in go.mod — no new packages needed.

## Architecture Patterns

### Router Pattern

The Router implements a simple screen registration and activation pattern:

1. **Register(id, screen)** — adds screen to map, sets dimensions/theme, auto-activates if conditions met
2. **SwitchTo(id)** — changes active screen, syncs dimensions/theme
3. **View()** — delegates to active screen's View()

```go
// Current (buggy) implementation
func (r *Router) SwitchTo(id ScreenID) {
	if id == 0 {
		return
	}
	if s, ok := r.screens[id]; ok {  // BUG: activeID only set inside guard
		r.activeID = id
		r.active = s
		// ...
	}
}

// Fixed implementation
func (r *Router) SwitchTo(id ScreenID) {
	if id == 0 {
		return
	}
	r.activeID = id  // Set unconditionally
	if s, ok := r.screens[id]; ok {
		r.active = s
		// ...
	}
}
```

### Auto-Activate Guard

`Register()` has a guard that auto-activates a screen if it was SwitchTo'd before being registered:

```go
if r.activeID == id && r.active == nil {
	r.active = s
}
```

This guard fails when SwitchTo is called before Register because `activeID` remains 0.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Screen routing | Custom state machine | Router pattern | Simpler, already implemented |
| View delegation | Switch statement in View() | Router.View() | Separation of concerns |

## Common Pitfalls

### Pitfall 1: Timing Bug in SwitchTo
**What goes wrong:** `activeID` remains 0 when SwitchTo is called before Register
**Why it happens:** `activeID` assignment is inside the `if s, ok := r.screens[id]` guard
**How to avoid:** Set `activeID` unconditionally before the guard
**Warning signs:** Screens never activate, View() returns empty string

### Pitfall 2: Idempotent Register Calls
**What goes wrong:** Register() is called on every View() call without nil-guard
**Why it happens:** render*Content() functions call Register() to ensure screen exists
**How to avoid:** Check if screen is already registered before calling Register()
**Warning signs:** Performance overhead, unnecessary SetDimensions/SetTheme calls

## Code Examples

### Bug Location: router.go:46-58

```go
// Source: internal/ui/tui/router.go
func (r *Router) SwitchTo(id ScreenID) {
	if id == 0 {
		return
	}
	if s, ok := r.screens[id]; ok {
		r.activeID = id  // BUG: Only set inside guard
		r.active = s
		if r.active != nil {
			r.active.SetDimensions(r.width, r.height)
			r.active.SetTheme(r.theme)
		}
	}
}
```

### Auto-Activate Guard: router.go:39-41

```go
// Source: internal/ui/tui/router.go
// Auto-activate if this screen was SwitchTo'd before Register was called
if r.activeID == id && r.active == nil {
	r.active = s
}
```

### Impact in Init: app.go:44-46

```go
// Source: internal/ui/tui/app.go
m.screen = ScreenHome
m.ensureReplModel()
m.switchScreen(m.screen)  // Calls SwitchTo(ScreenHome) before Register
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SwitchTo inside guard | Set activeID unconditionally | This fix | Fixes screen activation |

**Deprecated/outdated:**
- None — this is a bug fix, not a pattern change

## Assumptions Log

> All claims in this research were verified against the codebase — no user confirmation needed.

None — all findings are verified from source code analysis.

## Open Questions

1. **None** — The bug is well-understood and the fix is straightforward.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go testing (stdlib) |
| Config file | none — standard go test |
| Quick run command | `go test ./internal/ui/tui/... -run TestRouter -count=1` |
| Full suite command | `go test ./internal/ui/tui/... -count=1` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REPL-01 | Router.SwitchTo sets activeID unconditionally | unit | `go test ./internal/ui/tui/... -run TestRouter_SwitchTo -count=1` | ❌ Wave 0 |
| REPL-02 | Register auto-activates when SwitchTo called first | unit | `go test ./internal/ui/tui/... -run TestRouter_AutoActivate -count=1` | ❌ Wave 0 |
| REPL-03 | View() returns active screen content | unit | `go test ./internal/ui/tui/... -run TestRouter_View -count=1` | ❌ Wave 0 |
| REPL-04 | Multiple SwitchTo calls before Register work | unit | `go test ./internal/ui/tui/... -run TestRouter_MultipleSwitchTo -count=1` | ❌ Wave 0 |
| REPL-05 | SwitchTo(0) is no-op | unit | `go test ./internal/ui/tui/... -run TestRouter_SwitchToZero -count=1` | ❌ Wave 0 |
| REPL-06 | Existing tests pass after fix | regression | `go test ./internal/ui/tui/... -count=1` | ✅ |

### Sampling Rate
- **Per task commit:** `go test ./internal/ui/tui/... -run TestRouter -count=1`
- **Per wave merge:** `go test ./internal/ui/tui/... -count=1`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `router_test.go` — new file covering Router.SwitchTo, Register, View
- [ ] Test helper: mock Screenable implementation for isolated Router tests

## Security Domain

> Omitted — this is a TUI bug fix with no security implications.

## Sources

### Primary (HIGH confidence)
- `internal/ui/tui/router.go:46-58` — Bug location confirmed via code reading
- `internal/ui/tui/router.go:39-41` — Auto-activate guard confirmed
- `internal/ui/tui/app.go:44-46` — Init timing confirmed
- `internal/ui/tui/app_screens.go` — Render functions confirmed calling Register()

### Secondary (MEDIUM confidence)
- None — all findings are from direct code reading

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — already using Bubble Tea, no new deps
- Architecture: HIGH — Router pattern is clear from code
- Pitfalls: HIGH — Bug is confirmed via code reading

**Research date:** 2026-08-02
**Valid until:** 2026-09-01 (stable — bug fix, not feature)
