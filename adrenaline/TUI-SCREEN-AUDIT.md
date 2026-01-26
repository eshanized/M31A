# TUI Screen Audit Report

**Date:** 2026-06-10
**Scope:** All 75+ `.go` files in `internal/tui/` and sub-packages
**Methodology:** Full source review of types, routing, Update/View, and every screen model

---

## 1. CRITICAL — Session Resume Is Completely Broken

**Files:** `app_update.go:725–741`, `resume_model.go:76–83`

The resume screen emits:
```go
AppMsg{Screen: ScreenREPL, SessionID: id}
```

But `handleAppMsg` checks conditions in the wrong order:
```go
if msg.Screen != 0 || msg.Action != "" {     // ScreenREPL=1, so this is TRUE
    return m.routeAppMsgAction(msg)           // ← takes this path
}
if msg.SessionID != "" {                      // ← never reached
    return m.loadAndRestoreSession(msg.SessionID, true)
}
```

Since `ScreenREPL = 1` (non-zero), the first condition is always true and `routeAppMsgAction` is called, which ignores `SessionID`. **No session can ever be resumed from the session browser.**

**Fix:** Check `msg.SessionID != ""` before the screen/action check.

---

## 2. CRITICAL — Discuss Questions Never Populated

**Files:** `app_update_phase.go:43–54`, `types.go:178–193`

When the discuss phase completes with `NeedsAnswers: true`:
```go
case types.PhaseDiscuss:
    if msg.NeedsAnswers {
        m.screen = ScreenDiscuss
        if m.discussModel == nil {
            m.discussModel = NewDiscussModel(
                m.themeManager.Current(),
                m.discussQuestions,  // ← always nil/empty
                m.width, m.height,
            )
        }
```

`m.discussQuestions` is **never populated** from the `PhaseResultMsg`. The message struct has no field for questions. The discuss screen renders "No questions to answer" because `len(dm.questions) == 0`, immediately completing with empty answers.

**Fix:** Add a `Questions []string` field to `PhaseResultMsg` and populate `m.discussQuestions` before creating the model.

---

## 3. CRITICAL — Ledger Screen Shows "Loading..." Forever

**Files:** `app_update.go:880–885`, `ledger.go:53–62`

When navigating to `ScreenLedger`, `ensureSubModel` creates the model:
```go
case ScreenLedger:
    if m.ledgerModel == nil {
        m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
        m.ledgerModel.width = m.width
        m.ledgerModel.height = m.height
    }
    return nil  // ← LoadEntries() never called!
```

`LedgerModel.LoadEntries()` is never called. The `loaded` field stays `false`, so the View permanently renders "Loading..." The user can press `r` to trigger `LoadEntries()`, but the initial load is missing.

**Fix:** Call `m.ledgerModel.LoadEntries()` after creating the model, or call it in the `Init()` method.

---

## 4. CRITICAL — Rollback Screen Shows "No commits found" Forever

**Files:** `app_update.go:887–891`, `rollback.go:54–67`

Same pattern as the ledger:
```go
case ScreenRollback:
    if m.rollbackModel == nil {
        m.rollbackModel = NewRollbackModel(...)
    }
    return nil  // ← LoadCommits() never called!
```

`RollbackModel.LoadCommits()` is never called. The `entries` field stays nil, so View renders "No commits found in this repository." There is no keybinding to trigger `LoadCommits()` — it's unreachable.

**Fix:** Call `m.rollbackModel.LoadCommits()` after creating the model, or return a `tea.Cmd` that loads commits asynchronously.

---

## 5. CRITICAL — HelpModel Is Fully Implemented but Completely Unreachable

**Files:** `help.go` (232 lines), `types.go:17–35`

`HelpModel` has full Update/View/Init, scrollable viewport, and complete keybinding documentation. However:
- **No `ScreenHelp` constant** in the Screen enum
- **No `helpModel` field** in `AppState`
- **No navigation path** — no slash command, keybinding, or AppMsg routes to it
- The `/help` command writes text into the REPL instead of opening the HelpModel
- The `?` key is documented in help as "Toggle this help" but is never bound

**232 lines of dead code.**

