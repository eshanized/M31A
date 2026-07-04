---
phase: 05-gap-remediation
plan: 03
subsystem: security
tags: [command-injection, ssrf, keychain, input-validation, normalization]

# Dependency graph
requires: [05-01-PLAN.md]
provides:
  - "Robust command normalization preventing obfuscation bypass"
  - "Variable expansion detection blocking injection via env vars"
  - "Keychain service name sanitization preventing path traversal"
affects: [05-04, 05-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [regexp for ANSI stripping, variable expansion detection, path separator sanitization]

key-files:
  created: []
  modified:
    - internal/tools/bash.go
    - pkg/keychain/keychain_linux.go

key-decisions:
  - "Used regexp for ANSI escape code removal (compiled at package level for efficiency)"
  - "Added sanitizeService as defense-in-depth alongside existing validateService regex"
  - "SSRF protection already implemented in httpcheck.go via newSSRFProtectedTransport"

patterns-established:
  - "normalizeCommand for command preprocessing before pattern matching"
  - "containsVariableExpansion for detecting shell injection vectors"
  - "sanitizeService for path traversal prevention in keychain operations"

requirements-completed: [GAP-06, GAP-07]

# Metrics
duration: 4min
completed: 2026-07-04
---

# Phase 05 Plan 03: Security Hardening Summary

**Command injection normalization, variable expansion detection, and keychain path traversal prevention**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-04T01:53:28Z
- **Completed:** 2026-07-04T01:57:27Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments
- Added `normalizeCommand` to strip ANSI escape codes, collapse whitespace, and lowercase commands before pattern matching
- Added `containsVariableExpansion` to detect `$VARIABLE`, `${VARIABLE}`, and `$((expression))` injection vectors
- Updated `checkDangerousCommand` to use robust normalization and variable expansion detection
- Added `sanitizeService` to prevent path traversal in keychain service names
- Verified SSRF protection already fully implemented in httpcheck.go via `newSSRFProtectedTransport()`

## Task Commits

Each task was committed atomically:

1. **Task 1: Harden bash command injection detection (GAP-06)** - `7a8aa2d7` (fix)
2. **Task 2: Add SSRF protection to HTTPCheck (GAP-07)** - No code changes (already implemented)
3. **Task 3: Sanitize keychain service names (GAP-06)** - `ce1c0102` (fix)

## Files Created/Modified
- `internal/tools/bash.go` - Added `normalizeCommand`, `containsVariableExpansion`, ANSI escape regex, updated `checkDangerousCommand`
- `pkg/keychain/keychain_linux.go` - Added `sanitizeService` function for path traversal prevention

## Decisions Made
- Used compiled regexp at package level for ANSI escape removal to avoid per-call compilation overhead
- Added `sanitizeService` as defense-in-depth rather than replacing `validateService` - the strict regex validation is more secure, sanitization is a safety net
- SSRF protection in httpcheck.go already comprehensive: DNS pinning, private IP blocking, IP pinning, error wrapping with `ErrPrivateIPBlocked`

## Deviations from Plan

### Already-Implemented Features

**1. [Plan Spec] SSRF protection for HTTPCheck already existed**
- **Found during:** Task 2 (Add SSRF protection to HTTPCheck)
- **Issue:** Plan specified adding `isPrivateIP`, `createPinnedTransport`, and DNS pinning to httpcheck.go, but all were already implemented via `newSSRFProtectedTransport()` using `isPrivateIP`/`isReservedIP` from webfetch.go (same package)
- **Resolution:** No code changes needed - verified existing implementation matches and exceeds plan requirements
- **Files verified:** internal/tools/httpcheck.go, internal/tools/webfetch.go

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Variable expansion blocking in checkDangerousCommand**
- **Found during:** Task 1 (Harden bash command injection detection)
- **Issue:** Plan's `containsVariableExpansion` was a standalone function; integrated it directly into `checkDangerousCommand` to ensure all commands are checked for variable expansion before pattern matching
- **Fix:** Added variable expansion check as first validation step in `checkDangerousCommand`
- **Files modified:** internal/tools/bash.go
- **Verification:** go test passes for internal/tools
- **Committed in:** 7a8aa2d7 (part of Task 1 commit)

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None

## Threat Flags
None

## Next Phase Readiness
- Security hardening complete for command injection, SSRF, and keychain operations
- All modified packages pass tests
- Pattern established for command normalization and input sanitization

## Self-Check: PASSED

- [x] `internal/tools/bash.go` contains `normalizeCommand` function
- [x] `internal/tools/bash.go` contains `containsVariableExpansion` function
- [x] `internal/tools/bash.go` contains `checkDangerousCommand` with normalization
- [x] `pkg/keychain/keychain_linux.go` contains `sanitizeService` function
- [x] `internal/tools/httpcheck.go` has SSRF protection via `newSSRFProtectedTransport()`
- [x] Commit 7a8aa2d7 exists in git log
- [x] Commit ce1c0102 exists in git log

---
*Phase: 05-gap-remediation*
*Completed: 2026-07-04*
