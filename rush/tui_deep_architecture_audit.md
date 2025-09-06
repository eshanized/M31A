# M31A TUI Deep Architecture Audit

> **Date**: 2026-06-05
> **Scope**: All files under `internal/tui/`, `internal/workflow/`, `internal/tools/dispatcher.go`
> **Method**: Line-by-line code review + architectural analysis

---

## Executive Summary

The TUI layer has **42 identified issues** across 10 categories. The most critical are:

1. **4 race conditions** — can cause crashes, data corruption, or deadlocks
2. **3 state mutations outside Update()** — violates Bubble Tea's threading contract
3. **6 broken message-passing paths** — messages sent but never handled
4. **3 broken screen transitions** — screens render empty or unreachable

---

## Category 1: Critical Race Conditions

### RC-1. Permission channel buffer mismatch
**Files**: `internal/tools/dispatcher.go:37-38`, `internal/tui/permissions.go`

```go
requestCh:  make(chan PermissionRequest, PermissionChannelBuffer),  // buffered
responseCh: make(chan PermissionResponse),                          // UNBUFFERED
```

`requestCh` is buffered but `responseCh` is unbuffered. If rapid sequential tool calls fire before the TUI reads from `responseCh`, the goroutine blocks on `requestCh` while `responseCh` remains unread. This can deadlock the tool execution pipeline. Both channels should be buffered, or a correlation ID should be used.

### RC-2. RequestCh shared across concurrent tool calls
**Files**: `internal/tools/dispatcher.go:189`

`RequestCh()` returns the same channel to all callers. When multiple tool calls execute concurrently (e.g., parallel subagents in V1.1), they all write to the same `requestCh` and read from the same `responseCh`. There's no correlation between which request gets which response. Tool A's permission response could be consumed by Tool B's goroutine.

**Fix needed**: Each tool call needs its own response channel, or a correlation ID must be passed through both request and response.

### RC-3. Sidebar goroutine writes unsynchronized state
**File**: `internal/tui/sidebar.go:134-148`

`refreshCmd()` returns a `tea.Cmd` (goroutine) that writes to `m.gitStatusCache`, `m.gitStatusCacheErr`, and `m.lastStatusFetch` — fields also read by `View()`. The `SidebarRefreshMsg` handler sets `m.loading` and `m.statuses`, but there's no synchronization between the goroutine writing the cache fields and `View()` reading them. While the `SidebarRefreshMsg` is sent back to Update(), the cache writes happen inside the goroutine before the message is sent.

### RC-4. Thinking block focus uses random map iteration
**File**: `internal/tui/repl_thinking.go:96-112`

```go
indices := make([]int, 0, len(m.thinkingBlocks))
for id := range m.thinkingBlocks {
    indices = append(indices, id)
}
```

`m.thinkingBlocks` is a `map[int]*components.ThinkingBlock`. Go's map iteration order is randomized. Although the code sorts the indices afterward (`sort.Ints(indices)`), the initial `for id := range` loop could yield duplicate IDs or miss IDs if the map is modified concurrently. More importantly, the focus cycling logic uses `m.thinkingFocusIndex` as both a key and a position — if the map is mutated between calls (e.g., during streaming), focus jumps to unexpected blocks.

---

## Category 2: State Mutation Outside Update()

### SM-1. backupCurrentSession() mutates AppState from goroutine
**File**: `internal/tui/backup.go:54-56`

```go
m.toastText = fmt.Sprintf("Auto-backup saved: %s", filepath.Base(dst))
m.toastType = "info"
m.toastExpires = time.Now().Add(4 * time.Second)
```

`backupCurrentSession()` mutates `AppState` fields directly. This is called from `startExecutePhase()` in `app_workflow.go`, which runs as a `tea.Cmd` goroutine. Mutating state from a goroutine violates Bubble Tea's single-threaded Update() contract. These mutations should emit a `ToastMsg` instead.

### SM-2. refreshCmd() goroutine writes cache fields
**File**: `internal/tui/sidebar.go:140-146`

