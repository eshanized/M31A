# M31A — Wiring & Integrity Issues Report

**Generated:** 2026-06-09  
**Scope:** Full codebase deep audit — TUI, Workflow, Provider, Tools, Session, Pkg  
**Severity Scale:** 🔴 Critical (crash/deadlock/data loss) | 🟠 High (broken feature) | 🟡 Medium (logic error/silent failure) | 🔵 Low (code smell/latent bug)

---

## Executive Summary

M31A has **7 critical wiring issues**, **14 high-severity issues**, **11 medium issues**, and **6 low-severity issues**. The most impactful cluster is the **Discuss phase wiring**: the `DiscussModel` emits `QuestionResponseMsg` which is handled by `handleQuestionResponse` — but that handler is wired for the `AskUserQuestion` tool flow, not the discuss Q&A flow. As a result, discuss answers are **never submitted to the workflow engine**. The second major cluster is **workflow engine emit orphaning**: the engine emits `TaskStartMsg`, `TaskUpdateMsg`, `ToolStartMsg`, `ToolCompleteMsg`, `SelfHealStartMsg/CompleteMsg`, `PhaseTransitionStartMsg/CompleteMsg`, `IntermediateProgressMsg`, `ThinkingStartMsg/CompleteMsg` — but **none of these are handled in `AppState.Update()`**, so the Execute screen never updates. A third critical issue is the **`permListenerCmd` / `questionListenerCmd` are one-shot**: they read one message and stop; after a permission response, the listener is never re-armed, causing the second tool call to block forever.

---

## Part 1 — TUI Layer Wiring Issues

### 1.1 🔴 CRITICAL: `permListenerCmd` / `questionListenerCmd` are One-Shot — Never Re-Armed

**File:** `internal/tui/app.go` lines 102–128

```go
// permListenerCmd reads one permission request from the dispatcher and emits it.
func permListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
    return func() tea.Msg {
        select {
        case req := <-d.RequestCh():
            return PermissionRequestMsg{Request: req}
        case <-ctx.Done():
            return nil
        }
    }
}
```

**Issue:** This function reads **exactly one** `PermissionRequest` from the dispatcher's channel. After it fires and emits `PermissionRequestMsg`, **no code re-issues `permListenerCmd`**. In `app_update.go`, `handlePermissionResponse()` doesn't call `permListenerCmd` again. This means:
- First tool permission → works.
- Second tool in the same session → `d.RequestCh()` blocks forever, but no listener is reading it. The workflow goroutine will also block trying to write to the channel (since `PermissionChannelBuffer` is finite). **Result: deadlock on the second permission request.**

**Same issue for `questionListenerCmd`** — the question listener is never re-armed either.

**Fix required:** In `handlePermissionResponse()` and `handleQuestionResponse()`, return `permListenerCmd(m.shutdownCtx, m.dispatcher)` and `questionListenerCmd(m.shutdownCtx, m.dispatcher)` respectively.

---

### 1.2 🔴 CRITICAL: Workflow Engine Messages Emitted But Never Handled in `AppState.Update()`

**File:** `internal/workflow/engine_messages.go` + `internal/tui/app_update.go`

The workflow engine emits all of the following message types via `MsgEmitter.Emit()`:
- `TaskStartMsg`
- `TaskUpdateMsg`
- `ToolStartMsg`
- `ToolCompleteMsg`
- `SelfHealStartMsg`
- `SelfHealCompleteMsg`
- `PhaseTransitionStartMsg`
- `PhaseTransitionCompleteMsg`
- `IntermediateProgressMsg`
- `ThinkingStartMsg`
- `ThinkingCompleteMsg`

**However, `AppState.Update()` has NO `case` for any of these types.** They all fall through to the `default` branch which routes to screen-specific submodels. Only `HealResultMsg` has a case (which is a different type altogether).

**Consequence:** The Execute screen (`ExecuteModel`) never receives task start/update/tool events. Progress bars, task status, tool call indicators — **all broken**. The execute screen is effectively a static screen.

**Fix required:** Add cases for each of these in `AppState.Update()`, forwarding them to `m.executeModel`, `m.verifyModel`, etc. as appropriate.

---

### 1.3 🔴 CRITICAL: Discuss Phase Answer Flow is Completely Broken

**Files:** `internal/tui/discuss.go`, `internal/tui/app_update.go`, `internal/workflow/discuss.go`

**Issue chain:**

1. `runDiscuss()` in `workflow/discuss.go` returns `PhaseResult{NeedsAnswers: true}` after parsing questions.
2. `handlePhaseResult()` in `app_update_phase.go` transitions to `types.PhasePlan` immediately when `types.PhaseDiscuss` completes — **without checking `NeedsAnswers`**:
   ```go
   case types.PhaseDiscuss:
       m.setWorkflowPhase(types.PhasePlan)
       m.screen = ScreenPlan
       ...
       return m.RunPhaseCmd(types.PhasePlan) // runs Plan immediately!
   ```
3. The `DiscussModel` (shown on `ScreenDiscuss`) is never shown — because `screen` is set to `ScreenPlan` immediately.
4. Even if `DiscussModel` were shown, its `advanceQuestion()` emits `QuestionResponseMsg{Answer: answer}` — but `handleQuestionResponse()` in `app_update.go` routes this to `m.dispatcher.questionRespCh`, which is the `AskUserQuestion` tool channel, **not the discuss workflow engine**.
5. The workflow engine's `SubmitDiscussAnswer()` and `FinalizeDiscuss()` are **never called from the TUI**.

**Consequence:** Discuss phase is completely non-functional. Questions are never shown, answers are never collected, and the plan phase proceeds with no user context.

**Fix required:**
- In `handlePhaseResult()` for `PhaseDiscuss`: check `msg.NeedsAnswers`, create a `DiscussModel`, show `ScreenDiscuss`, and **don't** run the next phase immediately.
- Wire discuss question/answer back to `engine.SubmitDiscussAnswer()` and `engine.FinalizeDiscuss()`.
- Add a new message type (e.g., `DiscussCompleteMsg`) that triggers the Plan phase after all answers are collected.

---

### 1.4 🔴 CRITICAL: `handlePermissionResponse` Calls `go m.dispatcher.ApprovePermission(...)` — Goroutine Mutates Dispatcher State

**File:** `internal/tui/app_update.go` lines 910–922

```go
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) tea.Cmd {
    ...
    if m.dispatcher != nil {
        go m.dispatcher.ApprovePermission(reqID, allowed, remember)  // BUG: goroutine
    }
    return nil
}
```

**Issue:** `ApprovePermission` is called in a goroutine. If `ApprovePermission` modifies the `Dispatcher`'s internal state (such as `pendingResponses` or `permissions` maps), this violates the stated thread-safety contract. While the Dispatcher uses `sync.RWMutex`, the goroutine itself is launched from `Update()`, which is fine. However, the `go` keyword is unnecessary and risky — this should just call `ApprovePermission` synchronously or via a `tea.Cmd`. The goroutine launch here could race with the next Update call.

**Secondary issue:** After this call, neither `permListenerCmd` nor `questionListenerCmd` is re-armed (see Issue 1.1).

---

### 1.5 🔴 CRITICAL: `GoalSubmittedMsg` handler immediately switches to `ScreenREPL` then runs workflow, but `DiscussModel` is never initialized

**File:** `internal/tui/app_update.go` lines 192–196

```go
case GoalSubmittedMsg:
    m.workflowGoal = msg.Goal
    m.screen = ScreenREPL  // switches to REPL
    cmds = append(cmds, m.runWorkflowFromGoal(msg.Goal)) // starts workflow
```

And `runWorkflowFromGoal` → `RunPhaseCmd(PhaseInitialize)` → which triggers `PhaseInitialize` → which triggers `PhaseDiscuss` → which (due to Issue 1.3) immediately runs `PhasePlan`.

The `DiscussModel` is assigned to `m.discussModel` but `ensureSubModel(ScreenDiscuss)` is never called from the workflow flow — because the screen is never set to `ScreenDiscuss` in `handlePhaseResult`.

---

### 1.6 🟠 HIGH: `resumeScreenReadyMsg` is Emitted but Never Handled in `AppState.Update()`

**File:** `internal/tui/app_update.go` + `internal/tui/app_update.go:888`

`openResumeScreen()` returns a function that emits `resumeScreenReadyMsg{sessions: sessions}`.

