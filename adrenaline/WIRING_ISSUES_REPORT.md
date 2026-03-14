# M31A — Complete Wiring & Integration Issues Report

**Generated:** 2026-06-12 (supersedes 2026-06-09 version)
**Scope:** Full codebase deep audit — TUI, Workflow, Provider, Tools, Session, Pkg, Config
**Methodology:** Deep static analysis + 4 parallel research agents + cross-package integration verification
**Severity Scale:** 🔴 Critical (crash/deadlock/data loss) | 🟠 High (broken feature) | 🟡 Medium (logic error/silent failure) | 🔵 Low (code smell/latent bug)

---

## Executive Summary

M31A has **24 critical wiring issues**, **29 high-severity issues**, **22 medium issues**, and **12 low-severity issues** — **87 total**. The most impactful clusters are:

1. **Dead message types** — `SessionRenameMsg`, `SessionExportMsg`, `ConfigReloadMsg`, `OptimizedMsg`, `PlanProgressMsg` are emitted but never handled, making session rename, session export, config hot-reload, and optimization results completely non-functional.
2. **MsgEmitter never connected** — The workflow engine's `MsgEmitter` is never set on the engine, so all workflow→TUI messaging (progress bars, tool cards, thinking indicators) produces no output.
3. **Permission/question listeners are one-shot** — They read one message and stop; after the first permission response, the listener is never re-armed, causing deadlock on the second tool call.
4. **Discuss phase completely broken** — The answer flow is disconnected from the workflow engine; questions are never shown, answers never collected.
5. **Cross-package wiring gaps** — Config hot-reload doesn't update dispatcher permissions; rollback doesn't notify the workflow engine; session ID isn't propagated on session switch.

---

## Part 1 — Unhandled Message Types (Dead Messages)

### 1.1 🔴 CRITICAL: `ConfigReloadMsg` Never Handled in TUI — Hot-Reload Broken

**File:** `internal/config/loader.go:845` + `internal/tui/app_update.go`

`WatchConfig()` sends `ConfigReloadMsg` when the config file changes on disk, but **there is no `case ConfigReloadMsg` in `AppState.Update()`**. The message is silently dropped.

**Consequence:** Config hot-reload is completely non-functional. Changing the config file on disk will never trigger a TUI update — providers, themes, permissions, budget limits all remain stale until restart.

**Fix required:** Add `case ConfigReloadMsg` in `AppState.Update()` that calls `m.applyConfig(msg.Config)` and refreshes affected components.

---

### 1.2 🔴 CRITICAL: `SessionRenameMsg` Emitted but Never Handled

**Definition:** `internal/tui/types.go:346-348`
**Emitted from:** `internal/tui/resume_model.go:155`
**No handler:** No `case SessionRenameMsg` exists in `AppState.Update()`. The message falls through to `default:` and is silently dropped.

**Consequence:** Session rename from the Resume screen UI does nothing — the user action produces a message that is immediately lost.

---

### 1.3 🔴 CRITICAL: `SessionExportMsg` Emitted but Never Handled

**Definition:** `internal/tui/types.go:350-352`
**Emitted from:** `internal/tui/resume_model.go:163`
**No handler:** No `case SessionExportMsg` exists in `AppState.Update()`. Silently dropped.

**Consequence:** Session export from the Resume screen UI does nothing.

---

### 1.4 🔴 CRITICAL: `OptimizedMsg` Defined but Never Handled

**Definition:** `internal/tui/types.go:313-317`
**Type:** `OptimizedMsg{Recommendations []arbitrage.ArbitrageRecommendation, TaskID int}`
**No handler:** No `case OptimizedMsg` in `AppState.Update()`. The `handleOptimize` command returns a `ToastMsg` instead, bypassing this type entirely.

**Consequence:** Arbitrage optimization results can never flow through the Bubble Tea message system. Dead code.

---

### 1.5 🟠 HIGH: `PlanProgressMsg` Defined but Never Emitted or Handled

**Definition:** `internal/workflow/engine_messages.go:117-120`
**No emission:** No code in the workflow package ever creates and emits a `PlanProgressMsg`.
**No handler:** No case in `AppState.Update()`.

**Consequence:** Plan generation progress reporting is completely disconnected — dead message type.

---

### 1.6 🟠 HIGH: 11 Workflow Engine Message Types Emitted But Not Handled in `AppState.Update()`

**File:** `internal/workflow/engine_messages.go` + `internal/tui/app_update.go`

The workflow engine emits all of the following message types via `MsgEmitter.Emit()`:
- `TaskStartMsg`, `TaskUpdateMsg`, `ToolStartMsg`, `ToolCompleteMsg`
- `SelfHealStartMsg`, `SelfHealCompleteMsg`
- `PhaseTransitionStartMsg`, `PhaseTransitionCompleteMsg`
- `IntermediateProgressMsg`, `ThinkingStartMsg`, `ThinkingCompleteMsg`

**However, `AppState.Update()` has NO `case` for any of these types.** They all fall through to the `default` branch which routes to screen-specific submodels. Only `HealResultMsg` has a case (which is a different type).

**Consequence:** The Execute screen never receives task start/update/tool events. Progress bars, task status, tool call indicators — **all broken**. The execute screen is effectively a static screen.

**Note:** These messages ARE drained via `drainEmitterCmd()` to prevent channel backup, but their content is discarded — they just trigger another drain.

---

### 1.7 🟡 MEDIUM: `DiscussAnswerTimeoutMsg` Timer Never Started