**Fix:** Add `ScreenHelp`, add `helpModel *HelpModel` to `AppState`, wire `/help` and `?` to navigate to it, add it to `ensureSubModel`, `renderActiveScreen`, and `routeKeyMsg`.

---

## 6. HIGH — Metrics Screen Ignores WindowSizeMsg

**Files:** `app_update.go:411–443`, `app_update.go:715–719`

The `default:` case in `Update` forwards messages for `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, and `ScreenDiscuss` — but **not `ScreenMetrics`**.

In `routeKeyMsg`, `ScreenMetrics` only handles `esc`/`q`:
```go
case ScreenMetrics:
    switch msg.String() {
    case "esc", "q":
        m.screen = ScreenREPL
        return nil
    }
```

`WindowSizeMsg` is dispatched to `handleWindowResize` (which doesn't resize `metricsModel`), and then the message is NOT forwarded to `MetricsModel.Update()`. The metrics screen never resizes after creation.

Additionally, `handleWindowResize` resizes `planModel`, `executeModel`, `verifyModel`, `settingsModel`, `cmdPalette`, `msModel`, `resumeModel`, `diffModel`, but **not**: `goalInput`, `ledgerModel`, `rollbackModel`, `shipModel`, `metricsModel`, `firstRunModel`, `configModel`, `discussModel`.

**Fix:** Add `ScreenMetrics` (and `ScreenGoalInput`, `ScreenLedger`, `ScreenRollback`, `ScreenConfig`, `ScreenDiscuss`) to the `default:` message forwarding in `Update`, and add their models to `handleWindowResize`.

---

## 7. HIGH — DiscussModel.Init() Never Called

**Files:** `app_update.go:829–912`, `app_update_phase.go:43–54`

`ensureSubModel` has **no case for `ScreenDiscuss`**:
```go
switch screen {
case ScreenREPL: ...
case ScreenModelSelector: ...
// ... no ScreenDiscuss case
}
```

When the discuss phase creates the DiscussModel in `handlePhaseResult`, it calls `NewDiscussModel()` directly but never calls `Init()`. The textinput's `Blink` command is never started, so the cursor won't blink.

**Fix:** Add `ScreenDiscuss` to `ensureSubModel` and call `dm.Init()`, or call `Init()` directly after creating the model and append the returned cmd.

---

## 8. HIGH — Question Response Message Type Mismatch

**Files:** `app_update.go:1186–1194`, `components/question.go`

In `handleQuestionKey`:
```go
func (m *AppState) handleQuestionKey(msg tea.KeyMsg) tea.Cmd {
    if m.questionModel == nil || m.questionRequest == nil { ... }
    _, cmd := m.questionModel.Update(msg)
    return cmd  // ← returns QuestionModel's internal cmd
}
```

The `QuestionModel.Update()` returns commands that emit `tools.QuestionResponse` (from the tools package), but `AppState.Update()` handles `QuestionResponseMsg` (a TUI-local type). Unless the `QuestionModel` is specifically wired to emit `QuestionResponseMsg` (not `tools.QuestionResponse`), the response is silently dropped and the question modal stays on screen forever.

**Fix:** Verify the QuestionModel emits `QuestionResponseMsg` (not `tools.QuestionResponse`), or add a handler for `tools.QuestionResponse` in `AppState.Update()`.

---

## 9. HIGH — PlanModel Viewport Shows Stale Content

**Files:** `app_update_phase.go:55–72`, `plan_model.go:36–58`

When discuss completes without answers, the PlanModel is created immediately:
```go
case types.PhaseDiscuss:
    // ...
    m.screen = ScreenPlan
    if m.planModel == nil {
        m.planModel = NewPlanModel(
            msg.Tasks,  // ← discuss phase tasks (may be empty)
            ...
        )
    }
    return m.RunPhaseCmd(types.PhasePlan)  // ← starts plan phase