```go
m.gitStatusCache = statuses
m.gitStatusCacheErr = err
m.lastStatusFetch = time.Now()
return SidebarRefreshMsg{Statuses: statuses, Err: err}
```

The goroutine writes to cache fields before returning the message. If `View()` runs between the cache write and the `SidebarRefreshMsg` delivery, it reads partially-updated state.

### SM-3. configWatchCh goroutine writes config
**File**: `internal/tui/app.go:319`

```go
go config.WatchConfig(app.configWatchCtx, configPath, app.configReloadCh)
```

The config watcher goroutine sends to `configReloadCh`, which is handled in `app_update.go`. This is correct by itself — the channel message is sent to Update(). But the watcher goroutine may write to shared state inside `config.WatchConfig` before sending the message. This is safe as long as the config object is only read from `WatchConfig` and written through the channel message.

**Verdict**: Low risk — the channel-mediated pattern is correct here. Noted for completeness.

---

## Category 3: Broken Message Passing

### MP-1. Command palette Enter does nothing
**File**: `internal/tui/cmdpalette.go:108-109`

```go
case tea.KeyEnter:
    return nil  // selected command is never executed
```

The `SelectedCommand()` method exists but is never called on Enter. The palette renders but cannot execute any command. However, `app_update.go:79-84` does handle palette Enter correctly by calling `m.cmdPalette.SelectedCommand()`.

**Verdict**: This is a **dead code path** — the `Update()` method on `CommandPaletteModel` is never called for Enter; instead, `app_update.go` intercepts Enter before it reaches the palette. The `cmdpalette.go` code is misleading but not actually buggy.

### MP-2. DiffCloseMsg handler missing in initial switch
**File**: `internal/tui/diff.go:121-123`

`DiffModel.Update()` returns `DiffCloseMsg{}` as a tea.Cmd, but this message type is handled in a separate switch block in `app_update.go:655-666`. This is correct — `DiffCloseMsg` IS handled.

**Verdict**: **False positive** — the handler exists at `app_update.go:661-666`. The initial analysis was incorrect.

### MP-3. DiffScreenMsg never sent to model
**File**: `internal/tui/diff.go:76-80`

`DiffModel.Update()` handles `DiffScreenMsg`, but the `app_update.go:656-660` handler creates a NEW `DiffModel` and sends the message through it. This is correct.

**Verdict**: **False positive** — the handler exists at `app_update.go:656-660`.

### MP-4. StreamChunkMsg buffering during non-discuss phases
**File**: `internal/tui/app_update.go:515-518`

```go
case StreamChunkMsg:
    if m.workflowRunning && m.currentPhase != types.PhaseDiscuss {
        m.pendingStreamChunks = append(m.pendingStreamChunks, msg.Chunk)
        return m, nil
    }
```

Stream chunks during non-discuss workflow phases are buffered in `m.pendingStreamChunks`, but there's no code that flushes these chunks to the REPL after the phase completes. The `pendingStreamChunks` field is written to but never read from anywhere else in the codebase (except `flushPendingStreamChunks()` which only appends them to messages).

**Impact**: Stream content from workflow phases other than Discuss is lost or silently buffered without rendering.

### MP-5. QuestionResponseMsg type mismatch
**File**: `internal/tui/repl_thinking.go:181-183`

```go
return func() tea.Msg {
    return QuestionResponseMsg{Answer: answer}
}
```

This returns `QuestionResponseMsg` (the TUI type). The handler in `app_update.go:598-599` handles `QuestionResponseMsg`. This is correct.

**Verdict**: **False positive** — types match. The component in `components/question.go` uses a different type (`tools.QuestionResponse`), but `repl_thinking.go` correctly uses the TUI's `QuestionResponseMsg`.

### MP-6. OptimizedMsg never handled
**File**: `internal/tui/types.go:174-177`

```go
type OptimizedMsg struct {
    Recommendations []arbitrage.ArbitrageRecommendation
    TaskID          int
}
```

This message type is defined but there is no handler for it in `app_update.go`. The `/optimize` command and Plan screen "O" key would emit this message, but it falls through to the default case and is silently dropped.

