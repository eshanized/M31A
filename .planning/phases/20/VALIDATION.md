# Phase 20 Validation

## Nyquist Matrix

| ID | Finding | Code Evidence Required | Test Requirement |
|----|---------|------------------------|------------------|
| V1 | Token Usage tracking broken | `internal/tui/streaming.go` correctly passes `Usage` | `TestStartStreamCmdUsage` added/updated |
| V2 | TUI Sidebar concurrency | `internal/tui/sidebar.go` uses `tea.Msg` | `TestSidebarConcurrency` runs without race |
| V3 | Workflow config ignored | `internal/workflow/engine.go` calls `e.modelForPhase` | `TestEnginePhaseConfig` confirms active phase config |
| V4 | Synchronization failure | `internal/tui/app_update.go` synchronizes model/session | `TestTUIEngineSync` added |
| V5 | Resumable workflow deadlock | `internal/tui/app.go` defers session creation | `TestResumableSession` verifies correct session ID |
| V6 | Phase transition logic | `internal/workflow/initialize.go` removes transition | `TestPhaseTransitions` passes end to end |
