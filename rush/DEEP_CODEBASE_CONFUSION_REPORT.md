# Deep Codebase Confusion & Bug Report

> **Date:** 2026-06-08  
> **Author:** Deep audit of M31A codebase  
> **Status:** Build passes (`go build ./...`), all tests pass (`go test ./...`)  
> **Method:** Line-by-line analysis of every critical file — not surface-level scan

---

## Executive Summary

The codebase compiles and tests pass, but there are **critical logical bugs** that will cause incorrect runtime behavior. These are not compilation errors — they are semantic/wiring bugs that only manifest during actual execution. I have categorized them by severity.

---

## CRITICAL BUGS (Will cause incorrect behavior at runtime)

### BUG-1: User Messages Doubled in LLM Context

**Severity:** CRITICAL  
**Files:** `internal/tui/repl.go:205-256`, `internal/tui/app_update_commands.go:99-162`

**Problem:** When the user types a regular (non-slash) message, `handleEnterKey()` adds it to `m.messages` at line 247:
```go
newMsg := makeAssistantMsg(input)
newMsg.Role = "user"
m.messages = append(m.messages, newMsg)
```

Then it returns `SlashCommandMsg{Command: input}` which routes through `handleSlashCommand` → `sendChatMessage`. Inside `sendChatMessage` (line 131-142):
```go
var msgs []types.Message
if m.replModel != nil {
    for _, msg := range m.replModel.Messages() {
        if !msg.SkipForLLM {
            msgs = append(msgs, msg)
        }
    }
}
msgs = append(msgs, types.Message{Role: "user", Content: input})
```

**Result:** The user message appears **twice** in the LLM context — once from `m.messages` (already added) and once appended explicitly. The LLM will see the same user turn duplicated, causing confused responses and wasted tokens.

**Confusion:** Why was this designed this way? Was it intentional to have the REPL display the message and separately build the LLM context? If so, `sendChatMessage` should NOT append a new user message — it should use the last message from `m.messages`.

---

### BUG-2: Discuss Phase Answers Never Collected

**Severity:** CRITICAL  
**Files:** `internal/tui/app_update_phase.go:39-54`, `internal/workflow/discuss.go`

**Problem:** When the discuss phase completes, `handlePhaseResult` transitions directly to `PhasePlan` at line 40-54:
```go
case types.PhaseDiscuss:
    m.setWorkflowPhase(types.PhasePlan)
    m.screen = ScreenPlan
    ...
    return m.RunPhaseCmd(types.PhasePlan)
```

The `PhaseResult` from discuss has `NeedsAnswers: len(questions) > 0`, but this flag is **never checked**. The `DiscussModel` exists in `internal/tui/discuss.go` with full Q&A UI, but it is **never activated** during the workflow flow.

**Result:** The discuss phase generates clarifying questions but they are silently discarded. The plan phase runs without any user answers, making the "discuss" phase entirely decorative.

**Confusion:** The `DiscussModel` has a complete implementation (questions, answers, timeout, keyboard handling). Where is it supposed to be wired in? The `handlePhaseResult` for `PhaseDiscuss` should:
1. Check `msg.NeedsAnswers`
2. Create and show `DiscussModel` with the questions
3. Wait for answers before transitioning to `PhasePlan`

---

### BUG-3: Workflow Engine Has No Message Emitter

**Severity:** CRITICAL  
**Files:** `internal/tui/app.go:55-94`, `internal/tui/app.go:130-185`

**Problem:** `RunPhaseCmd` runs the workflow phase in a goroutine but **never calls `engine.SetMsgEmitter()`**. The `initWorkflowEngine()` function creates the engine and sets git, but the `channelEmitter` (defined in `app_channel.go`) is never wired up.

The engine's `emit()` method (line 349-353) sends messages through `e.msgEmitter`, which is `nil`:
```go
func (e *Engine) emit(msg tea.Msg) {
    if e.msgEmitter != nil {
        e.msgEmitter.Emit(msg)
    }
}
```

