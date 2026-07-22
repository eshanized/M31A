# Phase 3 Planning Complete

## Summary

Phase 3: Internal Package Organization has been planned with 3 plans across 3 waves.

## Plans Created

### Plan 03-01: Tools Reorganization (Wave 1)
- **Objective:** Move tool implementations to subdirectories per D-01 and D-02
- **Tasks:**
  1. Create new tool subdirectories (git/, todo/, codeanalysis/, network/) and move files
  2. Update tool registration and imports in defaults.go, dispatcher.go, tools_reexport.go
  3. Update test files and verify compilation
- **Files:** 16 tool files moved to new locations

### Plan 03-02: TUI Reorganization (Wave 2)
- **Objective:** Group TUI files by responsibility per D-04 and D-05
- **Tasks:**
  1. Create TUI responsibility directories (core/, handlers/, input/, update/, routing/) and move core files
  2. Move handler and input files
  3. Move update and routing files
- **Files:** 30 TUI files moved to new directories

### Plan 03-03: Import Updates and Test Consolidation (Wave 3)
- **Objective:** Update all import paths and consolidate tests per D-09, D-11, D-12
- **Tasks:**
  1. Update import paths across codebase using sed/goimports
  2. Consolidate test files into tests/ directory structure
  3. Final verification and cleanup
- **Files:** All files importing internal/tools or internal/ui/tui

## Wave Structure

- **Wave 1:** Tools reorganization (autonomous)
- **Wave 2:** TUI reorganization (depends on Wave 1)
- **Wave 3:** Import updates and test consolidation (depends on Wave 2)

## Locked Decisions Implemented

- D-01: Move ALL tool implementations to subdirectories
- D-02: Group related tools into shared subdirectories
- D-04: Group TUI by responsibility
- D-05: Split app_*.go files by function
- D-08: Ideal structure - move everything to proper home
- D-09: Use sed/goimports to fix all imports
- D-11: Consolidate test files into tests/ directory
- D-12: Mirror source directory structure in tests/

## Verification

All plans have been validated:
- Frontmatter validation: PASS
- Plan structure validation: PASS
- All tasks have required fields (files, action, verify, done)
- All tasks have read_first and acceptance_criteria

## Next Steps

Run `/gsd-execute-phase 03` to execute the plans.
