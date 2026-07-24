# 01-04 Plan Summary — History, Defensive Coding, and Cleanup

## Plan Reference
- **Plan:** [01-04-PLAN.md](./01-04-PLAN.md)
- **Phase:** 01 — Audit Bug Fixes
- **Wave:** 4

## Bugs Fixed

| Bug | Description | Severity | Approach |
|-----|-------------|----------|----------|
| B25 | Duplicate history entries from Transition+SetPhase | Low | Verified resolved by B02 Transition fix; test confirms single entry |
| B26 | Unbounded history growth in long sessions | Low | History capped at maxHistorySize (1000 entries) with oldest-first trimming |
| B27 | `doublestar.Match` errors silently swallowed | Low | Errors logged via `slog.Warn` in `matchToolName`, `matchValue` |
| B28 | Bare type assertion in DNS cache eviction | Low | Comma-ok guard added to `evictOldest` |
| B29 | Default model capabilities regenerated on every call | Low | Cached in `modelCapabilitiesCache` on first `DetectCapabilities` |
| B30 | `Stop()` doesn't drain all channels | Low | Unified `drainChannels()` drains requestCh, questionReqCh, responseCh |

## Commits

1. `c7fcb7e7` — B25/B26: history verification and cap
2. `7c726d0d` — B27/B28/B29/B30: defensive coding fixes

## Key Design Decisions

- **B25 verification**: Since B02 (Plan 01-01) already routed RunPhase through Transition(), SetPhase is only called during checkpoint restore. The verification test confirms no duplicate history entries in the normal TUI flow.
- **B26 cap (1000)**: Chosen as a generous limit that prevents memory issues in long sessions while preserving sufficient history for debugging. Trimmed from the front (oldest first).
- **B27 slog.Warn**: Uses the existing slog infrastructure rather than adding error return values, which would change function signatures and callers.
- **B30 unified drain**: Single `drainChannels()` method handles all three channel types in one select loop, cleaner than three separate drain loops.

## Phase 01 Final Status

All 30 bugs (B01–B30) across 4 plans are fixed:

| Plan | Wave | Bugs | Status |
|------|------|------|--------|
| 01-01 | 1 | B01, B02, B03, B06, B07, B08/B09 | Complete |
| 01-02 | 2 | B04, B05, B12, B13, B20, B23, B24 | Complete |
| 01-03 | 3 | B10, B11, B14, B15, B16, B17, B18, B19, B21, B22 | Complete |
| 01-04 | 4 | B25, B26, B27, B28, B29, B30 | Complete |

## Verification

- [x] Build passes (`go build ./...`)
- [x] Race tests pass for all new race-sensitive fixes
- [x] Pre-existing test failures confirmed unchanged
- [x] All 30 bugs fixed with tests
