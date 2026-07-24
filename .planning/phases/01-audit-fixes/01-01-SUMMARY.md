---
phase: 01-audit-fixes
plan: 01
subsystem: engine
tags: [workflow, permissions, filelocking, provider, fallback]

# Dependency graph
requires: []
provides:
  - Fixed workflow engine compaction and transition validation
  - Deny-wins permission evaluation
  - Cross-process file locking via fcntl
  - Shutdown-safe cache synchronization
  - Expanded provider fallback triggers
affects: [01-02, 01-03, 01-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [deny-wins-permissions, fcntl-filelocking, cache-mutex-protection]

key-files:
  created:
    - internal/engine/workflow/run_phase_test.go
    - internal/core/types/fileutil_fcntl_test.go
  modified:
    - internal/engine/workflow/execute.go
    - internal/engine/workflow/engine.go
    - internal/tools/permissions.go
    - internal/core/types/fileutil.go
    - internal/ui/tui/handler_stream.go
    - internal/integrations/provider/fallback_test.go

key-decisions:
  - "Used deny-wins evaluation for permissions (deny always overrides allow regardless of order)"
  - "Switched from flock(2) to fcntl(F_SETLK) for kernel-managed cross-process exclusion"
  - "Added cacheMu RWMutex to protect cache field during shutdown"

patterns-established:
  - "Deny-wins permission evaluation: deny rules always override allow rules"
  - "fcntl-based file locking for cross-process mutual exclusion"
  - "Cache field synchronization using RWMutex"

requirements-completed: [B01, B02, B03, B06, B07, B08, B09]

coverage:
  - id: D1
    description: "Workflow engine compaction result captured and applied to messages"
    requirement: B01
    verification:
      - kind: unit
        ref: "internal/engine/workflow/run_phase_test.go#TestProactiveCompactCheck_ResultApplied"
        status: pass
    human_judgment: false
  - id: D2
    description: "RunPhase validates transitions via stateMachine.Transition(), rejects invalid phase changes"
    requirement: B02
    verification:
      - kind: unit
        ref: "internal/engine/workflow/run_phase_test.go#TestRunPhase_InvalidTransitionRejected"
        status: pass
      - kind: unit
        ref: "internal/engine/workflow/run_phase_test.go#TestRunPhase_ValidTransitionAccepted"
        status: pass
      - kind: unit
        ref: "internal/engine/workflow/run_phase_test.go#TestRunPhase_ConcurrentTransitions"
        status: pass
    human_judgment: false
  - id: D3
    description: "Permission evaluation uses deny-wins: deny rule always overrides allow regardless of order"
    requirement: B03
    verification:
      - kind: unit
        ref: "internal/tools/permissions_test.go#TestCheckPermission_DenyWins"
        status: pass
      - kind: unit
        ref: "internal/tools/permissions_test.go#TestCheckPermission_DenyAlwaysWins"
        status: pass
      - kind: unit
        ref: "internal/tools/permissions_test.go#TestCheckPermission_DenyClassCoverage"
        status: pass
    human_judgment: false
  - id: D4
    description: "FileLock uses fcntl(F_SETLK) for cross-process mutual exclusion, not flock(2)"
    requirement: B06
    verification:
      - kind: unit
        ref: "internal/core/types/fileutil_fcntl_test.go#TestFileLock_MutualExclusion"
        status: pass
      - kind: unit
        ref: "internal/core/types/fileutil_fcntl_test.go#TestFileLock_UnlockReleases"
        status: pass
    human_judgment: false
  - id: D5
    description: "Shutdown sets e.cache to nil under mutex protection, no data race"
    requirement: B07
    verification:
      - kind: unit
        ref: "internal/engine/workflow/engine_race_test.go#TestShutdown_CacheRace"
        status: pass
    human_judgment: false
  - id: D6
    description: "Auth/credit/model-not-found errors and mid-stream SSE errors trigger provider fallback"
    requirement: B08
    verification:
      - kind: unit
        ref: "internal/integrations/provider/fallback_test.go#TestFallbackTrigger_AuthErrors"
        status: pass
      - kind: unit
        ref: "internal/integrations/provider/fallback_test.go#TestFallbackTrigger_CreditErrors"
        status: pass
      - kind: unit
        ref: "internal/integrations/provider/fallback_test.go#TestFallbackTrigger_ModelNotFound"
        status: pass
    human_judgment: false
  - id: D7
    description: "Mid-stream SSE errors (io.ErrUnexpectedEOF, io.EOF, json.SyntaxError) trigger fallback"
    requirement: B09
    verification:
      - kind: unit
        ref: "internal/integrations/provider/fallback_test.go#TestFallbackTrigger_MidStreamErrors"
        status: pass
      - kind: unit
        ref: "internal/integrations/provider/fallback_test.go#TestFallbackTrigger_SyntaxError"
        status: pass
    human_judgment: false

# Metrics
duration: 45min
completed: 2026-07-24
status: complete
---

# Phase 01 Plan 01: Fix critical/high-severity bugs Summary

**Fixed 6 critical/high-severity bugs: compaction result discard, transition validation bypass, deny-wins permissions, fcntl file locking, shutdown cache race, and provider fallback triggers**

## Performance

- **Duration:** 45 min
- **Started:** 2026-07-24T05:15:00Z
- **Completed:** 2026-07-24T06:00:00Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments
- B01: Captured proactiveCompactCheck return value and assigned back to messages
- B02: Routed RunPhase through stateMachine.Transition() to validate phase changes
- B03: Implemented deny-wins permission evaluation (deny always overrides allow)
- B06: Switched FileLock from flock(2) to fcntl(F_SETLK) for cross-process exclusion
- B07: Protected e.cache with cacheMu RWMutex during shutdown
- B08/B09: Expanded provider fallback triggers for auth/credit/model and mid-stream errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix workflow engine bugs (B01, B02)** - `c1e5dbda` (fix)
2. **Task 2: Fix permission and file locking bugs (B03, B06, B07)** - `f70aef2b` (fix)
3. **Task 3: Fix provider fallback bugs (B08, B09)** - `a166be3d` (fix)

## Files Created/Modified
- `internal/engine/workflow/run_phase_test.go` - Tests for B01/B02 fixes
- `internal/engine/workflow/execute.go` - B01: capture compaction return value
- `internal/engine/workflow/engine.go` - B02: Route through Transition(), B07: cache mutex
- `internal/tools/permissions.go` - B03: deny-wins evaluation
- `internal/tools/permissions_test.go` - Tests for B03 deny-wins
- `internal/core/types/fileutil.go` - B06: fcntl-based locking
- `internal/core/types/fileutil_fcntl_test.go` - Tests for B06 fcntl locking
- `internal/engine/workflow/engine_race_test.go` - B07: cache race test
- `internal/ui/tui/handler_stream.go` - B08/B09: expanded fallback triggers
- `internal/integrations/provider/fallback_test.go` - Tests for B08/B09

## Decisions Made
- Used deny-wins evaluation for permissions (deny always overrides allow regardless of order)
- Switched from flock(2) to fcntl(F_SETLK) for kernel-managed cross-process exclusion
- Added cacheMu RWMutex to protect cache field during shutdown
- Used doublestar matching for permission patterns (* doesn't match /)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Doublestar pattern matching: `*` doesn't match `/` in paths, so test patterns needed adjustment
- fcntl locks are per-process, not per-FD within the same process, so cross-process testing requires separate processes

## Next Phase Readiness
- All 6 critical/high-severity bugs fixed with passing tests
- CI-clean after all fixes
- Ready for Phase 02 (data race and concurrency bugs)

---
*Phase: 01-audit-fixes*
*Completed: 2026-07-24*
