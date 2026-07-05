---
phase: 06-tui-refactoring
plan: 03
subsystem: ui
tags: [streaming, performance, markdown, viewport, debounce, virtualization]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Navigation foundation, sidebar visibility"
  - phase: 06-02
    provides: "Accessibility foundation, focus ring"
provides:
  - "Lightweight inline markdown parser for streaming (bold, italic, code, links, headings)"
  - "Streaming render rate increased from 5fps to 10fps"
  - "Viewport virtualization — only visible messages rendered"
  - "Resize event debouncing (100ms) to prevent flicker"
affects: [06-04, 06-05, 06-06, 06-07, 06-08]

# Tech tracking
tech-stack:
  added: []
  patterns: [lightweightMarkdown render pattern, calculateVisibleRange virtualization, resize debounce timer]

key-files:
  created:
    - internal/tui/components/markdown_lite.go
  modified:
    - internal/tui/repl_state.go
    - internal/tui/repl_model.go
    - internal/tui/repl.go

key-decisions:
  - "LightweightMarkdown uses string scanning (not regex) for speed on every streaming tick"
  - "minRenderInterval changed from 200ms (5fps) to 100ms (10fps) matching spinner tick rate"
  - "Viewport virtualization uses estimated line heights for range calculation (not full render)"
  - "Resize debounce uses 100ms timer with width-change-only triggers (height changes ignored)"
  - "Regexes already pre-compiled in toolcard.go — no changes needed (verified)"

patterns-established:
  - "LightweightMarkdown pattern: component-based inline parser for streaming without Glamour"
  - "calculateVisibleRange pattern: estimate visible messages from scroll offset + buffer"
  - "ResizeTickMsg debounce pattern: timer-based re-render prevention for WindowSizeMsg"

requirements-completed: [TUI-03]

# Metrics
duration: 4min
completed: 2026-07-05
---

# Phase 06 Plan 03: Streaming Performance Summary

**Lightweight markdown parsing at 10fps, viewport virtualization, resize debouncing**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-05T02:16:26Z
- **Completed:** 2026-07-05T02:21:16Z
- **Tasks:** 2
- **Files created/modified:** 5

## Accomplishments
- Created LightweightMarkdown component for fast inline formatting (bold, italic, code, links, headings) without Glamour's O(n) re-parse cost
- Increased streaming render rate from 5fps (200ms) to 10fps (100ms) matching spinner tick rate
- Added viewport virtualization: calculateVisibleRange() estimates which messages are visible and only renders those + buffer
- Small conversations (<=20 messages) render all; larger ones use virtual range to skip off-screen messages
- Added 100ms resize debounce timer to prevent flicker during rapid resize events
- Only re-renders on width changes (height-only changes are ignored for debounce)
- Verified regexes already pre-compiled in toolcard.go (no per-frame compilation)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement lightweight streaming markdown parser and increase render rate** - `4a4177b3` (feat)
2. **Task 2: Add viewport virtualization and debounce resize events** - `8833d33c` (feat)

## Files Created/Modified
- `internal/tui/components/markdown_lite.go` - New LightweightMarkdown component with inline formatting
- `internal/tui/repl_state.go` - Changed minRenderInterval to 100ms, integrated lightweight parser in renderStreamingContent(), added calculateVisibleRange()
- `internal/tui/repl_model.go` - Added lightweightMarkdown, visibleRange, virtualBuffer, resizeTimer fields
- `internal/tui/repl.go` - Added ResizeTickMsg type, debounce timer in WindowSizeMsg handler, pending resize flag in TickMsg

## Decisions Made
- Used string scanning (not regex) for lightweight markdown parser to minimize per-tick overhead
- Viewport virtualization uses estimated line heights (8 lines/message average) for range calculation
- Resize debounce only triggers on width changes to avoid unnecessary re-renders during vertical scrolling
- Full Glamour rendering still used for finalized messages — lightweight parser only during streaming

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing functionality] Added ResizeTickMsg type for debounce coordination**
- **Found during:** Task 2
- **Issue:** The debounce timer fires in a goroutine but Bubble Tea requires message-based coordination
- **Fix:** Added ResizeTickMsg type and resizePending flag to coordinate timer callback with Update() loop
- **Files modified:** internal/tui/repl.go, internal/tui/repl_model.go
- **Commit:** 8833d33c

**2. [Rule 1 - Bug] Fixed initial width check for first WindowSizeMsg**
- **Found during:** Task 2
- **Issue:** First WindowSizeMsg had lastWidth=0, causing the debounce to skip the initial render
- **Fix:** Added special case: always render on first init (lastWidth == 0) regardless of debounce
- **Files modified:** internal/tui/repl.go
- **Commit:** 8833d33c

---

**Total deviations:** 2 auto-fixed (1 missing functionality, 1 bug fix)
**Impact on plan:** Both necessary for correctness — debounce requires message coordination and first render must not be debounced.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Streaming performance foundation complete for subsequent plans
- Lightweight markdown parser available for any streaming-related features
- Viewport virtualization can be extended with more accurate line height tracking

## Self-Check: PASSED

All files and commits verified.

---
*Phase: 06-tui-refactoring*
*Completed: 2026-07-05*
