# Phase 30 — OpenCode-Inspired TUI Visual Redesign

**Gathered:** 2026-06-08
**Status:** Ready for planning
**Source:** tui_plan.md design document + Phase 29 structural rewrite

## Phase Boundary

Visual redesign of the Phase 29 TUI rewrite to match opencode's polished aesthetic. All structural code is already in place from Phase 29 — this phase changes only colors, styles, layout, spacing, and visual hierarchy. No architectural changes.

## Current State (Post-Phase 29)

The TUI was rewritten from scratch with clean code architecture in Phase 29:
- 71 Go files in `internal/tui/`, 4 in `internal/tui/theme/`, 12+ in `internal/tui/components/`
- 16 screens, 25+ message types, full streaming pipeline, permission modal, workflow integration
- 537 tests passing with race detector, all 5 platforms building

**But** the visual design retains the original purple palette (`#7C3AED` brand, `#0F0F1A` bg) — the opencode-inspired warm-amber redesign from `tui_plan.md` was **not** implemented.

## What Must NOT Change

- `internal/types/` — all core types
- `internal/provider/` — LLMProvider interface, Registry
- `internal/tools/` — Dispatcher, PermissionRequest/Response
- `internal/workflow/` — Engine, PhaseResult
- `internal/config/` — Config struct
- `internal/errors/` — all sentinel errors
- `internal/tui/types.go` — screen enum, message types (but colors can change)
- `internal/tui/constants.go` — TUI layout constants
- Streaming logic in `repl_stream.go`, `streaming.go`
- Key binding logic in `keybindings.go`, `keybindings_screens.go`
- Screen routing pattern (`screen` enum, `renderActiveScreen()`)
- Theme Manager API (`NewManager`, `Current()`, `Cycle()`, `Mode`)

## Target Design

- **Brand**: `#7C3AED` (purple) → `#D77757` (warm amber)
- **Background**: `#0F0F1A` → `#0D0D0D` (true dark)
- **Surface**: `#1E1E2E` → `#1A1A1A`
- **Header**: 1 line, no version/health, git branch fallback when idle
- **Status bar**: 3-zone (`⌂ cwd  ⎇ branch · operation · hints cost`), no background fill
- **Tool cards**: thin borders, inline status
- **Screens**: consistent visual hierarchy with wave grouping, progress bars, stats grid
- **Typography**: consistent padding, truncation, 256-color fallback

## Implementation Decisions

### D-01: Incremental Changes Only
Each wave changes only the files listed in its plan. No structural rewrites — only visual refinements to existing working code.

### D-02: Theme Invalidation on Palette Change
When color constants change in `theme/colors.go`, all `applyThemeStyles()` computed styles recalculate automatically. Verify header, statusbar, toolcard, and screen styles update correctly.

### D-03: Screenshot Comparison Not Possible
This is a TUI — no visual regression testing. Verify visually by building and running `./m31a`. Use `go test -race -cover ./internal/tui/...` for functional correctness after each wave.

### D-04: Dependency Order
Waves are ordered so that later waves build on earlier ones:
- Wave 1 (Theme) → all other waves depend on correct palette
- Wave 2 (Header) → uses theme styles from Wave 1
- Wave 3 (Status Bar) → independent, but uses theme from Wave 1
- Wave 4 (REPL) → uses header, status bar from Waves 2/3
- Wave 5 (Screens) → uses theme, header from Waves 1/2
- Waves 6-10 → independent refinements

### D-05: Build Verification After Each Wave
```
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
go test -race -cover ./internal/tui/...
```

## Implementation Plan

This phase has 10 waves, mapped to 10 PLAN files:

| Plan | Wave | Title | Est. Time | Dependencies |
|------|------|-------|-----------|--------------|
| 30-01 | 1 | Theme & Color System | 1 day | None |
| 30-02 | 2 | Header Redesign | 1 day | 30-01 |
| 30-03 | 3 | Status Bar Redesign | 0.5 day | 30-01 |
| 30-04 | 4 | REPL Screen Polish | 2 days | 30-02, 30-03 |
| 30-05 | 5 | Screen-by-Screen Polish | 3 days | 30-01 |
| 30-06 | 6 | Component Library Enhancement | 2 days | 30-01 |
| 30-07 | 7 | Sidebar & Diff Polish | 1 day | 30-01 |
| 30-08 | 8 | Animation & Micro-interactions | 1 day | 30-01 |
| 30-09 | 9 | Typography & Spacing Audit | 1 day | 30-04, 30-05 |
| 30-10 | 10 | Accessibility & Terminal Compatibility | 0.5 day | 30-01 |

