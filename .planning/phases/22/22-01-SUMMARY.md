---
phase: 22
plan: 01
subsystem: bug-fixes
tags: [bug-fix, config-wiring, firstrun, verify, ship, provider]

# Dependency graph
requires: []
provides:
  - "BUG-01: First-run validation uses config base URLs"
  - "BUG-05: Self-heal confirmation triggers actual healing"
  - "BUG-06: New session redirects to REPL, not first-run"
  - "Config wiring: all 8 config fields passed to provider Options"
affects: [tui, provider, workflow]

# Tech tracking
tech-stack:
  added: []
  patterns: ["callback pattern for cross-component communication", "public HealTask method on workflow engine"]

key-files:
  created: []
  modified:
    - internal/tui/firstrun.go
    - internal/tui/verify.go
    - internal/tui/ship.go
    - internal/tui/app.go
    - internal/tui/app_update.go
    - internal/tui/app_update_workflow.go
    - internal/tui/types.go
    - internal/workflow/engine.go
    - internal/tui/ship_test.go
    - internal/tui/screens_test.go
    - internal/tui/app_test.go

key-decisions:
  - "FirstRunOpts variadic parameter for backward-compatible constructor"
  - "HealFunc callback pattern for VerifyModel to trigger healing without direct engine dependency"
  - "Public HealTask method on workflow engine for TUI-triggered healing"
  - "AppMsg.Action field for screen transition actions beyond simple screen switches"

patterns-established:
  - "Cross-component callback pattern: models receive callbacks for actions requiring parent context"
  - "Public engine methods exposed via workflowEngineInterface for TUI integration"

requirements-completed: [BUG-01, BUG-05, BUG-06, Config Wiring]

# Metrics
duration: 14min
completed: 2026-06-05
---

# Phase 22 Plan 01: Critical Bug Fixes & Config Wiring Summary

**Config-driven first-run validation URLs, self-heal confirmation wired to workflow engine, new session redirects to REPL instead of first-run wizard**

## Performance

- **Duration:** 14 min
- **Started:** 2026-06-05T00:53:52Z
- **Completed:** 2026-06-05T01:08:45Z
- **Tasks:** 4 (3 bugs fixed, 1 pre-existing config wiring verified)
- **Files modified:** 11

## Accomplishments
- BUG-01: First-run screen now uses config base URLs (OpenRouterBaseURL, ZenBaseURL) with fallback to defaults
- BUG-05: Self-heal confirmation (Y key) triggers actual healing via workflow engine's HealTask method
- BUG-06: New session (N key on ship screen) redirects to REPL with fresh session, not first-run wizard
- Config wiring verified: all 8 config fields (base URLs, referer, title, cache TTLs, health check thresholds) already wired to provider Options

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix First-Run Validation URLs (BUG-01)** - `00f7743` (fix)
2. **Task 2: Fix Self-Heal Trigger (BUG-05)** - `793ee89` (fix)
3. **Task 3: Fix New Session Redirect (BUG-06)** - `5bfb664` (fix)
4. **Task 4: Wire Config Fields to Provider Options** - pre-existing (verified)

## Files Created/Modified
- `internal/tui/firstrun.go` - Added FirstRunOpts, base URL parameters to validateAPIKey, config-driven URLs
- `internal/tui/verify.go` - Added HealFunc callback, healTask invocation on confirmation
- `internal/tui/ship.go` - Changed new session redirect from ScreenFirstRun to ScreenREPL
- `internal/tui/app.go` - Updated NewFirstRunModel call with config values, added HealTask to workflowEngineInterface
- `internal/tui/app_update.go` - Added new_session action handler for session creation
- `internal/tui/app_update_workflow.go` - Set heal callback on VerifyModel creation
- `internal/tui/types.go` - Added Action field to AppMsg
- `internal/workflow/engine.go` - Added public HealTask method for TUI-triggered healing
- `internal/tui/ship_test.go` - Updated test expectations for ScreenREPL
- `internal/tui/screens_test.go` - Updated test expectations for ScreenREPL
- `internal/tui/app_test.go` - Added HealTask to mock workflow engine

## Decisions Made
- Used variadic `FirstRunOpts` parameter to maintain backward compatibility with existing test calls
- HealFunc callback pattern avoids direct dependency on workflow engine from VerifyModel
- Public HealTask method on engine handles the full heal cycle (load tasks, emit messages, call LLM, re-verify, save)
- AppMsg.Action field enables screen transitions with side effects (session creation, workflow reset)

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written.

---

**Total deviations:** 0 auto-fixed
**Impact on plan:** No scope creep. All fixes are minimal and targeted.

## Issues Encountered
- Pre-existing `go vet` warning in `internal/workflow/engine_verify.go:114` (context leak) — not introduced by this plan, out of scope

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 3 critical bugs (BUG-01, BUG-05, BUG-06) fixed and verified
- Config wiring confirmed complete
- Ready for Phase 22 remaining plans (Wave 2-4 fixes)

---
*Phase: 22-Hardcoded Values and Logical Bug Fixes*
*Completed: 2026-06-05*