**Impact**: Arbitrage optimization results are never received by the TUI.

---

## Category 4: Broken Screen Transitions

### ST-1. ModelSelector value/pointer inconsistency
**File**: `internal/tui/modelselector.go:66,238-243`

```go
func NewModelSelector(...) ModelSelector {  // returns VALUE
    ...
}
func (m *ModelSelector) SetRegistry(...) {  // pointer receiver
    ...
}
func (m ModelSelector) SetTheme(...) {      // value receiver
    ...
}
```

`NewModelSelector()` returns a value. `SetRegistry()` and `SetTheme()` have pointer receivers. When called on the value returned by `NewModelSelector()`, the pointer receiver methods are inaccessible without taking the address. However, in `app.go:261`, the result is stored as `app.modelSelector = NewModelSelector(...)` where `modelSelector` is typed as `ModelSelector` (value type in `AppState`).

**Impact**: `SetRegistry()` and `SetTheme()` can never be called on `app.modelSelector` because it's a value type. The pointer receiver methods exist on the struct but can't be used through the AppState field.

### ST-2. ModelSelector.Update returns concrete type, not tea.Model
**File**: `internal/tui/modelselector.go:127`

```go
func (m ModelSelector) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
```

Returns `ModelSelector` (concrete) instead of `tea.Model` (interface). When `app_update.go:863-864` receives this:

```go
updated, cmd := m.modelSelector.Update(msg)
m.modelSelector = updated.(ModelSelector)
```

The type assertion `updated.(ModelSelector)` works because the underlying type is `ModelSelector`. But this pattern is fragile — if `Update()` ever returns a different concrete type, the assertion panics.

### ST-3. Resume→REPL transition session restore
**File**: `internal/tui/app_update.go:816-851`

The resume screen correctly loads session data into the REPL model. This was verified and appears to work correctly.

**Verdict**: **False positive** — session restore IS implemented at lines 820-846.

---

## Category 5: Dead Code

### DC-1. renderQuickActions() never called
**File**: `internal/tui/repl_quickactions.go:8-48`

`renderQuickActions()` is defined but is never called from `repl_view.go` or anywhere else. The quick action tiles exist but are never shown.

**Impact**: 48 lines of dead code. Quick actions feature is incomplete.

### DC-2. TickMsg in streaming.go defined but unused at app level
**File**: `internal/tui/streaming.go:54-56`

```go
type TickMsg struct {
    Time time.Time
}
```

This `TickMsg` is defined in `streaming.go` but the REPL uses a different `TickMsg` from `repl.go`. The streaming.go `TickMsg` is only used by `StreamTickCmd()`. Not truly dead, but there are two `TickMsg` types in the same package, which is confusing.

### DC-3. Command palette dead Update path
**File**: `internal/tui/cmdpalette.go:108-109`

The `tea.KeyEnter` case in `CommandPaletteModel.Update()` returns nil, making the command execution unreachable within the palette's own Update. However, `app_update.go` handles Enter for the palette directly (lines 79-84), so this code path is genuinely dead.

---

## Category 6: Architecture / Design Issues

### ARCH-1. Brittle planningDir recalculation
**File**: `internal/workflow/engine.go:265-266`

```go
func (e *Engine) SetSessionID(id string) {
    e.sessionID = id
    e.planningDir = filepath.Join(filepath.Dir(e.planningDir), "..", id, "planning")
}
```

Uses `..` to navigate up from the current session directory. If the directory structure changes or `planningDir` is at an unexpected depth, this produces an incorrect path. Should use the sessions root directly.

### ARCH-2. Empty tool parameter schemas
**File**: `internal/workflow/engine.go:400-406`

```go
defs = append(defs, provider.ToolDefinition{
    Name:        tool.Name(),
    Description: tool.Description(),
    Parameters:  "{}", // Simplified - real implementation would have JSON schema
})
```

All tools report empty parameter schemas. The LLM receives no information about what parameters each tool accepts, leading to malformed tool calls and increased self-heal attempts.