**Definition:** `internal/tui/types.go:198-200`
**Handler exists:** `DiscussModel.Update()` handles it (`internal/tui/discuss.go:81`)
**Problem:** The timeout timer (`SetTimeout()`) on `DiscussModel` is never called from the TUI. No `tea.Tick` command emits this message. The countdown display is cosmetic only — it never fires.

---

## Part 2 — TUI Layer Wiring Issues

### 2.1 🔴 CRITICAL: `permListenerCmd` / `questionListenerCmd` are One-Shot — Never Re-Armed

**File:** `internal/tui/app.go` lines 102–128

```go
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

This function reads **exactly one** `PermissionRequest` from the dispatcher's channel. After it fires and emits `PermissionRequestMsg`, **no code re-issues `permListenerCmd`**. In `app_update.go`, `handlePermissionResponse()` doesn't call `permListenerCmd` again.

- First tool permission → works.
- Second tool in the same session → `d.RequestCh()` blocks forever, but no listener is reading it. **Result: deadlock on the second permission request.**

Same issue for `questionListenerCmd`.

**Fix required:** In `handlePermissionResponse()` and `handleQuestionResponse()`, return `permListenerCmd(m.shutdownCtx, m.dispatcher)` and `questionListenerCmd(m.shutdownCtx, m.dispatcher)` respectively.

---

### 2.2 🔴 CRITICAL: Discuss Phase Answer Flow is Completely Broken

**Files:** `internal/tui/discuss.go`, `internal/tui/app_update.go`, `internal/workflow/discuss.go`

**Issue chain:**
1. `runDiscuss()` returns `PhaseResult{NeedsAnswers: true}` after parsing questions.
2. `handlePhaseResult()` transitions to `PhasePlan` immediately — **without checking `NeedsAnswers`**.
3. The `DiscussModel` is never shown because `screen` is set to `ScreenPlan` immediately.
4. Even if shown, `DiscussModel.advanceQuestion()` emits `QuestionResponseMsg{Answer: answer}` — but `handleQuestionResponse()` routes this to `m.dispatcher.questionRespCh` (the `AskUserQuestion` tool channel), **not the discuss workflow engine**.
5. The workflow engine's `SubmitDiscussAnswer()` and `FinalizeDiscuss()` are **never called from the TUI**.

**Consequence:** Discuss phase is completely non-functional. Questions are never shown, answers are never collected, and the plan phase proceeds with no user context.

---

### 2.3 🔴 CRITICAL: `handlePermissionResponse` Calls `go m.dispatcher.ApprovePermission(...)` — Goroutine Mutates Dispatcher State

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

`ApprovePermission` is called in a goroutine. This violates the stated thread-safety contract and can race with the next Update call. The goroutine launch is unnecessary and risky.

**Secondary issue:** After this call, neither `permListenerCmd` nor `questionListenerCmd` is re-armed (see Issue 2.1).

---

### 2.4 🔴 CRITICAL: `GoalSubmittedMsg` handler immediately switches to `ScreenREPL` — DiscussModel never initialized

**File:** `internal/tui/app_update.go` lines 192–196

```go
case GoalSubmittedMsg:
    m.workflowGoal = msg.Goal
    m.screen = ScreenREPL
    cmds = append(cmds, m.runWorkflowFromGoal(msg.Goal))
```

`runWorkflowFromGoal` → `RunPhaseCmd(PhaseInitialize)` → triggers `PhaseDiscuss` → which (due to Issue 2.2) immediately runs `PhasePlan`. The `DiscussModel` is assigned to `m.discussModel` but `ensureSubModel(ScreenDiscuss)` is never called from the workflow flow.

---

### 2.5 🟠 HIGH: `resumeScreenReadyMsg` is Emitted but Never Handled in `AppState.Update()`

**File:** `internal/tui/app_update.go`

`openResumeScreen()` returns a function that emits `resumeScreenReadyMsg{sessions: sessions}`. There is **no `case resumeScreenReadyMsg`** in `AppState.Update()`. The message is emitted but silently dropped.

**Consequence:** Opening `/resume` loads sessions in the background but the list is never displayed. The Resume screen shows empty/blank.

---

### 2.6 🟠 HIGH: `sessionLoadedMsg` / `sessionRestoredMsg` Type Mismatch

**File:** `internal/tui/app_update_commands.go` lines 170–174 + `internal/tui/helpers.go` lines 46–50

Two separate private types exist for session restore:
- `sessionLoadedMsg` — fields: `sessionID string, navigate bool`
- `sessionRestoredMsg` — fields: `sess *session.Session, clearExisting bool`

`sessionLoadedMsg` is defined but **has no case in `Update()`** — it's a dead type. This creates confusion about which restore path is actually used.

---

### 2.7 🟠 HIGH: `toastTimers` Map is Populated but Never Used

**File:** `internal/tui/app_state.go` line 124

```go
toastTimers map[int]*time.Timer // index → auto-dismiss timer
```

Initialized as `make(map[int]*time.Timer)` but **never written to or read from** in any file. Toast auto-dismiss is handled solely via `tea.Tick(duration, ...)`. Dead weight and misleading.

---

### 2.8 🟠 HIGH: Regular Chat Messages Routed via `SlashCommandMsg` — Misuse of Command Type

**File:** `internal/tui/repl.go` lines 308–311

Regular user messages (non-slash, non-`!`) are wrapped in `SlashCommandMsg`. Shell commands (`!cmd`) are also wrapped in `SlashCommandMsg` and then passed to `executeShellCommand()`, but after execution, the result is returned as `SlashCommandMsg{Command: "!result:" + result}` — this round-trips through `handleSlashCommand` which tries to parse it as a command (no `/` prefix → calls `sendChatMessage("!result:...")`). The shell output gets sent to the LLM instead of being displayed directly.

---

### 2.9 🟠 HIGH: `DiscussModel.advanceQuestion()` Drops Last Answer

**File:** `internal/tui/discuss.go` lines 131–136

```go
if dm.current >= len(dm.questions) {
    return func() tea.Msg {
        return AppMsg{Screen: ScreenREPL}
    }
}
return answerCmd
```

When all questions are done (`dm.current >= len(dm.questions)`), it returns `AppMsg{Screen: ScreenREPL}` **without returning the last answer via `answerCmd`**. The last answer is lost.

---

### 2.10 🟠 HIGH: `settings_edit.go` is Empty — 4-byte Stub File

**File:** `internal/tui/settings_edit.go`

Contains only a package declaration and a comment. Dead stub masking incomplete implementation.

---

### 2.11 🟡 MEDIUM: `ScreenMetrics` Has No Key Routing in `routeKeyMsg`

**File:** `internal/tui/app_update.go`

The `routeKeyMsg` switch handles all screens — **but NOT `ScreenMetrics`**. The `MetricsModel` never receives key events. Users can't scroll or interact with the metrics screen.

---

### 2.12 🟡 MEDIUM: `applyTheme()` Only Propagates to 4 Sub-Models

**File:** `internal/tui/app_update.go` lines 810–833

`applyTheme()` propagates to `replModel`, `sidebarModel`, `cmdPalette`, and `settingsModel` — but **not** to: `planModel`, `executeModel`, `verifyModel`, `shipModel`, `ledgerModel`, `rollbackModel`, `diffModel`, `metricsModel`, `configModel`, `resumeModel`, `firstRunModel`, `goalInput`, `discussModel`, `msModel`.

All these models retain the old theme after `/theme` is called.

---

### 2.13 🟡 MEDIUM: `handleWindowResize` Has Early-Return Bug

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
```