**Result:** All workflow progress messages are silently dropped:
- `TaskStartMsg`, `TaskUpdateMsg` — task execution progress
- `ToolStartMsg`, `ToolCompleteMsg` — tool call status
- `SelfHealStartMsg`, `SelfHealCompleteMsg` — healing attempts
- `PhaseTransitionStartMsg`, `PhaseTransitionCompleteMsg` — phase transitions
- `ThinkingStartMsg`, `ThinkingCompleteMsg` — LLM processing status
- `IntermediateProgressMsg` — long operation progress

The TUI screens (Execute, Verify, Ship) will show stale/empty data because they never receive these messages.

**Confusion:** `app_channel.go` defines `channelEmitter` specifically for this purpose, but it's never instantiated or connected. Was this wiring lost during refactoring?

---

### BUG-4: Discuss Phase Streaming Chunks Silently Dropped

**Severity:** HIGH  
**Files:** `internal/workflow/discuss.go:60-65`, `internal/tui/app_update.go`

**Problem:** The discuss phase emits `StreamChunkMsg` events via `e.msgEmitter.Emit(m31types.StreamChunkMsg{...})` at line 61-65. But even if the emitter were wired (BUG-3), the TUI's `Update()` function has no handler for `StreamChunkMsg` (or its alias `types.StreamChunkMsg`).

The only stream handlers in `app_update.go` are for `StreamMsg`, `StreamDoneMsg`, and `StreamErrorMsg` — these are the REPL's direct streaming types, not the workflow engine's `StreamChunkMsg`.

**Result:** Even with a working emitter, discuss phase progressive rendering would not work. The chunks would arrive at `Update()` and fall through to the default case.

**Confusion:** The discuss phase uses a different streaming mechanism (`StreamChunkMsg`) than the REPL's direct chat (`StreamMsg`). Were these supposed to be unified? The `StreamChunkMsg` type exists in `internal/types/types.go` but has no corresponding handler in the TUI.

---

### BUG-5: Question Modal is a No-Op

**Severity:** HIGH  
**Files:** `internal/tui/app_update.go:781-787`

**Problem:** `handleQuestionKey` immediately dismisses with an empty answer on ANY keypress:
```go
func (m *AppState) handleQuestionKey(msg tea.KeyMsg) tea.Cmd {
    if m.questionRequest == nil {
        m.screen = ScreenREPL
        return nil
    }
    return m.handleQuestionResponse(QuestionResponseMsg{Answer: ""})
}
```

There is no text input handling, no option selection, no way for the user to actually type an answer. Every keypress (including typing) sends an empty answer.

**Result:** The `AskUserQuestion` tool is effectively unusable. Any question asked during workflow execution will be answered with an empty string.

**Confusion:** The `renderQuestionModal` in `app_view.go:280-311` renders a full modal with text and instructions ("Type your answer and press ↵"), but the key handler doesn't allow typing. Was there supposed to be a `textinput.Model` here like in the DiscussModel?

---

### BUG-6: Permission "A" Key (Approve & Remember) Not Implemented

**Severity:** MEDIUM  
**Files:** `internal/tui/app_update.go:753-778`

**Problem:** The spec says: `Y`/`Enter` = approve, `N`/`Esc` = deny, `A` = approve and remember. But `handlePermissionKey` only handles `y`/`enter` and `n`/`esc`:
```go
case "y", "enter":
    return m.handlePermissionResponse(...)
case "n", "esc":
    return m.handlePermissionResponse(...)
```

There is no `case "a":` handler. The "approve and remember" feature is unimplemented.

**Result:** Users cannot permanently approve a tool for future use. Every dangerous tool invocation requires explicit approval every time.

---

## HIGH-SEVERITY ISSUES

### ISSUE-7: `RenderPermissionModal` Uses 0×0 Dimensions

**Severity:** HIGH  
**File:** `internal/tui/app_view.go:379`

**Problem:** Line 379 passes `lipgloss.Place(0, 0, ...)` instead of `lipgloss.Place(m.width, m.height, ...)`:
```go
return lipgloss.Place(0, 0, lipgloss.Center, lipgloss.Center, ...)
```

The other callers (like `renderQuestionModal` at line 303) correctly use `m.width, m.height`.

**Result:** The permission modal will not be properly centered in the terminal. It may render at default position or be invisible depending on lipgloss behavior with zero dimensions.

---

