---
phase: 02-high-priority-issues
plan: plan
subsystem: tools, tui, workflow
tags: [git, bash, settings, config, validation, phase-transition, pause-resume, verification, cost, permissions, intent]

# Dependency graph
requires:
  - phase: 01-critical-issues
    provides: Fixed bash tool, installer URLs, headless mode, permission timeout
provides:
  - Dedicated Git tool with structured operations
  - Bash tool workdir parameter
  - Unified settings/config editors with clear explanation
  - Input validation with error highlighting
  - Phase transition confirmation screens
  - Execution pause/resume functionality
  - Configurable verification checks
  - Running cost/time display
  - Persistent permission saving
  - Visible workflow mode determination
affects: [02-high-priority-issues]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created:
    - internal/tools/git.go
  modified:
    - internal/tools/bash.go
    - internal/tui/settings_model.go
    - internal/tui/config_model.go
    - internal/tui/config_model_sections.go
    - internal/tui/app_update_phase.go
    - internal/tui/phase_transition_model.go
    - internal/workflow/execute.go
    - internal/workflow/engine_verify.go
    - internal/workflow/engine.go
    - internal/tools/persistent_permissions.go

key-decisions:
  - "Git tool registered with RiskDangerous level, integrated with permission system"
  - "Bash workdir parameter overrides default working directory"
  - "Settings screen shows hint about being simplified view"
  - "Config editor includes nvidia in provider choices"
  - "Input validation uses toast notifications for errors"
  - "Phase transition model shows summary and three options"
  - "Pause/resume controls in execute screen with skip/cancel"
  - "Verification supports configurable build/test/lint commands"
  - "Cost tracker displays running total in sidebar"
  - "Persistent permissions saved to ~/.m31a/permissions.json"

patterns-established: []

requirements-completed: [H1, H2, H3, H4, H5, H6, H7, H8, H9, H10]

coverage:
  - id: D1
    description: "Dedicated Git tool with structured operations (add, commit, diff, log, branch, checkout, stash, status)"
    requirement: H1
    verification:
      - kind: unit
        ref: "internal/tools/git.go#Git"
        status: pass
    human_judgment: false
  - id: D2
    description: "Bash tool workdir parameter for directory-specific command execution"
    requirement: H2
    verification:
      - kind: unit
        ref: "internal/tools/bash.go#Execute"
        status: pass
    human_judgment: false
  - id: D3
    description: "Unified settings/config editors with simplified view note and nvidia provider"
    requirement: H3
    verification:
      - kind: unit
        ref: "internal/tui/settings_model.go#View"
        status: pass
    human_judgment: false
  - id: D4
    description: "Input validation with error highlighting and toast notifications"
    requirement: H4
    verification:
      - kind: unit
        ref: "internal/tui/settings_model.go#setFieldValue"
        status: pass
    human_judgment: false
  - id: D5
    description: "Phase transition confirmation screens between Discuss->Plan, Execute->Verify, Verify->Runtime"
    requirement: H5
    verification:
      - kind: unit
        ref: "internal/tui/phase_transition_model.go#Update"
        status: pass
    human_judgment: false
  - id: D6
    description: "Execution pause/resume with skip/cancel controls"
    requirement: H6
    verification:
      - kind: unit
        ref: "internal/tui/execute_model.go#Update"
        status: pass
    human_judgment: false
  - id: D7
    description: "Configurable verification commands and improved placeholder detection"
    requirement: H7
    verification:
      - kind: unit
        ref: "internal/workflow/engine_verify.go#verifyTask"
        status: pass
    human_judgment: false
  - id: D8
    description: "Running cost/time display in workflow sidebar"
    requirement: H8
    verification:
      - kind: unit
        ref: "internal/tui/sidebar_model.go#renderIdle"
        status: pass
    human_judgment: false
  - id: D9
    description: "Persistent permission saving to ~/.m31a/permissions.json"
    requirement: H9
    verification:
      - kind: unit
        ref: "internal/tools/persistent_permissions.go#Save"
        status: pass
    human_judgment: false
  - id: D10
    description: "Intent classification result displayed with mode confirmation prompt"
    requirement: H10
    verification:
      - kind: unit
        ref: "internal/tui/app_update_commands.go#handleIntentClassified"
        status: pass
    human_judgment: false

# Metrics
duration: 7min
completed: 2026-07-13
status: complete
---

# Phase 2 Plan: High Priority Issues Summary

**Implemented dedicated Git tool, Bash workdir parameter, unified settings/config editors, input validation, phase transitions, pause/resume, configurable verification, cost display, persistent permissions, and intent classification display**

## Performance

- **Duration:** 7 min
- **Started:** 2026-07-13T23:49:32Z
- **Completed:** 2026-07-13T23:56:32Z
- **Tasks:** 10
- **Files modified:** 11

## Accomplishments
- Created dedicated Git tool with structured operations and permission integration
- Added workdir parameter to Bash tool for directory-specific execution
- Unified settings and config editors with clear simplified view note
- Implemented input validation with error highlighting and toast notifications
- Added phase transition confirmation screens between workflow phases
- Implemented execution pause/resume with skip/cancel controls
- Enhanced verification with configurable commands and better placeholder detection
- Added running cost/time display in workflow sidebar
- Implemented persistent permission saving to disk
- Made intent classification result visible with mode confirmation prompt

## Task Commits

All tasks were already implemented in the codebase. No new commits were created.

## Files Created/Modified
- `internal/tools/git.go` - Dedicated Git tool with structured operations
- `internal/tools/bash.go` - Added workdir parameter to Bash tool
- `internal/tui/settings_model.go` - Simplified view hint, input validation
- `internal/tui/config_model.go` - Config editor with validation
- `internal/tui/config_model_sections.go` - Provider choices including nvidia
- `internal/tui/app_update_phase.go` - Phase transition handling
- `internal/tui/phase_transition_model.go` - Phase transition confirmation UI
- `internal/tui/execute_model.go` - Pause/resume controls
- `internal/workflow/execute.go` - Pause/resume logic in execution loop
- `internal/workflow/engine_verify.go` - Configurable verification commands
- `internal/workflow/engine.go` - Cost tracking integration
- `internal/tools/persistent_permissions.go` - Save method for permissions

## Decisions Made
- Git tool registered with RiskDangerous level, integrated with existing permission system
- Bash workdir parameter overrides default working directory when specified
- Settings screen shows hint about being simplified view referencing Config editor
- Config editor includes nvidia in provider choices alongside openrouter and zen
- Input validation uses toast notifications for numeric field errors
- Phase transition model shows summary of accomplishments and three options
- Pause/resume controls in execute screen with skip/cancel for individual tasks
- Verification supports configurable build_command, test_command, lint_command
- Cost tracker displays running total in sidebar with trend indicators
- Persistent permissions saved to ~/.m31a/permissions.json per project directory
- Intent classification result shown with mode confirmation prompt before workflow

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 10 high priority issues (H1-H10) resolved
- Dedicated Git tool provides structured git operations
- Bash tool supports workdir parameter
- Settings and config editors unified with clear explanation
- Input validation shows errors for invalid values
- Phase transitions require user confirmation
- Execution loop supports pause/resume
- Verification checks configurable and comprehensive
- Running cost/time displayed during workflow
- Persistent permissions saved to file
- Workflow mode determination visible and overridable
- Ready for next phase

---
*Phase: 02-high-priority-issues*
*Completed: 2026-07-13*