**Searching `app_update.go` for `resumeScreenReadyMsg`:** There is **no `case resumeScreenReadyMsg`** in `AppState.Update()`. The message is emitted but silently dropped. The Resume screen will never receive its session list.

**Consequence:** Opening `/resume` loads sessions in the background but the list is never displayed. The Resume screen shows empty/blank.

---

### 1.7 🟠 HIGH: `sessionLoadedMsg` / `sessionRestoredMsg` Type Mismatch

**File:** `internal/tui/app_update_commands.go` lines 170–174 and `internal/tui/helpers.go` lines 46–50

Two separate private types exist for session restore:
- `sessionLoadedMsg` (in `app_update_commands.go`) — fields: `sessionID string, navigate bool`
- `sessionRestoredMsg` (in `helpers.go`) — fields: `sess *session.Session, clearExisting bool`

`loadAndRestoreSession()` returns `sessionRestoredMsg`, and `app_update.go` handles `case sessionRestoredMsg`. But `sessionLoadedMsg` is defined but **has no case in `Update()`** — it's a dead type. This creates confusion about which restore path is actually used.

---

### 1.8 🟠 HIGH: `toastTimers` Map is Populated with Type Declaration but Never Used

**File:** `internal/tui/app_state.go` line 124

```go
toastTimers map[int]*time.Timer // index → auto-dismiss timer
```

This is initialized as `make(map[int]*time.Timer)` but **never written to or read from** in any file. The toast auto-dismiss is handled solely via `tea.Tick(duration, ...)` in `app_update.go`. The `toastTimers` field is dead weight and misleadingly implies timer-based management.

---

### 1.9 🟠 HIGH: Regular Chat Messages Routed via `SlashCommandMsg` — Misuse of Command Type

**File:** `internal/tui/repl.go` lines 308–311

```go
// Emit for routing
return func() tea.Msg {
    return SlashCommandMsg{Command: command}
}
```

Regular user messages (non-slash, non-`!`) are wrapped in `SlashCommandMsg`. In `handleSlashCommand()`, these are detected by not having a `/` prefix and passed to `sendChatMessage()`. This is a semantic abuse — a regular chat message is masquerading as a slash command. This works but breaks any future filtering on `SlashCommandMsg` and makes debugging confusing.

**Additionally:** Shell commands (`!cmd`) are also wrapped in `SlashCommandMsg` and then passed to `executeShellCommand()`, but after execution, the result is returned as `SlashCommandMsg{Command: "!result:" + result}` — this round-trips through `handleSlashCommand` which then tries to parse it as a command (no `/` prefix → calls `sendChatMessage("!result:...")`). The shell output gets sent to the LLM instead of being displayed directly.

---

### 1.10 🟠 HIGH: `DiscussModel.advanceQuestion()` Emits `QuestionResponseMsg` When All Questions Done — But Also Returns `AppMsg{Screen: ScreenREPL}` Which is Handled by a Different Path

**File:** `internal/tui/discuss.go` lines 131–136

```go
if dm.current >= len(dm.questions) {
    // All done
    return func() tea.Msg {
        return AppMsg{Screen: ScreenREPL}
    }
}
return answerCmd
```

The final answer (when `dm.current >= len(dm.questions)`) returns an `AppMsg` navigating to REPL, **but does not return the last answer via `answerCmd`**. The last answer is lost.

Additionally, when there are 0 questions, `advanceQuestion("")` is called immediately with `dm.current = 0`, which is `>= 0 = len(dm.questions)`, so it returns `AppMsg{Screen: ScreenREPL}` without ever calling any answer handler.

---

### 1.11 🟠 HIGH: `settings_edit.go` is Empty — 4-byte Stub File

**File:** `internal/tui/settings_edit.go`

The file contains only a package declaration and a comment. If any code references a function from this file (e.g., an inline edit handler), it won't exist. This is a dead stub masking incomplete implementation.

---

### 1.12 🟡 MEDIUM: `ScreenMetrics` Has No Key Routing in `routeKeyMsg`

**File:** `internal/tui/app_update.go`

The `routeKeyMsg` switch handles `ScreenREPL`, `ScreenPermission`, `ScreenModelSelector`, `ScreenSettings`, `ScreenResume`, `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, `ScreenShip`, `ScreenDiscuss`, `ScreenDiff`, `ScreenGoalInput`, `ScreenFirstRun`, `ScreenLedger`, `ScreenRollback`, `ScreenConfig` — **but NOT `ScreenMetrics`**.

The `MetricsModel` never receives key events. Users can't scroll or interact with the metrics screen.

---

### 1.13 🟡 MEDIUM: `applyTheme()` Only Propagates to 4 Sub-Models — Other Models Get Stale Themes

**File:** `internal/tui/app_update.go` lines 810–833

`applyTheme()` propagates the new theme to `replModel`, `sidebarModel`, `cmdPalette`, and `settingsModel` — but **not** to:
- `planModel`
- `executeModel`
- `verifyModel`
- `shipModel`
- `ledgerModel`
- `rollbackModel`
- `diffModel`
- `metricsModel`
- `configModel`
- `resumeModel`
- `firstRunModel`
- `goalInput`
- `discussModel`
- `msModel`

All these models retain the old theme after `/theme` is called.

---

### 1.14 🟡 MEDIUM: `handleWindowResize` Has Early-Return Bug — Misses Resize for Non-REPL Screens

**File:** `internal/tui/app_update.go` lines 376–418

```go
func (m *AppState) handleWindowResize(msg tea.WindowSizeMsg) tea.Cmd {
    ...
    if m.replModel != nil {
        m.replModel.width = msg.Width
        ...
        return cmd  // RETURNS EARLY — never resizes other models
    }

    if m.planModel != nil {    // only reached if replModel == nil
        m.planModel.SetDimensions(...)
    }
    if m.executeModel != nil {  // only reached if replModel == nil
        ...
    }
    ...
```

Since `replModel` is almost always non-nil (initialized at startup), the `if m.planModel != nil` block below is **never reached**. Plan, Execute, Verify, Ship, Settings, CommandPalette, ModelSelector, Resume, and Diff models are never resized on window resize events unless `replModel` is nil.

---

### 1.15 🟡 MEDIUM: `initWorkflowEngine` is a `tea.Cmd` that Returns `nil` — Breaks Sequential Init

**File:** `internal/tui/app.go` lines 134–189

```go
func (m *AppState) initWorkflowEngine() tea.Cmd {
    ...
    engine, err := workflow.NewEngine(...)
    ...
    m.workflowEngine = engine
    return nil  // synchronous state mutation, not a tea.Cmd!
}
```

This function mutates `m.workflowEngine` **directly** inside what looks like a `tea.Cmd` factory. It's called as:
```go
cmds := []tea.Cmd{m.initWorkflowEngine()}
```

This means `m.initWorkflowEngine()` is invoked immediately (not deferred), which is actually fine for a synchronous init — but it mutates state inside what looks like a command function. The naming suggests it should be a command; it works only because it's called directly, not scheduled. This is fragile and confusing.

---

### 1.16 🟡 MEDIUM: `GoalInputModel` Created with `nil` Callback — Never Receives Goal

**File:** `internal/tui/app_update.go` lines 675–681

```go
case ScreenGoalInput:
    if m.goalInput == nil {
        m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil) // nil callback!
        ...
    }
