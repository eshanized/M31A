---
phase: 06-tui-refactoring
plan: 02
subsystem: ui
tags: [a11y, accessibility, sgr, osc1337, focus-ring, screen-reader, health-status]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Navigation foundation, sidebar visibility, breadcrumb system"
provides:
  - "ANSI SGR fallback announcements for non-iTerm2 terminals"
  - "Screen reader announcements for streaming, phase changes, errors, success, progress"
  - "Visible focus ring on REPL textarea and home screen input"
  - "Text-based health status indicators alongside color (Live/Slow/Offline/Checking)"
  - "Expanded terminal detection for kitty, alacritty, Windows Terminal, tmux, SSH"
affects: [06-03, 06-04, 06-05, 06-06, 06-07, 06-08]

# Tech tracking
tech-stack:
  added: []
  patterns: [SGR fallback for OSC 1337, RenderFocusRing component pattern, text+color status indicators]

key-files:
  created: []
  modified:
    - internal/tui/a11y/announce.go
    - internal/tui/a11y/announce_test.go
    - internal/tui/a11y/terminal.go
    - internal/tui/components/cursor_indicator.go
    - internal/tui/repl_view.go
    - internal/tui/home_view.go
    - internal/tui/settings_model.go

key-decisions:
  - "SGR bold (SGR 1) used as primary fallback for non-OSC1337 terminals"
  - "Announce() now returns SGR-wrapped text instead of empty string on unsupported terminals"
  - "RenderFocusRing uses theme brand color with rounded border for visual distinction"
  - "Focus ring hidden when modals/overlays have focus (slash, mention, quick-actions)"
  - "Health status labels: ● Live, ◐ Slow, ○ Offline, ⟳ Checking alongside color"

patterns-established:
  - "SGR fallback pattern: all a11y announcements use OSC 1337 primary + SGR fallback"
  - "Focus ring pattern: RenderFocusRing(content, focused, theme, width) component"
  - "Text+color status pattern: icon + name + text label + color for all status indicators"

requirements-completed: [TUI-02]

# Metrics
duration: 4min
completed: 2026-07-05
---

# Phase 06 Plan 02: Accessibility Summary

**ANSI SGR fallback announcements for all terminals, visible keyboard focus ring, text-based health status indicators**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-05T02:10:53Z
- **Completed:** 2026-07-05T02:14:29Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Screen reader announcements now work on all terminals (OSC 1337 for iTerm2/WezTerm, SGR bold/italic/underline fallback for everything else)
- Added 5 new announcement functions: AnnounceStreamContent, AnnouncePhaseChange, AnnounceError, AnnounceSuccess, AnnounceProgress
- Visible focus ring (brand-colored rounded border) on REPL textarea and home screen input
- Health status in settings tab shows text labels (Live/Slow/Offline/Checking) alongside colored icons

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ANSI SGR fallbacks and expanded screen reader announcements** - `6a8400c8` (feat)
2. **Task 2: Add visible focus indicators and text-based health status** - `cba7c0e2` (feat)

## Files Created/Modified
- `internal/tui/a11y/announce.go` - SGR fallback in Announce(), 5 new announcement functions
- `internal/tui/a11y/announce_test.go` - Updated tests for SGR fallback, added tests for new functions
- `internal/tui/a11y/terminal.go` - Added SupportsSGR() helper
- `internal/tui/components/cursor_indicator.go` - Added RenderFocusRing() function
- `internal/tui/repl_view.go` - Applied focus ring to REPL textarea in View() and ViewContent()
- `internal/tui/home_view.go` - Applied focus ring to home screen input
- `internal/tui/settings_model.go` - Health status text labels alongside color

## Decisions Made
- Announce() returns SGR-wrapped text on non-OSC1337 terminals instead of empty string (breaking change from prior behavior, but required for D-22)
- Updated existing tests to verify SGR fallback behavior
- Focus ring uses theme.Brand color with RoundedBorder for consistency with existing card styling
- Health status uses ●/◐○/⟳ icons with text labels for color-blind accessibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated existing tests for new Announce() behavior**
- **Found during:** Task 1
- **Issue:** Existing tests expected Announce() to return empty on non-OSC1337 terminals, but the plan required SGR fallback (non-empty)
- **Fix:** Updated test assertions to verify SGR bold wrapping instead of empty string
- **Files modified:** internal/tui/a11y/announce_test.go
- **Verification:** All a11y tests pass
- **Committed in:** 6a8400c8 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug fix for test alignment)
**Impact on plan:** Necessary for correctness -- tests must match new SGR fallback behavior.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Accessibility foundation complete for all subsequent TUI refactoring plans
- SGR fallback pattern established for any future screen reader features
- Focus ring component available for reuse across all interactive screens

## Self-Check: PASSED

All files and commits verified.

---
*Phase: 06-tui-refactoring*
*Completed: 2026-07-05*
