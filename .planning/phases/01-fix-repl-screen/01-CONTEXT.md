# Phase 01: Fix REPL Screen - Context

**Gathered:** 2026-08-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Surgical bug fix: Router.SwitchTo() silently fails to set activeID when the target screen isn't registered yet, which breaks Register()'s auto-activate guard and permanently prevents the REPL screen from rendering. The fix is bounded to `internal/ui/tui/router.go` plus a regression test. No broader TUI refactoring.

</domain>

<decisions>
## Implementation Decisions

### Router Fix
- **D-01:** Set `r.activeID = id` unconditionally in SwitchTo(), regardless of whether the screen is registered. If the screen IS registered, also set `r.active` as before. If it isn't registered yet, leave `r.active` untouched — Register()'s auto-activate guard (`r.activeID == id && r.active == nil`) will pick it up when the screen registers. — **Reversibility:** reversible — the fix is localized to one method in router.go.

### Test Coverage
- **D-02:** Add a single regression test that reproduces the exact bug: call SwitchTo(id) for an unregistered screen, then Register(id, mockModel), then assert Router.View() returns the mock model's content (not ""). This test should fail on the old code and pass on the fix. No additional edge cases in this change.

### Idempotency Issue (Report Only)
- **D-03:** Four screens call Register() on every View() call without a model nil-guard. These are idempotent but wasteful. Report only — do NOT fix in this change unless trivially one line. Affected screens:
  - ScreenFirstRun (line 161): Register + SwitchTo called every render
  - ScreenConfig (line 227): Register called every render
  - ScreenNotifications (line 271): Register called every render
  - ScreenDecisions (line 741): Register called every render
  - All other screens have Register inside the model nil-guard (called once).

### app_nav.go Workaround
- **D-04:** Do NOT touch app_nav.go's routeToScreen() re-invocation logic (which re-registers and re-SwitchTo's screens on navigation). The router fix should make this workaround unnecessary, but leave it in place for now — the new test will confirm whether it's still needed. If the test passes without touching routeToScreen(), the workaround is inert and can be cleaned up later.

### Build and Verify
- **D-05:** Run `go build ./... && go test ./internal/ui/tui/... -race -v` after the fix. Report any failures in full. Do not proceed to broader TUI changes.

### agent's Discretion
- Agent may choose whether to add a comment explaining the unconditional activeID set in SwitchTo() — the existing comments already describe the auto-activate pattern.
- Agent may choose the mock model implementation for the test (e.g., a minimal Screenable that returns a fixed string from View()).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Bug Fix
- `internal/ui/tui/router.go` — Router.SwitchTo(), Register(), View() methods (the file being fixed)
- `internal/ui/tui/app_screens.go` — Per-screen render functions that call Register() (idempotency audit)
- `internal/ui/tui/app_nav.go` — routeToScreen() workaround (do not modify, but understand)
- `internal/ui/tui/router_test.go` — New regression test location (create if needed)

### Requirements
- `.planning/REQUIREMENTS.md` — REPL-01 through REPL-06 (this bug fix unblocks all REPL requirements)
- `.planning/ROADMAP.md` — Phase 1 goal and success criteria

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Router` struct (router.go): Clean interface with Register/SwitchTo/View/SetTheme/SetDimensions
- `Screenable` interface: Already defined, mock can implement it easily
- `ScreenFirstRun`, `ScreenREPL`, etc.: Screen ID constants already exist

### Established Patterns
- Bubble Tea Elm architecture: All state mutations through Update() only
- Nil-guard pattern: Most render functions check `if m.model == nil` before Register
- Auto-activate guard: `Register()` uses `if r.activeID == id && r.active == nil` to activate screens SwitchTo'd before registration

### Integration Points
- `Router.SwitchTo()` is called from `Init()` (via switchScreen) and from render functions
- `Router.Register()` is called from `routeToScreen()` and from per-screen render functions
- `Router.View()` is called from the main render path to get the active screen's content

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The fix is surgical: set activeID unconditionally, add one regression test, run build+test.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-Fix REPL Screen*
*Context gathered: 2026-08-02*