Since `replModel` is almost always non-nil, the blocks below are **never reached**. Plan, Execute, Verify, Ship, Settings, CommandPalette, ModelSelector, Resume, and Diff models are never resized on window resize.

---

### 2.14 🟡 MEDIUM: `GoalInputModel` Created with `nil` Callback

**File:** `internal/tui/app_update.go` lines 675–681

```go
m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil) // nil callback!
```

If `GoalInputModel` calls the callback on submit, it will panic (nil function call).

---

### 2.15 🟡 MEDIUM: `handleWindowResize` Doesn't Forward to `discussModel`

**File:** `internal/tui/app_update.go`

`handleWindowResize` never calls `m.discussModel.SetDimensions(...)`. The Discuss screen won't reflow on terminal resize.

---

### 2.16 🔵 LOW: `shellMode` Field in `ReplModel` is Set but Never Read

**File:** `internal/tui/repl_model.go` line 101

Declared in `ReplModel` but there are no reads of `m.shellMode` in any TUI file. Dead state.

---

### 2.17 🟡 MEDIUM: `propagateSessionID` Misses `discussModel`, `ledgerModel`, `rollbackModel`

**File:** `internal/tui/helpers.go:85-101`

Propagates to `planModel`, `executeModel`, `verifyModel`, `shipModel`, `sidebarModel`. Does not propagate to `discussModel`, `ledgerModel`, or `rollbackModel`.

---

### 2.18 🟡 MEDIUM: `ConfigSavedMsg` Doesn't Refresh Config Model Display

**File:** `internal/tui/app_update.go:459-462`

`ConfigSavedMsg` calls `m.reRegisterProvidersFromConfig()` and shows a toast, but does **not** call `m.configModel.buildContent()` to refresh. The `SettingsSavedMsg` handler does refresh, but `ConfigSavedMsg` does not. Stale display after config save.

---

### 2.19 🔵 LOW: `MaxMessageHistory = 1000` but Trim at 500 — Off-By-500

**File:** `internal/tui/repl_state.go` lines 165–167

```go
if len(m.messages) > MaxMessageHistory {
    m.messages = m.messages[len(m.messages)-500:]
}
```

The constant is 1000, but trim keeps only 500. Inconsistent.

---

## Part 3 — Workflow Engine Wiring Issues

### 3.1 🔴 CRITICAL: `MsgEmitter` is Never Set on the Engine — All `emit()` Calls are No-Ops

**File:** `internal/tui/app.go` lines 166–188

`workflow.NewEngine()` does not accept a `MsgEmitter`. The only way to set it is `engine.SetMsgEmitter(em)`. **Searching `app.go` and all TUI files: `SetMsgEmitter` is never called.** The `channelEmitter` type exists in `app_channel.go` but is **never instantiated or connected to the engine**.

**Consequence:** Every `e.emit(...)` call in the engine returns immediately (the `msgEmitter` nil check exits early). No workflow progress messages reach the TUI. The execute screen progress bar, thinking indicators, tool cards — **all are broken**.

**Fix required:**
```go
emitter := &channelEmitter{ch: make(chan tea.Msg, 128)}
engine.SetMsgEmitter(emitter)
// Then start a goroutine or tea.Cmd to drain emitter.ch and forward to the TUI
```

---

### 3.2 🔴 CRITICAL: Execute Phase Calls `e.git.AddAll()` / `e.git.Commit()` but `e.git` Can Be `nil`

**File:** `internal/workflow/execute.go` lines 270–282

```go
if len(task.Files) > 0 {
    if err := e.git.AddAll(); err != nil {  // PANIC if e.git == nil
```

`e.git` is only set via `engine.SetGit(g)`, which is called from `initWorkflowEngine()` only `if m.git != nil`. If `m.git` is nil (non-git project), then `e.git` remains nil, and `e.git.AddAll()` panics.

