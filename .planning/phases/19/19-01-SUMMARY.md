---
phase: 19
plan: 01
subsystem: tech-debt
tags: [refactor, config, workflow, tools, types]

# Dependency graph
requires:
  - phase: 17
    provides: Post-audit baseline with all critical/high issues resolved
provides:
  - Per-channel channelCloser eliminating global sync.Map
  - Reflection-based config merge for all sections
  - Configurable discuss timeout via UIConfig
  - Single DefaultPermissionTimeout constant in types
  - Root commit fallback replacing HEAD~50
  - Git error observability in ship and execute phases
  - UpdateModelCapabilities for runtime model updates
  - Extended normalizeToolName aliases for FileEdit and WebFetch
affects: [workflow, tui, config, tools, provider]

# Tech tracking
tech-stack:
  added: [reflect]
  patterns: [reflection-based-struct-merge, per-channel-once, root-commit-fallback]

key-files:
  created: []
  modified:
    - internal/tui/app.go - channelCloser struct replaces global sync.Map
    - internal/tui/app_update.go - uses channelCloser API and types.DefaultPermissionTimeout
    - internal/tui/app_workflow.go - configurable DiscussTimeout
    - internal/tui/repl.go - removed dead Stream field from ChatRequest
    - internal/tui/app_test.go - updated tests for channelCloser API
    - internal/tui/streaming_test.go - updated tests for channelCloser API
    - internal/provider/interface.go - removed Stream field from ChatRequest
    - internal/provider/openrouter/client.go - UpdateModelCapabilities function
    - internal/workflow/engine.go - removed Stream field, added normalizeToolName aliases
    - internal/workflow/verify.go - root commit fallback via git rev-list
    - internal/workflow/ship.go - git error logging via slog.Warn
    - internal/workflow/execute.go - git error logging via slog.Warn
    - internal/config/loader.go - reflection-based mergeConfig
    - internal/config/types.go - DiscussTimeout field in UIConfig
    - internal/types/constants.go - DefaultPermissionTimeout constant
    - internal/tools/dispatcher.go - uses types.DefaultPermissionTimeout

key-decisions:
  - "TD-1: channelCloser wraps chan struct{} with sync.Once per instance; eliminates unbounded global map"
  - "TD-2: Stream field removed from ChatRequest; provider clients always hardcode stream:true"
  - "TD-3: UpdateModelCapabilities allows runtime updates to hardcoded capability map"
  - "TD-6: Reflection-based mergeConfig handles all struct kinds automatically; no manual field updates needed"
  - "BUG-1: git rev-list --max-parents=0 HEAD is correct root commit lookup vs. brittle HEAD~50"
  - "BUG-3: Single DefaultPermissionTimeout constant in types/ prevents dispatcher/TUI divergence"

patterns-established:
  - "channelCloser: per-channel sync.Once wrapper for safe close-once semantics"
  - "reflect-based-struct-merge: automatic overlay of non-zero config values"

requirements-completed: [TD-1, TD-2, TD-3, TD-4, TD-5, BUG-1, BUG-2, BUG-3, BUG-4]

# Metrics
duration: 13min
completed: 2026-06-04
---

# Phase 19 Plan 01: Comprehensive Codebase Concerns Fixes Summary

**Six tech debt items and four known bugs resolved: global sync.Map eliminated, dead Stream field removed, reflect-based config merge, configurable discuss timeout, permission timeout constant, root commit fallback, and git error observability**

## Performance

- **Duration:** 13 min
- **Started:** 2026-06-04T01:31:35Z
- **Completed:** 2026-06-04T01:44:36Z
- **Tasks:** 2
- **Files modified:** 16

## Accomplishments
- Eliminated global `closeOnces` sync.Map with per-channel `channelCloser` struct (TD-1)
- Removed dead `Stream` field from `ChatRequest` (TD-2)
- Added `UpdateModelCapabilities()` for runtime model updates (TD-3)
- Extended `normalizeToolName` with FileEdit and WebFetch aliases (TD-4)
- Removed deprecated `safeClose()` function (TD-5)
- Replaced manual 150-line `mergeConfig` with 30-line reflection-based implementation (TD-6)
- Replaced brittle `HEAD~50` fallback with proper root commit lookup via `git rev-list` (BUG-1)
- Made discuss Q&A timeout configurable via `UIConfig.DiscussTimeout` (BUG-2)
- Created single `DefaultPermissionTimeout` constant in `types/constants.go` (BUG-3)
- Added `slog.Warn` logging for git errors in ship.go and execute.go (BUG-4)

## Task Commits

Each task was committed atomically:

1. **Task 1: Tech Debt Fixes — TD-1 through TD-5** - `fb76693` (fix)
2. **Task 2: TD-6 Config Merge + Bug Fixes (BUG-1 through BUG-4)** - `a738276` (fix)

## Files Created/Modified
- `internal/tui/app.go` - channelCloser struct replaces global sync.Map; msgDoneCloser field
- `internal/tui/app_update.go` - uses channelCloser API; types.DefaultPermissionTimeout
- `internal/tui/app_workflow.go` - configurable DiscussTimeout from UIConfig
- `internal/tui/repl.go` - removed dead Stream field from ChatRequest construction
- `internal/tui/app_test.go` - updated tests for channelCloser API
- `internal/tui/streaming_test.go` - updated tests for channelCloser API
- `internal/provider/interface.go` - removed Stream field from ChatRequest struct
- `internal/provider/openrouter/client.go` - UpdateModelCapabilities function
- `internal/workflow/engine.go` - removed Stream field, added FileEdit/WebFetch aliases
- `internal/workflow/verify.go` - root commit fallback via git rev-list
- `internal/workflow/ship.go` - git error logging via slog.Warn
- `internal/workflow/execute.go` - git error logging via slog.Warn
- `internal/config/loader.go` - reflection-based mergeConfig with reflect package
- `internal/config/types.go` - DiscussTimeout field in UIConfig
- `internal/types/constants.go` - DefaultPermissionTimeout constant
- `internal/tools/dispatcher.go` - uses types.DefaultPermissionTimeout

## Decisions Made
- **TD-1:** channelCloser wraps chan struct{} with sync.Once per instance; eliminates unbounded global map without changing safeCloseOnce signature semantics
- **TD-2:** Stream field removed from ChatRequest; provider clients always hardcode stream:true in JSON body construction
- **TD-3:** UpdateModelCapabilities allows runtime updates to hardcoded capability map without recompilation
- **TD-6:** Reflection-based mergeConfig handles all struct kinds automatically; eliminates manual field-by-field merge that required updates on every config change
- **BUG-1:** git rev-list --max-parents=0 HEAD is correct root commit lookup vs. brittle HEAD~50 that fails on repos with <50 commits
- **BUG-3:** Single DefaultPermissionTimeout constant in types/ prevents dispatcher and TUI permission timeout divergence

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 19-01 complete (TD-1 through TD-5, TD-6, BUG-1 through BUG-4)
- Ready for remaining Phase 19 plans (security, performance, test coverage)

---
*Phase: 19-comprehensive-concerns-fixes*
*Completed: 2026-06-04*

## Self-Check: PASSED
