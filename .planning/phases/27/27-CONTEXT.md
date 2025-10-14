# Phase 27 — TUI Component Decomposition — Context

**Gathered:** 2026-06-07
**Status:** Ready for planning
**Source:** Codebase analysis + pattern study

<domain>
## Phase Boundary

Split large TUI files in `internal/tui/` into focused modules without breaking the project. Follow existing decomposition patterns (model/view/state separation). No single file should exceed 400 lines after decomposition.

</domain>

<decisions>
## Implementation Decisions

### D-01: Decomposition Strategy
Split by concern following existing patterns:
- `*_model.go` — struct definition, constructor, Update(), domain logic
- `*_view.go` — View() and all render* helper methods
- `*_state.go` — getter/setter methods, state mutation helpers
- `*_tabs.go` — tab-specific renderers (for Settings)

### D-02: No Import Path Changes
All files remain in `internal/tui/` package. No new sub-packages created. This ensures all existing test files pass without modification.

### D-03: Wave Ordering
- Wave 1: Core app files (app.go, app_update.go, app_update_workflow.go) — most referenced, highest risk
- Wave 2: Settings & REPL — large files but self-contained
- Wave 3: Screen models — independent of each other
- Wave 4: Verification — ensure everything compiles and tests pass

### D-04: Test Compatibility
Test files are NOT split. They continue testing the same types and functions. Since all files remain in the same package, test imports work unchanged.

### D-05: File Size Target
Maximum 400 lines per file after decomposition. Files currently under 300 lines are not candidates.

### the agent's Discretion
- Exact split boundaries within each file
- Whether to extract additional helper functions into separate files
- Naming of extracted files beyond the model/view/state convention

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Decomposition Patterns
- `internal/tui/modelselector.go` + `modelselector_list.go` + `modelselector_view.go` — exemplar model/list/view split
- `internal/tui/repl.go` + `repl_view.go` + `repl_stream.go` + `repl_thinking.go` + `repl_commands.go` — exemplar multi-file decomposition
- `internal/tui/app.go` + `app_update.go` + `app_update_workflow.go` + `app_view.go` + `app_workflow.go` — exemplar app decomposition

### Type Definitions
- `internal/tui/types.go` — Screen enum, message types, AppMsg definitions
- `internal/tui/keybindings.go` — KeyContext, KeyAction, KeyBinding, KeyRegistry

### Test Files (must not break)
- `internal/tui/app_test.go` (2642 lines) — largest test file, tests AppState
- `internal/tui/settings_test.go` (957 lines) — tests SettingsModel
- `internal/tui/repl_test.go` (1023 lines) — tests ReplModel
- `internal/tui/commands_test.go` (2227 lines) — tests command handlers

</canonical_refs>

<specifics>
## Specific Ideas

### Files to Split (by priority)

**HIGH PRIORITY (>800 lines):**
1. `app_update.go` (1157 lines) — Extract interactive handlers (permission, question, discuss) to `app_update_interactive.go`
2. `settings.go` (1103 lines) — Split into `settings_model.go` + `settings_view.go` + `settings_tabs.go`
3. `repl.go` (937 lines) — Extract struct + constructor to `repl_model.go`, getters/setters to `repl_state.go`
4. `resume.go` (861 lines) — Split into `resume_model.go` + `resume_view.go`
5. `firstrun.go` (804 lines) — Split into `firstrun_model.go` + `firstrun_view.go`
6. `app.go` (794 lines) — Extract AppState + NewApp to `app_state.go`, channel types to `app_channel.go`

**MEDIUM PRIORITY (500-800 lines):**
7. `app_update_workflow.go` (637 lines) — Extract permission handlers to `app_update_permission.go`
8. `plan.go` (589 lines) — Split into `plan_model.go` + `plan_view.go`
9. `execute.go` (521 lines) — Split into `execute_model.go` + `execute_view.go`
10. `repl_view.go` (535 lines) — Extract `renderWelcome()` to `repl_welcome.go`

**LOWER PRIORITY (400-500 lines):**
11. `diff.go` (436 lines) — Split into `diff_model.go` + `diff_view.go`
12. `ship.go` (427 lines) — Split into `ship_model.go` + `ship_view.go`

</specifics>

<deferred>
## Deferred Ideas

- Splitting test files (not needed — test files can remain large)
- Creating new sub-packages (would require import path changes)
- Refactoring component interfaces (out of scope — this is file organization only)
- Splitting files under 300 lines (already acceptable size)

</deferred>

---

*Phase: 27-tui-component-decomposition*
*Context gathered: 2026-06-07 via codebase analysis*