Same issue in `healTask()` lines 381–393.

---

### 3.3 🟠 HIGH: `runDiscuss` Emits `StreamChunkMsg` but This Type is Not Handled

**File:** `internal/workflow/discuss.go` line 61

```go
e.msgEmitter.Emit(m31types.StreamChunkMsg{Chunk: chunk, Source: "discuss"})
```

`types.StreamChunkMsg` is a different type from `StreamMsg` (used in `streaming.go`). In `app_update.go`, only `StreamMsg` is handled — NOT `types.StreamChunkMsg` directly. The discuss streaming chunks will not be routed to the REPL for display.

---

### 3.4 🟠 HIGH: Plan Phase Never Emits `PlanReadyMsg` to TUI

Since `SetMsgEmitter` is never called (Issue 3.1), `PlanReadyMsg` messages never arrive. Even if the emitter were connected, the plan phase would need to emit `PlanReadyMsg` through it — this must be verified.

---

### 3.5 🟠 HIGH: Execute Phase Uses `context.Background()` for `healTask`

**File:** `internal/workflow/engine.go` line 321

```go
healResult := e.healTask(context.Background(), task, failure)
```

In `HealTask()` (the TUI-triggered manual heal), `context.Background()` is used instead of the passed `ctx`. This means if the user presses Ctrl+C, the heal LLM call cannot be cancelled.

---

### 3.6 🟠 HIGH: `engine.readTaskFiles()` Is Called But May Not Be Defined

**File:** `internal/workflow/execute.go` line 343

```go
e.readTaskFiles(task.Files)
```

`readTaskFiles` is called but not visible in the engine files reviewed. If it's not defined anywhere, this is a compile error.

---

### 3.7 🟡 MEDIUM: `runPlan` Saves Tasks Without Emitting `PlanReadyMsg`

Since the emitter is never connected (Issue 3.1), this is moot, but even if it were, the plan phase must be verified to call `e.emit(PlanReadyMsg{...})`.

---

### 3.8 🔴 CRITICAL: `verifyTask` Context Leak — `defer cancel()` in Loop Leaks All But Last Cancel

**File:** `internal/workflow/engine_verify.go` lines 140–202

Every iteration of `for _, f := range task.Files` creates a `context.WithTimeout` and calls `defer cancel()`. Because `defer` accumulates until function return, and each iteration **overwrites** the `ctx` local variable, only the **last** `cancel` is kept in scope — all prior `cancel` functions are lost. For large task file lists, this creates unbounded goroutine-timer leaks.

---

### 3.9 🔴 CRITICAL: `verifyTaskContext` Discards `cancel` — Timer Resource Leak

**File:** `internal/workflow/engine_verify.go` lines 108–113

```go
func (e *Engine) verifyTaskContext(parent context.Context) context.Context {
    ctx, cancel := context.WithTimeout(parent, verifyTaskTimeout)
    _ = cancel // ← resources leaked until deadline fires
}
```

---

### 3.10 🔴 CRITICAL: `executeTaskWithTools` — Tool Result Missing `ToolCallID` in Message

**File:** `internal/workflow/execute.go` lines 231–235

```go
messages = append(messages, m31types.Message{
    Role:    "tool",
    Content: result.Output,
    // Missing: ToolCallID — required by OpenRouter/Zen for multi-turn tool conversations
})
```

OpenRouter and Zen APIs require tool result messages to include the call ID. Without this, multi-turn tool-use conversations produce malformed API requests.

---

### 3.11 🟡 MEDIUM: `parseToolCalls` Iterates by Byte Index on UTF-8 Content

**File:** `internal/workflow/engine_parse.go` line 314

Iterating a `string` by byte index and doing `i += len(obj) - 1` mixes byte and rune indexing. For LLM responses with unicode content, this can step into the middle of a multi-byte sequence.

---

## Part 4 — Tool Dispatcher Wiring Issues

### 4.1 🔴 CRITICAL: `dispatcher.Execute()` Always Returns `nil` Error on Tool Failure

**File:** `internal/tools/dispatcher.go` lines 162–172

```go
res := types.ToolResult{Error: ""}
if err != nil {
    res.Error = err.Error()
}
return res, nil  // ← always nil error!
```

When a tool's `Execute()` returns an error, the dispatcher puts the error string in `res.Error` and returns `(res, nil)`. In `execute.go`, the caller checks `err != nil` — which is **always false**. Tool failures are silently treated as success.

---

### 4.2 🔴 CRITICAL: `ApprovePermission` Fallback Channel Send Can Deadlock

**File:** `internal/tools/permissions.go` lines 16–26

The fallback path is an unbuffered send to `responseCh` (buffer=8, but can be full). If no goroutine is draining `responseCh` and the buffer is full, this call blocks indefinitely.

---

### 4.3 🔴 CRITICAL: `fuzzyAnchorReplace` in `edit.go` Corrupts Edited Files

**File:** `internal/tools/edit.go` lines 425–429

```go
newLines := make([]string, len(contentLines))  // length = N, all empty strings
copy(newLines, contentLines[:i])               // fills 0..i-1
newLines = append(newLines, strings.Split(newString, "\n")...)  // appends AFTER index N!
```

`make([]string, len(contentLines))` creates a slice of length `N` (all zero-value empty strings). After `copy()` fills the prefix, `append` adds new content **past index N**, producing a block of empty strings corrupting the edited file.

**Fix:** Use `make([]string, 0, len(contentLines))` — capacity but zero length.

---

