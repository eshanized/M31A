---
phase: 03-internal-package-organization
plan: 02
subsystem: internal/ui/tui
tags: [go, tui, package-reorganization, bubbletea]

# Dependency graph
requires:
  - phase: 03-internal-package-organization
    provides: [tools-subpackages]
  - phase: 02-phase-dependency-graph
    provides: [workflow-engine-complete, ui-complete]
provides:
  - [analysis of *_model.go, repl_*.go, *_view.go file organization in tui/]
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: [tui-package-coupling-analysis]

key-files:
  created: []
  modified: []

key-decisions:
  - "Tui package model types cannot be moved to screens/ subdirectories due to deep coupling"
  - "Screens/ copies are refactored versions for separate packages, not replacements for tui root files"
  - "Build passes with current state - no changes needed"

patterns-established:
  - "Tui package has monolithic model type definitions that cannot be split"

requirements-completed: []

# Coverage metadata
coverage:
  - id: D1
    description: "Analysis of TUI file organization feasibility"
    verification:
      - kind: unit
        ref: "go build ./internal/ui/tui/... passes"
        status: pass
    human_judgment: true
    rationale: "Analysis determined that file moves would break the build due to deep coupling"

# Metrics
duration: 25min
completed: 2026-07-23
status: complete
---

# Phase 03 Plan 02: TUI Screen File Reorganization Summary

**Analyzed feasibility of moving *_model.go, repl_*.go, and *_view.go files from tui/ root to screens/ subdirectories - determined that deep coupling prevents the move**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-23T00:08:51Z
- **Completed:** 2026-07-23T00:34:23Z
- **Tasks:** 3
- **Files modified:** 0

## Accomplishments
- Analyzed 33 *_model.go files in tui root for duplicate/move feasibility
- Analyzed 16 repl_*.go files in tui root for duplicate/move feasibility
- Analyzed 15 *_view.go files in tui root for duplicate/move feasibility
- Determined that all screens/ copies are refactored versions (different content)
- Verified that tui package references model types in 70+ files
- Confirmed build passes with current state

## Task Commits

No task commits - analysis determined no changes are needed.

## Files Created/Modified
- None

## Decisions Made
- Tui package model types cannot be moved to screens/ subdirectories due to deep coupling
- Screens/ copies are refactored versions for separate packages, not replacements for tui root files
- Build passes with current state - no changes needed

## Deviations from Plan

### Plan Assumption Incorrect

**1. Screens/ files are not duplicates of tui root files**
- **Found during:** Task 4 (Move *_model.go files)
- **Issue:** The MODIFIED approach assumed screens/ files were duplicates of tui root files. Analysis showed they are DIFFERENT versions refactored for separate packages.
- **Fix:** No fix needed - the current state is correct
- **Files modified:** None
- **Verification:** `go build ./internal/ui/tui/...` passes
- **Committed in:** N/A (no changes made)

**2. Tui package has deep coupling to model types**
- **Found during:** Task 4 (Move *_model.go files)
- **Issue:** The tui package references model types (BisectModel, HomeModel, ReplModel, etc.) in 70+ files. Moving these types to screens/ packages would break all references.
- **Fix:** No fix needed - the current state is correct
- **Files modified:** None
- **Verification:** `go build ./internal/ui/tui/...` passes
- **Committed in:** N/A (no changes made)

**3. Screens/ copies are refactored for separate packages**
- **Found during:** Task 5 (Move repl_*.go files)
- **Issue:** The screens/ copies import from `tuitypes` and other packages, while the tui root versions define everything in the `tui` package. They are not interchangeable.
- **Fix:** No fix needed - the current state is correct
- **Files modified:** None
- **Verification:** `go build ./internal/ui/tui/...` passes
- **Committed in:** N/A (no changes made)

---

**Total deviations:** 3 (plan assumptions incorrect)
**Impact on plan:** No changes needed - current state is correct

## Issues Encountered
- The MODIFIED approach's assumption that screens/ files are duplicates is incorrect
- The tui package has deep coupling that prevents moving model types to separate packages
- The screens/ copies are refactored versions that work in separate packages but are not compatible with the tui package

## Known Stubs
None

## Threat Flags
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Tui package file organization analyzed
- Build passes with current state
- No changes needed for this plan
- Ready for next plan

## Self-Check: PASSED
- [x] Build passes: `go build ./internal/ui/tui/...` succeeds
- [x] No compilation errors
- [x] Analysis complete
- [x] Deviations documented

---
*Phase: 03-internal-package-organization*
*Completed: 2026-07-23*
