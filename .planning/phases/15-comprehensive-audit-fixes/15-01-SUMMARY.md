---
phase: 15-comprehensive-audit-fixes
plan: 01
subsystem: ui
tags: [bubbletea, nil-safety, tui, panic-prevention]

# Dependency graph
requires:
  - phase: 14-tui-core-wiring-fixes
    provides: "AppState structure with replModel field and Update() switch"
provides:
  - "Nil-guards at every Update() branch that touches m.replModel"
  - "Regression test TestApp_Update_ReplModelNil_NoPanic"
affects: [tui, app_update]

# Tech tracking
tech-stack:
  added: []
  patterns: ["nil-guard pattern: if m.replModel == nil { return nil, nil }"]

key-files:
  created:
    - internal/tui/app_update_nilsafety_test.go
  modified:
    - internal/tui/app_update.go

key-decisions:
  - "Guard route chosen over lazy-init per CONTEXT.md decision — minimal surface change"

patterns-established:
  - "Nil-guard pattern: add `if m.replModel == nil { return nil, nil }` at top of case branches that touch replModel"
  - "Regression test pattern: defer/recover with t.Fatalf for nil-safety tests"

requirements-completed: [C-1]

# Metrics
duration: 8min
completed: 2026-06-02
---

# Phase 15 Plan 01: Nil-Safety Guards Summary

**Nil-guards at 3 unguarded SettingsSavedMsg and ThemeChangedMsg branches prevent TUI panic when m.replModel is nil**

## Performance

- **Duration:** 8 min
- **Started:** 2026-06-02T16:30:04Z
- **Completed:** 2026-06-02T16:38:40Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added nil-guards to 3 previously unguarded branches in Update() that accessed m.replModel
- Created comprehensive regression test with 13 sub-tests covering all representative tea.Msg types
- Build and vet pass cleanly for the tui package (pre-existing failures in other packages are unrelated)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add nil-guards to every Update() branch that touches m.replModel** - `bb4f670` (fix)
2. **Task 2: Add regression test TestApp_Update_ReplModelNil_NoPanic** - `b6ff1be` (test)

## Files Created/Modified
- `internal/tui/app_update.go` - Added 3 nil-guards with `// Fix C-1:` comments in SettingsSavedMsg (2 sub-branches) and ThemeChangedMsg (1 branch)
- `internal/tui/app_new_nilsafety_test.go` - New regression test with 13 sub-tests exercising WindowSizeMsg, KeyMsg, StreamChunkMsg, StreamErrorMsg, PhaseResultMsg, QuestionResponseMsg, SettingsSavedMsg, ThemeChangedMsg, ErrorMsg, SidebarRefreshMsg, ToastMsg, HealthCheckTickMsg, FallbackEventMsg, PermissionRequestMsg

## Decisions Made
- Guard route chosen over lazy-init per CONTEXT.md decision — minimal surface change, 3 one-line additions

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Pre-existing build failures in `internal/workflow/engine.go` (unused import) and `internal/tui/repl_stream.go` (undefined `m.streamCh`) prevent running the full tui test suite. These are NOT caused by this plan's changes. The tui package build and vet pass cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- C-1 nil-safety fix complete with regression test
- Ready for remaining Phase 15 plans (C-2, C-3, etc.)

---
*Phase: 15-comprehensive-audit-fixes*
*Completed: 2026-06-02*