```

The PlanModel is created with the discuss phase's `msg.Tasks` (likely empty or minimal). The plan phase runs and produces actual tasks, arriving as a `PhaseResultMsg` for `PhasePlan`. But in the `PhasePlan` result handler, the code creates `ExecuteModel` — it never updates `PlanModel.UpdateTasks()`.

`handlePlanReady` would update the tasks, but it's only triggered by `PlanReadyMsg`, which the workflow engine may not emit separately from `PhaseResultMsg`.

**Fix:** In `handlePhaseResult` for `PhasePlan`, call `m.planModel.UpdateTasks(msg.Tasks)` before transitioning to execute.

---

## 10. HIGH — GoalSubmittedMsg Always Restarts from PhaseInitialize

**Files:** `app_update.go:274–278`, `app_update.go:1102–1108`

```go
case GoalSubmittedMsg:
    m.workflowGoal = msg.Goal
    m.screen = ScreenREPL
    cmds = append(cmds, m.runWorkflowFromGoal(msg.Goal))

func (m *AppState) runWorkflowFromGoal(goal string) tea.Cmd {
    m.workflowPhase = types.PhaseInitialize  // ← always starts from beginning
    return m.RunPhaseCmd(types.PhaseInitialize)
}
```

This means `/resume-task` with `WorkflowResume: true` calls `processCommandResult`, which checks:
```go
if result.WorkflowResume && result.ResumeGoal != "" {
    m.workflowGoal = result.ResumeGoal
    m.workflowPhase = result.ResumePhase
    return m.runWorkflowFromGoal(result.ResumeGoal)  // ← ignores ResumePhase!
}
```

Even though `ResumePhase` is set, `runWorkflowFromGoal` always starts from `PhaseInitialize`. The resume phase is completely ignored.

**Fix:** Modify `runWorkflowFromGoal` to accept a starting phase parameter, or use `result.ResumePhase` directly.

---

## 11. HIGH — Theme Not Propagated to 8+ Models

**Files:** `app_update.go:995–1038`

`applyTheme` updates: replModel, sidebarModel, cmdPalette, settingsModel, planModel, executeModel, verifyModel, shipModel, discussModel, metricsModel, resumeModel.

**Missing theme propagation to:**
- `diffModel` — diff viewer keeps old colors after theme switch
- `goalInput` — goal input textarea keeps old colors
- `ledgerModel` — ledger table keeps old colors
- `rollbackModel` — commit browser keeps old colors
- `configModel` — config viewer keeps old colors
- `firstRunModel` — wizard keeps old colors
- `msModel` (ModelSelector) — model picker keeps old colors

**Fix:** Add theme propagation for all missing models in `applyTheme`.

---

## 12. MEDIUM — Workflow Phase Screens Shown Without Model Data

**Files:** `commands_workflow.go:51–119`

Commands like `/plan`, `/execute`, `/verify`, `/ship` navigate to their respective screens via `CommandResult{Screen: &screen}`. The `processCommandResult` calls `navigateToScreen`, which calls `ensureSubModel`. But `ensureSubModel` for plan/execute/verify/ship only resizes existing models — it doesn't create them:

```go
case ScreenPlan:
    if m.planModel != nil {
        m.planModel.SetDimensions(m.width, m.height)
    }
    return nil  // ← no model creation