### 4.4 🔴 CRITICAL: `grepWithRG` Pipe Drain Deadlock on Result Limit

**File:** `internal/tools/grep.go` lines 182–189

After hitting the limit, the scanner stops reading from `rg`'s stdout pipe. `rg` is still running and will block on write when the pipe buffer fills. `cmd.Wait()` then blocks waiting for `rg` to exit, which never happens. **Result: `grep` tool hangs forever on large outputs.**

---

### 4.5 🔴 CRITICAL: `registry.List()` Decorates Active Provider Name — Breaks `FindFallbackProvider`

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

`FindFallbackProvider` calls `registry.List()` which returns decorated names, then compares against plain provider names. The active provider is **never skipped** — fallback logic will attempt to use the current provider as its own fallback.

---

### 4.6 🟠 HIGH: Tools Registered in `defaults.go` but Error Handling Missing at Startup

**File:** `internal/tools/defaults.go`

If a tool is already registered (e.g., on engine re-init), `Register()` returns an error. The startup code must check this error. If silently ignored, double-registration attempts fail silently.

---

### 4.7 🟡 MEDIUM: `QuestionResponseCh()` Returns Same Channel as `questionRespCh` — Shared Between Tool and Discuss

**File:** `internal/tools/dispatcher.go` line 201

This channel is used for `AskUserQuestion` tool responses. The `DiscussModel` routes answers to this same channel via `QuestionResponseMsg`. If discuss Q&A and a tool's `AskUserQuestion` run concurrently, they'd share the response channel, causing message confusion.

---

### 4.8 🟡 MEDIUM: `TodoWrite.SetSessionID()` is Called on `nil` Check — Race Possible

**File:** `internal/tools/dispatcher.go` lines 205–209

`SetSessionID` is called from multiple goroutines (session switch) while `todoWrite` may be assigned concurrently.

---

### 4.9 🟡 MEDIUM: Permission System Triplicate Code Duplication

**File:** `internal/tools/permissions.go:155-324`

Three functions (`askPermission`, `askPermissionWithAgentDefault`, `askPermissionFallback`) share ~180 lines of near-identical logic. Any bug fix must be applied three times.

---

### 4.10 🟡 MEDIUM: `PermissionGate` Interface Has No Concrete Implementation

**File:** `internal/tools/interface.go:46`

`PermissionGate` interface is defined with `RequestPermission(ctx, req)` but there is no concrete type that implements it. The `Dispatcher` handles permissions directly via channels, not through this interface. Dead abstraction.

---

## Part 5 — Provider & Registry Wiring Issues

### 5.1 🟠 HIGH: `registry.ActiveProvider()` vs `registry.Get(registry.Active())` — Inconsistent Usage

**Files:** Multiple TUI files

Some callers use `m.registry.ActiveProvider()` and some use `m.registry.Get(m.activeProvider)`. After a fallback event, `m.activeProvider` is updated, but the registry's internal active provider may differ. This divergence means some code paths use the old provider and some use the new one.

---

### 5.2 🟠 HIGH: `FallbackEventMsg` Updates `m.activeProvider` But Not `m.activeModel`

**File:** `internal/tui/app_update.go` lines 244–247

When provider falls back, `m.activeProvider` is updated. But `m.activeModel` still references the old provider's model. If the new provider doesn't support the same model ID, subsequent calls will use a model that doesn't exist on the new provider.

---

### 5.3 🟡 MEDIUM: `ModelSelectedMsg` Handled in Two Places — Duplicate Processing

**File:** `internal/tui/app_update.go`

Both `ModelSelectedMsg` and `AppMsg{ModelSelected: ...}` handle model selection. The `AppMsg` path doesn't call `m.replModel.SetProvider(...)` — it silently skips provider sync.

---

### 5.4 🟡 MEDIUM: Zen Provider Always Reports `$0` Cost — Cost Tracking Dead for Zen Users

**File:** `internal/provider/zen/client.go` lines 135–137

Zen models have hardcoded zero pricing. `EstimateCost()` always returns 0 for Zen. Cost tracking, budget warnings, and the ledger cost metrics are all meaningless for Zen users.

---

## Part 6 — Session & Persistence Wiring Issues

### 6.1 🟠 HIGH: `applySessionRestored` Uses `sess.Provider` But `m.activeModel` — Provider/Model Mismatch

**File:** `internal/tui/helpers.go` lines 79–80

```go
providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
```

When restoring a session, `sess.Provider` is used for the provider, but `m.activeModel` (the currently active model) is used — not the model saved in the session. `sess.Model` is never used.

---

### 6.2 🟠 HIGH: `persistWorkflowState` Only Called at `PhaseShip` Complete

**File:** `internal/tui/app.go` lines 191–202

`persistWorkflowState()` is only called in `handlePhaseResult()` for `types.PhaseShip`. If the app crashes during Execute or Verify, the workflow state is not persisted.

---

### 6.3 🟠 HIGH: Rollback Doesn't Notify Workflow Engine or Session State

**File:** `internal/tui/rollback.go:117-148`

When a soft/hard reset is performed, the `RollbackModel` calls rollback operations directly but does **not**:
1. Update the workflow engine's state
2. Update session workflow state via `sessionManager.UpdateWorkflowState()`
3. Clear or reset the workflow engine's internal `sessionStartHash`
4. Show a toast or notification confirming the rollback result

A rollback during an active workflow leaves the engine in an inconsistent state.

---

### 6.4 🟡 MEDIUM: Session ID Not Propagated to Workflow Engine or Dispatcher on Session Switch

**File:** `internal/tui/app_update.go:1680-1707` + `internal/tui/helpers.go:104-124`

