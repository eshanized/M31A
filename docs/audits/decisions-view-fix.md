# Decisions View Fix Report

**Date:** 2026-07-11  
**Issue:** `renderDecisionsContent()` (called from `View()`) directly called `m.workflowEngine.SnapshotDecisions()`, bypassing the Bubble Tea `tea.Msg` → `Update()` → `AppState` data flow.

## Changes Made

### 1. Added `DecisionsSnapshotMsg` to workflow messages
**File:** `internal/workflow/engine_messages.go`
```go
// DecisionsSnapshotMsg carries a snapshot of the decision log to the TUI.
type DecisionsSnapshotMsg struct {
	Decisions []decision.DecisionReceipt
}
```

### 2. Engine emits snapshot when decisions are logged
**File:** `internal/workflow/engine.go`
```go
func (e *Engine) LogDecision(r decision.DecisionReceipt) {
	if e.state.decisionLog != nil {
		e.state.decisionLog.Log(r)
		e.emitDecisionsSnapshot()  // NEW
	}
}

func (e *Engine) emitDecisionsSnapshot() {
	if e.msgEmitter == nil {
		return
	}
	decisions := e.SnapshotDecisions()
	if len(decisions) > 0 {
		e.msgEmitter.Emit(DecisionsSnapshotMsg{Decisions: decisions})
	}
}
```

### 3. Added cached decisions field to AppState
**File:** `internal/tui/app_state.go`
```go
// Cached decisions from workflow engine (updated via DecisionsSnapshotMsg)
cachedDecisions []decision.DecisionReceipt
```

### 4. Handle message in Update()
**File:** `internal/tui/app_handlers_misc.go`
```go
func (m *AppState) handleDecisionsSnapshot(msg workflow.DecisionsSnapshotMsg) tea.Cmd {
	m.cachedDecisions = msg.Decisions
	return nil
}
```
**File:** `internal/tui/app_update.go` (case already existed)
```go
case workflow.DecisionsSnapshotMsg:
	cmds = append(cmds, m.handleDecisionsSnapshot(msg))
```

### 5. View() reads from cached field instead of engine
**File:** `internal/tui/app_view.go`
```go
func (m *AppState) renderDecisionsContent(chrome layout.PageChrome) string {
	// ...
	decisions := decision.RedactSlice(m.cachedDecisions)  // Was: m.workflowEngine.SnapshotDecisions()
	// ...
}
```

## Verification

```bash
$ go build ./...
# success

$ go vet ./...
# success

$ go test ./internal/tui/... ./internal/workflow/... -count=1
ok  	github.com/eshanized/M31A/internal/tui	3.153s
ok  	github.com/eshanized/M31A/internal/workflow	4.910s
```

## Remaining `workflowEngine` calls in TUI (all in Update() handlers - correct)

| File | Method | Context |
|------|--------|---------|
| `app.go:196` | type assertion | Startup init only |
| `app.go:768` | `SetModel` | Auto-arbitrage (Update path) |
| `app_handlers_misc.go:121-127` | `SetPhaseModel` | Model picker handler (Update) |
| `app_input.go:197` | `SetWorkflowMode` | Input handler (Update) |
| `app_input.go:340` | `Transition` | Input handler (Update) |
| `app_session.go:65` | `SetWorkflowMode` | Session handler (Update) |
| `app_session.go:107` | type assertion | Session restore (Update) |
| `app_session.go:281` | `SubmitDiscussAnswer` | Discuss handler (Update) |
| `app_session.go:292` | `FinalizeDiscuss` | Discuss handler (Update) |
| `app_session.go:306` | `Transition` | Discuss handler (Update) |
| `app_update_commands.go:446` | type assertion | Command handler (Update) |
| `app_update_phase.go:105` | `Transition` | Phase handler (Update) |
| `app_update_phase.go:116` | `DiscussState` | Phase handler (Update) - returns value copy |
| `app_update_phase.go:143` | `SkipDiscuss` | Phase handler (Update) |
| `app_update_phase.go:155` | `Transition` | Phase handler (Update) |
| `app_update_phase.go:201-202` | `PlanContent/PlanVersion` | Phase handler (Update) - now mutex-protected |
| `app_update_phase.go:247` | `Transition` | Phase handler (Update) |
| `app_update_phase.go:285` | `Transition` | Phase handler (Update) |
| `app_update_phase.go:347` | `Transition` | Phase handler (Update) |
| `app_update_phase.go:368` | `SetRefinementFeedback` | Phase handler (Update) |
| `helpers.go:134` | `SetSessionID` | Session restore (Update) |

**Note:** All calls are in Update() handlers or one-time initialization. No View()-path access remains.

## Manual Verification

Since there's no automated test for rendered view content:
1. Run the TUI (`go run ./cmd/m31a`)
2. Start a workflow (`/new` with a goal)
3. Navigate to Decisions screen (press `d` or navigate via sidebar)
3. Observe decision log renders as workflow progresses and logs decisions
4. Verify no panic or empty state when decisions exist

The decisions now flow: `Engine.LogDecision()` → `DecisionsSnapshotMsg` → `Update()` → `AppState.cachedDecisions` → `View()` reads cached field. This follows the Bubble Tea Elm architecture correctly.