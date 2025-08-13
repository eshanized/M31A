---
phase: 15-comprehensive-audit-fixes
plan: 02
subsystem: tui-streaming
tags: [bubbletea, channels, goroutines, sync.Once, race-condition]

# Dependency graph
requires:
  - phase: 14-tui-core-wiring
    provides: "StartStreamCmd signature, streamCh/repl_stream.go patterns, safeClose helper"
  - phase: 15-comprehensive-audit-fixes/01
    provides: "Nil-safety guards in Update() method"
provides:
  - "Single-owner channel pattern in StartStreamCmd (C-3)"
  - "sync.Once-based safeCloseOnce replacing racy check-then-close (H-9)"
  - "Per-stream snapshot documentation confirming no shared mutable state (H-14)"
  - "Eliminated streamDone channel; streamCh closure IS the done signal (M-21)"
  - "4 regression tests covering C-3, H-9, M-21"
affects: [streaming, repl, tui]

# Tech tracking
tech-stack:
  added: []
  patterns: [sync.Once-per-channel, single-owner-channels, per-stream-snapshot]

key-files:
  created: []
  modified:
    - internal/tui/streaming.go
    - internal/tui/repl.go
    - internal/tui/repl_stream.go
    - internal/tui/app.go
    - internal/tui/streaming_test.go

key-decisions:
  - "Kept streamCh as a return value from StartStreamCmd for the continuation pattern (BT only calls a cmd once, so the REPL needs the channel reference for the next read)"
  - "safeCloseOnce uses sync.Map of sync.Once per channel-pointer — race-free under concurrent callers"
  - "Eliminated separate streamDone channel entirely; closing streamCh IS the done signal"

patterns-established:
  - "Channel ownership: goroutine that writes owns the channel; callers get read-only references"
  - "safeCloseOnce: sync.Once-per-channel for defensive double-close protection"
  - "Per-stream snapshot: streamContent/streamSegments live on ReplModel, only mutated by BT update loop"

requirements-completed: [C-3, H-9, H-14, M-21]

# Metrics
duration: 12min
completed: 2026-06-02
---

# Phase 15 Plan 02: Stream Pipeline Channel Ownership Refactor Summary

**Single-owner channel pattern in StartStreamCmd with sync.Once-based safeCloseOnce eliminating race conditions and close-of-closed-channel panics**

## Performance

- **Duration:** 12 min
- **Started:** 2026-06-02T17:01:56Z
- **Completed:** 2026-06-02T17:14:22Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments
- StartStreamCmd allocates channels internally; goroutine owns write side, REPL stores read-only reference
- Eliminated racy check-then-close pattern with sync.Once-backed safeCloseOnce
- Eliminated separate streamDone channel; closing streamCh IS the done signal (M-21)
- 4 new regression tests pass with -race, covering all 4 audit findings

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor StartStreamCmd to single-owner channel pattern (C-3, M-21)** - `1db520e` (fix)
2. **Task 2: Replace safeClose with sync.Once-based safeCloseOnce (H-9, H-14)** - `bcd24a4` (fix)
3. **Task 3: Add regression tests for C-3, H-9, M-21** - `e53f5bc` (test)

## Files Created/Modified
- `internal/tui/streaming.go` - StartStreamCmd owns channels internally; single-channel design (streamDone eliminated)
- `internal/tui/repl.go` - Updated doc comments for streamCh ownership model
- `internal/tui/repl_stream.go` - Updated doc comments for H-14 per-stream snapshot
- `internal/tui/app.go` - safeCloseOnce backed by sync.Map of sync.Once; safeClose deprecated alias
- `internal/tui/streaming_test.go` - 4 new regression tests for C-3, H-9, M-21

## Decisions Made
- Kept streamCh as a return value from StartStreamCmd: BT only calls a cmd once, so the REPL needs the channel reference for continuation. The goroutine owns the channel; the REPL stores a read-only reference.
- safeCloseOnce uses sync.Map of sync.Once per channel-pointer (fmt.Sprintf("%p", ch) key). Race-free under concurrent callers.
- Eliminated separate streamDone channel entirely. Closing streamCh IS the done signal. The cmd returns nil when the channel closes.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None

## Threat Flags
None — all threat mitigations from the plan's threat model are implemented.

## Next Phase Readiness
- Stream pipeline channel ownership is fixed; no more close-of-closed panics
- safeCloseOnce is race-free; no more concurrent-close races
- Ready for 15-03 (LLM input safety & output parsing hardening)
- Pre-existing failure: TestApp_Update_ReplModelNil_NoPanic/QuestionResponseMsg (nil-replModel guard missing from Phase 15-01)

---
*Phase: 15-comprehensive-audit-fixes*
*Completed: 2026-06-02*

## Self-Check: PASSED

All key files exist on disk. All 3 task commits verified in git log. Build and vet pass. All 4 regression tests pass with -race.
