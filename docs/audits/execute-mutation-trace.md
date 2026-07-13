# ExecuteModel Mutation Trace

**Date:** 2026-07-13
**Purpose:** Enumerate every mutation site on ExecuteModel before migration, classify SAFE/UNSAFE/UNCLEAR

---

## Mutation Sites

### 1. Direct Field Assignments

| # | Location | Code | Classification | Rationale |
|---|----------|------|----------------|-----------|
| 1 | `helpers.go:122` | `m.executeModel.sessionID = id` | **SAFE** | Called from `setSessionID()` with value passed in via message |
| 2 | `handler_workflow.go:76` | `m.executeModel.paused = msg.Paused` | **SAFE** | Called from `handleExecutePauseMsg()` with value from `ExecutePauseMsg` |
| 3 | `app_view.go:612-613` | `m.executeModel.width = chrome.ContentWidth()` / `height = chrome.ContentHeight()` | **SAFE** | Called from view renderer with computed dimensions |
| 4 | `app_update_phase.go:167` | `m.executeModel.tasks = tasks` | **SAFE** | Called from PhaseExecute fast path with tasks from `PhaseExecuteCompleteMsg` |
| 5 | `app_update_phase.go:361` | `m.executeModel.tasks = tasks` | **SAFE** | Called from `handlePlanApprove()` with tasks from `m.planModel.tasks` (concrete reference) |
| 6 | `app_session.go:327` | `m.executeModel.tasks = tasks` | **SAFE** | Called from session restore with tasks loaded from file |
| 7 | `app_nav.go:61-62, 346-347` | `m.executeModel.width = cw` / `height = ch` | **SAFE** | Called from nav handlers with computed dimensions |
| 8 | `app_input.go:54-55` | `m.executeModel.width = contentW` / `height = contentH` | **SAFE** | Called from input handler with computed dimensions |
| 9 | `app_input.go:400` | `m.executeModel.theme = t` | **SAFE** | Called from input handler with theme from `themeManager.Current()` |

### 2. Method Calls

| # | Location | Code | Classification | Rationale |
|---|----------|------|----------------|-----------|
| 10 | `app_handlers_workflow.go:23` | `m.executeModel.SetCurrentTask(i)` | **SAFE** | Called from `handleWorkflowTaskStart()` with index computed from message task ID |
| 11 | `app_handlers_workflow.go:31` | `m.executeModel.SetCurrentTask(len(m.executeModel.tasks) - 1)` | **SAFE** | Called from `handleWorkflowTaskStart()` when task not found, index computed from local state |
| 12 | `app_handlers_workflow.go:24,32` | `m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)` | **SAFE** | Called from `handleWorkflowTaskStart()` with values from `TaskStartMsg` |
| 13 | `app_handlers_workflow.go:71` | `m.executeModel.UpdateTaskStatus(msg.Task.ID, status)` | **SAFE** | Called from `handleWorkflowTaskUpdate()` with values from `TaskUpdateMsg` |
| 14 | `app_handlers_workflow.go:106` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleWorkflowToolStart()` with values from `ToolStartMsg` |
| 15 | `app_handlers_workflow.go:124` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleWorkflowToolComplete()` with values from `ToolCompleteMsg` |
| 16 | `app_handlers_workflow.go:140` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleWorkflowSelfHealStart()` with values from `SelfHealStartMsg` |
| 17 | `app_handlers_workflow.go:157` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleWorkflowSelfHealComplete()` with values from `SelfHealCompleteMsg` |
| 18 | `app_handlers_workflow.go:369` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleExecuteQualityGate()` with values from `ExecuteQualityGateMsg` |
| 19 | `app_handlers_workflow.go:379` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleExecuteLoopDetect()` with values from `ExecuteLoopDetectMsg` |
| 20 | `app_handlers_workflow.go:445` | `m.executeModel.AppendLiveOutput(...)` | **SAFE** | Called from `handleTaskDiffSummary()` with values from `TaskDiffSummaryMsg` |
| 21 | `app_routing.go:99` | `m.executeModel.Update(msg)` | **SAFE** | Standard Bubble Tea update pattern |
| 22 | `app_handlers_tick.go:49` | `m.executeModel.Update(msg)` | **SAFE** | Standard Bubble Tea tick forwarding |

### 3. Indirect Accesses

| # | Location | Code | Classification | Rationale |
|---|----------|------|----------------|-----------|
| 23 | `app_handlers_workflow.go:21` | `for i, t := range m.executeModel.tasks` | **SAFE** | Read-only iteration in `handleWorkflowTaskStart()` |
| 24 | `app_handlers_workflow.go:30` | `m.executeModel.tasks = append(m.executeModel.tasks, msg.Task)` | **SAFE** | Append with task from `TaskStartMsg` |
| 25 | `app_handlers_workflow.go:41` | `m.sidebarModel.InitTaskProgress(len(m.executeModel.tasks))` | **SAFE** | Read-only length access for sidebar init |

---

## Plan -> Execute Task-Copy Handoff

**Location:** `app_update_phase.go:344-365` (`handlePlanApprove()`)

```go
tasks := []types.Task{}
if m.planModel != nil {
    tasks = m.planModel.tasks
}
if m.executeModel == nil {
    cw, ch := m.contentDimensions()
    m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), cw, ch)
} else {
    m.executeModel.tasks = tasks
}
```

**Classification:** **SAFE**

- `m.planModel` is a concrete `*PlanModel` reference in `AppState`
- Tasks are copied from Plan to Execute at approval time
- This is the **only** place Execute reads from another screen's concrete state
- Confirmed: Plan does NOT read Execute's tasks (no bidirectional coupling)

---

## Summary

| Classification | Count |
|----------------|-------|
| **SAFE** | 25 |
| **UNSAFE** | 0 |
| **UNCLEAR** | 0 |

**All mutation sites are SAFE.** Every mutation is called from an Update()-path handler with values passed in via messages, or from view/nav handlers with computed values. No mutation reads live engine or goroutine state directly.

---

## Cross-Screen Coupling

### Execute reads Plan (confirmed)
- `handlePlanApprove()` copies `m.planModel.tasks` to `m.executeModel.tasks`
- One-way coupling only (Plan does NOT read Execute)

### Sidebar reads Execute (not a migration concern)
- `m.sidebarModel.InitTaskProgress(len(m.executeModel.tasks))` - read-only length access
- This is in `handleWorkflowTaskStart()` which is an Update()-path handler

---

## Migration Readiness

**Status:** CLEAN - Ready to proceed with migration

All 25 mutation sites are SAFE. The Plan->Execute task-copy handoff is confirmed as the only cross-screen coupling, and it's one-way (Execute reads Plan, not vice versa). No live engine or goroutine state is accessed directly.

---

## Related Files

- `internal/tui/execute_model.go` - Model implementation (336 lines)
- `internal/tui/app_handlers_workflow.go` - Primary mutation site (10+ calls)
- `internal/tui/app_update_phase.go` - Phase transitions and task-copy handoff
- `internal/tui/app_view.go` - View rendering
- `internal/tui/app_nav.go` - Navigation dimension updates
- `internal/tui/app_input.go` - Theme and dimension updates
- `internal/tui/handler_workflow.go` - Pause/resume handler
- `internal/tui/helpers.go` - Session ID setter
- `internal/tui/app_session.go` - Session restore
- `internal/tui/app_handlers_tick.go` - Tick forwarding