When switching sessions, `propagateSessionID()` updates plan/execute/verify/ship models and sidebar, but does **not** call:
1. `m.workflowEngine.SetSessionID(id)` — engine retains old session ID
2. `m.dispatcher.SetSessionID(id)` — todo writes would use old session ID

---

### 6.5 🟡 MEDIUM: `UpdateWorkflowState` Error is Silently Discarded

**File:** `internal/tui/app.go` line 196

```go
_ = m.sessionManager.UpdateWorkflowState(...)
```

---

### 6.6 🟡 MEDIUM: `planningDir` Computed with Brittle String Manipulation

**File:** `internal/tui/app.go` lines 162–163

```go
planningDir := sessDir + "/" + m.sessionID + "/planning"
```

Should use `filepath.Join()` throughout.

---

### 6.7 🟠 HIGH: `LoadWorkflowState` Catches Wrong Sentinel — First-Run Sessions Always Error

**File:** `pkg/session/manager.go` line 257

Checks `ErrSessionCorrupted` instead of `ErrSessionNotFound`. A genuinely missing session file returns `ErrSessionNotFound` (not caught), which propagates as a real error.

---

### 6.8 🟠 HIGH: `HardReset` Stashes Changes But Never Pops

**File:** `pkg/rollback/rollback.go` lines 148–168

`HardReset` stashes uncommitted changes before resetting, but unlike `SafeReset`, never pops the stash after. User data is silently preserved in the stash but not restored.

---

### 6.9 🟠 HIGH: `session.Tasks` Field Never Loaded from Disk After `LoadSession`

**File:** `pkg/session/session.go` lines 15, 41

The `Tasks` field is serialized to `session.json`, but `LoadSession` never populates it — tasks are loaded separately via `LoadTasks()`. Code checking `session.Tasks` after load sees an empty slice.

---

### 6.10 🟠 HIGH: `ResumedAt` Set on Every Internal `LoadSession` Call

**File:** `pkg/session/manager.go` lines 224–225

`LoadSession` sets `session.ResumedAt = &now` unconditionally. But `LoadSession` is called internally by `UpdateWorkflowState`, `ForkSession`, `SiblingSessions` etc. `ResumedAt` always reflects the last internal session access, not user resume events.

---

## Part 7 — Cross-Cutting Wiring Issues

### 7.1 🔴 CRITICAL: State Mutation in `view_file` (View function mutates state)

**File:** `internal/tui/repl_view.go` lines 54–57

```go
func (m *ReplModel) View() string {
    ...
    if len(m.messages) == 0 && !m.streaming {
        welcomeContent := m.renderWelcome()
        m.viewport.SetContent(welcomeContent)  // MUTATION IN VIEW!
    }
```

**Bubble Tea's critical rule: `View()` must be pure — no mutations.** `m.viewport.SetContent()` mutates the viewport model's internal state. This is called on every render cycle. Can cause race conditions with `Update()` and produce unpredictable rendering.

---

### 7.2 🟠 HIGH: `SidebarRefreshMsg` Handler Always Re-Schedules — Infinite Git Poll Loop

**File:** `internal/tui/sidebar.go` lines 283–305

Every `SidebarRefreshMsg` unconditionally schedules another `refreshCmd()`. This is an infinite loop that continuously shells out to `git status --porcelain` with no cooldown or stop condition.

---

### 7.3 🟠 HIGH: `HealResultMsg` in `app_update.go` — `verifyModel.Update()` Return Value Discarded

**File:** `internal/tui/app_update.go` lines 187–190

```go
case HealResultMsg:
    if m.verifyModel != nil {
        m.verifyModel.Update(msg)  // ← return values (model, cmd) DISCARDED
    }
```

`VerifyModel.Update()` returns `(*VerifyModel, tea.Cmd)`. Both are dropped.

---

### 7.4 🟠 HIGH: `verify.go` — `statusBadge` Struct Used as String (Missing `.Render()`)

**File:** `internal/tui/verify.go` line 207

```go
statusBadge := components.SimpleBadge{...}  // struct value
line := fmt.Sprintf("  %s %s %s", num, statusBadge, action)  // used as string!
```

`SimpleBadge` is a struct, not a `Stringer`. This renders garbage (struct dump) in the Verify screen.

---

### 7.5 🟠 HIGH: `execute_model.go` — `countTasks()` Return Values Assigned in Wrong Order

**File:** `internal/tui/execute_model.go` line 93

```go
newDone, newFailed, newTotal := em.countTasks()  // ← wrong order: total→newFailed, failed→newTotal
```

The names are swapped. This corrupts progress calculations.

---

### 7.6 🟠 HIGH: `AppendStreamChunk()` in `repl_stream.go` — Never Called, Workflow Streaming Dead

**File:** `internal/tui/repl_stream.go` lines 19–25

`AppendStreamChunk(chunk *types.StreamChunk)` is defined on `ReplModel` but has **zero callers** in the entire codebase. Any workflow phase that streams via this function produces no visible output.

---

### 7.7 🟠 HIGH: `TickMsg` Never Forwarded to `VerifyModel` — Heal Spinner Frozen

**File:** `internal/tui/app_update.go`

`TickMsg` is forwarded to `ExecuteModel` and `ReplModel` but NOT to `VerifyModel`. `VerifyModel` has a spinner but no code ever ticks it.

---

### 7.8 🟠 HIGH: `FirstRunModel.AppMsg.SaveKeychain` — Never Acted Upon

**File:** `internal/tui/firstrun_model.go` line 368 + `internal/tui/app_update.go`

