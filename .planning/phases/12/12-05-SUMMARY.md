---
phase: 12-ux-editor-experience-adaptations
plan: 05
subsystem: tui
tags: repl, filepath, editor-context, regex, mime-detection

# Dependency graph
requires:
  - phase: 02-tui-foundation
    provides: ReplModel, enter key handler, message lifecycle
  - phase: 03-message-rendering-pipeline
    provides: message rendering, streaming
  - phase: 10-provider-layer-adaptations
    provides: provider interaction patterns
provides:
  - Editor Context Auto-Include (@filepath resolution in REPL)
affects: [12-03, 12-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "User input scanned for @filepath patterns before LLM request"
    - "File content inlined as --- path ---\\n{content}\\n--- blocks"
    - "Binary/large files replaced with descriptive placeholders"

key-files:
  created: []
  modified: [internal/tui/repl.go, internal/tui/repl_test.go]

key-decisions:
  - "@filepath regex requires path separator (/) — rejects @user, @mention"
  - "Expanded content used for both display and LLM context (single m.messages slice)"
  - "filepath preserves original user reference format (@./path retained as ./path)"
  - "Binary detection via first 512 bytes mime sniff (http.DetectContentType)"
  - "100KB hard cap prevents context bloat from large files"

patterns-established:
  - "Regex-based file reference scanning with ReplaceAllStringFunc callbacks"
  - "Deferred file reads — stat first, size check, mime sniff, then full read"

requirements-completed: [ADOPT-14]

# Metrics
duration: 2min
completed: 2026-06-01
---

# Phase 12 Plan 05: Editor Context Auto-Include Summary

**@filepath auto-include with regex-based file reference resolution, 100KB size limit, binary detection, and 10 test subtests**

## Performance

- **Duration:** 2 min
- **Started:** 2026-06-01T00:35:27Z
- **Completed:** 2026-06-01T00:37:46Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `fileRefPattern` regex matching `@./path`, `@../path`, `@path/to/file`, `@/absolute/path` (rejects `@user`/`@mention`)
- Implemented `expandFileRefs` on `*ReplModel` — resolves `@filepath` to file contents relative to cwd
- 100KB hard cap with `[file too large: N bytes, max 100KB]` placeholder
- Binary detection via `http.DetectContentType` 512-byte sniff with `[binary: type, N bytes]` placeholder
- Missing files, directories, and unresolvable paths left as-is per D-06
- Integrated into enter key handler — expansion happens before ChatRequest
- 10 passing test subtests covering all edge cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement expandFileRefs method in repl.go** - `cb656b1` (feat)
2. **Task 2: Write tests for expandFileRefs** - `2d7a8f6` (test)

**Plan metadata:** (committed below)

## Files Created/Modified
- `internal/tui/repl.go` - Added `fileRefPattern` regex, `cwd` field, `SetCwd()`, `expandFileRefs()`, enter key integration
- `internal/tui/repl_test.go` - Added `TestExpandFileRefs_*` (10 subtests), `testReplModelWithCwd` helper

## Decisions Made
- Regex requires path separator `/` — `@user` and `@mention` are never mistaken for file paths
- Expanded content uses the same `m.messages` slice (display == LLM context) for V1 simplicity
- Path in output delimiter preserves original reference format (e.g., `@./test.txt` → `--- ./test.txt ---`)
- Binary detection uses Go's `http.DetectContentType` which follows MIME sniffing standard
- 100KB limit balances context window budgets vs. useful file inclusion

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `expandFileRefs` ready for use in chat workflow — any user message with `@filepath` will auto-include file contents
- Can be extended in future with `@~` home directory expansion or autocomplete for file paths
- Tests pass and `go vet` clean

---

*Phase: 12-ux-editor-experience-adaptations*
*Completed: 2026-06-01*