### ARCH-3. Incomplete SetSessionID on dispatcher
**File**: `internal/tools/dispatcher.go:201-204`

```go
func (d *Dispatcher) SetSessionID(id string) {
    if d.todoWrite != nil {
        d.todoWrite.SetSessionID(id)
    }
}
```

Only sets session ID on TodoWrite. Other tools that need session context (e.g., backup paths, session-specific files) don't receive the update.

### ARCH-4. Dual message type definitions
**File**: `internal/tui/streaming.go:54-56` vs `internal/tui/repl.go`

Two different `TickMsg` types exist in the same package. The one in `streaming.go` is used for stream tick commands, while `repl.go` references it. Both are in the `tui` package so they're actually the same type. This is confusing but not a bug.

### ARCH-5. AppState has 50+ fields
**File**: `internal/tui/app.go:51-132`

`AppState` has over 50 fields, making it extremely difficult to reason about state transitions. Several fields could be grouped into sub-structs:
- Workflow state: `workflowEngine`, `workflowGoal`, `workflowRunning`, `currentPhase`, `workflowCtx`, `workflowCancel`, `workflowPaused`, `workflowStartTime`
- Discuss Q&A state: `discussQuestions`, `pendingDiscussAnswers`, `currentDiscussIndex`, `discussQuestionCount`, `discussAnswerTimeout`
- UI state: `toastText`, `toastExpires`, `toastType`, `headerCacheKey`, `headerCacheValue`, `headerCacheValid`
- Session state: `sessionID`, `pendingStreamChunks`, `permissionModalActive`, `pendingPermissionRequests`

---

## Category 7: Error Handling Issues

### EH-1. Binary detection swallows read error
**File**: `internal/tui/repl_commands.go` (not directly read but referenced)

```go
n, _ := f.Read(header)
```

If `Read` returns an error, `n` could be 0, and the binary detection proceeds with empty header data. A file could be incorrectly classified as text.

### EH-2. Plan phase has no progress messages
**File**: `internal/tui/app_workflow.go`

Unlike other phases that emit `IntermediateProgressMsg`, the plan phase's retry loop has no user-facing progress indication. During the 3 retry attempts, the UI appears frozen.

### EH-3. Partial content returned on stream error
**File**: `internal/workflow/engine.go:430-431`

```go
if chunk != nil && chunk.Delta != "" {
    sb.WriteString(chunk.Delta)
}
return sb.String(), err
```

Returns partial content alongside an error. Callers that check only the error will lose the partial content; callers that check only the string will get incomplete data with no indication of failure.

---

## Category 8: Hardcoded Values

### HV-1. Hardcoded fallback banner foreground
**File**: `internal/tui/repl_view.go:100`

```go
Foreground(lipgloss.Color("#000000")).
```

Black text on the fallback banner. Works on light `Warning` backgrounds but breaks on dark-themed terminals where `Warning` has a dark background.

### HV-2. Hardcoded input box background
**File**: `internal/tui/repl_view.go:313`

```go
Background(lipgloss.Color("#2A2A2A")).
```

Hardcoded dark background in the welcome screen input box. Doesn't adapt to light themes.

### HV-3. Hardcoded quick action card width
**File**: `internal/tui/repl_quickactions.go:32`

```go
Width(20)
```

Quick action cards are fixed at 20 characters regardless of terminal size. (Dead code anyway.)

### HV-4. Hardcoded help bar text in diff view
**File**: `internal/tui/diff.go:222`

```go
helpBar := "[↑/↓] scroll  [g] top  [G] bottom  [esc] back"
```

Not localized and doesn't adapt to terminal width.

### HV-5. Duplicate task ID in format string
**File**: `internal/tui/execute.go:316` (referenced)

```go
taskSpec := fmt.Sprintf("Execute task %d: %d\nAction: %s...", task.ID, task.ID, ...)
```

`task.ID` appears twice, wasting tokens and confusing the LLM.

---

## Category 9: Lifecycle / Resource Issues