### ISSUE-8: `ShipSummary` Type Duplicated Across Packages

**Severity:** MEDIUM  
**Files:** `internal/workflow/ship.go:20-28`, `internal/tui/ship_model.go:14-31`

**Problem:** Two `ShipSummary` types exist with different fields:
- `workflow.ShipSummary`: `TaskDone`, `TaskTotal`, `TaskFailed`, `TaskSkipped`, `Commits`, `Duration`, `SessionID`
- `tui.ShipSummary`: adds `Model`, `Provider`, `TotalTokens`, `TotalCost`, `FilesAdded`, `FilesModified`, `FilesDeleted`, `Insertions`, `Deletions`, `Duration` (string)

In `app_update_phase.go:84-95`, the code creates a TUI `ShipSummary` but the workflow engine returns `workflow.ShipSummary`. The fields don't align — the TUI version expects `Model`, `Provider`, `TotalTokens`, `TotalCost` which aren't in the workflow version.

**Result:** The ship screen will show empty model/provider/cost information because the data isn't transferred from the workflow result to the TUI summary.

---

### ISSUE-9: `WorkflowPhase` Passed as Progress String

**Severity:** LOW  
**File:** `internal/workflow/engine.go:248`

**Problem:** `SaveState` is called with `string(to)` as the progress/detail argument:
```go
e.sessionMgr.SaveState(e.sessionID, to, "transitioning", string(to))
```

This means STATE.md will contain entries like `progress: "plan"` instead of something meaningful like `"5 tasks generated"`.

**Result:** STATE.md files contain phase names as progress text, making them less useful for debugging.

---

### ISSUE-10: Dead Reference in `RenderPermissionModal`

**Severity:** LOW  
**File:** `internal/tui/app_view.go:378`

**Problem:** Line 378 has `_ = strings.Repeat` which is an unused reference that does nothing:
```go
_ = strings.Repeat
```

This is dead code left over from when the function used `strings.Repeat` to build the modal.

---

## ARCHITECTURAL CONFUSIONS

### CONFUSION-1: Why Does `handleEnterKey` Route Through `SlashCommandMsg`?

The REPL's `handleEnterKey` wraps ALL input (including regular chat) as `SlashCommandMsg`. The `handleSlashCommand` then has a fall-through path for non-slash input that calls `sendChatMessage`. This creates an indirect routing path where:
1. User types "hello"
2. `handleEnterKey` adds message to REPL, returns `SlashCommandMsg{Command: "hello"}`
3. `handleSlashCommand` sees no `/` prefix, calls `sendChatMessage`
4. `sendChatMessage` reads ALL messages (including the just-added one) and appends ANOTHER user message

**Question:** Was this intentional? Should regular messages use a different message type (e.g., `ChatMessageMsg`) to avoid the double-add problem?

---

### CONFUSION-2: Two Parallel Streaming Systems

The codebase has two distinct streaming mechanisms:
1. **REPL direct chat:** `StartStreamCmd` → `StreamMsg` / `StreamDoneMsg` / `StreamErrorMsg` (in `streaming.go`)
2. **Workflow engine:** `engine.emit(StreamChunkMsg{...})` via `MsgEmitter` (in `engine.go`)

These are completely separate. The REPL streaming works (goroutine → channel → tea.Cmd), but the workflow streaming has no TUI handler.

**Question:** Were these supposed to be unified? The workflow engine's `streamLLM` and `streamLLMStreaming` methods consume the stream internally and return results, but the discuss phase tries to emit progressive chunks that have no handler.

---

### CONFUSION-3: `DiscussModel` Exists but is Never Used in Workflow Flow

The `DiscussModel` in `internal/tui/discuss.go` is a complete implementation with:
- Question display with progress indicator
- Text input for answers
- Skip/advance functionality
- Timeout support

But the workflow's `handlePhaseResult` for `PhaseDiscuss` never creates or shows this model. The discuss phase's questions are returned in `PhaseResult.Messages` but never presented to the user.

**Question:** Is the `DiscussModel` intended for a different flow (e.g., manual `/discuss` command) rather than the automated workflow?

---

### CONFUSION-4: `PermissionContext.Source` Logic

