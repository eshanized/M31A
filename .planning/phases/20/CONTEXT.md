# Phase 20 Context: Internal Wiring and Logic Fixes

## Goal
Fix all the issues identified in `rush/internal_wiring_and_logic_report.md`. This includes internal wiring issues, interface mismatches, dependency injection flaws, state management errors, and logical bugs.

## Reference Material
- **Target Report:** `rush/internal_wiring_and_logic_report.md`
- **Key Target Files:** 
  - `internal/tui/streaming.go`
  - `internal/tui/sidebar.go`
  - `internal/workflow/engine.go`
  - `internal/tui/app_update.go`
  - `internal/tui/app.go`
  - `internal/workflow/initialize.go`

## Specific Issues to Address
1. Token Usage Tracking Reset in streaming pipeline (`internal/tui/streaming.go`)
2. TUI State Concurrency Violation by directly mutating model fields from `tea.Cmd` goroutine (`internal/tui/sidebar.go`)
3. Ignored Per-Phase Model Configuration by workflow engine (`internal/workflow/engine.go`)
4. TUI/Engine Synchronization Failure regarding session switching and model selection (`internal/tui/app_update.go`)
5. Resumable Workflow Deadlock due to initializing fresh session before checking for resumable workflows (`internal/tui/app.go`)
6. Redundant Phase Transition Logic embedded within the workflow engine (`internal/workflow/initialize.go`)
