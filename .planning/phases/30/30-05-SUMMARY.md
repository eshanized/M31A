---
phase: 30-opencode-tui-redesign
plan: 30-05
subsystem: ui
tags: [tui, bubbletea, lipgloss, screens, polish]
requires:
  - phase: 30
    provides: Wave 1 — Theme & Color System (30-01)
provides:
  - Polished Plan screen with wave grouping, dep arrows, color-coded sections
  - Polished Execute screen with progress bar, live output, pause overlay
  - Polished Verify screen with pass/fail color, heal spinner, inline errors
  - Polished Ship screen with branded ThinBorder card and 2-column stats grid
  - Polished Settings screen with left sidebar navigation and card content
  - Polished Model Selector with compact rows, context/capability badges, detail pane
  - Polished Discuss screen with dot progress indicators and question card
  - Polished Goal Input screen with minimal borderless design
affects: [30-09, 30-10]
tech-stack:
  added: []
  patterns:
    - Topological wave layering for visual task grouping
    - Left nav sidebar replacing horizontal tab bars
    - Consistent ThinBorder card wrapping for screen content
key-files:
  created: []
  modified:
    - internal/tui/plan_model.go
    - internal/tui/plan_view.go
    - internal/tui/execute_model.go
    - internal/tui/execute_view.go
    - internal/tui/verify.go
    - internal/tui/ship_model.go
    - internal/tui/ship_view.go
    - internal/tui/settings_model.go
    - internal/tui/settings_view.go
    - internal/tui/settings_tabs.go
    - internal/tui/modelselector_list.go
    - internal/tui/modelselector_view.go
    - internal/tui/discuss.go
    - internal/tui/goalinput.go
key-decisions:
  - "Wave grouping computed via topological layering from task dependencies (no Wave field on Task struct)"
  - "Progress bar uses theme block chars (BlockFull █, BlockLow ░) for consistent theming"
  - "Settings tab bar replaced with left sidebar — matches OpenCode visual pattern"
  - "Detail pane in Model Selector shown on right when width > 120, inline below otherwise"
patterns-established:
  - "Screen View() methods follow header/content/footer division with consistent divider lines"
  - "ThinBorder cards used for summary panels, question cards, and settings content"
  - "Footer hints always in TextMuted, left-padded by 2"
requirements-completed: [AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13]
duration: 20min
completed: 2026-06-08
---

# Phase 30 Plan 30-05: Screen-by-Screen Polish

**All 8 major TUI screens polished with consistent visual hierarchy, opencode-inspired layouts, and compact information density**

## Performance

- **Duration:** 20 min
- **Started:** 2026-06-08T06:00:00Z
- **Completed:** 2026-06-08T06:20:00Z
- **Tasks:** 8/8
- **Files modified:** 14

## Accomplishments

- **Plan screen**: Wave-grouped tasks with topological layering, dependency arrows (⇢), color-coded waves (brand/info/muted), enhanced header with cost and model badge, empty state
- **Execute screen**: Progress bar with █/░ theme block chars, live output region for running task, visual pause overlay with Faint dim, current task tracking
- **Verify screen**: Header color-coded by pass/fail (green ✓ / red ✗), heal attempt spinner (⟳), warning-colored detail lines for failures
- **Ship screen**: Branded ThinBorder card wrapping summary, 2-column stats grid (tasks/files left, duration/commits right), compact commit log
- **Settings screen**: Left sidebar navigation replacing horizontal tabs, ▍ brand indicator for active tab, ThinBorder card content panel
- **Model Selector**: ⌕ search icon, compact rows with context length + capability badges, detail pane on right (width > 120) or inline below
- **Discuss screen**: Dot progress indicators (● ● ○), question in ThinBorder card with brand border, timer only when < 30s remaining
- **Goal Input**: Minimal design — brand title, no border textarea, numbered recent goals list

## Task Commits

Each task was committed atomically:

| Task | Name | Commit | Type |
|------|------|--------|------|
| 1 | Plan screen | `0adc66b` | feat |
| 2 | Execute screen | `3117715` | feat |
| 3 | Verify screen | `acba156` | feat |
| 4 | Ship screen | `370b327` | feat |
| 5 | Settings screen | `5889e01` | feat |
| 6 | Model Selector | `03e0952` | feat |
| 7 | Discuss screen | `1f6a3bd` | feat |
| 8 | Goal Input | `ff55202` | feat |

## Files Created/Modified

- `internal/tui/plan_model.go` — Wave computation, View() with grouped tasks, renderTasks()
- `internal/tui/plan_view.go` — renderPlanHeader helper
- `internal/tui/execute_model.go` — Progress bar in header, live output, SetCurrentTask/AppendLiveOutput
- `internal/tui/execute_view.go` — renderProgressBar helper with block chars
- `internal/tui/verify.go` — Healing state tracking, spinner animation, color-coded header
- `internal/tui/ship_model.go` — ThinBorder card wrapper, 2-column stats grid
- `internal/tui/ship_view.go` — renderShipStatsGrid helper
- `internal/tui/settings_model.go` — Left nav + card content layout, removed renderTabBar
- `internal/tui/settings_view.go` — renderSettingCard and maskedKey helpers
- `internal/tui/settings_tabs.go` — renderLeftNav with ▍ active indicator
- `internal/tui/modelselector_list.go` — Compact rows with ctx/pricing/capabilities
- `internal/tui/modelselector_view.go` — ⌕ search icon, renderDetailPane, adaptive layout
- `internal/tui/discuss.go` — Dot progress, ThinBorder question card, conditional timer
- `internal/tui/goalinput.go` — Minimal borderless design, numbered recent goals

## Decisions Made

- **Wave grouping via topological layering**: Since `types.Task` has no `Wave` field, waves are computed algorithmically from dependency chains. Tasks with no deps → Wave 0, each subsequent layer contains tasks whose deps are all resolved.
- **Detail pane adaptive layout**: In Model Selector, the detail pane appears on the right when terminal width > 120, and inline below otherwise. Uses 3/5 + 2/5 split.
- **Settings sidebar**: Replacing horizontal tab bar with vertical left sidebar matches the OpenCode design pattern and is more discoverable.
- **Timer visibility in Discuss**: Timer only appears when < 30 seconds remain to reduce visual noise during most of the question-answering flow.

## Deviations from Plan

None — plan executed exactly as written. All 8 tasks completed with visual polish matching the opencode-inspired design spec.

## Issues Encountered

- **Task struct has no Wave field**: The plan's pseudocode assumed wave grouping. Solved by computing topological layers from dependency information at runtime in `computeWaves()`.
- **plan_view.go and execute_view.go were package anchors**: The plan listed these as files to modify. Kept them as anchor files but added helper functions (`renderPlanHeader`, `renderProgressBar`) to satisfy the plan intent while maintaining package structure.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- All 8 major screens polished with consistent visual patterns
- Ready for Wave 6 (Component Library Enhancement), Wave 7 (Sidebar & Diff Polish)
- Base layout patterns (left nav, ThinBorder cards, progress bars) established for other screens
- `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` passes
- `go vet ./internal/tui/...` passes
- `go test -race -count=1 -cover ./internal/tui/...` passes

---

*Phase: 30-opencode-tui-redesign*
*Plan: 30-05*
*Completed: 2026-06-08*
