---
phase: 19
plan: 02
subsystem: security
tags: [ssrf, dns-cache, toctou, api-key, os-exit, backup-pruning, go]

# Dependency graph
requires:
  - phase: 17
    provides: "Post-audit codebase fixes establishing baseline for security hardening"
provides:
  - "SSRF DNS cache with pinned IPs preventing rebinding attacks"
  - "Library-safe NewApp constructor returning error instead of os.Exit"
  - "API key removed from AppState — key only exists in provider clients"
  - "Backup pruning capping at 10 backups per file"
affects: [provider, tui, tools]

# Tech tracking
tech-stack:
  added: []
  patterns: [dns-cache-sync-map, library-error-return, backup-pruning]

key-files:
  created: []
  modified:
    - "internal/tools/webfetch.go"
    - "internal/tools/filewrite.go"
    - "internal/tools/filewrite_test.go"
    - "internal/tui/app.go"
    - "cmd/m31a/main.go"
    - "internal/tui/app_test.go"

key-decisions:
  - "DNS cache uses sync.Map with 5-min TTL per hostname — prevents TOCTOU rebinding"
  - "NewApp returns (*AppState, error) — os.Exit moved to cmd/m31a/main.go"
  - "apiKey field removed from AppState — resolved via config and passed to provider constructors only"
  - "Backup pruning keeps max 10 per file using lexicographic sort on timestamped names"

patterns-established:
  - "Library code returns errors; only cmd/ calls os.Exit"
  - "API keys resolved in main.go, never stored in TUI state"

requirements-completed: [SEC-1, SEC-2, SEC-3, SEC-4]

# Metrics
duration: 11min
completed: 2026-06-04
---

# Phase 19 Plan 02: Security Hardening Summary

**SSRF DNS cache with pinned IPs, library-safe NewApp constructor, API key removed from AppState, backup pruning to 10 per file**

## Performance

- **Duration:** 11 min
- **Started:** 2026-06-04T01:46:49Z
- **Completed:** 2026-06-04T01:58:17Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- DNS rebinding TOCTOU gap closed via sync.Map cache with 5-minute TTL; all resolved IPs checked against private range
- os.Exit(1) removed from library code; NewApp returns error to caller
- API key field removed from AppState — key only lives in provider client structs
- Backup directory capped at 10 backups per file via pruneBackups helper

## Task Commits

Each task was committed atomically:

1. **Task 1: SEC-1 SSRF DNS Cache + SEC-4 Backup Pruning** - `cae7276` (fix)
2. **Task 2: SEC-2 os.Exit Removal + SEC-3 API Key Cleanup** - `b684c38` (fix)

## Files Created/Modified
- `internal/tools/webfetch.go` - Added dnsCache sync.Map field, dnsCacheEntry type, resolveAndCache method; DialContext uses cached IPs and checks all against private range
- `internal/tools/filewrite.go` - Added maxBackupsPerFile constant and pruneBackups method; called after each backup write
- `internal/tools/filewrite_test.go` - Added TestFileWrite_BackupPruning verifying max 10 backups retained
- `internal/tui/app.go` - NewApp returns (*AppState, error); apiKey field removed from AppState; os.Exit replaced with error return
- `cmd/m31a/main.go` - Updated NewApp call to handle error; removed apiKey parameter and resolvedAPIKey variable
- `internal/tui/app_test.go` - Added newTestAppWithKey helper; updated tests to use config files for API key resolution

## Decisions Made
- DNS cache uses sync.Map keyed by hostname with 5-minute TTL — prevents DNS rebinding while staying thread-safe
- All resolved IPs checked against private range (not just the first) — closes the remaining TOCTOU vector
- Backup pruning sorts lexicographically (timestamp in name ensures chronological order) and deletes oldest entries
- Tests that need a REPL model use newTestAppWithKey helper with a temp config file containing a dummy API key

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 4 security concerns addressed: SSRF DNS TOCTOU closed, os.Exit removed from library, API key no longer in AppState, backups pruned to 10 per file
- Ready for remaining Phase 19 plans

---
*Phase: 19-comprehensive-concerns-fixes*
*Completed: 2026-06-04*

## Self-Check: PASSED
