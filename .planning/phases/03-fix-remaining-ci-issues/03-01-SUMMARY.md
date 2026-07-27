---
phase: 03-fix-remaining-ci-issues
plan: 01
subsystem: testing
tags: [lint, testing, security, goldmark]

requires:
  - phase: 02-fix-ci-regressions
    provides: test suite fixes for root causes A-F
provides:
  - lint-clean codebase with io.SeekStart replacing deprecated os.SEEK_SET
  - passing tests with proper session.Manager initialization and context cancellation
  - goldmark v1.8.4 resolving GO-2026-5320 XSS vulnerability
affects: [ci, lint, testing, security]

tech-stack:
  added: []
  patterns: [session.NewManager initialization pattern, context cancellation in tests]

key-files:
  created: []
  modified:
    - internal/core/types/fileutil.go
    - internal/ui/tui/commands/commands_all_test.go
    - internal/tools/extra_test.go
    - go.mod
    - go.sum

key-decisions:
  - "Used io.SeekStart (stdlib) instead of suppressing lint warning"
  - "Used context.WithCancel for test timeout fix (preserves production blocking behavior)"
  - "Upgraded goldmark to v1.8.4 (minimum safe version is v1.7.17)"

patterns-established:
  - "session.NewManager(tmpDir, tmpDir, session.ManagerOpts{}) for test initialization"
  - "context.WithCancel + immediate cancel() for testing blocking channel operations"

requirements-completed: [REQ-01, REQ-02, REQ-03, REQ-04, REQ-05]

coverage:
  - id: D1
    description: "Deprecated os.SEEK_SET replaced with io.SeekStart in fileutil.go"
    requirement: REQ-01
    verification:
      - kind: unit
        ref: "internal/core/types/fileutil.go"
        status: pass
    human_judgment: false
  - id: D2
    description: "TestRegistry_Execute_PhaseAliases passes without nil pointer dereference"
    requirement: REQ-02
    verification:
      - kind: unit
        ref: "internal/ui/tui/commands/commands_all_test.go#TestRegistry_Execute_PhaseAliases"
        status: pass
    human_judgment: false
  - id: D3
    description: "TestAskUserQuestion_ChannelFull completes without timeout"
    requirement: REQ-02
    verification:
      - kind: unit
        ref: "internal/tools/extra_test.go#TestAskUserQuestion_ChannelFull"
        status: pass
    human_judgment: false
  - id: D4
    description: "Goldmark upgraded to v1.8.4, govulncheck reports no vulnerabilities"
    requirement: REQ-04
    verification:
      - kind: other
        ref: "govulncheck ./..."
        status: pass
    human_judgment: false

duration: 9min
completed: 2026-07-27
status: complete
---

# Phase 3 Plan 1: Fix Remaining CI Issues Summary

**Lint-clean codebase with io.SeekStart, passing tests, and goldmark v1.8.4 security fix**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-27T02:47:04Z
- **Completed:** 2026-07-27T02:56:10Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- Replaced all 3 deprecated `os.SEEK_SET` occurrences with `io.SeekStart` in fileutil.go
- Fixed `TestRegistry_Execute_PhaseAliases` nil pointer dereference by using `session.NewManager()`
- Fixed `TestAskUserQuestion_ChannelFull` infinite block by using `context.WithCancel`
- Upgraded `goldmark` from v1.5.2 to v1.8.4 to resolve GO-2026-5320 XSS vulnerability

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix deprecated os.SEEK_SET lint warnings** - `6999a275` (fix)
2. **Task 2: Fix two failing tests with proper initialization** - `156f133c` (fix)
3. **Task 3: Upgrade goldmark to fix XSS vulnerability** - `5fc87616` (fix)

## Files Created/Modified
- `internal/core/types/fileutil.go` - Added `"io"` import, replaced `os.SEEK_SET` with `io.SeekStart`
- `internal/ui/tui/commands/commands_all_test.go` - Use `session.NewManager()` with `t.TempDir()`
- `internal/tools/extra_test.go` - Use `context.WithCancel` for channel-full test
- `go.mod` - goldmark v1.5.2 → v1.8.4
- `go.sum` - Updated dependency checksums

## Decisions Made
- Used `io.SeekStart` (stdlib) instead of suppressing lint warning — cleanest fix
- Used `context.WithCancel` for test timeout fix — preserves production blocking behavior
- Upgraded goldmark to v1.8.4 — latest stable version with security fix

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Disk quota exceeded in /tmp during lint — resolved by setting TMPDIR to /home/snigdha/tmp
- Pre-existing test failures in WebFetch and other packages — not related to this phase's changes

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 3 tasks completed with atomic commits
- Lint, test, and security fixes ready for CI verification
- Phase 3 is the final gate before CI passes on GitHub Actions

## Self-Check: PASSED
- All 3 os.SEEK_SET occurrences replaced with io.SeekStart ✓
- TestRegistry_Execute_PhaseAliases passes ✓
- TestAskUserQuestion_ChannelFull passes ✓
- govulncheck reports no vulnerabilities ✓
- go build succeeds ✓
- make lint passes with 0 issues ✓

---
*Phase: 03-fix-remaining-ci-issues*
*Completed: 2026-07-27*