```

If the user types `/plan` before any workflow has run, `planModel` is nil and the screen renders "Loading plan..." (from `renderPlanContent`'s nil check) forever.

**Fix:** Either block phase navigation when no workflow is active, or create placeholder models.

---

## 13. MEDIUM — ShipModel Missing esc/q Key Handler

**Files:** `ship_model.go:54–69`

```go
func (sm *ShipModel) Update(msg tea.Msg) (*ShipModel, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyMsg:
        switch msg.String() {
        case "enter", "n":
            return sm, func() tea.Msg { return AppMsg{Screen: ScreenREPL} }
        case "d":
            return sm, func() tea.Msg { return AppMsg{Action: "view_ship_diff"} }
        }
    }
    return sm, nil
}
```

No `esc` or `q` handler. The user must press `enter` or `n` to leave the ship screen. Every other screen supports `esc` to go back. This is inconsistent and confusing.

**Fix:** Add `case "esc", "q":` to return to REPL.

---

## 14. MEDIUM — ScreenMetrics Has No Dedicated Update Forwarding

**Files:** `app_update.go:411–443`

The `default:` case in `Update` forwards unhandled messages to 4 screens:
```go
case ScreenModelSelector: // forward
case ScreenSettings:      // forward
case ScreenResume:        // forward
case ScreenDiscuss:       // forward
```

Missing: `ScreenMetrics`, `ScreenGoalInput`, `ScreenLedger`, `ScreenRollback`, `ScreenConfig`, `ScreenDiff`, `ScreenShip`, `ScreenPlan`, `ScreenExecute`, `ScreenVerify`.

Some of these (plan, execute, verify, ship, diff) are handled in `routeKeyMsg` with their own Update calls, but `WindowSizeMsg` and other non-key messages are dropped.

**Fix:** Add all active screens to the `default:` message forwarding, or better yet, create a generic forwarding mechanism.

---

## 15. MEDIUM — ResumeModel Created Without Init() Call

**Files:** `app_update.go:383–391`

```go
case resumeScreenReadyMsg:
    if m.resumeModel == nil {
        rm := NewResumeModel(msg.sessions, m.themeManager.Current())
        m.resumeModel = rm
    } else {
        m.resumeModel.Refresh(msg.sessions)
    }
    m.screen = ScreenResume
```

`ResumeModel.Init()` is never called. Currently `Init()` returns nil so this is not a visible bug, but it's inconsistent with other screens.

---

## 16. MEDIUM — FirstRunModel Width Not Set on Creation

**Files:** `app_update.go:454–459`

```go
case ScreenFirstRun:
    if m.firstRunModel == nil {
        fm := NewFirstRunModel(m.themeManager.Current(), m.registry, m.shutdownCtx)
        m.firstRunModel = fm
    }
    return m.firstRunModel.Init()
```

`SetDimensions` is never called. The model has `width=0, height=0` until a `WindowSizeMsg` arrives. The `effectiveWidth()` method falls back to 80, which may cause layout misalignment on wide terminals during the first render.

**Fix:** Call `fm.SetDimensions(m.width, m.height)` after creation.

---

## 17. MEDIUM — DiffModel Dimensions Set But Not Via SetDimensions

**Files:** `app_update.go:358–369`

```go
case DiffScreenMsg:
    if m.diffModel == nil {
        m.diffModel = NewDiffModel(m.themeManager.Current())
    }
    m.diffModel.SetDiff(msg.Diff)
    m.diffModel.SetTitle(msg.Title)
    m.diffModel.width = m.width
    m.diffModel.height = m.height
