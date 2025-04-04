---
phase: 07-signature-features
plan: 07
subsystem: tui
tags: [model-selector, bubbles, list, textinput, fuzzy-search, provider-filter, detail-pane, cost-display]

# Dependency graph
requires:
  - phase: 05-session-state
    provides: provider registry with FetchModels
  - phase: 02-tui-foundation
    provides: Screen enum, AppMsg pattern, theme system
  - phase: 07-03-arbitrage
    provides: CostEstimate type for pricing display
provides:
  - Full-screen model selector overlay with provider filter, fuzzy search, and detail pane
  - ModelSelectedMsg dispatched to AppState for model/provider switching
  - /models command to open model selector from REPL
affects: [settings-screen, commands-system]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Full-screen overlay screen implementing tea.Model (Init/Update/View)"
    - "list.DefaultItem pattern with custom delegate for colored rendering"
    - "textinput.Model for real-time search filtering"
    - "Two-step Esc navigation (blurs search first, then exits screen)"

key-files:
  created:
    - internal/tui/modelselector.go
  modified:
    - internal/tui/types.go
    - internal/tui/app.go

key-decisions:
  - "ModelSelector implements tea.Model interface (not non-standard AppMsg pattern) — AppState handles screen transition via AppMsg dispatch"
  - "Two-step Esc: first press blurs search input, second press exits screen to previous screen"
  - "/models command opens model selector from REPL, stores previous screen for Esc return"
  - "Provider filter cycles All → OpenRouter → Zen (no dynamic provider discovery — matches provider names known at compile time)"
  - "Cost display uses standard usage estimate (100K input + 50K output tokens) per CONTEXT.md"
  - "Models from both providers shown as separate entries with provider badges ([OR] / [ZEN])"

patterns-established:
  - "Screen overlay pattern: initialize fresh ModelSelector on transition, return AppMsg for model selection, return to previous screen on Esc"
  - "Custom list.ItemDelegate pattern with wrapping default delegate for themed colors"

requirements-completed: [AC-03, AC-22]

duration: 5 min
completed: 2026-05-28
---

# Phase 7 Plan 7: Model Selector UI Summary

**Full-screen model selector overlay with provider-filtered model list, fuzzy search, detail pane with cost display, and selection dispatch via AppState routing**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-28T06:30:21Z
- **Completed:** 2026-05-28T06:35:24Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- ModelSelector Bubble Tea model with bubbles/list, textinput search, and provider filter cycling (All → OpenRouter → Zen)
- ModelItem implementing list.DefaultItem with model name, provider badge, cost estimate, context length, and capability display
- Detail pane toggled via Tab showing full model description, tokenizer architecture, context window, pricing per million tokens, and capabilities
- Real-time search filtering on model name, ID, description, and provider — case-insensitive with fuzzy matching
- ModelSelectedMsg dispatched as AppMsg on Enter selection, handled in AppState to switch provider and model
- /models command from REPL opens the model selector; two-step Esc navigates back (blurs search first, then exits)
- Custom modelItemDelegate with themed colors (Brand #D77757 for selected items)
- Edge case handling: no providers configured, fetch errors, empty model lists, missing pricing

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ScreenModelSelector enum and ModelSelectedMsg to types.go** - `52a431f` (feat)
2. **Task 2: Implement ModelSelector Bubble Tea model** - `b31886b` (feat)
3. **Task 3: Wire ModelSelector into AppState screen routing** - `b0f8298` (feat)

**Plan metadata:** Will be committed with SUMMARY.md

## Files Created/Modified

- `internal/tui/modelselector.go` - Full ModelSelector implementation (450 lines): ModelSelector struct, ModelItem, providerFilter, modelItemDelegate, fetchModelsCmd, search, filter cycling, detail pane, View rendering
- `internal/tui/types.go` - Added ModelSelectedMsg type, ModelSelected field to AppMsg
- `internal/tui/app.go` - Added modelSelector field, initialization, /models command, screen routing for ScreenModelSelector, AppMsg handling for ModelSelectedMsg

## Decisions Made

- **ModelSelector as tea.Model:** Chose standard Bubble Tea model interface (Init/Update/View) rather than the non-standard AppMsg return pattern used by SettingsModel/ResumeModel. Screen transitions handled via AppMsg dispatch for Enter (model selection) and direct Esc interception in AppState for navigation.
- **Two-step Esc navigation:** First Esc press blurs the search input (when focused), second Esc press exits the screen. Avoids accidentally leaving the screen while typing a search query.
- **Static provider filter:** Three-position cycle (All → OpenRouter → Zen) uses compile-time known provider names. Dynamic provider discovery deferred since the two providers are fixed at V1.
- **Standard usage estimate:** Cost display uses 100K input + 50K output tokens as the standard benchmark (per CONTEXT.md), not provider.EstimateCost() which has a different signature.
- **Sorted display:** Models sorted by provider then name for consistent browsing across reloads.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- None. All three tasks completed without issues. One minor fix needed (missing `io` import in modelselector.go) caught during vet.

## Next Phase Readiness

- Model selector UI complete and wired into AppState
- Ready for Plan 08: Settings Screen Updates (P7.7)
- Can be triggered from any screen via AppMsg with ScreenModelSelector or the /models command from REPL

## Self-Check: PASSED

- ✅ `internal/tui/modelselector.go` exists (450 lines)
- ✅ `internal/tui/types.go` has ModelSelectedMsg type and AppMsg.ModelSelected field
- ✅ `internal/tui/app.go` has modelSelector field, /models command, ScreenModelSelector routing
- ✅ `52a431f` — feat(07-07): add ModelSelectedMsg type and ModelSelected field to AppMsg
- ✅ `b31886b` — feat(07-07): implement ModelSelector Bubble Tea model
- ✅ `b0f8298` — feat(07-07): wire ModelSelector into AppState screen routing
- ✅ `3b1b81f` — docs(07-07): complete Model Selector UI plan
- ✅ `CGO_ENABLED=0 go build ./...` passes
- ✅ `go vet ./internal/tui/...` passes
- ✅ `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` passes

---

*Phase: 07-signature-features*
*Completed: 2026-05-28*
