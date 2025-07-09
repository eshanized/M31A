---
phase: 16-UX-Polish
plan: 02
subsystem: ux
tags: [error-handling, ux, user-messages, discoverability]

# Dependency graph
requires:
  - phase: 16-01
    provides: Trust & Safety Fixes foundation
provides:
  - Human-readable error messages for all 19+ sentinel errors
  - Command typo suggestions via Levenshtein distance
  - Actionable guidance on error paths (suggests /settings, /provider, /help)
  - Sanitized provider error bodies (HTML stripped, truncated)
  - Specific session error types (NotFound, Corrupted, Permission)
affects: [tui, tools, provider, session, keychain, workflow]

# Tech tracking
tech-stack:
  added: []
  patterns: [UserMessage error mapping, sanitizeProviderError, levenshtein fuzzy matching]

key-files:
  created:
    - internal/errors/errors_test.go
  modified:
    - internal/errors/errors.go
    - internal/tui/app_update.go
    - internal/tui/repl_stream.go
    - internal/tui/commands.go
    - internal/tui/commands_config.go
    - internal/tui/commands_test.go
    - internal/tui/typed_errors_test.go
    - internal/tui/streaming_test.go
    - internal/tools/fileread.go
    - internal/tools/fileread_test.go
    - internal/tools/dispatcher.go
    - internal/tools/dispatcher_test.go
    - internal/provider/openrouter/client.go
    - internal/provider/openrouter/client_test.go
    - internal/provider/zen/client.go
    - pkg/session/manager.go
    - pkg/session/manager_test.go
    - pkg/keychain/keychain_linux.go
    - internal/workflow/plan.go

key-decisions:
  - "UserMessage uses errors.Is for sentinel matching, then string pattern fallback for unwrapped errors"
  - "sanitizeProviderError is duplicated in both provider packages (not shared) to avoid cross-package dependency"
  - "Levenshtein distance threshold of 2 for command suggestions (tunable via bestDist constant)"
  - "Session errors split into ErrSessionNotFound, ErrSessionCorrupted, ErrSessionPermission for distinct failure modes"
  - "Context-exceeded detection requires HTTP 400 + specific patterns (not just 'context' substring)"

patterns-established:
  - "UserMessage pattern: sentinel errors map to actionable guidance strings"
  - "sanitizeProviderError: HTML strip + 200-char truncation for error body safety"
  - "Levenshtein suggestion: threshold-based fuzzy matching for UX discoverability"

requirements-completed: []

# Metrics
duration: 21min
completed: 2026-06-02
---

# Phase 16 Plan 02: Error UX Fixes Summary

**UserMessage mapping for all 19 sentinels, Levenshtein command suggestions, sanitized provider errors, and actionable guidance on every error path**

## Performance

- **Duration:** 21 min
- **Started:** 2026-06-02T20:42:31Z
- **Completed:** 2026-06-02T21:03:25Z
- **Tasks:** 15 (14 unique + 1 test-fix commit)
- **Files modified:** 20

## Accomplishments
- UserMessage() function maps all 19 sentinel errors + pattern-matched unwrapped errors to friendly, actionable messages
- Levenshtein distance command typo suggestions (/hlel → "Did you mean /help?")
- Provider error bodies sanitized (HTML stripped, 200-char truncation, status code mapping)
- Context-exceeded false positives eliminated (requires HTTP 400 + specific patterns)
- Distinct session error types: ErrSessionNotFound, ErrSessionCorrupted, ErrSessionPermission
- Every error path provides actionable guidance (suggests /settings, /provider, /help, etc.)
- All TUI, tools, provider, and session tests pass

## Task Commits

Each task was committed atomically:

1. **Tasks 1+2: UserMessage for human-readable error display** - `c24ad4b` (feat)
2. **Task 3: FileRead stat error handling** - `acb525a` (feat)
3. **Task 4: Provider error body sanitization** - `fdf38f8` (feat)
4. **Task 5: Context-exceeded false positive fix** - `37664d8` (feat)
5. **Task 6: Command typo suggestions** - `19fe580` (feat)
6. **Task 7: Session error specificity** - `61bfa79` (feat)
7. **Task 8: GPG decryption error message** - `55f4e11` (feat)
8. **Task 9: Unknown tool lists available tools** - `0e02bdb` (feat)
9. **Task 10: Tool input JSON error with raw input** - `c4c65b2` (feat)
10. **Task 11: /models suggests /provider** - `d03f122` (feat)
11. **Task 12: /key suggests /settings** - `a396e38` (feat)
12. **Task 13: /optimize shows enable command** - `5847923` (feat)
13. **Task 15: Plan validation error accumulation** - `6b5efb4` (feat)
14. **Test fixes for new error format** - `24fa3c3` (fix)

**Plan metadata:** (pending)

## Files Created/Modified
- `internal/errors/errors.go` - UserMessage() mapping all sentinels + pattern matching
- `internal/errors/errors_test.go` - 31 test cases for UserMessage
- `internal/tui/app_update.go` - Uses UserMessage for ErrorMsg
- `internal/tui/repl_stream.go` - Uses UserMessage in renderErrorBanner default case
- `internal/tui/commands.go` - Levenshtein distance + suggestCommand for typos
- `internal/tui/commands_config.go` - Improved /models, /key, /optimize error messages
- `internal/tools/fileread.go` - Stat errors preserve actual error with path context
- `internal/tools/dispatcher.go` - Lists available tools, includes raw JSON in parse errors
- `internal/provider/openrouter/client.go` - sanitizeProviderError + isContextExceeded
- `internal/provider/zen/client.go` - sanitizeProviderError + isContextExceeded
- `pkg/session/manager.go` - ErrSessionNotFound/Permission for distinct failure modes
- `pkg/keychain/keychain_linux.go` - GPG error suggests pass init or /settings
- `internal/workflow/plan.go` - Accumulated validation errors across retries

## Decisions Made
- UserMessage uses errors.Is for sentinel matching, then string pattern fallback for unwrapped errors
- sanitizeProviderError duplicated in both provider packages to avoid cross-package dependency
- Levenshtein distance threshold of 2 for command suggestions
- Session errors split into three distinct types for programmatic handling
- Context-exceeded detection requires HTTP 400 + specific patterns (not just 'context' substring)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Updated pre-existing tests for new error format**
- **Found during:** Task 6 (command typo suggestions)
- **Issue:** TestCommandRegistry and TestInvalidCommands expected lowercase "unknown command" but Execute now capitalizes to "Unknown command"
- **Fix:** Changed test assertions to use strings.ToLower() for case-insensitive matching
- **Files modified:** internal/tui/commands_test.go
- **Verification:** All TUI tests pass
- **Committed in:** 24fa3c3

**2. [Rule 1 - Bug] Updated streaming test for UserMessage format**
- **Found during:** Task 1 (UserMessage)
- **Issue:** TestReplModel_HandleStreamErrorMsg expected raw "unexpected EOF" but renderErrorBanner now shows "Connection lost — try again"
- **Fix:** Updated test assertion to match new friendly message
- **Files modified:** internal/tui/streaming_test.go
- **Verification:** Test passes
- **Committed in:** 24fa3c3

---

**Total deviations:** 2 auto-fixed (2 pre-existing test format mismatches)
**Impact on plan:** Both auto-fixes necessary for test correctness with new error format. No scope creep.

## Issues Encountered
- Task 14 (Session Load Errors) was already covered by Task 7's changes — session load errors already include session ID, file path, and specific failure reason after Task 7 implementation

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All error paths now provide user-friendly, actionable messages
- Ready for remaining Phase 16 plans (additional UX polish)

---
*Phase: 16-UX-Polish*
*Completed: 2026-06-02*
