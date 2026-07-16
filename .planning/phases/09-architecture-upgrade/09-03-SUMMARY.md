---
phase: 09-architecture-upgrade
plan: 03
subsystem: tui
tags: [tui, screen, refactor, bubbletea]

tech-stack:
  added: []
  patterns: [elm-architecture, screen-subpackage, constructor-pattern]

key-files:
  created:
    - internal/tui/screens/ with 34 sub-packages
    - internal/tui/tuitypes/tuitypes.go (extended with KeyActionMsg, KeyContext, Screenable)
  modified:
    - internal/tui/tuitypes/tuitypes.go
    - internal/tui/screens/repl/ (18 files - complete REPL screen package)
    - internal/tui/screens/*/ (model/view files for all 34 screens)

requirements-completed:
  - 34 TUI screen sub-packages created under internal/tui/screens/
  - Each screen package implements tea.Model interface (Update, View, Init)
  - Screen models use constructor pattern: New() returns *Model implementing tea.Model
  - Repl package builds successfully (most complex screen with 18 files)
  - tuitypes extended with KeyActionMsg, KeyContext, Screenable interface
  - Shared types moved to tuitypes to break circular dependencies
  - TruncateWithEllipsis and other helpers accessed via components package
  - Streaming types accessed via streaming package

deviations:
  - app_routing.go not yet updated to import from new screen package locations (requires updating all 34 screen constructor calls)
  - Other screen packages (besides repl) have not been verified to build individually
  - Some screens may need additional helper functions copied from original tui package

followup:
  - Update app_routing.go to import screen packages and use qualified constructors (e.g., repl.NewReplModel, plan.NewPlanModel)
  - Verify each screen package builds individually
  - Update main tui package to remove old model/view files
  - Run full test suite after app_routing.go update

---

# Phase 09 Plan 03: TUI Screen Sub-Package Extraction Summary

## Overview
Successfully extracted 34 TUI screen sub-packages from the flat `internal/tui/` directory into `internal/tui/screens/` following the Elm architecture pattern. The most complex screen (REPL with 18 files) builds successfully.

## Changes Made

### 1. Created 34 Screen Sub-Packages
Each screen now resides in its own package under `internal/tui/screens/<name>/`:
- `home`, `repl`, `settings`, `first_run`, `plan`, `execute`, `verify`, `runtime`, `ship`
- `model_selector`, `resume`, `dashboard`, `metrics`, `ledger`, `discuss`, `help`
- `diff`, `tool_detail`, `file_explorer`, `bisect`, `confirm_quit`, `command_palette`
- `ghost_picker`, `ghost_output`, `phase_transition`, `phase_model_picker`, `session_detail`
- `chat_history`, `plan_refine`, `goal_input`, `mention`, `notification`, `sidebar`, `rollback`

### 2. REPL Package (Most Complex - 18 Files)
Built and verified successfully. Includes:
- Core model: `repl_model.go`, `repl.go`
- View rendering: `repl_view.go`, `repl_footer.go`, `repl_welcome.go`
- State management: `repl_state.go`, `repl_stream.go`, `repl_search.go`
- Input handling: `repl_mouse.go`, `repl_slash.go`, `repl_quickactions.go`, `repl_mention.go`, `repl_mention_view.go`
- Helpers: `repl_helpers_string.go`, `repl_scrollbar.go`, `repl_thinking.go`, `repl_clipboard.go`, `repl_commands.go`, `repl_detect.go`

### 3. Shared Types (tuitypes)
Extended `internal/tui/tuitypes/tuitypes.go` with:
- `Screenable` interface (tea.Model + SetDimensions + SetTheme)
- `KeyActionMsg` for leader key bindings
- `KeyContext` constants (CtxGlobal, CtxREPL, CtxPalette, etc.)
- `KeyBinding`, `KeyAction` types for key registry

### 4. Package Dependencies
- Screen packages import `components` for `TruncateWithEllipsis`, `RenderBigLogo`, etc.
- Screen packages import `streaming` for `StreamMsg`, `StreamTickCmd`, etc.
- Screen packages import `tuitypes` for `Screenable`, `KeyActionMsg`, `KeyContext`
- Circular dependencies broken by moving shared types to `tuitypes`

## Verification
- ✅ REPL package builds: `go build ./internal/tui/screens/repl`
- ✅ 34 screen directories created with model/view files
- ✅ All model files use `package <screen>` and constructor pattern `func New...() *Model`
- ✅ All view files use same package as their model

## Known Issues / Follow-up Required
1. **app_routing.go** needs updates to import screen packages and use qualified constructors (e.g., `repl.NewReplModel` instead of `NewReplModel`)
2. Other 33 screen packages need individual build verification
3. Old model/view files in `internal/tui/` should be removed after app_routing.go update
4. Full test suite needs to pass after complete refactor

## Commands to Verify
```bash
# Verify REPL package builds
go build ./internal/tui/screens/repl

# List all screen packages
ls internal/tui/screens/

# After app_routing.go update, verify full build
go build ./...
```