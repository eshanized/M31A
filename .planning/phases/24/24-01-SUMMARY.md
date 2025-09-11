---
phase: 24-tui-redesign
plan: 01
subsystem: ui
tags: [theme, lipgloss, bubbletea, sparkline, starfield, header, sidebar, statusbar, permission]

# Dependency graph
requires:
  - phase: 22
    provides: "Working TUI shell with all screens and shared chrome"
provides:
  - "New theme color tokens (Brand #7C3AED, Accent #06B6D4, Success #10B981, Warning #F59E0B, Error #EF4444)"
  - "Card/phase/metric/timeline style fields on Theme struct"
  - "RenderSparkline function with braille characters"
  - "RenderStarfield component for FirstRun galaxy metaphor"
  - "Redesigned header with block anchors and phase breadcrumb"
  - "Multi-segment StatusBar with workflow state"
  - "Configurable sidebar width (default 120)"
  - "Enhanced PermissionModal with countdown bar and double-border tool card"
affects: [24-02, 24-03, 24-04, 24-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [
    "Braille sparkline rendering with normalized float64 data",
    "Deterministic starfield with seeded hash",
    "Phase breadcrumb ordering function",
    "Half-block countdown bar (█░) for permission timeout"
  ]

key-files:
  created:
    - internal/tui/components/starfield.go
    - internal/tui/components/sparkline_test.go
    - internal/tui/components/starfield_test.go
  modified:
    - internal/tui/theme/theme.go
    - internal/tui/theme/colors.go
    - internal/tui/theme/theme_test.go
    - internal/tui/components/sparkline.go
    - internal/tui/header.go
    - internal/tui/header_test.go
    - internal/tui/statusbar.go
    - internal/tui/sidebar.go
    - internal/tui/components/permission.go
    - internal/tui/app_update.go
    - internal/tui/app_workflow.go

key-decisions:
  - "Brand color changed from #D77757 (warm orange) to #7C3AED (violet) per proposal"
  - "TextMuted changed from #9AA0A6 to #475569 for darker muted text in dark theme"
  - "Sidebar width increased from 42 to 120 for better git status display"
  - "Permission modal command box uses DoubleBorder (blocking semantics) instead of RoundedBorder"
  - "PhaseBreadcrumb is a pure function, not a method on AppState"

patterns-established:
  - "BrailleChars for low-density sparklines, SparklineChars for high-density"
  - "Deterministic seeded hash for starfield rendering"
  - "Half-block progress bars (█░) for timeout visualization"

requirements-completed: [TUI-01, TUI-02]

# Metrics
duration: 12min
completed: 2026-06-05
---

# Phase 24 Plan 01: Theme & Shared Chrome Foundation Summary

**New violet/cyan theme palette with braille sparklines, starfield component, and redesigned header/statusbar/sidebar/permission modal**

## Performance

- **Duration:** 12 min
- **Started:** 2026-06-05T22:45:21Z
- **Completed:** 2026-06-05T22:57:00Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments
- Complete new color palette across dark and light themes matching the TUI redesign proposal
- RenderSparkline with braille characters for usage frequency visualization
- RenderStarfield deterministic galaxy metaphor for FirstRun screen
- Header redesigned with block-character anchors (▓▓▓ M31A ▓) and phase breadcrumb
- StatusBar supports workflow phase and question progress display
- Sidebar width configurable at 120 columns (was fixed at 42)
- PermissionModal enhanced with double-border tool card and half-block countdown bar

## Task Commits

Each task was committed atomically:

1. **Task 1: Theme Enhancement — New Color Tokens and Style Fields** - `cf98362` (feat)
2. **Task 2: Sparkline Enhancement + Starfield Component** - `9cf2f68` (feat)
3. **Task 3: Shared Chrome Redesign — Header, StatusBar, Sidebar, PermissionModal** - `c6a2230` (feat)

## Files Created/Modified
- `internal/tui/theme/theme.go` - Added CardBorder*, Phase*, Metric*, TimelineDate style fields
- `internal/tui/theme/colors.go` - Updated Dark() and Light() with new color palette
- `internal/tui/theme/theme_test.go` - Updated tests to match new color values
- `internal/tui/components/sparkline.go` - Added RenderSparkline with BrailleChars
- `internal/tui/components/sparkline_test.go` - New test file for sparkline
- `internal/tui/components/starfield.go` - New RenderStarfield component
- `internal/tui/components/starfield_test.go` - New test file for starfield
- `internal/tui/header.go` - Block anchors, PhaseBreadcrumb, renderPhaseBreadcrumb
- `internal/tui/header_test.go` - Added block anchor, phase breadcrumb, width tests
- `internal/tui/statusbar.go` - WorkflowPhase, QuestionProgress fields
- `internal/tui/sidebar.go` - defaultSidebarWidth=120, SetWidth/GetWidth
- `internal/tui/components/permission.go` - Double-border tool card, countdown bar
- `internal/tui/app_update.go` - Updated sidebarWidth → defaultSidebarWidth references
- `internal/tui/app_workflow.go` - Updated sidebarWidth → defaultSidebarWidth references

## Decisions Made
- Brand color changed from #D77757 (warm orange) to #7C3AED (violet) per proposal spec
- TextMuted darkened from #9AA0A6 to #475569 for better contrast hierarchy
- Sidebar width increased from 42 to 120 columns for meaningful git status display
- Permission modal command box uses DoubleBorder for blocking semantics (distinct from panel RoundedBorder)
- PhaseBreadcrumb is a stateless pure function for testability

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated theme_test.go for new color values**
- **Found during:** Task 1 (Theme Enhancement)
- **Issue:** Existing theme tests asserted old color values (#0D0D0D, #D77757, #C45C3A) which failed after palette update
- **Fix:** Updated test assertions to match new palette (#0F0F1A, #7C3AED, #7C3AED)
- **Files modified:** internal/tui/theme/theme_test.go
- **Verification:** All theme tests pass
- **Committed in:** cf98362 (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed sidebarWidth constant references in app files**
- **Found during:** Task 3 (Shared Chrome Redesign)
- **Issue:** app_update.go and app_workflow.go referenced the old `sidebarWidth` constant name after rename to `defaultSidebarWidth`
- **Fix:** Updated all references from `sidebarWidth` to `defaultSidebarWidth`
- **Files modified:** internal/tui/app_update.go, internal/tui/app_workflow.go
- **Verification:** `go build ./internal/tui/...` passes
- **Committed in:** c6a2230 (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Theme foundation complete — all new tokens and style fields available for screen redesigns
- Sparkline and Starfield components ready for ModelSelector and FirstRun screens
- Header, StatusBar, Sidebar, PermissionModal ready for integration into redesigned screens
- All subsequent plans (24-02 through 24-05) depend on these tokens and components

---
*Phase: 24-tui-redesign*
*Completed: 2026-06-05*

## Self-Check: PASSED
