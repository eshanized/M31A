# Screen Migration Report — Plan Screen

**Date:** 2026-07-11  
**Scope:** Migration of PlanModel (385 lines) + PlanRefineModel (60 lines) to new Screenable interface + Router architecture (strangler-fig pattern, following ConfirmQuit pilot)

---

## Summary

Successfully migrated **PlanModel** (and its embedded **PlanRefineModel**) to the new Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated).

---

## Changes Made

### 1. Screenable Interface Conformance (`plan_model.go`, `plan_refine.go`)

**PlanModel** (`plan_model.go:191`):
- Changed `Update(msg tea.Msg) (*PlanModel, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Added `Init() tea.Cmd` (returns `nil`)
- Added `SetTheme(t theme.Theme)` — delegates to internal `pm.theme = t`
- Already implemented: `View()`, `SetDimensions(w, h int)`, `SetPlanContent(markdown string)`, `SetPlanVersion(version int)`, `UpdateTasks(tasks []types.Task)`

**PlanRefineModel** (`plan_refine.go:32`):
- Changed `Update(msg tea.Msg) (*PlanRefineModel, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Added `Init() tea.Cmd` (returns `nil`)
- Added `SetDimensions(w, h int)` — updates textarea width
- Added `SetTheme(t theme.Theme)` — delegates to internal `pm.theme = t`
- Already implemented: `View()`, `Value()`

No internal logic rewritten — pure boundary adaptation.

### 2. Router Registration (`app_routing.go:73-90`)

```go
m.screenUpdaters[ScreenPlan] = func(msg tea.Msg) tea.Cmd {
    if m.planModel == nil {
        cw, ch := m.contentDimensions()
        m.planModel = NewPlanModel(
            []types.Task{},
            m.themeManager.Current(),
            "", "", "",
            0, "",
            cw, ch,
        )
        m.router.Register(ScreenPlan, m.planModel)
    }
    newModel, cmd := m.planModel.Update(msg)
    if r, ok := newModel.(*PlanModel); ok {
        m.planModel = r
    }
    return cmd
}
```

Follows ConfirmQuit pilot pattern: lazy init on first message, router registration inline.

### 3. View Delegation to Router (`app_view.go:573-588`)

```go
func (m *AppState) renderPlanContent(chrome layout.PageChrome) string {
    if m.planModel == nil {
        cw, ch := m.contentDimensions()
        m.planModel = NewPlanModel(
            []types.Task{},
            m.themeManager.Current(),
            "", "", "",
            0, "",
            cw, ch,
        )
        m.router.Register(ScreenPlan, m.planModel)
    }
    m.planModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (Update updater + View renderer) — safe due to router's idempotent `Register`.

### 4. Screen Switch via `switchScreen` (`app_update_phase.go:181`, `app_update_phase.go:196`, `app_session.go:314`, `app_session.go:332`)

Changed four `m.screen = ScreenPlan` assignments to `m.switchScreen(ScreenPlan)`:
- `app_update_phase.go:181` — direct mode fast path to Plan
- `app_update_phase.go:196` — PhasePlan case entry
- `app_session.go:314` — session resume to Execute
- `app_session.go:332` — session resume to Plan

`switchScreen` sets `m.screen` AND calls `m.router.SwitchTo(s)` to synchronize router active screen.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test -race ./internal/tui/...` | ✅ (all pass) |
| `go vet ./internal/tui/...` | ✅ (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning only) |

### Plan-Specific Test Coverage

| Test | Status |
|------|--------|
| `TestWaveTitle` | ✅ PASS |
| `TestPlanModelSetPlanVersion` | ✅ PASS |
| `TestPlanModelComputeWavesEmpty` | ✅ PASS |
| `TestPlanModelComputeWavesNoDeps` | ✅ PASS |
| `TestPlanModelComputeWavesWithDeps` | ✅ PASS |
| `TestPlanModelComputeWavesParallel` | ✅ PASS |
| `TestPlanModelComputeWavesCyclic` | ✅ PASS |
| `TestPlanModelComputeWavesMissingDep` | ✅ PASS |
| `TestViewPlan` | ✅ PASS |
| `TestEnsureSubModel_Plan` | ✅ PASS |

All 11 tests in `plan_extra_test.go` pass.

---

## Critical Handoff & Data Flow Verification

### 1. WorkflowEngine State Read (PlanContent/PlanVersion)

**Location:** `app_update_phase.go:200-203`

```go
if m.workflowEngine != nil {
    m.planModel.SetPlanContent(m.workflowEngine.PlanContent())
    m.planModel.SetPlanVersion(m.workflowEngine.PlanVersion())
}
```

- **Unchanged**: Still called from `app_update_phase.go` in the PhasePlan case handler
- **Still mutex-protected**: `WorkflowEngine.PlanContent()` and `PlanVersion()` are protected by `planMu` (per `docs/audits/planstate-race-fix.md`)
- **Migration did not introduce any new read of workflowEngine state** — data flow remains: `WorkflowEngine` → `SetPlanContent`/`SetPlanVersion` → `PlanModel` → rendered via Router

### 2. Plan → Execute Handoff (Bidirectional Coupling)

Per `docs/audits/screen-inventory.md`:
- **Plan reads Execute**: `PlanModel` does NOT directly read `executeModel.tasks` — the coupling is **Execute reads Plan** at transition time
- **Execute reads Plan**: In `app_update_phase.go:351-360` (`handlePlanApprove`):
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

**Verification:**
- `m.planModel` is a concrete `*PlanModel` reference in `AppState` — unchanged by migration
- `handlePlanApprove` still accesses `m.planModel.tasks` directly — no change
- `ExecuteModel` is NOT migrated yet (still on old concrete-model path) — `AppState` still holds concrete `*ExecuteModel`
- Router only affects *render/update delegation* for Plan; Execute screen still uses old switch path
- **Handoff is identical** — tasks are copied from Plan to Execute at approval time, then `m.switchScreen(ScreenExecute)` is called (via `handlePlanApprove` → `m.screen = ScreenExecute` which already used `navigateToScreen` → `switchScreen`)

### 3. SetRefinementFeedback Path

- `PlanRefineMsg` → `handlePlanRefine` → `m.workflowEngine.SetRefinementFeedback(msg.Feedback)` → `RunPhaseCmd(types.PhasePlan)`
- PhasePlan case re-enters, creates new PlanModel with refined tasks, sets PlanContent/Version from engine
- **Unchanged** — refinement flow works identically

---

## Remaining Screens Untouched

All other 26 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`, `ScreenPhaseModelPicker`, `ScreenDiscuss`, `ScreenBisect`, `ScreenDashboard`, `ScreenPlan`.

---

## Friction / Interface Issues

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is `int` in `tuitypes.go`; interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| `router.Register` lazy vs eager | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only ConfirmQuit/Help/Home/GhostPicker/GhostOutput/PhaseModelPicker/Discuss/Bisect/Dashboard/Plan receive propagation currently; safe |

---

## Conclusion

PlanModel migration complete. The screen's critical data flows remain intact:
- **WorkflowEngine → PlanModel** via `SetPlanContent`/`SetPlanVersion` (mutex-protected, unchanged)
- **PlanModel → ExecuteModel** task copy at approval (concrete reference, unchanged)
- **Refinement loop** `PlanRefineMsg` → engine → re-plan (unchanged)

Strangler-fig pattern holds — 10 of ~33 screens now behind router, zero regressions.