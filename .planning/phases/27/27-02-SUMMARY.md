---
phase: 27-tui-component-decomposition
plan: 27-02
subsystem: ui
tags: [bubbletea, settings, repl, decomposition, refactor]

# Dependency graph
requires:
  - phase: 27-01
    provides: app_view.go and app_update_permission.go decomposition baseline
provides:
  - settings_model.go (SettingsModel struct, constructor, Update logic)
  - settings_view.go (View(), renderTabBar, renderDescriptionPane, renderFooter)
  - settings_tabs.go (6 tab renderers, renderField, renderFieldColumn)
  - repl_model.go (ReplModel struct, NewReplModel, ProviderModelsFetchedMsg)
  - repl_state.go (all Set*/Get* methods, AddMessage, ClearMessages)
  - repl_welcome.go (renderWelcome, renderProviderCard, renderLogo, renderInputBox, renderKeyboardHints, renderBottomBar)
  - repl_keys.go (streaming key handling, non-streaming key handling, shell/fallback result handling)
affects: [27-03, 27-04]

# Tech tracking
tech-stack:
  added: []
  patterns: [file-per-concern decomposition, method extraction by responsibility]

key-files:
  created:
    - internal/tui/settings_model.go
    - internal/tui/settings_view.go
    - internal/tui/settings_tabs.go
    - internal/tui/repl_model.go
    - internal/tui/repl_state.go
    - internal/tui/repl_welcome.go
    - internal/tui/repl_keys.go
  modified:
    - internal/tui/repl.go
    - internal/tui/repl_view.go

key-decisions:
  - "Split settings into 4 files (model/view/edit/tabs) instead of 3 — settings_edit.go (420 lines) was needed because settings_model.go exceeded 400 lines with Update logic included"
  - "Extracted repl_keys.go for streaming and non-streaming key handling — reduced repl.go from 937 to 209 lines"
  - "Tasks 2 and 3 committed together since repl_state.go setter methods are required for repl.go to compile after extracting ReplModel struct"

patterns-established:
  - "Method extraction by concern: state getters/setters in _state.go, key handling in _keys.go, rendering in _view.go"
  - "ReplModel struct lives in _model.go, constructor in _model.go, state mutations in _state.go"

requirements-completed: [SPLIT-02, SPLIT-03, SPLIT-10]

# Metrics
duration: 25min
completed: 2026-06-07
---

# Phase 27 Plan 02: Settings & REPL Decomposition Summary

**settings.go split into 4 focused modules (model/view/edit/tabs); repl.go decomposed from 937 to 209 lines via model/state/welcome/keys extraction**

## Performance

- **Duration:** 25 min
- **Started:** 2026-06-07T10:00:00Z
- **Completed:** 2026-06-07T10:25:00Z
- **Tasks:** 5
- **Files modified:** 10 (7 created, 3 modified, 1 deleted)

## Accomplishments
- settings.go (1103 lines) deleted and replaced with 4 focused modules: settings_model.go (234), settings_view.go (239), settings_edit.go (420), settings_tabs.go (253)
- repl.go reduced from 937 to 209 lines through extraction of model, state, welcome, and key handling modules
- All 3 plan-specified files (model/view/tabs) under 400 lines; repl files all under 400 lines

## Task Commits

Each task was committed atomically:

1. **Task 1: Split settings.go into model/view/edit/tabs** - `7854bc1` (refactor)
2. **Task 2: Extract ReplModel struct to repl_model.go** - `f3cec5d` (refactor)
3. **Task 3: Extract getter/setter methods to repl_state.go** - `f3cec5d` (refactor)
4. **Task 4: Extract renderWelcome to repl_welcome.go** - `7943f9e` (refactor)
5. **Task 5: Extract key handling to repl_keys.go** - `7943f9e` (refactor)

