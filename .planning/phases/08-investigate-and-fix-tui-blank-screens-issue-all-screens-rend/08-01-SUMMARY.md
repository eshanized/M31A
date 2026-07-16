---
phase: 08-investigate-and-fix-tui-blank-screens-issue-all-screens-rend
plan: 01
subsystem: ui
tags: [bubbletea, screenable, router, tui, dimensions]

# Dependency graph
requires: []
provides:
  - "ReplModel implements full Screenable interface (Init, Update, View, SetDimensions, SetTheme)"
  - "Router.Register() called for all screens including ReplModel"
  - "WindowSizeMsg propagation via SetDimensions verified"
affects: [08-02, 08-03]

# Tech tracking
tech-stack:
  added: []
  patterns: [screenable-interface-compliance, router-registration]

key-files:
  created: []
  modified:
    - internal/tui/repl.go
    - internal/tui/app_nav.go

key-decisions:
  - "ReplModel.Update returns (Screenable, tea.Cmd) to satisfy Screenable interface"

patterns-established:
  - "All screens must implement 5 Screenable methods: Init, Update, View, SetDimensions, SetTheme"
  - "Router.Register() called in both routeToScreen() and ensureSubModel() for all screens"

requirements-completed: [FR-1.1, FR-1.2, FR-1.3, FR-1.5, AC-5, NFR-1, NFR-2]

# Coverage metadata
coverage:
  - id: D1
    description: "ReplModel implements full Screenable interface with correct Update signature"
    requirement: FR-1.1
    verification:
      - kind: automated
        ref: "go build ./internal/tui/"
        status: pass
    human_judgment: false
  - id: D2
    description: "Router.Register() called for ReplModel on first visit and transitions"
    requirement: FR-1.2
    verification:
      - kind: automated
        ref: "grep -n 'router.Register.*ScreenREPL' internal/tui/app_nav.go"
        status: pass
    human_judgment: false
  - id: D3
    description: "Debug logging for WindowSizeMsg, View(), and Init() for diagnostics"
    requirement: FR-1.5
    verification:
      - kind: automated
        ref: "grep -n 'slog.Debug' internal/tui/app_input_resize.go internal/tui/app_view.go internal/tui/app.go"
        status: pass
    human_judgment: false

# Metrics
duration: 5min
completed: 2026-07-16
status: complete
---

# Phase 8 Plan 01: Screenable Interface Compliance Summary

**ReplModel Update signature fixed to (Screenable, tea.Cmd) and Router.Register() added for ScreenREPL**

## Performance

- **Duration:** 5 min
- **Started:** 2026-07-16T01:11:00Z
- **Completed:** 2026-07-16T01:16:31Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Fixed ReplModel.Update signature to return (Screenable, tea.Cmd) satisfying Screenable interface
- Added Router.Register(ScreenREPL, m.replModel) in both routeToScreen() and ensureSubModel()
- Verified all 34+ screens implement complete Screenable interface (Init, Update, View, SetDimensions, SetTheme)
- Confirmed debug logging exists for WindowSizeMsg, View(), and Init()

## Task Commits

Each task was committed atomically:

1. **Task 1: Add SetDimensions to ReplModel** - Already implemented (pre-existing)
2. **Task 2: Audit and fix Screenable interface compliance** - `pending` (fix)
3. **Task 3: Add debug logging** - Already implemented (pre-existing)

**Plan metadata:** `pending` (docs: complete plan)

## Files Created/Modified
- `internal/tui/repl.go` - Changed Update signature from (tea.Model, tea.Cmd) to (Screenable, tea.Cmd)
- `internal/tui/app_nav.go` - Added Router.Register(ScreenREPL, m.replModel) in routeToScreen() and ensureSubModel()

## Decisions Made
- ReplModel.Update returns (Screenable, tea.Cmd) to satisfy Screenable interface, enabling router registration

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ReplModel.Update signature mismatch**
- **Found during:** Task 2 (Audit Screenable compliance)
- **Issue:** ReplModel.Update returned (tea.Model, tea.Cmd) instead of (Screenable, tea.Cmd), preventing Screenable interface satisfaction
- **Fix:** Changed return type to (Screenable, tea.Cmd)
- **Files modified:** internal/tui/repl.go
- **Verification:** Build passes, all callers use type assertion which works with new signature
- **Committed in:** pending

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Fix was necessary for Screenable interface compliance. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Screenable interface audit complete for all 34+ screens
- Router registration working for all screens including ReplModel
- Ready for Plan 08-02 (theme/color compatibility and dimension guards)

---
*Phase: 08-investigate-and-fix-tui-blank-screens-issue-all-screens-rend*
*Completed: 2026-07-16*
