# Screen Migration Report — Resume & SessionDetail

**Date:** 2026-07-12
**Scope:** Migration of ResumeModel (212 lines) and SessionDetailModel (122 lines) to Screenable interface + Router architecture (strangler-fig pattern)

---

## Summary

Successfully migrated **ResumeModel** and **SessionDetailModel** to the new Screenable interface + Router architecture. These are a functional pair — Resume's cursor selection feeds SessionDetail via the `ensureSubModel(ScreenSessionDetail)` handoff path — and the handoff is confirmed intact. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated).

---

## Changes Made

### 1. Screenable Interface Conformance

**ResumeModel** (`resume_model.go:95`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions(w, h int)`, `SetTheme(theme.Theme)`
- No internal logic rewritten — pure boundary adaptation

**SessionDetailModel** (`sessiondetail_model.go:49`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- Already implemented: `Init()`, `SetDimensions(w, h int)`, `SetTheme(theme.Theme)`
- No internal logic rewritten — pure boundary adaptation

### 2. Router Registration (`app_routing.go`)

**ScreenResume** (`app_routing.go:275-284`):
```go
m.screenUpdaters[ScreenResume] = func(msg tea.Msg) tea.Cmd {
    if m.resumeModel == nil {
        m.resumeModel = NewResumeModel(nil, m.themeManager.Current())
        m.router.Register(ScreenResume, m.resumeModel)
    }
    newModel, cmd := m.resumeModel.Update(msg)
    if r, ok := newModel.(*ResumeModel); ok {
        m.resumeModel = r
    }
    return cmd
}
```

Lazy init with empty sessions (`nil`). Actual session data arrives asynchronously via `resumeScreenReadyMsg` → `Refresh()`. Router registration ensures the screen is in the router's map for `SwitchTo`.

**ScreenSessionDetail** (`app_routing.go:394-405`):
```go
m.screenUpdaters[ScreenSessionDetail] = func(msg tea.Msg) tea.Cmd {
    if m.sessionDetailModel == nil {
        cw, ch := m.contentDimensions()
        m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
        m.router.Register(ScreenSessionDetail, m.sessionDetailModel)
    }
    newModel, cmd := m.sessionDetailModel.Update(msg)
    if r, ok := newModel.(*SessionDetailModel); ok {
        m.sessionDetailModel = r
    }
    return cmd
}
```

Standard lazy-init pattern matching other migrated screens.

### 3. View Delegation to Router (`app_view.go`)

**renderResumeContent** (`app_view.go:623-631`):
```go
func (m *AppState) renderResumeContent(chrome layout.PageChrome) string {
    if m.resumeModel == nil {
        m.resumeModel = NewResumeModel(nil, m.themeManager.Current())
        m.router.Register(ScreenResume, m.resumeModel)
    }
    m.resumeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

**renderSessionDetailContent** (`app_view.go:766-774`):
```go
func (m *AppState) renderSessionDetailContent(chrome layout.PageChrome) string {
    if m.sessionDetailModel == nil {
        m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
        m.router.Register(ScreenSessionDetail, m.sessionDetailModel)
    }
    m.sessionDetailModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (Update updater + View renderer) — safe due to router's idempotent Register.

---

## Handoff Verification (Resume → SessionDetail)

### Critical Functional Path Trace

**Step 1: Resume emits navigation request** (`resume_model.go:173-183`)
```go
case "d":
    if len(rm.sessions) > 0 {
        id := rm.sessions[rm.cursor].ID
        return rm, func() tea.Msg {
            return AppMsg{
                Screen:    ScreenSessionDetail,
                SessionID: id,
            }
        }
    }
```
User presses 'd' → gets session ID from `rm.sessions[rm.cursor].ID` → emits `AppMsg{Screen: ScreenSessionDetail, SessionID: id}`.

**Step 2: AppMsg handler intercepts SessionID** (`app_update.go:532-534`)
```go
if msg.SessionID != "" {
    return m.loadAndRestoreSession(msg.SessionID, true)
}
```
The `SessionID != ""` check fires first, routing to `loadAndRestoreSession`. This loads the session async and returns `sessionRestoredMsg`, which restores the session to REPL. **This is a pre-existing behavior** — the 'd' key's `AppMsg{Screen: ScreenSessionDetail, SessionID: id}` is intercepted by the session restore path, not the screen routing path. This existed before migration and is unchanged by it.

**Step 3: Actual handoff path — navigateToScreen** (`app_nav.go:241-271`)
When `navigateToScreen(ScreenSessionDetail)` is called (from `/open_session_detail` command or other paths):
1. `navigateToScreen` pushes current screen to back-stack
2. Calls `ensureSubModel(ScreenSessionDetail)` (`app_nav.go:437-456`)
3. Creates `SessionDetailModel` if nil
4. **Loads session from Resume's cursor** (`app_nav.go:444-454`):
   ```go
   if m.sessionDetailModel.sess == nil && m.sessionManager != nil {
       if m.resumeModel != nil && len(m.resumeModel.sessions) > 0 {
           idx := m.resumeModel.cursor
           if idx >= 0 && idx < len(m.resumeModel.sessions) {
               sid := m.resumeModel.sessions[idx].ID
               if sess, err := m.sessionManager.LoadSession(sid); err == nil && sess != nil {
                   m.sessionDetailModel.SetSession(sess)
               }
           }
       }
   }
   ```
5. Calls `switchScreen(ScreenSessionDetail)` → `router.SwitchTo(ScreenSessionDetail)`
6. Router delegates to registered `SessionDetailModel.View()`

**Step 4: SessionDetail renders** (`sessiondetail_model.go:69-121`)
`View()` renders session info from `sd.sess` (set by `SetSession` in step 3).

### Why This Path Is Unaffected by Migration

1. **`SetSession()` is a direct method call** on `*SessionDetailModel` in `ensureSubModel` — the concrete pointer is stored in `AppState.sessionDetailModel`, unchanged by the interface boundary change.
2. **Router registration** is idempotent — dual registration in updater + renderer is safe.
3. **`switchScreen`** already calls `router.SwitchTo` — the router stays in sync.
4. **No changes to `ensureSubModel(ScreenSessionDetail)`** — the handoff logic is entirely in `app_nav.go`, which we did not modify.
5. **`Update()` signature change** is a pure boundary adaptation — the concrete type returned is still `*SessionDetailModel`, and the type assertion in the updater (`newModel.(*SessionDetailModel)`) succeeds.

### Session Data Flow for Resume

1. `navigateToScreen(ScreenResume)` → `openResumeScreen()` → async `ListSessions()`
2. Async load returns `resumeScreenReadyMsg{sessions, total}`
3. Handler (`app_update.go:433-443`):
   - If `resumeModel == nil`: creates with `NewResumeModel(msg.sessions, ...)` + sets total count
   - If already exists: calls `Refresh(msg.sessions)` + `SetTotalCount(msg.total)`
4. Router delegates to `ResumeModel.View()` for rendering

**No changes to the session data flow.** The `Refresh()` method and `resumeScreenReadyMsg` handler are untouched.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | PASS |
| `go test -race ./internal/tui/...` | PASS (all 8 sub-packages) |
| `go vet ./internal/tui/...` | PASS (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | PASS (same pre-existing warning only) |

### Test Coverage Specific to Migration

| Test | Status |
|------|--------|
| `TestEnsureSubModel_SessionDetail` | PASS |
| `TestEnsureSubModel_Resume` | PASS |
| `TestScreenUpdaters_AllRegistered` (includes ScreenResume, ScreenSessionDetail) | PASS |
| `TestInitScreenUpdaters` (includes ScreenResume, ScreenSessionDetail) | PASS |
| `TestScreenNames_AllScreensHaveNames` (includes ScreenResume, ScreenSessionDetail) | PASS |
| `TestViewResume` | PASS |
| `TestViewSessionDetail` | PASS |
| `TestHandleSessionDetailRequestMsg_NilModel` | PASS |
| `TestResumeSetters` | PASS |
| `TestResumeRefresh` | PASS |
| `TestResumeRefreshSearching` | PASS |
| `TestResumeClampScroll` | PASS |
| `TestResumeVisibleRows` | PASS |
| `TestResumeModel_SetTotalCount` | PASS |
| `TestResumeModel_TruncationIndicator` | PASS |
| `TestResumeModel_NoTruncationIndicator_WhenAllVisible` | PASS |

---

## Files Changed

| File | Change |
|------|--------|
| `internal/tui/resume_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/sessiondetail_model.go` | `Update` return type: `tea.Model` → `Screenable` |
| `internal/tui/app_routing.go` | Lazy init + `router.Register` for ScreenResume, ScreenSessionDetail |
| `internal/tui/app_view.go` | Lazy init + `router.Register` + `router.View()` for renderResumeContent, renderSessionDetailContent |

---

## Remaining Screens Untouched

All other screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles: `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`, `ScreenGhostPicker`, `ScreenGhostOutput`, `ScreenPhaseModelPicker`, `ScreenDiscuss`, `ScreenBisect`, `ScreenDashboard`, `ScreenPlan`, `ScreenConfig`, `ScreenGoalInput`, `ScreenFileExplorer`, `ScreenLedger`, `ScreenRollback`, `ScreenMetrics`, `ScreenToolDetail`, `ScreenDiff`, `ScreenCommandPalette`, `ScreenModelSelector`, `ScreenMetrics`, **ScreenResume**, **ScreenSessionDetail**.

---

## Conclusion

Resume + SessionDetail migration complete. The functional pair's handoff path (`ensureSubModel` → `resumeModel.cursor` → `LoadSession` → `SetSession`) works identically after migration. The strangler-fig pattern continues to hold — old and new paths coexist, zero regressions, minimal friction. Ready to proceed with remaining screens.
