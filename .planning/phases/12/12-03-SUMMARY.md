---
phase: 12-ux-editor-experience-adaptations
plan: 03
subsystem: tui
tags: frecency, prompt-history, repl, persistence, json

requires:
  - phase: 05-session-state-configuration
    provides: JSON file persistence patterns (~/.m31a/ directory)
  - phase: 02-tui-foundation
    provides: ReplModel, Bubble Tea input handling, key events
provides:
  - FrecentHistory struct with frecency scoring
  - Persistent prompt history at ~/.m31a/prompt_history.json
  - Arrow-up/down frecency navigation with prefix matching
  - Enter key upserts prompts into frecency history
affects:
  - 12-05-editor-context-auto-include (same ReplModel)

tech-stack:
  added: []
  patterns:
    - Frecency scoring: score = frequency / (hours_since_last_use + 1)
    - Atomic file write via temp file + os.Rename
    - Async goroutine save: non-blocking fire-and-forget
    - Prefix/substring search with frecency-ranked results

key-files:
  created:
    - internal/tui/history.go
  modified:
    - internal/tui/repl.go
    - internal/tui/repl_test.go

key-decisions:
  - "Frecency formula: score = frequency / (hours_since_last_use + 1) — never divide by zero"
  - "Upsert placed after inputHistory append but guarded against shell/slash commands"
  - "Arrow-up wraps around through frecency results (historyPos cycles 0..N-1)"
  - "Async save via goroutine with slog.Debug logging — never blocks user input"
  - "Max 1000 entries with frecency-based eviction on Save"
  - "Empty prefix returns all entries sorted by frecency"
  - "Search matches by HasPrefix OR Contains for generous prefix matching"

patterns-established:
  - "Non-blocking disk writes: go func() { save(); slog.Debug on error }()"
  - "Frecency ranking: sort by score descending for best-match-first UX"

requirements-completed: [ADOPT-11]

duration: 2min
completed: 2026-06-01
---

# Phase 12 Plan 03: Prompt History with Frecency — Summary

**Persistent frecency-ranked prompt history at ~/.m31a/prompt_history.json with arrow-key navigation, prefix matching, and atomic writes**

## Performance

- **Duration:** 2 min
- **Started:** 2026-06-01T00:31:09Z
- **Completed:** 2026-06-01T00:33:09Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Created `FrecentHistory` struct in `internal/tui/history.go` with `Load`, `Save`, `Upsert`, `Search`, `Frecency`, `All`, `Size`, `Clear` methods
- Integrated into `ReplModel`: enter key upserts prompts, arrow-up/down navigate frecency history with prefix matching when textarea has content
- 7 test functions covering upsert dedup, search, frecency formula, eviction, persistence round-trip, empty input rejection, and clear

## Task Commits

Each task was committed atomically:

1. **Task 1: Create FrecentHistory struct** - `4d12528` (feat)
2. **Task 2: Integrate into repl.go arrow-up/down** - `a3535f0` (feat)
3. **Task 3: Write tests** - `451e67d` (test)

## Files Created/Modified
- `internal/tui/history.go` - FrecentHistory struct, HistoryEntry, HistoryData, all methods including atomic write
- `internal/tui/repl.go` — frecentHistory field, SetFrecentHistory, enter upsert + async save, arrow-up/down frecency search
- `internal/tui/repl_test.go` — 7 TestFrecentHistory_* test functions

## Decisions Made
- **Guarded upsert:** Only LLM-bound prompts are upserted (shell `!` and slash `/` commands excluded) even though the upsert point is after `inputHistory` append
- **Async save:** Save runs in a goroutine with `slog.Debug` for error logging — never blocks user input flow
- **Arrow-up cycle:** Cycles through frecency results (most relevant first) with wrap-around at both ends
- **Frecency formula:** `frequency / (hours_since_last_use + 1)` — the `+1` prevents division by zero for entries used right now
- **Search semantics:** `HasPrefix OR Contains` — any entry containing the search text matches, ranked by frecency

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## Next Phase Readiness
- Prompt history with frecency ready for REPL use
- Ready for Plan 05 (Editor Context Auto-Include) which also modifies `internal/tui/repl.go`

## Self-Check: PASSED

- [x] `internal/tui/history.go` exists
- [x] `internal/tui/repl.go` exists and modified
- [x] `internal/tui/repl_test.go` exists and modified
- [x] Commit `4d12528` — feat: create FrecentHistory
- [x] Commit `a3535f0` — feat: integrate into repl.go
- [x] Commit `451e67d` — test: add tests
- [x] All 7 TestFrecentHistory_* tests pass
- [x] `go vet ./internal/tui/...` passes
- [x] `CGO_ENABLED=0 go build ./internal/tui/...` passes

---

*Phase: 12-ux-editor-experience-adaptations*
*Completed: 2026-06-01*
