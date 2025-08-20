# Phase 20: Internal Wiring and Logic Fixes - Research

**Researched:** 2026-06-04
**Domain:** Go Backend, Bubble Tea TUI, State Management, Concurrency
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Fix all 6 specific issues identified in `rush/internal_wiring_and_logic_report.md`.
- Target files strictly include: `internal/tui/streaming.go`, `internal/tui/sidebar.go`, `internal/workflow/engine.go`, `internal/tui/app_update.go`, `internal/tui/app.go`, `internal/workflow/initialize.go`.
- Must respect Bubble Tea constraints (single-threaded state mutation, pure `Update` loops, `tea.Msg` for async returns).

### the agent's Discretion
- Exact implementations of the fixes within the targeted files are left to the agent, provided they satisfy the architectural constraints.

### Deferred Ideas (OUT OF SCOPE)
- None specified.
</user_constraints>

## Summary

This research phase audited the 6 critical internal wiring and logical flaws detailed in `rush/internal_wiring_and_logic_report.md`. The issues span data race conditions within TUI updates, ignored agent configurations, and state synchronization failures between the UI and the Workflow Engine.

The recommended fixes correct these issues strictly adhering to Bubble Tea constraints, including adopting message-based passing over goroutine mutation, centralizing workflow transitions in the UI state machine, and explicitly re-syncing the workflow engine upon UI interactions.

**Primary recommendation:** Apply the 6 actionable code modifications to the specified files to safely resolve the internal wiring bugs without compromising the TUI's architecture.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| State Mutation | TUI (`Update()`) | — | Bubble Tea is strictly single-threaded; all mutations must be via `tea.Msg`. |
| Session Management | TUI (`session.Manager`) | Workflow Engine | TUI loads sessions on startup and injects IDs; Engine runs bounded contexts within them. |
| Phase Transitions | TUI (`AppState`) | — | The TUI is the application's central state machine and must coordinate all phase checkpoints. |
| API Orchestration | Workflow Engine | — | The engine executes phase logic, streams LLM data, and delegates tools. |

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | Existing session.json structures and Checkpoints | Ensure backward compatibility; resuming an active session requires reading `session.json`. |
| Live service config | None | — |
| OS-registered state | None | — |
| Secrets/env vars | None | — |
| Build artifacts | None | — |

## Common Pitfalls

### Pitfall 1: Mutating state from `tea.Cmd`
**What goes wrong:** TUI panics or behaves erratically under load.
**Why it happens:** Bubble Tea is not thread-safe. A `tea.Cmd` is spawned in a goroutine and writing to `Model` fields directly creates a data race.
**How to avoid:** Always return a struct conforming to `tea.Msg` from `tea.Cmd` and process the state change inside `Update(tea.Msg)`.

### Pitfall 2: Recreating Workflow Engine silently changes SessionID
**What goes wrong:** UI says "Resuming workflow", but the engine writes to a completely new folder/session ID.
**Why it happens:** Calling `sessionManager.NewSession` implicitly whenever the engine is initialized generates new UUIDs.
**How to avoid:** Load the session ID explicitly via `sessionManager.LoadSession` if resuming; only use `NewSession` on a truly new start.

## Implementation Plan

### 1. Token Usage Tracking Reset
**File:** `internal/tui/streaming.go`
**Location:** `StartStreamCmd`
**Fix:** The stream pipeline resets the token `Usage` to 0. Stop overwriting `lastUsage` on the "done" chunk.
```go
// Replace logic inside `switch chunk.Type { ... }` with:
if chunk.Usage != nil {
    lastUsage = chunk.Usage
}
switch chunk.Type {
case "content":
    fullContent.WriteString(chunk.Delta)
// Do NOT reset lastUsage on case "done"
}
```

### 2. TUI State Concurrency Violation
**File:** `internal/tui/sidebar.go`
**Location:** `refreshCmd` and `Update`
**Fix:** Stop direct cache mutation inside the `refreshCmd` goroutine.
1. Add `FromCache bool` to `SidebarRefreshMsg`.
2. In `refreshCmd`, safely read cached statuses before the goroutine closure, and return them as `FromCache: true`. If fetching via git, return `FromCache: false`.
3. In `Update`, only update `m.gitStatusCache` and `m.lastStatusFetch` when handling `SidebarRefreshMsg` if `!msg.FromCache`.

### 3. Ignored Per-Phase Model Configuration
**File:** `internal/workflow/engine.go`
**Location:** `Engine` struct, `RunPhase`, `streamLLM`, `streamLLMStreaming`
**Fix:** `e.modelForPhase` is ignored.
1. Add `activePhase m31types.WorkflowPhase` field to `Engine`.
2. In `RunPhase`, set `e.activePhase = phase` at the very beginning.
3. In `streamLLM` and `streamLLMStreaming`, change `req.Model = e.modelID` to `req.Model = e.modelForPhase(e.activePhase)`.

