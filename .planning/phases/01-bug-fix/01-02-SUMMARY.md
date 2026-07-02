---
phase: 01-bug-fix
plan: 02
subsystem: concurrency
tags: [mutex, race-condition, atomic, toctou, sync]

# Dependency graph
requires:
  - phase: 01-bug-fix/01-01
    provides: "Safe type assertions and length checks for DNS resolution"
provides:
  - "Thread-safe SetCollector with proper lock ordering"
  - "Atomic restartServer operation without TOCTOU gap"
  - "Atomic DNS cache eviction protected by mutex"
  - "Correct pendingPermCount (no double decrement on failure)"
affects: [02-medium-fix, 03-low-fix]

# Tech tracking
tech-stack:
  added: []
  patterns: [sync.Mutex for eviction, deferred-only-on-success]

key-files:
  created: []
  modified:
    - internal/tools/dispatcher.go
    - internal/tools/devserver.go
    - internal/tools/dns_cache.go
    - internal/tools/permissions.go

key-decisions:
  - "DNS cache uses sync.Mutex for eviction atomicity (not RWMutex since eviction is write-heavy)"
  - "Permissions defer moved inside success path to prevent double decrement"

patterns-established:
  - "Lock-write-unlock before RLock for read-heavy propagation patterns"
  - "Single lock block for compound map operations (no TOCTOU gaps)"
  - "Defer only on success path when failure path has explicit cleanup"

requirements-completed: [BUG-08, BUG-09, BUG-10, BUG-12]

# Metrics
duration: 8min
completed: 2026-07-02
---

# Phase 01 Plan 02: Fix HIGH Concurrency Bugs Summary

**Mutex-protected SetCollector, atomic restartServer, atomic DNS cache eviction, and correct pendingPermCount counter**

## Performance

- **Duration:** 8 min
- **Started:** 2026-07-02T00:53:00Z
- **Completed:** 2026-07-02T01:01:18Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Fixed SetCollector data race by acquiring write lock before assigning d.collector
- Eliminated TOCTOU race in restartServer by using a single lock block for env lookup
- Made DNS cache eviction atomic with sync.Mutex protecting threshold check and eviction
- Fixed permissions double decrement by moving defer inside the success path only

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix SetCollector data race and TOCTOU in restartServer (H1, H2)** - `75e31b0` (fix)
2. **Task 2: Fix DNS cache TOCTOU and permissions double decrement (H3, H5)** - `6e72837` (fix)

## Files Created/Modified
- `internal/tools/dispatcher.go` - SetCollector now acquires d.mu.Lock() before writing d.collector
- `internal/tools/devserver.go` - restartServer uses single lock block for env lookup
- `internal/tools/dns_cache.go` - Added sync.Mutex for atomic eviction operations
- `internal/tools/permissions.go` - Deferred pendingPermCount decrement moved to success path only

## Decisions Made
- DNS cache uses sync.Mutex (not RWMutex) because eviction is write-heavy and infrequent
- Permissions counter defer moved inside success path to prevent double decrement on channel send failure

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all fixes are complete with no placeholder values.

## Threat Flags
None - fixes address existing threats (T-02-01, T-02-02) already in the threat register.

## Next Phase Readiness
- HIGH concurrency bugs resolved, ready for MEDIUM bug fixes
- All changes verified with `go build`, `go vet`, `go test`, and `go test -race`

## Self-Check: PASSED

All files found, all commits verified.

---
*Phase: 01-bug-fix*
*Completed: 2026-07-02*