Total: ~13 developer-days

## Files Changed (by Wave)

| File | Wave | Change Type |
|------|------|-------------|
| `internal/tui/theme/colors.go` | 1 | Rewrite — palette migration |
| `internal/tui/theme/theme.go` | 1 | Extension — new fields |
| `internal/tui/header.go` | 2 | Rewrite — leaner layout |
| `internal/tui/statusbar.go` | 3 | Refine — 3-zone, no bg |
| `internal/tui/repl_view.go` | 4 | Refine — remove borders |
| `internal/tui/components/toolcard.go` | 4 | Rewrite — thin borders |
| `internal/tui/components/message.go` | 4 | Refine — labels, spacing |
| `internal/tui/components/thinking.go` | 4 | Rewrite — panel style |
| `internal/tui/repl_welcome.go` | 4 | Refine — minimal |
| `internal/tui/plan_model.go` | 5 | Rewrite — wave grouping |
| `internal/tui/execute_model.go` | 5 | Rewrite — progress bar |
| `internal/tui/execute_view.go` | 5 | Refine — live output |
| `internal/tui/verify.go` | 5 | Rewrite — checklist |
| `internal/tui/ship_model.go` | 5 | Rewrite — summary card |
| `internal/tui/ship_view.go` | 5 | Refine — stats grid |
| `internal/tui/settings_model.go` | 5 | Rewrite — left-nav |
| `internal/tui/settings_view.go` | 5 | Refine — card panels |
| `internal/tui/modelselector.go` | 5 | Rewrite — two-pane |
| `internal/tui/modelselector_view.go` | 5 | Refine — detail pane |
| `internal/tui/discuss.go` | 5 | Refine — dot progress |
| `internal/tui/goalinput.go` | 5 | Refine — minimal |
| `internal/tui/components/badge.go` | 6 | Rewrite — full component |
| `internal/tui/components/divider.go` | 6 | New — section divider |
| `internal/tui/components/card.go` | 6 | New — reusable card |
| `internal/tui/cmdpalette.go` | 6 | Refine — bottom-anchor |
| `internal/tui/app_view.go` | 6 | Refine — toast overlay |
| `internal/tui/sidebar.go` | 7 | Refine — git graph |
| `internal/tui/diff_view.go` | 7 | Refine — syntax highlight |
| `internal/tui/diff_model.go` | 7 | Refine — data prep |
| `internal/tui/components/spinner.go` | 8 | New — if missing |
| `internal/tui/components/progress.go` | 8 | Extension — animation |
| `internal/tui/app_update.go` | 8 | Extension — transitions |
| All View() methods | 9 | Refine — padding/spacing |
| `internal/tui/truncate.go` | 9 | Extension — consistency |
| `internal/tui/theme/colors.go` | 10 | Extension — fallback |
| `internal/tui/constants.go` | 10 | Extension — min widths |

## Acceptance Criteria (from tui_plan.md)

1. Brand color migrated to `#D77757`, all screens use new palette
2. Header is clean 1-line with brand + optional phase + model + context meter
3. Status bar shows cwd, branch, operation, hints, cost in 3-zone layout
4. Tool cards use thin borders instead of double borders
5. Message rendering uses lowercase user label, consistent spacing
6. Plan screen groups tasks by wave with visual hierarchy
7. Execute screen shows progress bar and live tool output
8. Verify screen shows pass/fail checklist with inline errors
9. Ship screen shows summary card with stats grid
10. Settings screen uses left-nav tabs with content panels
11. Model selector has two-pane layout with detail view
12. Discuss screen has dot progress and card-styled questions
13. Goal input is minimal with recent goals panel
14. Badge component used consistently across all screens
15. Divider component replaces all manual `strings.Repeat("─")`
16. Toast notifications appear as overlay, not inline text
17. Command palette is bottom-anchored with categories
18. Sidebar has cleaner section layout and git graph aesthetic
19. 256-color fallback works on limited terminals
20. Responsive layout adapts to < 60 column terminals
21. `CGO_ENABLED=0 go build` succeeds
22. `go test -race -cover ./internal/tui/...` passes

---

*Phase: 30-opencode-tui-redesign*
*Context gathered: 2026-06-08 from tui_plan.md + Phase 29 codebase analysis*