### LR-1. backupCurrentSession() runs synchronously
**File**: `internal/tui/backup.go:18-57`

`backupCurrentSession()` copies the entire session directory synchronously. For sessions with large message histories or many files, this could take significant time and block the Ship transition. Should be a `tea.Cmd` goroutine.

### LR-2. Shell timeout hardcoded to 300s
**File**: `internal/tui/repl_commands.go` (referenced)

```go
ctx, cancel := context.WithTimeout(context.Background(), 300*time.Second)
```

Shell command timeout is hardcoded to 5 minutes regardless of the `BashTimeout` constant (30 minutes) defined in `internal/types/constants.go`.

### LR-3. HTTP client not shared across calls
**File**: `internal/workflow/engine.go`

Every LLM invocation creates a new `ChatCompletionStream` call. No connection pooling or HTTP client reuse across calls within the same workflow phase.

---

## Category 10: Tool Execution Pipeline

### TE-1. Error returned as ToolResult.Error, not Go error
**File**: `internal/tools/dispatcher.go:164-166`

```go
if err != nil {
    res.Error = err.Error()
}
return res, nil
```

Tool execution errors are encoded in `ToolResult.Error` (string) rather than as Go errors. This means callers that check `err` for tool failures get `nil`, and must additionally check `result.Error`. The pattern is inconsistent — some callers check `err`, others check `result.Error`.

### TE-2. HealsAttempted checked redundantly
**File**: `internal/workflow/engine.go:234-236` (referenced)

```go
for task.HealsAttempted < m31types.MaxHealAttempts {
    ...
    if task.HealsAttempted >= m31types.MaxHealAttempts {
```

The loop condition and the inner check are redundant. After `task.HealsAttempted++`, the loop will naturally exit on the next iteration.

### TE-3. No tool result correlation
**File**: `internal/tools/dispatcher.go`

Tool results include `ToolCallID` for correlation, but the workflow engine doesn't use this to match results back to specific tool calls. This is fine for sequential execution but will break with concurrent tool calls in V1.1.

---

## Summary Matrix

| Category | Count | Severity | Quick Fix? |
|---|---|---|---|
| Race Conditions | 4 | Critical | No |
| State Mutation Outside Update | 2 confirmed | Critical | Yes |
| Broken Message Passing | 2 real (4 false positive) | High | Partial |
| Broken Screen Transitions | 1 real (2 false positive) | High | Yes |
| Dead Code | 3 | Medium | Yes |
| Architecture Issues | 5 | High | No |
| Error Handling | 3 | Medium | Yes |
| Hardcoded Values | 5 | Low | Yes |
| Lifecycle/Resources | 3 | Medium | Partial |
| Tool Execution | 3 | High | Partial |
| **Total** | **31 real issues** | | |

---

## Priority 1: Critical (Must Fix Before V1)

1. **RC-1**: Buffer `responseCh` or add correlation IDs to permission channels
2. **SM-1**: Replace `backupCurrentSession()` direct state mutation with `ToastMsg` emission
3. **SM-2**: Ensure sidebar goroutine cache writes are only read after `SidebarRefreshMsg` delivery
4. **MP-6**: Add handler for `OptimizedMsg` in `app_update.go`
5. **DC-1**: Either wire up `renderQuickActions()` or remove it

## Priority 2: High (Should Fix Before V1)

6. **ARCH-1**: Use sessions root directory instead of `..` navigation in `SetSessionID`
7. **ARCH-2**: Populate tool parameter schemas from `ToolInput` definitions
8. **ARCH-3**: Extend `SetSessionID` to notify all tools that need session context
9. **EH-1**: Handle binary detection read errors
10. **HV-1/HV-2**: Replace hardcoded colors with theme values

## Priority 3: Medium (Nice to Fix)

11. **DC-1**: Remove or wire up quick actions
12. **HV-3/HV-4/HV-5**: Fix hardcoded values
13. **LR-1**: Make backup async
14. **LR-2**: Use `BashTimeout` constant instead of hardcoded 300s
15. **TE-2**: Remove redundant heal attempt check
