---
phase: 22
plan: 03
subsystem: tooling
tags: [constants, config-validation, webfetch, provider, refactor]

# Dependency graph
requires:
  - phase: 22
    provides: "BUG-01, BUG-05, BUG-06 fixed (22-01), BUG-03, BUG-04, BUG-02 fixed (22-02)"
provides:
  - "Named constants for all tool hardcoded values"
  - "Config validation for numeric fields"
  - "WebFetch User-Agent uses Version variable"
  - "Single source of truth for provider defaults"
affects: [tools, config, provider]

# Tech tracking
tech-stack:
  added: []
  patterns: ["named constants for tool limits", "config validation", "single source of truth"]

key-files:
  created:
    - internal/tools/constants.go
  modified:
    - internal/tools/bash.go
    - internal/tools/filewrite.go
    - internal/tools/glob.go
    - internal/tools/grep.go
    - internal/tools/webfetch.go
    - internal/tools/edit.go
    - internal/tools/dispatcher.go
    - internal/tools/filewrite_test.go
    - internal/types/constants.go
    - internal/provider/openrouter/client.go
    - internal/provider/zen/client.go
    - internal/tui/app.go
    - cmd/m31a/main.go

key-decisions:
  - "All tool magic numbers extracted to named constants in internal/tools/constants.go"
  - "Provider defaults (StaleCacheTTL, HealthLiveMs, HealthSlowMs) in internal/types/constants.go"
  - "WebFetch User-Agent uses Version variable set from main.go"

patterns-established:
  - "Tool constants: all hardcoded values in tools/ have named constants"
  - "Provider defaults: config is single source of truth, providers reference types/constants.go"

requirements-completed: [Tool Constants, Config Validation, Duplicated Constants, WebFetch User-Agent]

# Metrics
duration: 8min
completed: 2026-06-05
---

# Phase 22 Plan 03: Tool Constants & Config Validation Summary

**Named constants for 18 tool magic numbers, config validation already comprehensive, WebFetch User-Agent dynamic, provider defaults unified**

## Performance

- **Duration:** 8 min
- **Started:** 2026-06-05T01:30:00Z
- **Completed:** 2026-06-05T01:38:00Z
- **Tasks:** 5 (3 refactors, 1 feature, 1 already-complete)
- **Files modified:** 13

## Accomplishments
- Created `internal/tools/constants.go` with 18 named constants for all tool magic numbers
- Replaced all hardcoded values in Bash, FileWrite, Glob, Grep, WebFetch, Edit, Dispatcher
- Config validation was already comprehensive (Task 3 was pre-existing)
- Fixed WebFetch User-Agent to use Version variable instead of hardcoded "1.0"
- Added StaleCacheTTL, DefaultHealthLiveMs, DefaultHealthSlowMs to types/constants.go
- Updated both providers and main.go to reference single source of truth

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Tool Constants Package** - `af30778` (feat)
2. **Task 2: Replace Hardcoded Values in Tools** - `f5983c4` (feat)
3. **Task 3: Add Config Validation** - (pre-existing, no commit needed)
4. **Task 4: Fix WebFetch User-Agent** - `7742ff5` (feat)
5. **Task 5: Fix Duplicated Constants** - `bc26044` (feat)

## Files Created/Modified
- `internal/tools/constants.go` - New file with 18 named constants for tool magic numbers
- `internal/tools/bash.go` - BashKillGracePeriod, BashWaitTimeout
- `internal/tools/filewrite.go` - MaxBackupsPerFile, DirPermission, FilePermission
- `internal/tools/glob.go` - MaxGlobResults, DateFormat
- `internal/tools/grep.go` - MaxGrepPatternLength, DefaultMaxGrepResults
- `internal/tools/webfetch.go` - MaxRedirects, DefaultTimeoutSecs, MaxTimeoutSecs, DNSCacheTTL, Version variable
- `internal/tools/edit.go` - MinLinesForFuzzy, LevenshteinThreshold
- `internal/tools/dispatcher.go` - PermissionChannelBuffer, QuestionChannelBuffer, DefaultAgentName
- `internal/tools/filewrite_test.go` - Updated to use MaxBackupsPerFile
- `internal/types/constants.go` - Added StaleCacheTTL, DefaultHealthLiveMs, DefaultHealthSlowMs
- `internal/provider/openrouter/client.go` - Uses types.DefaultHealthLiveMs/SlowMs
- `internal/provider/zen/client.go` - Uses types.StaleCacheTTL/DefaultHealthLiveMs/SlowMs/DefaultContextLength
- `internal/tui/app.go` - Calls tools.SetVersion(version)
- `cmd/m31a/main.go` - Uses types.StaleCacheTTL

## Decisions Made
- All tool magic numbers extracted to named constants in internal/tools/constants.go
- Provider defaults (StaleCacheTTL, HealthLiveMs, HealthSlowMs) in internal/types/constants.go
- WebFetch User-Agent uses Version variable set from main.go
- Config validation was already comprehensive - no changes needed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Commit included unrelated files**
- **Found during:** Task 2
- **Issue:** `git add -A` staged unrelated files from previous phases
- **Fix:** Used `git add` with specific files for subsequent commits
- **Files modified:** N/A (commit artifact)
- **Verification:** Subsequent commits only include relevant files
- **Committed in:** f5983c4 (part of task commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor - commit included extra files but code changes are correct.

## Issues Encountered
- Pre-existing `go vet` warning in `internal/workflow/engine_verify.go:114` (context leak) — not introduced by this plan, out of scope

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 18 tool magic numbers extracted to named constants
- Config validation comprehensive
- WebFetch User-Agent dynamic
- Provider defaults unified
- Ready for Phase 22 remaining plans (Wave 4 fixes)

---
*Phase: 22-Hardcoded Values and Logical Bug Fixes*
*Completed: 2026-06-05*

## Self-Check: PASSED

- All key files exist on disk
- All 4 task commits present (af30778, f5983c4, 7742ff5, bc26044)
- SUMMARY.md committed