```

`SetDiff()` recreates the viewport using `dm.height`, but `dm.height` is set AFTER `SetDiff` is called (lines 364-365). So the viewport is created with `height=0`, making `vpH = 0 - 8 = -8`, clamped to 3. The viewport is always 3 rows until the next resize.

**Fix:** Set `width` and `height` BEFORE calling `SetDiff`, or call `SetDimensions` after setting dimensions.

---

## 18. MEDIUM — `popScreen()` Never Called

**Files:** `app_update.go:817–826`

```go
func (m *AppState) popScreen() tea.Cmd {
    if len(m.screenStack) > 0 { ... }
}
```

`screenStack` is pushed to in `navigateToScreen` but `popScreen()` is never called anywhere. The back-stack grows unbounded. Pressing `esc` on sub-screens navigates to REPL directly via `AppMsg{Screen: ScreenREPL}` instead of popping the stack.

**Fix:** Wire `esc` on sub-screens to call `popScreen()` instead of always going to REPL, or remove the unused back-stack.

---

## 19. LOW — `screenStack` Grows Unbounded

**Files:** `app_update.go:797–798`

```go
if m.screen != screen && m.screen != ScreenPermission && screen != ScreenPermission {
    m.screenStack = append(m.screenStack, m.screen)
}
```

Every non-permission screen navigation pushes to the stack, but nothing pops. After extended use, the stack can grow very large.

---

## 20. LOW — `renderPlanHeader` in `plan_view.go` Never Called

**Files:** `plan_view.go:15–31`

`renderPlanHeader` is defined but never called from any code path. The PlanModel's `View()` method doesn't call it, and no other code references it. Dead code.

---

## 21. LOW — `renderProgressBar` in `execute_view.go` Never Called

**Files:** `execute_view.go:15–33`

`renderProgressBar` is defined but never called. The execute model uses `renderAnimatedProgressBar` instead. Dead code.

---

## 22. LOW — `renderAnimatedProgressPct` Never Called

**Files:** `execute_view.go:62–67`

Dead code — defined but never referenced.

---

## 23. LOW — `animatedProgressBarWidth` Never Called

**Files:** `execute_view.go:49–51`

Dead code — defined but never referenced.

---

## 24. LOW — `truncateToVisibleWidth` Has TODO Instead of Implementation

**Files:** `app_view.go:223–230`

```go
func truncateToVisibleWidth(s string, maxW int) string {
    w := lipgloss.Width(s)
    if w <= maxW {
        return s
    }
    return s // TODO: use layout.TruncateToWidth when exported
}
```

The function returns the original string even when it exceeds `maxW`. Toast overlays may overflow the content area.

---

## 25. LOW — `SettingsModel.renderLeftNav()` Not Shown (Defined in settings_tabs.go)

**Files:** `settings_model.go:512`, `settings_tabs.go`

`View()` calls `s.renderLeftNav()` which is defined in `settings_tabs.go`. This is fine architecturally but the left nav renders all tab names as a vertical list — if the terminal is too narrow, the navWidth (20) plus the content card may exceed the available width, causing wrapping.

---

## Screen Inventory

### Defined Screens vs Implementation Status

| Screen | Constant | Model | View | Update | ensureSubModel | routeKeyMsg | Status |
|--------|----------|-------|------|--------|----------------|-------------|--------|
| FirstRun | `ScreenFirstRun` | OK | OK | OK | OK | OK | **Working** |
| REPL | `ScreenREPL` | OK | OK | OK | OK | OK | **Working** |
| ModelSelector | `ScreenModelSelector` | OK | OK | OK | OK | OK | **Working** |
| Settings | `ScreenSettings` | OK | OK | OK | OK | OK | **Working** |
| Resume | `ScreenResume` | OK | OK | OK | OK | OK | **Broken** (bug #1) |
| Permission | `ScreenPermission` | Modal | Modal | Modal | N/A | OK | **Working** |
| Plan | `ScreenPlan` | OK | OK | OK | Partial | OK | **Partial** (bug #9) |
| Execute | `ScreenExecute` | OK | OK | OK | Partial | OK | **Partial** (bug #12) |
| Verify | `ScreenVerify` | OK | OK | OK | Partial | OK | **Partial** (bug #12) |
| Ship | `ScreenShip` | OK | OK | OK | Partial | OK | **Partial** (bugs #12, #13) |
| Diff | `ScreenDiff` | OK | OK | OK | N/A | OK | **Partial** (bug #17) |
| Ledger | `ScreenLedger` | OK | OK | OK | Broken | OK | **Broken** (bug #3) |
| Rollback | `ScreenRollback` | OK | OK | OK | Broken | OK | **Broken** (bug #4) |
| GoalInput | `ScreenGoalInput` | OK | OK | OK | OK | OK | **Working** |
| Discuss | `ScreenDiscuss` | OK | OK | OK | Missing | OK | **Broken** (bugs #2, #7) |
| Metrics | `ScreenMetrics` | OK | OK | OK | OK | Partial | **Broken** (bug #6) |
| Config | `ScreenConfig` | OK | OK | OK | OK | OK | **Working** |
| Help | **MISSING** | OK | OK | OK | Missing | Missing | **Unreachable** (bug #5) |

---

## Summary

| Severity | Count | Description |
|----------|-------|-------------|
| CRITICAL | 5 | Session resume broken, discuss empty, ledger/rollback no-load, help unreachable |
| HIGH | 6 | Metrics resize, discuss init, question type mismatch, plan stale, goal restart, theme gaps |
| MEDIUM | 6 | Phase screens no-data, ship no-esc, message forwarding gaps, resume init, firstrun width, diff viewport |
| LOW | 6 | Dead code, unused back-stack, TODO truncation |

**Total issues: 23 bugs + 2 dead code blocks**
