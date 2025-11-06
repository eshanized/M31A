---
phase: 30
plan: 30-01
subsystem: ui
tags: theme, colors, lipgloss, borders, redesign

# Dependency graph
requires: []
provides:
  - Warm-amber brand palette (#D77757) for both Dark() and Light() themes
  - New Theme struct fields: DividerChar, HeaderHeight, ShadowColor, CompactMode, SelectionBg, CardPadding
  - Standardized border constants: NormalBorder, ThinBorder, DoubleBorder
  - Updated applyThemeStyles() with revised Header, Modal, ToolCard, and InputArea styles
affects: 30-02, 30-03, 30-04, 30-05, 30-06, 30-07, 30-08, 30-09, 30-10

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Theme struct embeds settings fields (DividerChar, CompactMode) alongside color fields"
    - "Border constants centralized in theme.go for reuse across all TUI components"
    - "Header style inherits terminal background (no Background() set)"

key-files:
  created: []
  modified:
    - internal/tui/theme/colors.go — Dark/Light palette migration, applyThemeStyles update, new field init
    - internal/tui/theme/theme.go — New Theme struct fields, border constants, DoubleBorder moved here
    - internal/tui/theme/theme_test.go — Updated test expectations for new palette values

key-decisions:
  - "Brand color (#D77757) is identical across Dark and Light themes — brand is brand"
  - "Header inherits terminal background (no Background() call) for cleaner integration"
  - "Modal uses NormalBorder (RoundedBorder) instead of DoubleBorder to reduce visual weight"
  - "ToolCard uses ThinBorder (┌─┐) instead of RoundedBorder for compact appearance"
  - "InputArea has no border — inherits terminal background for a cleaner REPL input area"
  - "DoubleBorder moved from colors.go to theme.go alongside other border constants"
  - "SelectionBg differs between Dark (#3C4043) and Light (#DADCE0)"

patterns-established:
  - "Border types: NormalBorder for cards/modals, ThinBorder for compact cards, DoubleBorder for important modals"

requirements-completed:
  - AC-01
  - AC-15
  - AC-19

# Metrics
duration: 8min
completed: 2026-06-08
---

# Phase 30 Plan 30-01: Theme & Color System Summary

**Migrated color palette from purple-based (#7C3AED) to opencode-inspired warm-amber (#D77757), added missing theme fields and standardized border constants**

## Performance

- **Duration:** 8 min
- **Started:** 2026-06-08
- **Completed:** 2026-06-08
- **Tasks:** 5 (all autonomous)
- **Files modified:** 3

## Accomplishments

- Dark() palette fully migrated: Background #0F0F1A→#0D0D0D, Brand #7C3AED→#D77757, all 14 color values updated to warm-amber scheme
- Light() palette fully migrated: Brand #7C3AED→#D77757, all 14 color values updated with open-code inspired light values
- 6 new fields added to Theme struct: DividerChar, HeaderHeight, ShadowColor, CompactMode, SelectionBg, CardPadding
- 3 border constants standardized: NormalBorder (RoundedBorder), ThinBorder (┌─┐), DoubleBorder (╔═╗)
- applyThemeStyles() recalculated: Header (no background), Modal (NormalBorder), ToolCard (ThinBorder), InputArea (no border)
- Tests updated and passing with race detector

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate Dark() palette** — `a4ff809` (feat)
2. **Task 2: Migrate Light() palette** — `41362fd` (feat)
3. **Task 3: Add missing theme properties to Theme struct** — `fb31199` (feat)
4. **Task 4: Add border style constants** — `d3af34b` (feat)
5. **Task 5: Recalculate applyThemeStyles() for new colors** — `3401d5d` (refactor)

**Test update:** `84370c4` (test: update test expectations for new color palette)

## Files Created/Modified

- `internal/tui/theme/colors.go` — Dark() and Light() color migration, applyThemeStyles() changes, new field initialization, removed duplicate DoubleBorder
- `internal/tui/theme/theme.go` — 6 new Theme struct fields, NormalBorder/ThinBorder/DoubleBorder constants added, DoubleBorder moved from colors.go
- `internal/tui/theme/theme_test.go` — Updated all color expectations to match new palette values

## Decisions Made

- **Brand consistency:** Brand color `#D77757` is identical across Dark and Light themes — the brand should not shift color between modes
- **Header styling:** Header no longer has a full-width background, inheriting terminal background instead for cleaner visual integration
- **Modal borders demoted:** Modals now use NormalBorder (rounded) instead of DoubleBorder — only important/protected modals should use double-line borders
- **ToolCard compacted:** Tool cards use ThinBorder (straight lines) for a denser, more modern look
- **InputArea borderless:** The REPL input area has no border, inheriting terminal background for a cleaner appearance
- **SelectionBg differs per mode:** Dark (#3C4043), Light (#DADCE0) — each mode's selection color matches its surface tones

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Tests not updated for new palette**
- **Found during:** Verification (after Task 5)
- **Issue:** Existing tests expected old color values (e.g., `#0F0F1A` for Background, `#7C3AED` for Brand) — all would fail
- **Fix:** Updated 5 test assertions across 3 test functions to expect new palette values
- **Files modified:** internal/tui/theme/theme_test.go
- **Verification:** `go test -race -count=1 -cover ./internal/tui/theme/...` passes
- **Committed in:** 84370c4 (separate test commit)

**2. [Rule 2 - Missing Critical] ToolLabel map initialization preserved in Dark() and Light()**
- **Found during:** Task 1 (Dark migration)
- **Issue:** Plan's code block omitted `ToolLabel: make(map[string]lipgloss.Style)` — would cause nil map panic in `buildBadgeStyles()`
- **Fix:** Retained `ToolLabel` initialization in both Dark() and Light() Theme literals
- **Files modified:** internal/tui/theme/colors.go
- **Verification:** `buildBadgeStyles()` runs without panic, `go test -race` passes
- **Committed in:** a4ff809, 41362fd (part of task commits)

---

**Total deviations:** 2 auto-fixed (2 missing critical)
**Impact on plan:** Both auto-fixes necessary for correctness. Tests would have failed without update; nil map panic would crash on any tool card render. No scope creep.

## Issues Encountered

None — all tasks executed as planned with only the two auto-fixes described above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Theme & color system foundation complete — ready for 30-02 (Header Redesign) and 30-03 (Status Bar Redesign)
- All border constants and theme fields available for downstream phases

## Self-Check: PASSED

All verification criteria met:
- ✅ `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` — PASS
- ✅ `go vet ./internal/tui/...` — PASS
- ✅ `go test -race -count=1 -cover ./internal/tui/theme/...` — PASS (95.4% coverage)
- ✅ `go test -race -count=1 -cover ./internal/tui/...` — PASS
- ✅ All 6 commits verified in git log
- ✅ Summary file exists at `.planning/phases/30/30-01-SUMMARY.md`

---

*Phase: 30*
*Completed: 2026-06-08*
