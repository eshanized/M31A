---
phase: 15-comprehensive-audit-fixes
plan: 04
subsystem: tools
tags: [ssrf, bash, permissions, typed-errors, security]

requires:
  - phase: 15-comprehensive-audit-fixes-01
    provides: "Nil-safety guards in TUI Update()"
  - phase: 15-comprehensive-audit-fixes-02
    provides: "Stream channel ownership refactor"
  - phase: 15-comprehensive-audit-fixes-03
    provides: "LLM input safety hardening"
provides:
  - "Typed ErrInvalidTimeout sentinel for Bash timeout validation"
  - "Typed ErrPrivateIPBlocked sentinel for WebFetch SSRF protection"
  - "Bash hard output cap via limitWriter gating pipe writes"
  - "WebFetch IPv6 ULA/link-local/IPv4-mapped coverage"
  - "Dispatcher typed ErrPermissionDenied for shell-mode blocks"
affects: [tools, errors, webfetch, bash, dispatcher]

tech-stack:
  added: []
  patterns: ["limitWriter with io.Writer chain for output bounding"]

key-files:
  created:
    - internal/tools/bash_security_test.go
    - internal/tools/webfetch_security_test.go
    - internal/tools/toolinput_test.go
  modified:
    - internal/errors/errors.go
    - internal/tools/bash.go
    - internal/tools/webfetch.go
    - internal/tools/dispatcher.go
    - internal/tools/bash_test.go

key-decisions:
  - "limitWriter wraps pipe writer (not MultiWriter) so output is actually bounded"
  - "Shell-mode permission block returns typed error as second return value"
  - "isPrivateIP uses net.IP methods (IsPrivate, IsLoopback) plus manual fc00::/7 and fe80::/10 checks"

patterns-established:
  - "limitWriter: io.Writer wrapper that drops bytes past limit, used for output bounding"
  - "Typed error wrapping: fmt.Errorf with %w for all security-related error paths"

requirements-completed: [C-5, C-6, C-7, M-1, M-2]

# Metrics
duration: 15min
completed: 2026-06-02
---

# Phase 15-04: Tool Security Summary

**Typed SSRF/TIMEOUT/PERMISSION sentinels, Bash 50K output cap via limitWriter, WebFetch IPv6 coverage**

## Performance

- **Duration:** 15 min
- **Started:** 2026-06-02T22:15:00Z
- **Completed:** 2026-06-02T22:30:00Z
- **Tasks:** 4
- **Files modified:** 9

## Accomplishments
- Two new typed sentinels (ErrInvalidTimeout, ErrPrivateIPBlocked) in errors.go
- Bash tool rejects negative/zero/excessive timeout with typed error
- Bash output capped at 50K chars via limitWriter chain on pipe writer
- WebFetch blocks IPv6 ULA (fc00::/7), link-local (fe80::/10), IPv4-mapped
- Dispatcher shell-mode block returns typed ErrPermissionDenied
- All 14 new regression tests pass with -race

## Task Commits

1. **Task 1-4: Implementation** - `7fd4188` (fix)
2. **Task 1-4: Tests** - `db3c28b` (test)

## Files Created/Modified
- `internal/errors/errors.go` - Added ErrInvalidTimeout, ErrPrivateIPBlocked sentinels
- `internal/tools/bash.go` - Typed timeout errors, limitWriter chain, output cap marker
- `internal/tools/webfetch.go` - Extended isPrivateIP for IPv6, typed ErrPrivateIPBlocked
- `internal/tools/dispatcher.go` - Typed ErrPermissionDenied for shell-mode block
- `internal/tools/bash_test.go` - Updated limitWriter tests for new `w` field
- `internal/tools/bash_security_test.go` - 6 regression tests for C-5/C-6/M-2
- `internal/tools/webfetch_security_test.go` - 6 regression tests for C-7
- `internal/tools/toolinput_test.go` - 3 regression tests for M-1

## Decisions Made
- limitWriter wraps the pipe writer directly (not via MultiWriter) so output is actually bounded at BashOutputLimit
- Shell-mode permission denial returns typed error as second return (not string in ToolResult.Error) so callers can use errors.Is
- isPrivateIP uses net.IP.IsPrivate()/IsLoopback() plus manual fc00::/7 and fe80::/10 byte checks for IPv6 coverage

## Deviations from Plan
None - plan executed as written.

## Issues Encountered
- limitWriter originally used in MultiWriter alongside pipe writer — pipe still received full data. Fixed by making limitWriter the sole gatekeeper wrapping the pipe writer.
- TestWebFetch_Allows_PublicDNS failed because httptest.NewServer binds to 127.0.0.1 (loopback). Fixed to test that public hostnames don't trigger ErrPrivateIPBlocked.

## Next Phase Readiness
- Tool security surface hardened with typed errors
- Ready for Wave 2 remaining plans (15-05, 15-06)

---
*Phase: 15-comprehensive-audit-fixes*
*Completed: 2026-06-02*