In `permissions.go:89`, when no rule matches, the code returns:
```go
return false, &PermissionContext{Source: "risk_level"}, nil
```

Then in `dispatcher.go:127-153`, the permission check logic:
```go
if !allowedByRule {
    ...
    } else if pctx != nil && pctx.Source == "rule" && pctx.RuleAction == "ask" {
        // rule-based ask
    } else if pctx != nil && pctx.Source == "agent_default" && pctx.RuleAction == "ask" {
        // agent-default ask
    } else {
        if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
            // fallback ask for dangerous tools
        }
    }
}
```

The `Source: "risk_level"` path falls into the final `else` block, which only asks for `Dangerous` or `Destructive` tools. This means `Medium` risk tools (like `FileWrite`) are **silently allowed** without any permission prompt when no rule matches.

**Question:** Is this intentional? Should `FileWrite` (medium risk) require permission when no rule explicitly allows it?

---

### CONFUSION-5: `Registry.Get` Returns `ErrProviderUnreachable` for Missing Providers

In `registry.go:69-77`:
```go
func (r *Registry) Get(name string) (LLMProvider, error) {
    ...
    if !ok {
        return nil, m31errors.ErrProviderUnreachable
    }
    return p, nil
}
```

Using `ErrProviderUnreachable` (which means "network error connecting to provider") for a "provider not registered" case is semantically wrong. This could confuse error handling — a caller checking for `ErrProviderUnreachable` would think the provider exists but is down, when actually it was never registered.

**Question:** Should this be `ErrProviderNotFound` instead?

---

## SUMMARY OF FINDINGS

| # | Issue | Severity | Type | Impact |
|---|-------|----------|------|--------|
| BUG-1 | User messages doubled in LLM context | CRITICAL | Logic | Wasted tokens, confused LLM |
| BUG-2 | Discuss answers never collected | CRITICAL | Wiring | Discuss phase is decorative |
| BUG-3 | Workflow engine has no message emitter | CRITICAL | Wiring | All progress messages dropped |
| BUG-4 | Discuss streaming chunks dropped | HIGH | Wiring | Progressive rendering broken |
| BUG-5 | Question modal is a no-op | HIGH | Logic | AskUserQuestion tool unusable |
| BUG-6 | Permission "A" key missing | MEDIUM | Missing feature | Can't approve-and-remember |
| ISSUE-7 | Permission modal 0×0 dimensions | HIGH | Rendering | Modal not centered |
| ISSUE-8 | ShipSummary type duplicated | MEDIUM | Design | Data mismatch between packages |
| ISSUE-9 | Phase name as progress string | LOW | Design | Less useful STATE.md |
| ISSUE-10 | Dead `_ = strings.Repeat` | LOW | Dead code | No runtime impact |

---

## RECOMMENDATIONS

1. **BUG-1 Fix:** Remove the explicit user message append in `sendChatMessage`. The message is already in `m.messages` from `handleEnterKey`. Or alternatively, don't add it in `handleEnterKey` and let `sendChatMessage` be the sole owner.

2. **BUG-2 Fix:** In `handlePhaseResult` for `PhaseDiscuss`, check `msg.NeedsAnswers`. If true, create `DiscussModel`, set `m.screen = ScreenDiscuss`, and wait for answers before calling `RunPhaseCmd(PhasePlan)`.

3. **BUG-3 Fix:** In `RunPhaseCmd`, create a `channelEmitter` and call `engine.SetMsgEmitter(emitter)` before running the phase. Wire the emitter's channel to the TUI update loop.

4. **BUG-4 Fix:** Add a `case types.StreamChunkMsg:` handler in `app_update.go` that forwards chunks to the appropriate sub-model (REPL or discuss screen).

5. **BUG-5 Fix:** Add a `textinput.Model` to the question modal flow. Route keystrokes to it when the question modal is active. Only dismiss on Enter with the typed content.

6. **BUG-6 Fix:** Add `case "a":` to `handlePermissionKey` that calls `handlePermissionResponse` with `Remember: true`.

7. **ISSUE-7 Fix:** Change `lipgloss.Place(0, 0, ...)` to `lipgloss.Place(m.width, m.height, ...)` in `RenderPermissionModal`.