### 4. TUI / Engine Synchronization Failure
**Files:** `internal/tui/app_update.go`, `internal/tui/app_update_workflow.go`, `internal/workflow/engine.go`, `internal/tui/app.go`
**Fix:** Ensure backend aligns with TUI session and model state switching.
1. In `workflow.Engine`, add: `func (e *Engine) SetModel(modelID string, p provider.LLMProvider) { e.modelID = modelID; e.provider = p }`
2. Add `SetModel(modelID string, p provider.LLMProvider)` to `workflowEngineInterface` in `internal/tui/app.go`.
3. In `internal/tui/app_update.go` -> `case ScreenResume:` inside the `appMsg.SessionID != ""` block:
   ```go
   if m.workflowEngine != nil {
       m.workflowEngine.SetSessionID(appMsg.SessionID)
   }
   m.dispatcher.SetSessionID(appMsg.SessionID)
   ```
4. In `internal/tui/app_update_workflow.go` -> `handleAppMsg` -> `msg.ModelSelected != nil`, add:
   ```go
   if m.workflowEngine != nil && m.registry != nil {
       if p := m.registry.ActiveProvider(); p != nil {
           m.workflowEngine.SetModel(m.activeModel.ID, p)
       }
   }
   if m.replModel != nil {
       m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
   }
   ```

### 5. Resumable Workflow Deadlock
**Files:** `internal/tui/app.go`, `internal/tui/app_workflow.go`
**Fix:** Fix session precedence in `NewApp`.
1. In `internal/tui/app.go` `NewApp`, BEFORE calling `app.initWorkflowEngine()`, check for a resumable workflow:
   ```go
   if sessions, err := sessionMgr.ListSessions(); err == nil && len(sessions) > 0 {
       recent := sessions[0]
       if goal, phase, questions, err := sessionMgr.LoadWorkflowState(recent.ID); err == nil {
           if phase != types.PhaseIdle && phase != types.PhaseShip {
               app.sessionID = recent.ID
               app.workflowGoal = goal
               app.currentPhase = phase
               app.discussQuestions = questions
               app.toastText = fmt.Sprintf("Resumable workflow at %s. Use /workflow resume to continue.", phase)
               app.toastType = "info"
               app.toastExpires = time.Now().Add(10 * time.Second)
           }
       }
   }
   ```
2. Also in `NewApp` after `replModel` init, load history if resuming:
   ```go
   if app.sessionID != "" {
       if sess, err := sessionMgr.LoadSession(app.sessionID); err == nil && sess != nil {
           for _, msg := range sess.Messages { app.replModel.AddMessage(msg) }
       }
   }
   ```
3. Update `initWorkflowEngine` in `app_workflow.go` to re-use `m.sessionID`:
   ```go
   var s *session.Session
   var err error
   if m.sessionID != "" {
       s, err = m.sessionManager.LoadSession(m.sessionID)
   } else {
       s, err = m.sessionManager.NewSession(modelID, m.activeProvider)
   }
   ```
4. Delete `checkResumedWorkflowState` from `app_workflow.go`.

### 6. Redundant Phase Transition Logic
**Files:** `internal/workflow/initialize.go`, `internal/workflow/engine.go`, `internal/tui/app_update_workflow.go`, `internal/tui/app_workflow.go`, `internal/tui/app.go`
**Fix:** Remove overlapping transitions; let the TUI coordinate.
1. Remove `e.Transition(ctx, types.PhaseInitialize, types.PhaseDiscuss)` from `runInitialize`.
2. Remove `e.Transition(context.Background(), m31types.PhaseDiscuss, m31types.PhasePlan)` from `FinalizeDiscuss`.
3. Add `Transition(ctx context.Context, from, to types.WorkflowPhase) error` to `workflowEngineInterface` in `internal/tui/app.go`.
4. In `internal/tui/app_update_workflow.go` -> `handlePhaseResult`, call:
   ```go
   if m.workflowEngine != nil {
       if eng, ok := m.workflowEngine.(*workflow.Engine); ok {
           _ = eng.Transition(context.Background(), msg.Phase, nextPhase)
       }
   }
   ```
   *before* returning `RunPhaseCmd(m, nextPhase, m.workflowGoal)`.
5. In `internal/tui/app_workflow.go` -> `finalizeDiscussAndAdvance`, apply the exact same `eng.Transition(..., types.PhaseDiscuss, types.PhasePlan)` block before returning `RunPhaseCmd`.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go test |
| Config file | none |
| Quick run command | `go test ./internal/tui/... ./internal/workflow/...` |
| Full suite command | `make test` |

## Sources
### Primary (HIGH confidence)
- Codebase audit: Verified exact locations, goroutine behavior, interface designs, and state handling mechanisms via direct file inspection.
- The `rush/internal_wiring_and_logic_report.md` target document.