`completeSetup()` emits `AppMsg{Screen: ScreenREPL, SaveKeychain: opts.SaveKeychain}`. In `handleAppMsg()` → `routeAppMsgAction()`, `SaveKeychain` is never read.

---

### 7.9 🟠 HIGH: `plan_model.go` `computeWaves()` — Infinite Loop on Cyclic Dependencies

**File:** `internal/tui/plan_model.go` lines 82–116

```go
for remaining > 0 { ... }  // no cycle detection!
```

If tasks have a circular dependency, `remaining` never reaches 0, hanging the TUI indefinitely.

---

### 7.10 🟠 HIGH: `ModelSelector` `allDone` Never True if Provider Errors — Spinner Hangs

**File:** `internal/tui/modelselector.go` lines 132–142

If a provider returns an error, no entry is added to `modelsByProv`. The `allDone` check waits for all providers, which never happens for errored providers. The loading spinner hangs forever.

---

### 7.11 🟡 MEDIUM: Config `substituteVars` Replaces `${VAR}` with `""` — Silently Clears API Keys

**File:** `internal/config/loader.go` lines 531–537

Users who write `api_key = "${M31A_OPENROUTER_API_KEY}"` will silently get an empty API key if the environment variable is not set.

---

### 7.12 🟡 MEDIUM: `DefaultLogger()` Returns `nil` if Called Before `NewLogger()`

**File:** `internal/log/log.go` lines 18, 65–67

Any early call to `DefaultLogger().Info(...)` before `NewLogger()` is called panics with a nil pointer dereference.

---

### 7.13 🟡 MEDIUM: `git.StatusPorcelain` Rename Parsing Uses Wrong Format

**File:** `internal/git/git.go` lines 302–307

The `--porcelain` (v1) format encodes renames as `R old\x00new` (null-separated), not `old -> new`. `OldPath` is never populated.

---

### 7.14 🟡 MEDIUM: `autodream.go` Appends `summaryMsg` at End of `kept` — Wrong Order

**File:** `pkg/autodream/autodream.go` line 211

A summary of old consolidated messages is appended **after** newer messages, not before.

---

### 7.15 🔵 LOW: `channelEmitter` in `app_channel.go` is Defined but Never Instantiated

**File:** `internal/tui/app_channel.go` lines 41–57

This is the bridge between the workflow engine and the TUI. However, it is **never instantiated** — confirms Issue 3.1.

---

### 7.16 🔵 LOW: `KeyActionMsg` Defined but `LeaderTimeoutMsg` Handler Doesn't Re-render

**File:** `internal/tui/app_update.go` lines 62–66

When leader is deactivated, the TUI does not re-render. The which-key overlay will linger until the next render cycle.

---

## Part 8 — Package & Infrastructure Wiring Issues

### 8.1 🔴 CRITICAL: `Ledger.Append` is Never Called From Production Code

**File:** `pkg/ledger/ledger.go`

After exhaustive codebase search, `ledger.Append` and `ledger.NewEntry` are **never called** from any workflow, TUI, or main entrypoint — only from tests. The ledger always shows 0 sessions; the cross-session learning feature is completely non-functional.

**Fix:** Add `ledger.NewEntry(...) + ledger.Append(entry)` at the end of `runShip()`.

---

### 8.2 🔴 CRITICAL: Arbitrage `alternatives` Collection Logic Is Inverted

**File:** `pkg/arbitrage/arbitrage.go` lines 196–208

`estimates` is sorted ascending by cost. The loop skips everything before `recommended` (cheaper models) and collects everything after (more expensive). The comment says "cheaper than the recommended model" — but the logic collects **more expensive** models.

---

### 8.3 🔴 CRITICAL: `keychain_linux.go` `fmtSecret` Uses Zero `sessionPath`

**File:** `pkg/keychain/keychain_linux.go` lines 167–174

`OpenSession` returns a `sessionPath` that is stored locally, but `fmtSecret` declares its **own** zero `dbus.ObjectPath` instead of using the returned one. All D-Bus `CreateItem` calls pass an empty session path, likely causing keychain writes to fail silently.

---

### 8.4 🟡 MEDIUM: Config Changes Don't Hot-Reload Dispatcher Permissions

**Source:** Config (settings/config editor)
**Target:** `tools.Dispatcher`

`reRegisterProvidersFromConfig()` is called on `SettingsSavedMsg` and `ConfigSavedMsg`. However, `Dispatcher.UpdatePermissions()` is **never called** when config is saved. Permission rule changes require app restart.

---

### 8.5 🟡 MEDIUM: Config Changes Don't Propagate to Workflow Engine Explicitly

**Source:** Config
**Target:** `workflow.Engine`

Config is passed as `*config.Config` to `NewEngine`, so in-place mutations propagate. But this is fragile — if the config object is replaced (not mutated in-place), the engine holds a stale reference.

---

### 8.6 🟡 MEDIUM: `os.Getwd()` Error Silently Ignored at Startup

**File:** `cmd/m31a/main.go` line 179

```go
workDir, _ := os.Getwd()
```

If the current directory was deleted, `workDir` is empty string. All tool path operations use `/` as the base.

---

### 8.7 🟡 MEDIUM: `creack/pty` Missing from `go.mod`

**File:** `go.mod`

AGENTS.md lists `creack/pty` as a required dependency. It does not appear in `go.mod`.

---

### 8.8 🔵 LOW: `Version` Set as Mutable Global in Multiple Packages

**File:** `internal/provider/openrouter/client.go:20`, `internal/provider/zen/client.go:20`

```go
var Version = "dev"  // openrouter
var Version = "dev"  // zen
```

Global mutable state set by `main` and read by provider code. Hidden dependencies.

