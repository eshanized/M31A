---
phase: 12-ux-editor-experience-adaptations
plan: 01
subsystem: model-management
tags: model-variants, favorites, keybindings, config, session

requires:
  - phase: 07-signature-features
    provides: Model selector UI, keybinding system, config loader

provides:
  - Variant field on ModelInfo for capability modes (thinking/fast/extended/vision)
  - Recent/favorite model persistence to ~/.m31a/recent_models.json
  - Ctrl+M / Ctrl+Shift+M keyboard cycling through recent models
  - Per-agent model assignment in config.toml under [agents]
  - Model selector variant badge, favorite star indicator, f key toggle

affects: shell mode, prompt history

tech-stack:
  added: []
  patterns:
    - JSON file persistence with atomic writes for model metadata
    - Bubble Tea direct keybinding for model cycling (CtxREPL context)

key-files:
  created: []
  modified:
    - internal/types/types.go — ModelInfo.Variant *string field
    - internal/config/types.go — Config.Agents field, AgentsConfig struct
    - internal/config/loader.go — no structural changes needed (TOML reflection)
    - internal/tui/modelselector.go — variant badge, favorite star, f key toggle
    - internal/tui/keybindings.go — Ctrl+M / Ctrl+Shift+M bindings
    - internal/tui/app.go — cycleRecentModel handler
    - pkg/session/manager.go — RecentModelsData persistence methods

key-decisions:
  - "Variant *string with json:\"variant,omitempty\" — nil by default. Absent from JSON when nil for backward compat."
  - "Recent models stored in ~/.m31a/recent_models.json alongside session dir. Managed via Manager.atomicWrite."
  - "Recent list capped at 10 entries with dedup. Favorites stored as map[string]bool in same file."
  - "Ctrl+M cycles forward, Ctrl+Shift+M cycles backward through recent models. Only active in CtxREPL."
  - "Existing ctrl+x m chord retained as global fallback outside REPL context."
  - "AgentsConfig as simple struct with Default + 6 phase fields. TOML [agents] section mapped by reflection."

requirements-completed: [ADOPT-08]

duration: 4min
completed: 2026-06-01
---

# Phase 12 Plan 01: Model Variants & Favorites Summary

**Model variant classification, recent/favorite model persistence, keyboard model cycling, and per-agent config assignment**

## Performance

- **Duration:** 4 min
- **Started:** 2026-06-01T00:24:23Z
- **Completed:** 2026-06-01T00:28:25Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `Variant *string` field to `ModelInfo` for capability mode classification (thinking/fast/extended/vision)
- Added `AgentsConfig` struct with per-phase model ID assignment fields (Default + 6 workflow phases)
- Implemented `RecentModelsData` persistence with `LoadRecentModels`, `SaveRecentModels`, `AddRecentModel`, `ToggleFavorite`, `IsFavorite` — decoupled, capped at 10, atomic writes
- Added `Ctrl+M` (forward) and `Ctrl+Shift+M` (backward) REPL keybindings for cycling through recent models
- Added `cycleRecentModel` handler in `AppState` that wraps around the recent list
- Updated model selector with variant badge (dimmed italic), favorite star indicator (brand color), and `f` key toggle

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Variant field and per-agent config** - `090a9b7` (feat)
2. **Task 2: Recent/favorite persistence and keyboard cycling** - `5fa05c5` (feat)
3. **Task 3: Model selector UI updates** - `a24262f` (feat)

## Files Created/Modified

- `internal/types/types.go` - Added `Variant *string` to ModelInfo struct
- `internal/config/types.go` - Added `Agents AgentsConfig` field and `AgentsConfig` struct definition
- `internal/tui/modelselector.go` - Updated with variant badge, favorite star, `f` key, manager integration
- `internal/tui/keybindings.go` - Added `ctrl+m` and `ctrl+shift+m` bindings for CtxREPL
- `internal/tui/app.go` - Added `cycleRecentModel` handler, updated `NewModelSelector` call sites, added key action cases
- `internal/tui/modelselector_test.go` - Updated for new `NewModelSelector` signature
- `pkg/session/manager.go` - Added `RecentModelsData`, persistence methods, `recentModelsPath`

## Decisions Made

- `Variant *string` with `json:"variant,omitempty"` — nil by default for backward-compatible serialization
- Recent models file at `~/.m31a/recent_models.json`, managed via the same `Manager.atomicWrite` pattern used for session files
- `Ctrl+M` direct keybinding only active in `CtxREPL` (textarea focused); `ctrl+x m` chord retained as global fallback
- `AgentsConfig` uses simple string fields — TOML `[agents]` section mapped by `BurntSushi/toml` via reflection

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Model variant and favorites infrastructure ready
- Ready for Plan 02 (Shell Mode) and subsequent Wave 2 plans (Prompt History, Diff Viewer, Editor Context)

---

*Phase: 12-ux-editor-experience-adaptations*
*Completed: 2026-06-01*