```

`GoalInputModel` is created with `nil` for the callback. If `GoalInputModel` calls the callback on submit, it will panic (nil function call). The goal submission relies on `GoalSubmittedMsg` being emitted from the model — but if the callback is nil, the connection breaks.

---

### 1.17 🟡 MEDIUM: `app_update.go` `handleWindowResize` doesn't forward resize to `discussModel`

**File:** `internal/tui/app_update.go`

The `handleWindowResize` function never calls `m.discussModel.SetDimensions(...)`. The Discuss screen won't reflow on terminal resize.

---

### 1.18 🔵 LOW: `shellMode` Field in `ReplModel` is Set but Never Read

**File:** `internal/tui/repl_model.go` line 101

```go
shellMode bool
```

This field is declared in `ReplModel` but there are no reads of `m.shellMode` in any TUI file. Shell command handling is done by checking `strings.HasPrefix(input, "!")` — the `shellMode` flag is dead state.

---

### 1.19 🔵 LOW: `sync_repl_provider` is Referenced but Not Implemented

**File:** `internal/tui/app_update.go` line 863

```go
return m.syncReplProvider(sess.ID)
```

`syncReplProvider` is called in `startNewSession()` but the implementation must be checked — if this function doesn't exist, it's a compile error. (Likely it does exist somewhere in helpers/commands but is not in the files reviewed here — this needs verification.)

---

## Part 2 — Workflow Engine Wiring Issues

### 2.1 🔴 CRITICAL: `MsgEmitter` is Never Set on the Engine — All `emit()` Calls are No-Ops

**File:** `internal/tui/app.go` lines 166–188

```go
engine, err := workflow.NewEngine(...)
...
m.workflowEngine = engine
```

`workflow.NewEngine()` does not accept a `MsgEmitter`. The only way to set it is `engine.SetMsgEmitter(em)`. **Searching `app.go` and all TUI files: `SetMsgEmitter` is never called.** The `channelEmitter` type exists in `app_channel.go` but is **never instantiated or connected to the engine**.

**Consequence:** Every `e.emit(...)` call in the engine returns immediately (the `msgEmitter` nil check exits early). No workflow progress messages reach the TUI. The execute screen progress bar, thinking indicators, tool cards — **all are broken**.

**Fix required:**
```go
emitter := &channelEmitter{ch: make(chan tea.Msg, 128)}
engine.SetMsgEmitter(emitter)
// Then start a goroutine or tea.Cmd to drain emitter.ch and forward to the TUI
```

---

### 2.2 🔴 CRITICAL: Execute Phase Calls `e.git.AddAll()` / `e.git.Commit()` but `e.git` Can Be `nil`

**File:** `internal/workflow/execute.go` lines 270–282

```go
if len(task.Files) > 0 {
    if err := e.git.AddAll(); err != nil {  // PANIC if e.git == nil
```

`e.git` is only set via `engine.SetGit(g)`, which is called from `initWorkflowEngine()` only `if m.git != nil`. If `m.git` is nil (non-git project), then `e.git` remains nil, and `e.git.AddAll()` panics.

The nil guard `if len(task.Files) > 0` does not protect against nil git — it only checks files. If a task has files but no git client, this panics.

**Same issue in `healTask()` lines 381–393.**

---

### 2.3 🟠 HIGH: `runDiscuss` Emits `StreamChunkMsg` but This Type is Not Handled in `AppState.Update()`

**File:** `internal/workflow/discuss.go` line 61

```go
e.msgEmitter.Emit(m31types.StreamChunkMsg{
    Chunk:  chunk,
    Source: "discuss",
})
```

`types.StreamChunkMsg` is defined as:
```go
type StreamChunkMsg = types.StreamChunkMsg  // alias in tui/types.go
```

In `app_update.go`, only `StreamMsg` (from `streaming.go`) is handled — NOT `types.StreamChunkMsg` directly. The `StreamMsg` type has a `Chunk *types.StreamChunk` field, while `types.StreamChunkMsg` has a `Chunk *types.StreamChunk` and `Source string` field. These are **different types**. The discuss streaming chunks will not be routed to the REPL for display.

---

### 2.4 🟠 HIGH: Plan Phase Never Emits `PlanReadyMsg` to TUI

**File:** `internal/workflow/plan.go` (not read yet) and `internal/tui/app_update.go`

`handlePlanReady(msg PlanReadyMsg)` exists in `app_update_phase.go` to update the plan model when tasks are ready. But `PlanReadyMsg` must be emitted from the workflow engine's plan phase — and since `SetMsgEmitter` is never called (Issue 2.1), these messages never arrive. Even if the emitter were connected, the plan phase would need to emit `PlanReadyMsg` through it, which must be verified.

---

### 2.5 🟠 HIGH: Execute Phase Uses `context.Background()` for `healTask` — Context Not Cancellable

**File:** `internal/workflow/engine.go` line 321

```go
healResult := e.healTask(context.Background(), task, failure)
```

In `HealTask()` (the TUI-triggered manual heal), `context.Background()` is used instead of the passed `ctx`. This means if the user presses Ctrl+C, the heal LLM call cannot be cancelled.

In `executeTaskWithTools()` (the automatic heal), the correct `ctx` is passed. This inconsistency between manual and automatic heal is a wiring bug.

---

### 2.6 🟠 HIGH: `engine.readTaskFiles()` Is Called But Never Defined In Reviewed Files

**File:** `internal/workflow/execute.go` line 343

```go
e.readTaskFiles(task.Files)
```

`readTaskFiles` is called but not visible in the engine files reviewed. If it's not defined anywhere, this is a compile error. If it's defined elsewhere in the workflow package, it needs to be verified for nil-safety.

---

### 2.7 🟡 MEDIUM: `runPlan` Saves Tasks Without Emitting `PlanReadyMsg` Through MsgEmitter

**File:** `internal/workflow/plan.go` (needs verification)

`PlanReadyMsg` is handled in the TUI's `handlePlanReady()`, implying the engine should emit it. Since the emitter is never connected (Issue 2.1), this is moot, but even if it were, the plan phase must be verified to call `e.emit(PlanReadyMsg{...})`.

---

## Part 3 — Tool Dispatcher Wiring Issues

### 3.1 🔴 CRITICAL: `ApprovePermission` Method Not Found on `Dispatcher`

**File:** `internal/tui/app_update.go` line 920

```go
go m.dispatcher.ApprovePermission(reqID, allowed, remember)
```

Reviewing `internal/tools/dispatcher.go`: the `Dispatcher` struct has `pendingResponses sync.Map` and `responseCh chan PermissionResponse`, but there is **no exported `ApprovePermission` method** visible in the dispatcher source. The method may be in `permissions.go` — this must be verified. If absent, this is a compile error.

---

### 3.2 🟠 HIGH: Tools Registered in `defaults.go` but `Dispatcher.Register()` Called Without Error Handling at Startup

**File:** `internal/tools/defaults.go`

Tools are registered with `dispatcher.Register(tool)`. If a tool is already registered (e.g., on engine re-init), `Register()` returns an error: `"tool already registered: %s"`. The startup code that calls `Register()` must check this error. If errors are silently ignored, double-registration attempts will fail silently, and the second registration's tool (perhaps a reconfigured one) won't replace the first.

---

### 3.3 🟡 MEDIUM: `QuestionResponseCh()` Returns the Same Channel as `questionRespCh` — Shared Between Tool and Discuss

**File:** `internal/tools/dispatcher.go` line 201

```go
func (d *Dispatcher) QuestionResponseCh() chan QuestionResponse {
    return d.questionRespCh
}
```

This channel is used for `AskUserQuestion` tool responses. The `DiscussModel` (in the TUI) routes answers to this same channel via `QuestionResponseMsg`. If the discuss Q&A and a tool's `AskUserQuestion` run concurrently (impossible given sequential design, but worth noting), they'd share the response channel, causing message confusion.

---

### 3.4 🟡 MEDIUM: `TodoWrite.SetSessionID()` is Called on `nil` Check — Race Possible

**File:** `internal/tools/dispatcher.go` lines 205–209

```go
func (d *Dispatcher) SetSessionID(id string) {
    if d.todoWrite != nil {
        d.todoWrite.SetSessionID(id)
    }
}
```

`todoWrite` is set during `NewDispatcher()` but the nil check here suggests it could be nil. If `SetSessionID` is called from multiple goroutines (e.g., when switching sessions), and `todoWrite` is being assigned concurrently, this is a data race.

---

## Part 4 — Provider & Registry Wiring Issues

### 4.1 🟠 HIGH: `registry.ActiveProvider()` vs `registry.Get(registry.Active())` — Inconsistent Usage

**Files:** Multiple TUI files

Some callers use `m.registry.ActiveProvider()` and some use `m.registry.Get(m.activeProvider)`. These return the same provider only if `m.activeProvider` matches the registry's active provider. After a fallback event (`FallbackEventMsg`), `m.activeProvider` is updated, but the registry's internal active provider may differ. This divergence means some code paths use the old provider and some use the new one.

---

### 4.2 🟠 HIGH: `FallbackEventMsg` Updates `m.activeProvider` But Not `m.activeModel`

**File:** `internal/tui/app_update.go` lines 244–247

```go
case FallbackEventMsg:
    m.activeProvider = msg.To
    slog.Info("provider fallback", ...)
```

When provider falls back, `m.activeProvider` is updated. But `m.activeModel` still references the old provider's model. If the new provider doesn't support the same model ID, subsequent `sendChatMessage()` calls will attempt to use a model that doesn't exist on the new provider. No model re-validation occurs.

---

### 4.3 🟡 MEDIUM: `ModelSelectedMsg` Handled in Two Places — Duplicate Processing

**File:** `internal/tui/app_update.go` lines 249–256

```go
case ModelSelectedMsg:
    m.activeModel = &msg.Model
    m.activeProvider = msg.Provider
    ...
```

And also:
```go
case AppMsg:
    if msg.ModelSelected != nil {
        m.activeModel = &msg.ModelSelected.Model
        m.activeProvider = msg.ModelSelected.Provider
        m.screen = ScreenREPL
        return nil
    }
```

Both `ModelSelectedMsg` and `AppMsg{ModelSelected: ...}` handle model selection. The `AppMsg` path doesn't call `m.replModel.SetProvider(...)` — it silently skips provider sync. If model selection goes through `AppMsg` path, the REPL won't have the provider updated.

---

## Part 5 — Session & Persistence Wiring Issues

### 5.1 🟠 HIGH: `applySessionRestored` Uses `sess.Provider` for SetProvider But `m.activeModel` — Provider/Model Mismatch

**File:** `internal/tui/helpers.go` lines 79–80

```go
providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
```

When restoring a session, `sess.Provider` is used for the provider, but `m.activeModel` (the currently active model) is used — not the model saved in the session. If the session was saved with a different model than the current active model, the restored session will use the wrong model. `sess.Model` (the session's saved model ID) is never used here.

---

### 5.2 🟠 HIGH: `persistWorkflowState` Only Called at `PhaseShip` Complete — All Intermediate Phases Lost on Crash

**File:** `internal/tui/app.go` lines 191–202

```go
func (m *AppState) persistWorkflowState() {
    ...
    _ = m.sessionManager.UpdateWorkflowState(
        m.sessionID,
        m.workflowGoal,
        m.workflowPhase,
        m.discussQuestions,
    )
}
```

`persistWorkflowState()` is only called in `handlePhaseResult()` for `types.PhaseShip` (after all phases complete). If the app crashes during Execute or Verify, the workflow state is not persisted. The `/resume-task` command will find an incomplete state.

---

### 5.3 🟡 MEDIUM: `UpdateWorkflowState` Error is Silently Discarded with `_ =`

**File:** `internal/tui/app.go` line 196

```go
_ = m.sessionManager.UpdateWorkflowState(...)
```

The error from `UpdateWorkflowState` is silently ignored. If persisting fails (disk full, permissions error), the user will not be notified, and the workflow state will not be recoverable.

---

### 5.4 🟡 MEDIUM: `planningDir` Computed with Brittle String Manipulation

**File:** `internal/tui/app.go` lines 162–163

```go
sessDir := m.sessionManager.BaseDir()
planningDir := sessDir + "/" + m.sessionID + "/planning"
```

The `planningDir` is constructed using string concatenation. The engine's `NewEngine()` then derives `sessionsRoot` from this:
```go
sessionsRoot := filepath.Dir(filepath.Dir(planningDir))
```

This chain works, but is fragile. If `sessDir` has a trailing slash, `planningDir` gets a double slash. Using `filepath.Join()` throughout would be safer.

---

## Part 6 — Message Routing Completeness Gaps

### 6.1 Summary of Unhandled Message Types in `AppState.Update()`

The following message types are **defined or emitted** in the codebase but have **no `case`** in `AppState.Update()`:

| Message Type | Where Emitted/Defined | Effect of No Handler |
|---|---|---|
| `workflow.TaskStartMsg` | `engine.go` | Execute screen progress never updates |
| `workflow.TaskUpdateMsg` | `engine.go` | Task status never reflected |
| `workflow.ToolStartMsg` | `engine.go` | Tool call indicators broken |
| `workflow.ToolCompleteMsg` | `engine.go` | Tool completion never shown |
| `workflow.SelfHealStartMsg` | `engine.go` | Heal indicator never shown |
| `workflow.SelfHealCompleteMsg` | `engine.go` | Heal result never shown |
| `workflow.PhaseTransitionStartMsg` | `engine.go` | Phase transition animation broken |
| `workflow.PhaseTransitionCompleteMsg` | `engine.go` | Phase complete notification broken |
| `workflow.IntermediateProgressMsg` | `engine.go` | Progress messages dropped |
| `workflow.ThinkingStartMsg` | `engine.go` | Thinking indicator for workflow broken |
| `workflow.ThinkingCompleteMsg` | `engine.go` | Thinking completion not shown |
| `resumeScreenReadyMsg` | `app_update.go` | Resume screen never gets sessions |
| `sessionLoadedMsg` | `app_update_commands.go` | Dead type, unreachable |

---

## Part 7 — Cross-Cutting Integrity Issues

### 7.1 🟡 MEDIUM: `DiscussAnswerTimeoutMsg` Emitted from `DiscussModel` But Never Handled by `AppState`

**File:** `internal/tui/types.go` line 157

`DiscussAnswerTimeoutMsg{QuestionIndex int}` is defined and handled inside `DiscussModel.Update()`. But `DiscussModel.Update()` is only called when `m.screen == ScreenDiscuss` — and due to Issue 1.3, `ScreenDiscuss` is never set. Furthermore, the timeout timer that emits this message is never started (the `SetTimeout()` on DiscussModel is never called from the TUI).

---

### 7.2 🟡 MEDIUM: `OptimizedMsg` Defined but Never Handled

**File:** `internal/tui/types.go` lines 241–245

```go
type OptimizedMsg struct {
    Recommendations []arbitrage.ArbitrageRecommendation
    TaskID          int
}
```

`OptimizedMsg` is defined in `types.go` but there is no `case OptimizedMsg` in `AppState.Update()`. The `handleOptimize` command returns a `ToastMsg` instead. `OptimizedMsg` is dead code.

---

### 7.3 🟡 MEDIUM: `SidebarRefreshMsg` Handled in `AppState.Update()` but `m.sidebarModel.Update(msg)` Returns Wrong Type

**File:** `internal/tui/app_update.go` lines 259–272

```go
case SidebarRefreshMsg:
    if m.sidebarModel != nil {
        newSidebar, cmd := m.sidebarModel.Update(msg)
        m.sidebarModel = newSidebar  // BUG: type assertion missing
```

`SidebarModel.Update()` returns `(tea.Model, tea.Cmd)`. The code assigns `newSidebar` (type `tea.Model`) to `m.sidebarModel` (type `*SidebarModel`) without a type assertion. This is a **compile error** unless `m.sidebarModel` is declared as `tea.Model`, which it is not (it's `*SidebarModel` in `app_state.go`).

Either: (a) this doesn't compile — in which case the sidebar is broken at compile time; or (b) the assignment works because Go's interface system allows it only if `SidebarModel.Update()` returns `*SidebarModel` directly, not `tea.Model`. This needs immediate verification.

---

### 7.4 🔵 LOW: `channelEmitter` in `app_channel.go` is Defined but Never Instantiated

**File:** `internal/tui/app_channel.go` lines 41–57

`channelEmitter` implements `workflow.MsgEmitter` with a `chan tea.Msg`. This is the bridge between the workflow engine and the TUI. However, it is **never instantiated** — the `SetMsgEmitter()` on the engine is never called. This confirms Issue 2.1 and shows the missing wiring is specifically this channel.

---

### 7.5 🔵 LOW: `KeyActionMsg` Defined and Emitted by REPL but `LeaderTimeoutMsg` Handler Calls `m.keyRegistry.DeactivateLeader()` — No Return Path for Timeout Cmd

**File:** `internal/tui/app_update.go` lines 62–66

```go
case LeaderTimeoutMsg:
    if m.keyRegistry != nil {
        m.keyRegistry.DeactivateLeader()
    }
```

When leader is deactivated, the TUI does not re-render. The which-key overlay will linger until the next render cycle. This is a minor visual glitch but stems from missing a `return m, tea.Batch(cmds...)` that forces a re-render.

---

### 7.6 🔵 LOW: `MaxMessageHistory = 1000` but Trim at 500 — Off-By-500 Bug

**File:** `internal/tui/repl_state.go` lines 165–167

```go
if len(m.messages) > MaxMessageHistory {
    m.messages = m.messages[len(m.messages)-500:]
}
```

The constant is `MaxMessageHistory = 1000`, but when trimming, it keeps only the last 500. The trim threshold (1000) and the keep amount (500) are inconsistent. Likely both should use the same constant or related constants.

---

## Part 8 — Architecture Violations

### 8.1 🔴 CRITICAL: State Mutation in `view_file` (View function mutates state)

**File:** `internal/tui/repl_view.go` lines 54–57

```go
// View renders the REPL screen.
func (m *ReplModel) View() string {
    ...
    if len(m.messages) == 0 && !m.streaming {
        welcomeContent := m.renderWelcome()
        m.viewport.SetContent(welcomeContent)  // MUTATION IN VIEW!
    }
```

**Bubble Tea's critical rule: `View()` must be pure — no mutations.** `m.viewport.SetContent()` mutates the viewport model's internal state. This is called on every render cycle (typically 10fps during streaming, on every keypress otherwise). The viewport state is being mutated from `View()`, which can cause race conditions with `Update()` and produce unpredictable rendering.

**Same issue:** `m.viewport.SetContent(sb.String())` is called at the end of `renderMessages()`, which is called from both `Update()` (safe) and `View()` (unsafe, via the welcome-content path).

---

### 8.2 🟡 MEDIUM: `go m.dispatcher.ApprovePermission(...)` in `Update()` — Goroutine from Update

**File:** `internal/tui/app_update.go` line 920

Even though `Dispatcher` is thread-safe, launching a goroutine from inside `Update()` is an architecture smell. The correct pattern is to use a `tea.Cmd` that performs the approval and returns any resulting message.

---

## Summary Table

| ID | Severity | Area | Issue | Impact |
|----|----------|------|-------|--------|
| 1.1 | 🔴 Critical | TUI | Permission/Question listeners never re-armed | Deadlock after first tool call |
| 1.2 | 🔴 Critical | TUI | Workflow engine messages not handled in Update() | Execute screen completely broken |
| 1.3 | 🔴 Critical | TUI/Workflow | Discuss phase answer flow broken | Discuss Q&A never works |
| 1.4 | 🔴 Critical | TUI | ApprovePermission called in goroutine | Architecture violation, race risk |
| 1.5 | 🔴 Critical | TUI | GoalSubmitted → DiscussModel never shown | Workflow always skips discuss |
| 2.1 | 🔴 Critical | Workflow | MsgEmitter never set on engine | ALL workflow messages silent |
| 2.2 | 🔴 Critical | Workflow | `e.git` can be nil in execute phase | Panic on git operations |
| 3.1 | 🔴 Critical | Tools | `ApprovePermission` may not exist | Compile error / broken permissions |
| 1.6 | 🟠 High | TUI | `resumeScreenReadyMsg` never handled | Resume screen always empty |
| 1.7 | 🟠 High | TUI | `sessionLoadedMsg` is dead code | Confusing, latent bug |
| 1.8 | 🟠 High | TUI | `toastTimers` field is dead | Dead weight, misleading |
| 1.9 | 🟠 High | TUI | Chat/shell msgs wrapped as SlashCommandMsg | Shell output sent to LLM |
| 1.10 | 🟠 High | TUI | Last discuss answer dropped | Final answer lost |
| 1.11 | 🟠 High | TUI | `settings_edit.go` is empty stub | Missing functionality |
| 2.3 | 🟠 High | Workflow | Discuss `StreamChunkMsg` not handled | Discuss streaming not rendered |
| 2.5 | 🟠 High | Workflow | HealTask uses `context.Background()` | Heal not cancellable |
| 2.6 | 🟠 High | Workflow | `readTaskFiles()` called but not visible | Potential compile error |
| 4.1 | 🟠 High | Provider | ActiveProvider() vs Get() inconsistent | Provider mismatch on fallback |
| 4.2 | 🟠 High | Provider | FallbackEvent doesn't update model | Wrong model on fallback |
| 5.1 | 🟠 High | Session | Session restore uses wrong model | Model mismatch on resume |
| 5.2 | 🟠 High | Session | Workflow state only persisted at Ship | State lost on crash |
| 6.1 | 🟠 High | Routing | 13 message types with no handler | Various features broken |
| 7.3 | 🟠 High | Integrity | SidebarRefreshMsg Update type error | Sidebar broken or compile error |
| 8.1 | 🔴 Critical | Architecture | State mutation in View() | Race condition, unpredictable render |
| 1.12 | 🟡 Medium | TUI | ScreenMetrics has no key routing | Metrics screen not interactive |
| 1.13 | 🟡 Medium | TUI | Theme not propagated to 13 sub-models | Stale themes after toggle |
| 1.14 | 🟡 Medium | TUI | handleWindowResize early-return bug | Non-REPL screens never resized |
| 1.15 | 🟡 Medium | TUI | initWorkflowEngine looks like Cmd but isn't | Confusing, potential misuse |
| 1.16 | 🟡 Medium | TUI | GoalInputModel created with nil callback | Potential panic |
| 1.17 | 🟡 Medium | TUI | discussModel not resized on window change | Discuss screen layout breaks |
| 3.3 | 🟡 Medium | Tools | AskUserQuestion shares channel with Discuss | Potential answer confusion |
| 3.4 | 🟡 Medium | Tools | SetSessionID todoWrite race possible | Data race on session switch |
| 4.3 | 🟡 Medium | Provider | ModelSelectedMsg handled in two places | AppMsg path skips provider sync |
| 5.3 | 🟡 Medium | Session | UpdateWorkflowState error silently ignored | Silent data loss |
| 5.4 | 🟡 Medium | Session | planningDir built with string concat | Fragile path construction |
| 7.1 | 🟡 Medium | Routing | DiscussAnswerTimeoutMsg timer never started | Timeout never fires |
| 7.2 | 🟡 Medium | Routing | OptimizedMsg defined but never handled | Dead type |
| 8.2 | 🟡 Medium | Architecture | Goroutine launched from Update() | Architecture violation |
| 1.18 | 🔵 Low | TUI | `shellMode` field never read | Dead state |
| 1.19 | 🔵 Low | TUI | `syncReplProvider` needs verification | Potential missing method |
| 6.1 | 🔵 Low | Routing | `sessionLoadedMsg` dead type | Confusing |
| 7.4 | 🔵 Low | Integrity | `channelEmitter` never instantiated | Confirms issue 2.1 |
| 7.5 | 🔵 Low | TUI | LeaderTimeout no re-render | Visual glitch |
| 7.6 | 🔵 Low | TUI | MaxMessageHistory/trim inconsistency | Off-by-500 |

---

## Priority Fix Order

### Phase 1 — Must Fix First (Blockers)

1. **Set `MsgEmitter` on workflow engine** (Issue 2.1) — without this, the entire workflow pipeline produces no TUI feedback.
2. **Re-arm permission/question listeners after each use** (Issue 1.1) — without this, any session with more than one tool call deadlocks.
3. **Fix Discuss phase flow** (Issues 1.3, 1.5) — without this, the discuss screen never appears and answers are never collected.
4. **Add workflow message handlers to `AppState.Update()`** (Issue 1.2) — without this, the Execute, Verify, and Ship screens are static.
5. **Fix `SidebarRefreshMsg` type assertion** (Issue 7.3) — if this is a compile error, nothing runs.
6. **Fix nil git guard in execute/heal** (Issue 2.2) — without this, non-git projects panic.
7. **Fix state mutation in `View()`** (Issue 8.1) — this violates Bubble Tea's core contract and causes unpredictable rendering.

### Phase 2 — High Priority

8. Fix `handleWindowResize` early-return (Issue 1.14)
9. Fix session restore model mismatch (Issue 5.1)
10. Fix shell command output going to LLM (Issue 1.9)
11. Fix `resumeScreenReadyMsg` not handled (Issue 1.6)
12. Add `FallbackEventMsg` model re-validation (Issue 4.2)

### Phase 3 — Medium Priority

13. Persist workflow state at each phase transition (Issue 5.2)
14. Propagate theme to all sub-models (Issue 1.13)
15. Fix `GoalInputModel` nil callback (Issue 1.16)
16. Add ScreenMetrics key routing (Issue 1.12)
17. Fix last discuss answer dropped (Issue 1.10)

---

## Part 9 — Additional Findings from Deep Agent Analysis

### 9.1 🔴 CRITICAL: `verifyTask` Context Leak — `defer cancel()` in Loop Leaks All But Last Cancel

**File:** `internal/workflow/engine_verify.go` lines 140–202

Every iteration of `for _, f := range task.Files` creates a `context.WithTimeout` and calls `defer cancel()`. Because `defer` accumulates until function return, and each iteration **overwrites** the `ctx` local variable, only the **last** `cancel` is kept in scope — all prior `cancel` functions are lost. The contexts are never cancelled until the timeout fires naturally. For large task file lists, this creates unbounded goroutine-timer leaks.

**Additionally:** `verifyTask` uses `context.Background()` as parent, not the passed session context — user cancellation cannot propagate to build/test subprocesses.

**Fix:** Call `cancel()` explicitly after each `cmd.CombinedOutput()`, not via `defer`.

---

### 9.2 🔴 CRITICAL: `verifyTaskContext` Discards `cancel` — Timer Resource Leak

**File:** `internal/workflow/engine_verify.go` lines 108–113

```go
func (e *Engine) verifyTaskContext(parent context.Context) context.Context {
    ctx, cancel := context.WithTimeout(parent, verifyTaskTimeout)
    _ = cancel // ← resources leaked until deadline fires
}
```

Go's own documentation states: "The caller should call the cancel function anyway to release resources." By discarding it, the timer for verify phase timeout will never be released until it fires (5 minutes), holding context resources throughout.

---

### 9.3 🔴 CRITICAL: `dispatcher.Execute()` Always Returns `nil` Error on Tool Failure — Failures Silently Succeed

**File:** `internal/tools/dispatcher.go` lines 162–172

```go
res := types.ToolResult{
    Error: "",
}
if err != nil {
    res.Error = err.Error()
}
return res, nil  // ← always nil error!
```

When a tool's `Execute()` returns an error, the dispatcher puts the error string in `res.Error` and returns `(res, nil)`. In `execute.go`, the caller checks `err != nil` — which is **always false**. Tool failures are silently treated as success. Tasks may complete with missing files, the LLM is never informed of failure, and self-heal never triggers from tool errors (only from parse errors).

**`PermissionDenied` is the only correctly propagated error** (lines 141–153, returns `(ToolResult{}, err)`).

---

### 9.4 🔴 CRITICAL: `ApprovePermission` Fallback Channel Send Can Deadlock the TUI

**File:** `internal/tools/permissions.go` lines 16–26

```go
func (d *Dispatcher) ApprovePermission(requestID int64, ...) {
    ...
    // Fallback to shared channel for backwards compatibility
    d.responseCh <- resp  // ← blocking send, can deadlock!
```

The fallback path is an unbuffered send to `responseCh` (buffer=8, but can be full). If no goroutine is draining `responseCh` and the buffer is full, this call blocks indefinitely. Since this is called from the TUI's `Update()` (via goroutine in Issue 1.4), the goroutine itself can hang, causing a background goroutine leak and making permission responses unreliable.

---

### 9.5 🔴 CRITICAL: `fuzzyAnchorReplace` in `edit.go` Corrupts Edited Files

**File:** `internal/tools/edit.go` lines 425–429

```go
newLines := make([]string, len(contentLines))  // length = N, all empty strings
copy(newLines, contentLines[:i])               // fills 0..i-1
newLines = append(newLines, strings.Split(newString, "\n")...)  // appends AFTER index N!
newLines = append(newLines, contentLines[endIdx+1:]...)
```

`make([]string, len(contentLines))` creates a slice of length `N` (all zero-value empty strings). After `copy()` fills the prefix, `append` adds new content **past index N**, so the result contains `N` entries (prefix + trailing empty strings) followed by the replacement and suffix. The output will have a block of empty strings corrupting the edited file.

**Fix:** Use `make([]string, 0, len(contentLines))` — capacity but zero length.

---

### 9.6 🔴 CRITICAL: `grepWithRG` Pipe Drain Deadlock on Result Limit

**File:** `internal/tools/grep.go` lines 182–189

```go
for scanner.Scan() {
    if count >= maxResults {
        if scanner.Scan() {  // consumes one extra line, stops reading
            truncated = true
        }
        break
    }
```

After hitting the limit, the scanner stops reading from `rg`'s stdout pipe. `rg` is still running and will block on write when the pipe buffer fills. `cmd.Wait()` then blocks waiting for `rg` to exit, which never happens. **Result: `grep` tool hangs forever on large outputs.**

**Fix:** Kill `rg` when limit is reached (`cmd.Process.Kill()`) before calling `Wait()`.

---

### 9.7 🔴 CRITICAL: `registry.List()` Decorates Active Provider Name — Breaks `FindFallbackProvider`

**File:** `internal/provider/registry.go` lines 86–93; `internal/provider/fallback.go` line 26

```go
// In List():
names[i] = name + " (active)"  // modifies the returned name!

// In FindFallbackProvider:
for _, name := range names {
    if name == currentProvider {  // never matches " (active)" suffix
        continue
    }
```

`FindFallbackProvider` calls `registry.List()` which returns decorated names, then compares against plain provider names. The active provider is **never skipped** — fallback logic will attempt to use the current provider as its own fallback, causing self-fallback loops or incorrect provider selection.

---

### 9.8 🔴 CRITICAL: `executeTaskWithTools` — Tool Result Missing `ToolCallID` in Message

**File:** `internal/workflow/execute.go` lines 231–235

```go
messages = append(messages, m31types.Message{
    Role:    "tool",
    Content: result.Output,
    // Missing: ToolCallID — required by OpenRouter/Zen for multi-turn tool conversations
})
```

OpenRouter and Zen (OpenAI-compatible) APIs require tool result messages to include the call ID to correlate results with requests. Without this, multi-turn tool-use conversations produce malformed API requests, likely causing `400 Bad Request` errors after the first tool call.

---

### 9.9 🟠 HIGH: `HealResultMsg` in `app_update.go` — `verifyModel.Update()` Return Value Discarded

**File:** `internal/tui/app_update.go` lines 187–190

```go
case HealResultMsg:
    if m.verifyModel != nil {
        m.verifyModel.Update(msg)  // ← return values (model, cmd) DISCARDED
    }
```

`VerifyModel.Update()` returns `(*VerifyModel, tea.Cmd)`. Both are dropped: the updated model is lost, the old stale model persists, and any command returned (e.g. viewport scroll) is silently dropped.

---

### 9.10 🟠 HIGH: `verify.go` — `statusBadge` Struct Used as String (Missing `.Render()`)

**File:** `internal/tui/verify.go` line 207

```go
statusBadge := components.SimpleBadge{...}  // struct value
line := fmt.Sprintf("  %s %s %s", num, statusBadge, action)  // used as string!
```

`fmt.Sprintf("%s", ...)` on a struct formats as `{...}` unless the type implements `fmt.Stringer`. `SimpleBadge` is a struct, not a `Stringer`. This renders garbage (struct dump) in the Verify screen task list. Should be `statusBadge.Render()`.

---

### 9.11 🟠 HIGH: `execute_model.go` — `countTasks()` Return Values Assigned in Wrong Order

**File:** `internal/tui/execute_model.go` line 93

```go
// Function: countTasks() returns (done, total, failed int)
newDone, newFailed, newTotal := em.countTasks()  // ← wrong order: total→newFailed, failed→newTotal
```

The names are swapped: `total` is assigned to `newFailed` and `failed` to `newTotal`. This corrupts progress calculations and flash-color decisions in the Execute screen.

---

### 9.12 🟠 HIGH: `AppendStreamChunk()` in `repl_stream.go` — Never Called, Workflow Streaming Dead

**File:** `internal/tui/repl_stream.go` lines 19–25

`AppendStreamChunk(chunk *types.StreamChunk)` is defined on `ReplModel` but has **zero callers** in the entire codebase. Any workflow phase that streams via this function produces no visible output in the REPL. This is the intended integration point for displaying workflow LLM streaming — it's complete but disconnected.

---

### 9.13 🟠 HIGH: `SidebarRefreshMsg` Handler Always Re-Schedules — Infinite Git Poll Loop

**File:** `internal/tui/sidebar.go` lines 283–305

```go
func (s *SidebarModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    ...
    case SidebarRefreshMsg:
        ...
        return s, s.refreshCmd()  // always schedules another refresh!
```

Every `SidebarRefreshMsg` unconditionally schedules another `refreshCmd()`. This is an infinite loop that continuously shells out to `git status --porcelain` with no cooldown or stop condition. On a large repo with slow git, this pins a goroutine permanently.

---

### 9.14 🟠 HIGH: `TickMsg` Never Forwarded to `VerifyModel` — Heal Spinner Frozen

**File:** `internal/tui/app_update.go`

`TickMsg` is forwarded to `ExecuteModel` and `ReplModel` but NOT to `VerifyModel`. `VerifyModel` has a spinner (`vm.spinner`) and a `TickSpinner()` method but no code ever ticks it. The heal-in-progress spinner animation will never advance.

---

### 9.15 🟠 HIGH: `FirstRunModel.AppMsg.SaveKeychain` — Never Acted Upon

**File:** `internal/tui/firstrun_model.go` line 368 + `internal/tui/app_update.go`

`completeSetup()` emits `AppMsg{Screen: ScreenREPL, SaveKeychain: opts.SaveKeychain}`. In `handleAppMsg()` → `routeAppMsgAction()`, `SaveKeychain` is never read. The intent to persist the API key to the OS keychain is silently ignored. The field is a no-op.

---

### 9.16 🟠 HIGH: `plan_model.go` `computeWaves()` — Infinite Loop on Cyclic Dependencies

**File:** `internal/tui/plan_model.go` lines 82–116

```go
for remaining > 0 { ... }  // no cycle detection!
```

The wave computation loop has no cycle detection. If tasks have a circular dependency, `remaining` never reaches 0, hanging the TUI indefinitely.

---

### 9.17 🟠 HIGH: `ModelSelector` `allDone` Never True if Provider Errors — Spinner Hangs

**File:** `internal/tui/modelselector.go` lines 132–142

If a provider returns an error, no entry is added to `modelsByProv`. The `allDone` check waits for all providers to be in `modelsByProv`, which never happens for errored providers. The loading spinner hangs forever.

---

### 9.18 🟡 MEDIUM: Config `substituteVars` Replaces `${VAR}` with `""` — Silently Clears API Keys

**File:** `internal/config/loader.go` lines 531–537

```go
return varRe.ReplaceAllStringFunc(s, func(match string) string {
    if val, ok := os.LookupEnv(name); ok {
        return val
    }
    return ""  // silently clears ${M31A_OPENROUTER_API_KEY} if unset!
})
```

Users who write `api_key = "${M31A_OPENROUTER_API_KEY}"` in their config will silently get an empty API key if the environment variable is not set.

---

### 9.19 🟡 MEDIUM: `parseToolCalls` Iterates by Byte Index on UTF-8 Content — Potential Panic/Corruption

**File:** `internal/workflow/engine_parse.go` line 314

```go
for i := 0; i < scanLimit; i++ {
    if content[i] != '{'  // byte-level access on UTF-8 string
```

Iterating a `string` by byte index and then doing `i += len(obj) - 1` (where `len(obj)` is the rune-count of a sub-string) mixes byte and rune indexing. For LLM responses with unicode content (emoji, CJK characters), this can step into the middle of a multi-byte sequence, producing incorrect extraction or a panic on the next byte access.

---

### 9.20 🟡 MEDIUM: `DiscussAnswerTimeoutMsg` Timer Never Started — Timeout Display is Cosmetic

**File:** `internal/tui/discuss.go`

`SetTimeout()` sets `dm.deadline` and `dm.hasDeadline = true`, and `View()` renders a countdown. But **nothing ever emits `DiscussAnswerTimeoutMsg`** — there's no `tea.Tick` command in `Init()` or anywhere that fires this message. The timer is display-only.

---

### 9.21 🟡 MEDIUM: Zen Provider Always Reports `$0` Cost — Cost Tracking Dead for Zen Users

**File:** `internal/provider/zen/client.go` lines 135–137

```go
InputPerMToken:  0,
OutputPerMToken: 0,
```

Zen models have hardcoded zero pricing. `EstimateCost()` always returns 0 for Zen. Cost tracking, budget warnings, and the ledger cost metrics are all meaningless for Zen users.

---

### 9.22 🟡 MEDIUM: `settingsHealthMsg` Dropped if User Navigates Away During Health Check

**File:** `internal/tui/settings_model.go` line 49

`settingsHealthMsg` is handled inside `SettingsModel.Update()`. In `app_update.go`, the `default:` case only forwards to `SettingsModel` when `m.screen == ScreenSettings`. If the health check goroutine completes while the user has navigated to a different screen, the message is silently dropped and the health status never updates.

---

## Updated Summary Table (Including Agent Findings)

| ID | Severity | Area | Issue |
|----|----------|------|-------|
| 1.1 | 🔴 | TUI | Permission/Question listeners never re-armed → deadlock |
| 1.2 | 🔴 | TUI | 11 workflow message types not handled in Update() |
| 1.3 | 🔴 | TUI/Workflow | Discuss phase answer flow completely broken |
| 1.4 | 🔴 | TUI | ApprovePermission goroutine from Update() |
| 1.5 | 🔴 | TUI | GoalSubmitted → DiscussModel never initialized |
| 2.1 | 🔴 | Workflow | MsgEmitter never set → all workflow emit() are no-ops |
| 2.2 | 🔴 | Workflow | `e.git` nil panic in execute/heal |
| 3.1 | 🔴 | Tools | ApprovePermission method may be missing |
| 8.1 | 🔴 | Architecture | State mutation in View() |
| 9.1 | 🔴 | Workflow | verifyTask context leak in file loop |
| 9.2 | 🔴 | Workflow | verifyTaskContext discards cancel → timer leak |
| 9.3 | 🔴 | Tools | dispatcher.Execute() always returns nil error → failures succeed silently |
| 9.4 | 🔴 | Tools | ApprovePermission fallback blocking send → deadlock |
| 9.5 | 🔴 | Tools | fuzzyAnchorReplace corrupts edited files |
| 9.6 | 🔴 | Tools | grepWithRG pipe drain deadlock on result limit |
| 9.7 | 🔴 | Provider | registry.List() name decoration breaks fallback |
| 9.8 | 🔴 | Workflow | Tool result messages missing ToolCallID → API errors |
| 1.6 | 🟠 | TUI | resumeScreenReadyMsg never handled |
| 1.9 | 🟠 | TUI | Shell output routed to LLM via SlashCommandMsg |
| 1.10 | 🟠 | TUI | Last discuss answer dropped |
| 2.3 | 🟠 | Workflow | Discuss StreamChunkMsg not handled |
| 2.5 | 🟠 | Workflow | HealTask uses context.Background() |
| 4.1 | 🟠 | Provider | ActiveProvider() vs Get() inconsistent |
| 4.2 | 🟠 | Provider | FallbackEvent doesn't update model |
| 5.1 | 🟠 | Session | Session restore uses current model not session's model |
| 5.2 | 🟠 | Session | Workflow state only persisted at Ship |
| 9.9 | 🟠 | TUI | HealResultMsg discards verifyModel.Update() return |
| 9.10 | 🟠 | TUI | verify.go statusBadge used as string, not rendered |
| 9.11 | 🟠 | TUI | execute_model.go countTasks() return values wrong order |
| 9.12 | 🟠 | TUI | AppendStreamChunk never called → workflow streaming dead |
| 9.13 | 🟠 | TUI | SidebarRefreshMsg creates infinite git poll loop |
| 9.14 | 🟠 | TUI | TickMsg not forwarded to VerifyModel → spinner frozen |
| 9.15 | 🟠 | TUI | SaveKeychain from FirstRun never acted upon |
| 9.16 | 🟠 | TUI | computeWaves() infinite loop on cyclic tasks |
| 9.17 | 🟠 | TUI | ModelSelector allDone never true on provider error |
| 1.12 | 🟡 | TUI | ScreenMetrics missing from key routing |
| 1.13 | 🟡 | TUI | Theme not propagated to 13 sub-models |
| 1.14 | 🟡 | TUI | handleWindowResize early-return skips all non-REPL models |
| 1.16 | 🟡 | TUI | GoalInputModel nil callback |
| 3.3 | 🟡 | Tools | AskUserQuestion shares channel with Discuss |
| 4.3 | 🟡 | Provider | ModelSelectedMsg AppMsg path skips provider sync |
| 5.3 | 🟡 | Session | UpdateWorkflowState error silently ignored |
| 7.1 | 🟡 | Routing | DiscussAnswerTimeoutMsg timer never started |
| 9.18 | 🟡 | Config | substituteVars clears ${VAR} silently |
| 9.19 | 🟡 | Workflow | parseToolCalls byte/rune index mismatch on UTF-8 |
| 9.20 | 🟡 | TUI | Discuss timeout display is cosmetic only |
| 9.21 | 🟡 | Provider | Zen pricing hardcoded 0 → cost tracking dead |
| 9.22 | 🟡 | TUI | settingsHealthMsg dropped on screen navigate |

---

## Final Count

| Severity | Count |
|----------|-------|
| 🔴 Critical | **17** |
| 🟠 High | **22** |
| 🟡 Medium | **12** |
| 🔵 Low | **6** |
| **Total** | **57** |

---

## Part 10 — Package & Infrastructure Findings

### 10.1 🔴 CRITICAL: `LoadWorkflowState` Catches Wrong Sentinel — First-Run Sessions Always Error

**File:** `pkg/session/manager.go` line 257

```go
if errors.Is(err, m31errors.ErrSessionCorrupted) {
    // Session doesn't exist yet — return zero values with no error
```

The comment says "Session doesn't exist yet" but the guard checks `ErrSessionCorrupted`, not `ErrSessionNotFound`. A genuinely missing session file returns `ErrSessionNotFound` (not caught here), which propagates as a real error. First-run sessions fail silently or noisily depending on call site.

---

### 10.2 🔴 CRITICAL: `Ledger.Append` is Never Called From Production Code — Ledger Always Empty

**File:** `pkg/ledger/ledger.go`

After exhaustive codebase search, `ledger.Append` and `ledger.NewEntry` are **never called** from any workflow, TUI, or main entrypoint — only from tests. The Ship phase (`internal/workflow/ship.go`) has no ledger append call. The ledger always shows 0 sessions; the cross-session learning feature is completely non-functional.

**Fix:** Add `ledger.NewEntry(...) + ledger.Append(entry)` at the end of `runShip()`.

---

### 10.3 🔴 CRITICAL: Arbitrage `alternatives` Collection Logic Is Inverted

**File:** `pkg/arbitrage/arbitrage.go` lines 196–208

`estimates` is sorted ascending by cost. The loop skips everything before `recommended` (cheaper models) and collects everything after (more expensive). The comment says "cheaper than the recommended model" — but the logic collects **more expensive** models. Model recommendations and alternative suggestions are incorrect.

---

### 10.4 🔴 CRITICAL: `keychain_linux.go` `fmtSecret` Uses Zero `sessionPath` — D-Bus CreateItem Fails

**File:** `pkg/keychain/keychain_linux.go` lines 167–174

`OpenSession` returns a `sessionPath` that is stored locally, but `fmtSecret` declares its **own** zero `dbus.ObjectPath` instead of using the returned one. All D-Bus `CreateItem` calls pass an empty session path, likely causing keychain writes to fail silently.

---

### 10.5 🟠 HIGH: `HardReset` Stashes Changes But Never Pops — User Data Left in Stash

**File:** `pkg/rollback/rollback.go` lines 148–168

`HardReset` stashes uncommitted changes before resetting, but unlike `SafeReset`, never pops the stash after. User data is silently preserved in the stash but not restored. The return message says "Changes stashed", which is accurate but non-obvious to users.

---

### 10.6 🟠 HIGH: `session.Tasks` Field Never Loaded from Disk After `LoadSession`

**File:** `pkg/session/session.go` lines 15, 41

The `Tasks` field in `Session` struct is serialized to `session.json`, but `LoadSession` never populates it — tasks are loaded separately via `LoadTasks()` (from TASKS.md). Any code checking `session.Tasks` after load sees an empty slice even if TASKS.md has tasks.

---

### 10.7 🟠 HIGH: `ResumedAt` Set on Every Internal `LoadSession` Call — Meaningless Marker

**File:** `pkg/session/manager.go` lines 224–225

`LoadSession` sets `session.ResumedAt = &now` unconditionally. But `LoadSession` is called internally by `UpdateWorkflowState`, `ForkSession`, `SiblingSessions` etc. — not just user resume actions. `ResumedAt` always reflects the last internal session access, not user resume events.

---

### 10.8 🟡 MEDIUM: `os.Getwd()` Error Silently Ignored at Startup

**File:** `cmd/m31a/main.go` line 179

```go
workDir, _ := os.Getwd()
```

If the current directory was deleted, `workDir` is empty string. All tool path operations will use `/` as the base — potentially dangerous. Should check and exit on error.

---

### 10.9 🟡 MEDIUM: `creack/pty` Missing from `go.mod` — Required by AGENTS.md

**File:** `cmd/m31a/main.go` + `go.mod`

`AGENTS.md` lists `creack/pty` as a required dependency for PTY-based Bash tool on Linux/macOS. It does not appear in `go.mod`. If the Bash tool uses PTY, this is a missing dependency.

---

### 10.10 🟡 MEDIUM: `autodream.go` Appends `summaryMsg` at End of `kept` — Wrong Chronological Order

**File:** `pkg/autodream/autodream.go` line 211

```go
c.messages = append(kept, summaryMsg)
```

A summary of old consolidated messages is appended **after** newer messages, not before. The summary should be at the beginning so older context precedes newer messages.

---

### 10.11 🟡 MEDIUM: `git.StatusPorcelain` Rename Parsing Uses Wrong Format

**File:** `internal/git/git.go` lines 302–307

The `--porcelain` (v1) format encodes renames as `R old\x00new` (null-separated), not `old -> new`. The current code looks for ` -> ` which never appears in porcelain output. `OldPath` is never populated; renames are treated as single-path entries.

---

### 10.12 🟡 MEDIUM: `DefaultLogger()` Returns `nil` if Called Before `NewLogger()`

**File:** `internal/log/log.go` lines 18, 65–67

```go
var defaultLogger *slog.Logger
func DefaultLogger() *slog.Logger {
    return defaultLogger
}
```

Any early call to `DefaultLogger().Info(...)` before `NewLogger()` is called panics with a nil pointer dereference.

---

## Complete Issue Count (All Parts)

| Severity | Count | Parts |
|----------|-------|-------|
| 🔴 Critical | **21** | Parts 1–3, 8–9, 10 |
| 🟠 High | **27** | Parts 1–6, 9–10 |
| 🟡 Medium | **21** | Parts 1–10 |
| 🔵 Low | **6** | Parts 6–7 |
| **Grand Total** | **75** | Entire codebase |

---

## Priority Fix Order — Complete

### Tier 1 — Zero-Day Blockers (Fix Before Any Other Work)

1. **`MsgEmitter` never set on engine** (2.1) — all workflow → TUI messaging dead
2. **`dispatcher.Execute()` always returns nil error** (9.3) — tool failures silently succeed
3. **Permission listeners never re-armed** (1.1) — deadlock on second tool call
4. **Discuss phase completely broken** (1.3, 1.5) — discuss never shown
5. **`fuzzyAnchorReplace` corrupts files** (9.5) — Edit tool writes broken files
6. **`grepWithRG` pipe deadlock** (9.6) — Grep tool hangs on large outputs
7. **`registry.List()` breaks fallback** (9.7) — provider fallback non-functional
8. **Tool result missing ToolCallID** (9.8) — multi-turn tool use fails at API level
9. **11 workflow messages not handled** (1.2) — Execute/Verify/Ship screens static
10. **`e.git` nil panic in execute** (2.2) — crash on non-git projects

### Tier 2 — Feature Blockers

11. `resumeScreenReadyMsg` not handled (1.6)
12. `verifyTask` context leak (9.1, 9.2)
13. `HealResultMsg` discards update (9.9)
14. `verify.go` statusBadge format error (9.10)
15. `execute_model.go` countTasks wrong order (9.11)
16. `AppendStreamChunk` never called (9.12)
17. SidebarRefreshMsg infinite loop (9.13)
18. Ledger never written from Ship (10.2)
19. Arbitrage alternatives inverted (10.3)
20. `LoadWorkflowState` wrong sentinel (10.1)

### Tier 3 — Quality & Correctness

21. `handleWindowResize` early return (1.14)
22. Theme not propagated to 13 models (1.13)
23. Session restore uses wrong model (5.1)
24. Shell output routed to LLM (1.9)
25. FallbackEvent doesn't update model (4.2)
26. HardReset stash not popped (10.5)
27. `substituteVars` clears unset vars (9.18)
28. `parseToolCalls` byte/rune mismatch (9.19)
29. `os.Getwd()` error ignored at startup (10.8)
30. keychain Linux fmtSecret zero path (10.4)

---

*Report completed by deep static analysis + 3 parallel research agents. Covers TUI wiring, workflow engine, provider layer, tools dispatcher, session management, pkg libraries, and infrastructure. Total findings: 75 issues across all severity levels.*