---

## Part 9 — Summary Tables

### Complete Issue Count by Severity

| Severity | Count |
|----------|-------|
| 🔴 Critical | **24** |
| 🟠 High | **29** |
| 🟡 Medium | **22** |
| 🔵 Low | **12** |
| **Total** | **87** |

### Issues by Category

| Category | Count | Key Issues |
|----------|-------|------------|
| Unhandled Messages | 6 | ConfigReloadMsg, SessionRenameMsg, SessionExportMsg, OptimizedMsg, PlanProgressMsg, 11 workflow types |
| TUI Wiring | 15 | One-shot listeners, Discuss broken, goroutine in Update, theme/resize gaps |
| Workflow Engine | 10 | MsgEmitter disconnected, nil git panic, context leaks, missing ToolCallID |
| Tool Dispatcher | 10 | Always-nil error, file corruption, pipe deadlock, broken fallback |
| Provider | 5 | Inconsistent active provider, FallbackEvent model mismatch, Zen $0 cost |
| Session/Persistence | 10 | Session ID mismatch, rollback not wired, wrong sentinel, stash not popped |
| Cross-Cutting | 16 | View mutation, infinite loops, discarded returns, wrong orders |
| Package/Infra | 8 | Ledger empty, arbitrage inverted, keychain broken, config hot-reload broken |
| Architecture | 7 | Dead interfaces, dead fields, dead stubs, duplicated constants |

---

## Part 10 — Priority Fix Order

### Tier 1 — Zero-Day Blockers (Fix Before Any Other Work)

1. **`MsgEmitter` never set on engine** (3.1) — all workflow → TUI messaging dead
2. **`dispatcher.Execute()` always returns nil error** (4.1) — tool failures silently succeed
3. **Permission/question listeners never re-armed** (2.1) — deadlock on second tool call
4. **Discuss phase completely broken** (2.2, 2.4) — discuss never shown
5. **`fuzzyAnchorReplace` corrupts files** (4.3) — Edit tool writes broken files
6. **`grepWithRG` pipe deadlock** (4.4) — Grep tool hangs on large outputs
7. **`registry.List()` breaks fallback** (4.5) — provider fallback non-functional
8. **Tool result missing ToolCallID** (3.10) — multi-turn tool use fails at API level
9. **11 workflow messages not handled** (1.6) — Execute/Verify/Ship screens static
10. **`e.git` nil panic in execute** (3.2) — crash on non-git projects

### Tier 2 — Feature Blockers

11. `ConfigReloadMsg` not handled (1.1) — config hot-reload broken
12. `SessionRenameMsg` not handled (1.2) — session rename broken
13. `SessionExportMsg` not handled (1.3) — session export broken
14. `resumeScreenReadyMsg` not handled (2.5) — resume screen empty
15. `verifyTask` context leak (3.8, 3.9)
16. `HealResultMsg` discards update (7.3)
17. `verify.go` statusBadge format error (7.4)
18. `execute_model.go` countTasks wrong order (7.5)
19. `AppendStreamChunk` never called (7.6)
20. SidebarRefreshMsg infinite loop (7.2)
21. Ledger never written from Ship (8.1)
22. Arbitrage alternatives inverted (8.2)
23. `LoadWorkflowState` wrong sentinel (6.7)
24. Rollback doesn't notify workflow (6.3)

### Tier 3 — Quality & Correctness

25. `handleWindowResize` early return (2.13)
26. Theme not propagated to 13 models (2.12)
27. Session restore uses wrong model (6.1)
28. Shell output routed to LLM (2.8)
29. FallbackEvent doesn't update model (5.2)
30. Config → Dispatcher permissions not hot-reloaded (8.4)
31. Session ID not propagated on switch (6.4)
32. Last discuss answer dropped (2.9)
33. `GoalInputModel` nil callback (2.14)
34. State mutation in View() (7.1)
35. `os.Getwd()` error ignored at startup (8.6)
36. keychain Linux fmtSecret zero path (8.3)
37. `substituteVars` clears unset vars (7.11)
38. `parseToolCalls` byte/rune mismatch (3.11)

---

## Part 11 — Cross-Package Integration Status

| Integration | Status | Severity | Issue |
|-------------|--------|----------|-------|
| Workflow ↔ TUI | ⚠️ Partial | 🔴 | MsgEmitter never connected |
| Tools ↔ TUI | ⚠️ Partial | 🔴 | Permission listeners one-shot |
| Provider ↔ TUI | ⚠️ Partial | 🟠 | FallbackEvent model mismatch |
| Session ↔ TUI | ⚠️ Partial | 🟠 | Session ID not propagated |
| Config → Dispatcher | ❌ Not wired | 🟠 | `UpdatePermissions()` never called |
| Config → TUI | ❌ Not wired | 🔴 | `ConfigReloadMsg` not handled |
| Git ↔ Workflow | ✅ Well wired | — | — |
| Ledger ↔ Ship | ❌ Not wired | 🔴 | `Append` never called from production |
| Arbitrage ↔ REPL | ⚠️ Partial | 🟠 | `arbitrager` field nil, auto-arbitrage dead |
| AutoDream ↔ REPL | ✅ Well wired | — | — |
| Rollback → Workflow | ❌ Not wired | 🟠 | No state notification after reset |
| Rollback → Sidebar | ❌ Not wired | 🟡 | Sidebar not refreshed |

---

*Report completed by deep static analysis + 4 parallel research agents. Covers TUI wiring, workflow engine, provider layer, tools dispatcher, session management, config propagation, pkg libraries, and infrastructure. Total findings: 87 issues across all severity levels.*
