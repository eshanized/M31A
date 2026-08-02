# Phase 01: UX Documentation and NO_COLOR Support - Context

**Gathered:** 2026-08-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix documentation drift in KEYBINDINGS.md to match actual code behavior, and add NO_COLOR environment variable support for accessibility compliance.

This phase addresses:
1. Permission modal keybinding discrepancies between docs and code
2. Undocumented execute-screen keys (s/c/x)
3. Missing slash commands in documentation
4. NO_COLOR env var detection for terminal accessibility

</domain>

<decisions>
## Implementation Decisions

### Documentation Fixes
- **D-01:** Permission modal keys in KEYBINDINGS.md will be updated to match code: remove `e` (Exit), add `b` (Approve all), keep `n`/`enter`/`esc` as deny options — **Reversibility:** reversible — documentation-only change
- **D-02:** Execute-screen keys `s` (skip task), `c` (cancel task), `x` (cancel group) will be added to KEYBINDINGS.md under a new "Execute Screen" section — **Reversibility:** reversible — documentation-only change
- **D-03:** Missing slash commands will be added to KEYBINDINGS.md: `/refine`, `/pending`, `/agent`, `/agent-cancel`, `/complexity`, `/decisions`, `/agent-mode` — **Reversibility:** reversible — documentation-only change

### Code Changes
- **D-04:** NO_COLOR environment variable support will be added to theme initialization in `internal/ui/tui/theme/theme.go` — **Reversibility:** reversible — ~5 lines, standard accessibility pattern

### the agent's Discretion
- Exact wording and formatting of keybinding documentation
- Order of slash commands in the documentation
- Placement of NO_COLOR check in theme initialization (before or after color profile detection)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Documentation
- `docs/KEYBINDINGS.md` — Current keybinding documentation that needs updates
- `AUDIT_REPORT.md` §UX Findings — Source of truth for discrepancies

### Code
- `internal/ui/tui/components/permission.go` — Permission modal key handling (lines 112-130)
- `internal/ui/tui/app_session.go` — Permission key routing (lines 210-234)
- `internal/ui/tui/execute_model.go` — Execute screen key handling (lines 232-252)
- `internal/ui/tui/commands/commands.go` — Slash command registry (lines 220-310)
- `internal/ui/tui/theme/theme.go` — Theme initialization (lines 147-156)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `theme.NewManager()` — Theme manager initialization where NO_COLOR check will be added
- `DetectColorProfile()` — Existing color profile detection that NO_COLOR should integrate with
- `PermissionModal.Render()` — Permission modal rendering that shows correct keybindings

### Established Patterns
- Color profile detection uses `os.Getenv()` for environment variables
- Theme initialization follows a resolve pattern: base theme → profile fallback → accent override
- Keybinding documentation mirrors code structure with tables

### Integration Points
- `theme.go:147-156` — `NewManager()` function where NO_COLOR check will be added
- `theme.go:150` — `DetectColorProfile()` call that should respect NO_COLOR
- `KEYBINDINGS.md` — Documentation file that needs updates

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for:
- NO_COLOR implementation (check env var, return ProfileNone or similar)
- Documentation formatting (maintain existing table style)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-UX Documentation and NO_COLOR Support*
*Context gathered: 2026-08-02*
