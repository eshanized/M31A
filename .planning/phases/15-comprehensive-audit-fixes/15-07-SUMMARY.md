---
phase: 15-comprehensive-audit-fixes
plan: 07
status: complete
commit_range: pending
files_modified:
  - internal/tui/repl_stream.go
  - internal/tui/header.go
  - internal/tui/app.go
  - internal/tui/app_update.go
  - internal/tui/app_workflow.go
  - internal/tui/verify.go
files_created:
  - internal/tui/segment_concurrency_test.go
---

# 15-07 Summary — TUI Segment Logic

## Findings Addressed

### H-8 — Stream segment type transition race
**Status:** Fixed
**Change:** Extracted `closeActiveSegment()` helper on `ReplModel` that finalizes the current stream segment with duration stamping. Called explicitly on every type transition (thinking→content, content→thinking) and on `StreamDoneMsg`. The helper is pure (no goroutines, no external state), safe within the BT Update() loop.
**Files:** `internal/tui/repl_stream.go`

### H-10 — Header re-renders on every TickMsg
**Status:** Fixed
**Change:** Added `CachedRenderHeader(contextUsed, contextTotal)` on `AppState` with FNV-1a content-based cache key. Cache invalidates on: health status change, model change, theme change. The header string is not re-rendered via lipgloss when the cache key matches.
**Files:** `internal/tui/header.go`, `internal/tui/app.go`, `internal/tui/app_update.go`, `internal/tui/app_workflow.go`

### H-13 — Verify model H/S keys on single-task selection
**Status:** Fixed
**Change:** Added explicit `m.selected >= 0` lower-bound guard to both H (heal) and S (skip) key handlers in `VerifyModel.Update()`. The existing `m.selected < len(m.tasks)` already worked for single tasks, but the explicit lower bound prevents edge cases.
**Files:** `internal/tui/verify.go`

### M-25 — Stream shutdown ordering
**Status:** Already fixed (by 15-02). Verified that `StartStreamCmd`'s `safeCloseOnce` pattern correctly orders cancel → drain → close. Added regression test.
**Files:** `internal/tui/segment_concurrency_test.go`

### M-26 — Debug-mode header cache invalidation
**Status:** Fixed
**Change:** `computeHeaderKey` includes `os.Getenv("M31A_LOG_LEVEL")` in the FNV-1a hash. When the env var changes at runtime, the cache key changes and the header re-renders.
**Files:** `internal/tui/header.go`

## Tests Added (10 tests, all pass with -race)

| Test | Finding | What it verifies |
|------|---------|-----------------|
| `TestSegmentTransition_NoRace` | H-8 | 20 alternating thinking/content chunks + Done → segment type empty, message has segments |
| `TestSegmentTransition_StuckThinking` | H-8 | Thinking chunk then Done → thinking segment closed with duration stamp |
| `TestStreamShutdown_OrderingCancelFirst` | M-25 | 5 chunks sent, channel closed → all received (atomic counter) |
| `TestHeaderCache_HitReturnsSameString` | H-10 | Two calls with same state → identical string, cache valid |
| `TestHeaderCache_InvalidatedOnModelChange` | H-10 | Model change → different header string |
| `TestHeaderCache_InvalidatedOnHealthChange` | H-10 | Health status change → different header string |
| `TestHeaderCache_KeyIncludesLogLevel` | M-26 | M31A_LOG_LEVEL change → different cache key |
| `TestVerify_HealKey_SingleTask` | H-13 | Single failed task, H key → StatusPending |
| `TestVerify_SkipKey_SingleTask` | H-13 | Single task, S key → StatusSkipped |
| `TestVerify_HealKey_OutOfRange` | H-13 | selected=5 with 1 task, H key → no panic, no status change |