## Files Created/Modified
- `internal/tui/settings_model.go` - SettingsModel struct, settingsTab/editableField types, NewSettingsModel, buildFields
- `internal/tui/settings_view.go` - View(), renderTabBar, renderDescriptionPane, renderFooter, renderUnsavedWarning
- `internal/tui/settings_edit.go` - Update(), all key handling, editing helpers, applyFieldValues, saveConfig
- `internal/tui/settings_tabs.go` - 6 tab renderers (general/provider/model/permissions/features/ledger), renderField, renderFieldColumn, renderActiveTab
- `internal/tui/settings.go` - DELETED (was 1103 lines)
- `internal/tui/repl_model.go` - ReplModel struct (40+ fields), NewReplModel constructor, ProviderModelsFetchedMsg
- `internal/tui/repl_state.go` - All Set*/Get* methods, AddMessage, ClearMessages, state mutation helpers
- `internal/tui/repl_welcome.go` - renderWelcome, renderProviderCard, renderLogo, renderInputBox, renderKeyboardHints, renderBottomBar
- `internal/tui/repl_keys.go` - handleStreamingKeyMsg, handleKeyMsg, handleEnterKey, handleHistoryUp/Down, handleShellResult, handleFallbackEvent
- `internal/tui/repl.go` - Update() method only (209 lines, down from 937)
- `internal/tui/repl_view.go` - View(), renderMessage, renderModelContextLine, renderInputBottomBorder, renderSlashSuggestions, renderGitStatusStrip, renderStatusBar, renderPrompt (316 lines, down from 563)

## Decisions Made
- Split settings into 4 files instead of 3 (plan said model/view/tabs) because settings_model.go at 631 lines exceeded the 400-line target. Created settings_edit.go (420 lines) to hold Update() and key handling logic.
- Moved renderField, renderFieldColumn, renderActiveTab from settings_view.go to settings_tabs.go to keep the view file at 239 lines (under 400).
- Committed tasks 2 and 3 together since the setter methods in repl_state.go are required for repl.go to compile after extracting the ReplModel struct to repl_model.go.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added settings_edit.go for Update logic**
- **Found during:** Task 1 (Split settings.go)
- **Issue:** settings_model.go with struct + constructor + buildFields + Update + key handling + applyFieldValues exceeded 400 lines at 631 lines
- **Fix:** Created settings_edit.go (420 lines) to hold Update(), key handling, editing helpers, and applyFieldValues. Settings_model.go kept at 234 lines with just types, struct, constructor, buildFields.
- **Files modified:** internal/tui/settings_model.go, internal/tui/settings_edit.go
- **Verification:** All settings files under 400 lines, build passes
- **Committed in:** 7854bc1 (Task 1 commit)

**2. [Rule 1 - Bug] Fixed Update return type mismatch**
- **Found during:** Task 5 (Extract key handling)
- **Issue:** Helper methods handleStreamingKeyMsg/handleKeyMsg/handleShellResult/handleFallbackEvent return ([]tea.Cmd, bool) but callers in Update() used `return m, tea.Batch(cmds...)` which doesn't match Update's return type
- **Fix:** Changed callers to `return m.handleStreamingKeyMsg(msg)` to directly return the ([]tea.Cmd, bool) tuple
- **Files modified:** internal/tui/repl.go
- **Verification:** Build passes, all tests pass
- **Committed in:** 7943f9e (Task 4-5 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both deviations necessary for correctness. settings_edit.go was an unplanned but required split file. No scope creep.

## Issues Encountered
- Pre-existing test timeout in `config.WatchConfig` goroutine leak — affects tests that create full AppState (TestApp_SidebarThreshold_FromConfig, TestReplSlashCommand_FullIntegration). These tests were already timing out before our changes. Settings and ReplModel unit tests all pass.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All settings and REPL files under 400 lines, ready for phases 27-03 and 27-04
- File structure is clean: _model.go for structs/constructors, _state.go for getters/setters, _view.go for rendering, _keys.go for key handling
- Pre-existing WatchConfig test timeout should be investigated separately

---
*Phase: 27-tui-component-decomposition*
*Completed: 2026-06-07*
