---
phase: 01-critical-issues
plan: 01
subsystem: bash, installer, headless, tui
tags: [bash, security, installer, headless, permission, timeout]

# Dependency graph
requires: []
provides:
  - Fixed bash tool allowing legitimate shell syntax
  - Working installer URL construction matching goreleaser output
  - Functional --goal headless mode with full workflow execution
  - Visible permission timeout countdown from start
affects: [01-critical-issues]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - internal/tools/bash.go
    - install.sh
    - cmd/m31a/main.go
    - internal/tui/components/permission.go

key-decisions:
  - "Removed $(, ${, and backtick from obfuscation blocklist since containsVariableExpansion() handles actual injection attempts"
  - "Installer URLs now use lowercase m31a and keep v prefix to match goreleaser output format"
  - "Headless mode runs all 7 workflow phases sequentially with proper error handling"
  - "Permission timeout shows countdown from start of 10-minute period, not just last 5 minutes"

patterns-established: []

requirements-completed: [C1, C2, C3, C4]

coverage:
  - id: D1
    description: "Bash tool allows legitimate shell syntax like $(date) and ${DIR}"
    requirement: C1
    verification:
      - kind: unit
        ref: "internal/tools/bash.go#dangerousObfuscationPatterns"
        status: pass
    human_judgment: false
  - id: D2
    description: "Installer constructs correct URLs matching goreleaser output format"
    requirement: C2
    verification:
      - kind: manual_procedural
        ref: "install.sh URL construction"
        status: pass
    human_judgment: true
    rationale: "Requires actual download test to verify URL matches GitHub release"
  - id: D3
    description: "--goal headless mode executes full workflow with proper exit codes"
    requirement: C3
    verification:
      - kind: unit
        ref: "cmd/m31a/main.go#runHeadlessWorkflow"
        status: pass
    human_judgment: false
  - id: D4
    description: "Permission modal shows countdown from start of 10-minute timeout"
    requirement: C4
    verification:
      - kind: unit
        ref: "internal/tui/components/permission.go#Render"
        status: pass
    human_judgment: false

# Metrics
duration: 7min
completed: 2026-07-13
status: complete
---

# Phase 1 Plan 01: Critical Issues Summary

**Fixed bash tool obfuscation blocking legitimate shell syntax, installer URL 404s, unimplemented headless mode, and invisible permission timeout**

## Performance

- **Duration:** 7 min
- **Started:** 2026-07-13T04:21:56Z
- **Completed:** 2026-07-13T04:29:53Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments
- Removed false positives from bash tool obfuscation blocklist allowing legitimate shell commands
- Fixed installer URL construction to match goreleaser output format (lowercase, v-prefix)
- Implemented full --goal headless mode that runs all 7 workflow phases
- Made permission timeout visible from start of 10-minute period

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix Bash Tool Blocks Legitimate Shell Syntax (C1)** - `82665f59` (fix)
2. **Task 2: Fix Installer Downloads Will 404 (C2)** - `447cd537` (fix)
3. **Task 3: Implement --goal Headless Mode (C3)** - `1933f5e8` (feat)
4. **Task 4: Fix Permission Modal Timeout Is Invisible (C4)** - `0ec59583` (fix)

## Files Created/Modified
- `internal/tools/bash.go` - Removed $(", ${, and backtick from obfuscation blocklist
- `install.sh` - Fixed URL construction to use lowercase m31a and keep v prefix
- `cmd/m31a/main.go` - Implemented runHeadlessWorkflow with full workflow execution
- `internal/tui/components/permission.go` - Removed 5-minute threshold for timeout display

## Decisions Made
- Removed $(", ${, and backtick from obfuscation blocklist since containsVariableExpansion() handles actual injection attempts
- Installer URLs now use lowercase m31a and keep v prefix to match goreleaser output format
- Headless mode runs all 7 workflow phases sequentially with proper error handling
- Permission timeout shows countdown from start of 10-minute period, not just last 5 minutes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 4 critical issues (C1-C4) resolved
- Bash tool now allows legitimate shell syntax
- Installer URLs match goreleaser output format
- Headless mode fully implemented
- Permission timeout visible from start
- Ready for high-priority issues phase

---
*Phase: 01-critical-issues*
*Completed: 2026-07-13*
