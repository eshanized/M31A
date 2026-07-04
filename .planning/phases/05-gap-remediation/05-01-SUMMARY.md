---
phase: 05-gap-remediation
plan: 01
subsystem: concurrency
tags: [sync, mutex, goroutine, type-assertion, race-condition]

# Dependency graph
requires: []
provides:
  - "Thread-safe WorkflowCache dynamic context operations"
  - "Safe comma-ok type assertions preventing panics"
  - "Coordinator goroutine leak prevention"
affects: [05-02, 05-03, 05-04, 05-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [sync.RWMutex for dynamic context, comma-ok type assertions, select-based goroutine lifecycle]

key-files:
  created: []
  modified:
    - internal/workflow/workflow_cache.go
    - internal/tools/permissions.go
    - internal/tools/webfetch.go
    - internal/workflow/runtime.go
    - pkg/coordinator/coordinator.go

key-decisions:
  - "Used sync.RWMutex for WorkflowCache dynamic context (read-heavy access pattern)"
  - "Added 5-minute safety timeout in awaitDone to prevent indefinite goroutine leaks"

patterns-established:
  - "comma-ok type assertion for all net.Conn/addr casts"
  - "Mutex-protected dynamic context fields in WorkflowCache"

requirements-completed: [GAP-01, GAP-02, GAP-03]

# Metrics
duration: 7min
completed: 2026-07-04
---

# Phase 05 Plan 01: Concurrency and Correctness Fixes Summary

**Mutex-protected WorkflowCache dynamic context, comma-ok type assertions for net connections, and goroutine leak prevention in Coordinator**

## Performance

- **Duration:** 7 min
- **Started:** 2026-07-04T01:32:00Z
- **Completed:** 2026-07-04T01:38:41Z
- **Tasks:** 5
- **Files modified:** 5

## Accomplishments
- Fixed WorkflowCache data race with dynamicMu RWMutex protecting SetDynamicContext, GetDynamicContext, GetContextSnapshot, and InvalidateAll
- Replaced unsafe type assertions with comma-ok patterns in ApprovePermission, WebFetch TCPAddr, and findFreePort
- Eliminated goroutine leak potential in Coordinator.awaitDone with context-aware select

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix WorkflowCache data race (GAP-01)** - `b7acc3ec` (fix)
2. **Task 2: Fix ApprovePermission unsafe type assertion (GAP-02)** - `51384a73` (fix)
3. **Task 3: Fix WebFetch TLS connection type assertion (GAP-03)** - `d6aa0f2f` (fix)
4. **Task 4: Fix findFreePort unsafe type assertion (GAP-01)** - `d8784e33` (fix)
5. **Task 5: Fix Coordinator awaitDone goroutine leak (GAP-03)** - `5c41022d` (fix)

## Files Created/Modified
- `internal/workflow/workflow_cache.go` - Added dynamicMu RWMutex, protected SetDynamicContext, GetDynamicContext, GetContextSnapshot, InvalidateAll
- `internal/tools/permissions.go` - Replaced unsafe chan type assertion with comma-ok pattern
- `internal/tools/webfetch.go` - Replaced unsafe TCPAddr type assertion with comma-ok pattern
- `internal/workflow/runtime.go` - Replaced unsafe TCPAddr type assertion with comma-ok pattern in findFreePort
- `pkg/coordinator/coordinator.go` - Added select with ctx.Done() and 5-minute safety timeout in awaitDone

## Decisions Made
- Used sync.RWMutex (not Mutex) for WorkflowCache dynamic context since reads significantly outnumber writes
- Added 5-minute safety timeout in Coordinator.awaitDone rather than unlimited blocking - prevents goroutine leak if Complete() is never called without being so short it causes false timeouts

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Protected InvalidateAll dynamic context clearing**
- **Found during:** Task 1 (Fix WorkflowCache data race)
- **Issue:** Plan only specified protecting SetDynamicContext, GetDynamicContext, and GetContextSnapshot, but InvalidateAll also writes to dynamicContext and contextSnapshot without locking
- **Fix:** Added dynamicMu.Lock/Unlock around dynamic context field clearing in InvalidateAll
- **Files modified:** internal/workflow/workflow_cache.go
- **Verification:** go test -race passes for internal/workflow
- **Committed in:** b7acc3ec (part of Task 1 commit)

**2. [Rule 1 - Bug] Fixed unsafe TCPAddr type assertion in WebFetch**
- **Found during:** Task 3 (Fix WebFetch TLS connection type assertion)
- **Issue:** Plan showed tcpConn.RemoteAddr().(*net.TCPAddr) as the existing code, but the actual code already had comma-ok for the TCPConn assertion; only the nested TCPAddr assertion was unsafe
- **Fix:** Applied comma-ok to the inner tcpConn.RemoteAddr().(*net.TCPAddr) assertion
- **Files modified:** internal/tools/webfetch.go
- **Verification:** go test -race passes for internal/tools
- **Committed in:** d6aa0f2f (part of Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 bug)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None

## Threat Flags
None

## Next Phase Readiness
- Concurrency foundation is solid, ready for subsequent gap remediation plans
- Race detector passes on all modified packages

## Self-Check: PASSED

---
*Phase: 05-gap-remediation*
*Completed: 2026-07-04*
