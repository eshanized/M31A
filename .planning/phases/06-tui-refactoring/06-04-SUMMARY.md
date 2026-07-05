---
phase: 06-tui-refactoring
plan: 04
subsystem: ui
tags: [onboarding, tour, quick-mode, workflow-ux, first-run, commands]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Navigation foundation, sidebar visibility"
  - phase: 06-02
    provides: "Accessibility foundation, focus ring"
  - phase: 06-03
    provides: "Streaming performance, lightweight markdown"
provides:
  - "Getting-started tour component with 5 steps"
  - "Improved home screen with workflow explanation and categorized suggestions"
  - "/help getting-started command"
  - "/quick [on|off] toggle for simple task auto-skip"
  - "/skip <phase> command for manual phase skipping"
  - "Quick mode auto-detection heuristics for simple tasks"
  - "First-run 'What's next' guidance after setup"
  - "REPL welcome workflow explanation"
affects: [06-05, 06-06, 06-07, 06-08]

# Tech tracking
tech-stack:
  added: []
  patterns: [TourStep/TourModel component pattern, isSimpleTask heuristic pattern, adjustModeForQuickMode routing pattern]

key-files:
  created:
    - internal/tui/components/tour.go
  modified:
    - internal/tui/home_model.go
    - internal/tui/home_view.go
    - internal/tui/firstrun_view.go
    - internal/tui/repl_welcome.go
    - internal/tui/commands/commands_core.go
    - internal/tui/commands/commands.go
    - internal/tui/app_state.go
    - internal/tui/app_update_commands.go
    - internal/tui/handler_workflow.go
    - internal/tui/commands/commands_all_test.go

key-decisions:
  - "Tour component uses 5 steps: Welcome, How it works, Quick start, Navigation, Ready"
  - "Quick mode uses existing ModeDirect for simple tasks (skips Discuss, Plan, Verify)"
  - "isSimpleTask heuristics match common patterns: fix, bug, error, test, doc, config"
  - "/skip command uses SkipToPhase field in CommandResult for phase routing"
  - "Home screen shows categorized suggestions: Code, Explore, Debug"

patterns-established:
  - "TourModel pattern: renderable component with Next/Skip/IsCompleted lifecycle"
  - "Quick mode pattern: adjustModeForQuickMode(original, goal, quickMode) for phase skipping"
  - "CommandContext extension: QuickMode/SetQuickMode for toggle commands"

requirements-completed: [TUI-04, TUI-09]

# Metrics
duration: 11min
completed: 2026-07-05
---

# Phase 06 Plan 04: First-Time Experience & Quick Mode Summary

**Getting-started tour with 5 steps, improved home screen with workflow explanation, /help getting-started, /quick toggle, and /skip phase command**

## Performance

- **Duration:** 11 min
- **Started:** 2026-07-05T02:22:22Z
- **Completed:** 2026-07-05T02:33:52Z
- **Tasks:** 2
- **Files created/modified:** 11

## Accomplishments
- Created Tour component with 5 steps explaining M31A workflow, navigation, and quick start
- Improved home screen with workflow explanation, categorized suggestions, and first-visit hint
- Added /help getting-started command with comprehensive workflow guide
- Added /quick [on|off] command to toggle quick mode for simple tasks
- Added /skip <phase> command to manually skip workflow phases
- Quick mode auto-detects simple tasks (fix, bug, error, test, doc, config) and uses ModeDirect
- Updated first-run done step with usage instructions ("What's next")
- Added workflow explanation to REPL welcome screen

## Task Commits

Each task was committed atomically:

1. **Task 1: Create getting-started tour and improve home screen** - `5e3dfe0f` (feat)
2. **Task 2: Add quick mode and /help getting-started command** - `4cf9ea65` (feat)

## Files Created/Modified
- `internal/tui/components/tour.go` - New Tour component with TourStep, TourModel, 5-step flow
- `internal/tui/home_model.go` - Added firstVisit flag, showTour, SetFirstVisit, ShowTour, DismissTour
- `internal/tui/home_view.go` - Added workflow explanation, categorized suggestions, first-visit hint
- `internal/tui/firstrun_view.go` - Updated renderDone with "What's next" usage instructions
- `internal/tui/repl_welcome.go` - Added workflow explanation above provider/project cards
- `internal/tui/commands/commands_core.go` - Added handleGettingStarted, handleQuickMode, handleSkipPhase
- `internal/tui/commands/commands.go` - Added SkipToPhase to CommandResult, QuickMode/SetQuickMode to CommandContext
- `internal/tui/app_state.go` - Added quickMode field, SetQuickMode, IsQuickMode methods
- `internal/tui/app_update_commands.go` - Wired QuickMode/SetQuickMode into CommandContext, added SkipToPhase handling
- `internal/tui/handler_workflow.go` - Added isSimpleTask heuristic, adjustModeForQuickMode function
- `internal/tui/commands/commands_all_test.go` - Updated expected command count from 64 to 67

## Decisions Made
- Tour uses 5 steps (Welcome, How it works, Quick start, Navigation, Ready) for comprehensive onboarding
- Quick mode leverages existing ModeDirect (Init→Execute→Ship) rather than creating a new mode
- isSimpleTask uses substring matching on common patterns (fix, bug, error, test, doc, config)
- /skip command uses SkipToPhase string field for flexible phase routing
- Home screen suggestions use category labels (Code, Explore, Debug) for discoverability

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed undefined variable `t` in repl_welcome.go**
- **Found during:** Task 1
- **Issue:** Used `t.TextSecondary` but function uses `m.theme` not a local `t` variable
- **Fix:** Changed to `m.theme.TextSecondary`
- **Files modified:** internal/tui/repl_welcome.go
- **Verification:** Build passes, tests pass
- **Committed in:** 5e3dfe0f (Task 1 commit)

**2. [Rule 1 - Bug] Updated command count test for 3 new commands**
- **Found during:** Task 2
- **Issue:** TestDefaultCommands_AllRegistered expected 64 commands, now 67
- **Fix:** Updated expected list to include getting-started, quick, skip
- **Files modified:** internal/tui/commands/commands_all_test.go
- **Verification:** All tests pass
- **Committed in:** 4cf9ea65 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bug fixes for correctness)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all tour content, suggestions, and quick mode heuristics are fully implemented.

## Next Phase Readiness
- Onboarding and quick mode foundation complete for subsequent plans
- Tour component available for reuse across screens
- Quick mode pattern established for any future phase-skipping features

## Self-Check: PASSED

All files and commits verified.

---
*Phase: 06-tui-refactoring*
*Completed: 2026-07-05*
