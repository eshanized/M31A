# M31A TUI Screen Connectivity Map

> Maps every screen transition and the message flows between screens.

## Screen Enum

```
ScreenFirstRun     = 0
ScreenREPL         = 1
ScreenModelSelector = 2
ScreenSettings     = 3
ScreenResume       = 4
ScreenPermission   = 5
ScreenPlan         = 6
ScreenExecute      = 7
ScreenVerify       = 8
ScreenShip         = 9
ScreenDiff         = 10
```

---

## Transition Map

```
                    ┌──────────────────┐
                    │   ScreenFirstRun  │
                    └────────┬─────────┘
                             │ AppMsg{ScreenREPL}
                             ▼
┌──────────┐    /settings    ┌──────────┐    /models     ┌─────────────────┐
│ Resume   │◄───────────────│   REPL   │───────────────►│ ModelSelector   │
│          │    ctrl+x→resume│          │                 │                 │
└──────────┘                 └────┬─────┘                 └────────┬────────┘
     │                           │                                  │
     │ Enter                     │ /workflow <goal>                 │ Esc
     │                           ▼                                  │
     │                    ┌──────────────┐                          │
     │                    │  Initialize  │                          │
     │                    └──────┬───────┘                          │
     │                           │ auto                             │
     │                           ▼                                  │
     │                    ┌──────────────┐                          │
     │                    │   Discuss    │◄─────────────────────────┘
     │                    └──────┬───────┘
     │                           │ finalize/skip
     │                           ▼
     │                    ┌──────────────┐
     │                    │     Plan     │
     │                    └──────┬───────┘
     │                           │ auto
     │                           ▼
     │                    ┌──────────────┐
     │                    │   Execute    │
     │                    └──────┬───────┘
     │                           │ auto
     │                           ▼
     │                    ┌──────────────┐
     │                    │    Verify    │
     │                    └──────┬───────┘
     │                           │ auto
     │                           ▼
     │                    ┌──────────────┐
     └───────────────────►│     Ship     │
         new_session      └──────────────┘
```

---

## Key Findings: Connectivity Issues

### 1. ModelSelector → AppState: Broken Return Path

**Problem**: When the user selects a model in `ScreenModelSelector` and presses Enter, the model selector emits:
```go
return m, func() tea.Msg {
    return AppMsg{
        ModelSelected: &ModelSelectedMsg{Model: mi.Model, Provider: mi.Provider},
    }
}
```

But `app_update.go` has NO handler for `ModelSelectedMsg` in the `AppMsg` case. The `AppMsg` handler (line 502) calls `handleAppMsg(msg)`, but looking at the code, `ModelSelectedMsg` is never processed — the REPL's active model is never updated.

**Impact**: Selecting a model in the model selector has no effect. The model selector is purely cosmetic.

### 2. PermissionModal → Tool Execution: Channel Correlation Gap

**Flow**:
1. Tool execution calls `d.askPermission()` → writes to `requestCh`
2. TUI receives `PermissionRequestMsg` → shows modal
3. User presses Y/N → `PermissionResponseMsg` emitted
4. `handlePermissionResponse` writes to `responseCh`

**Problem**: There's only one `responseCh` shared by all permission requests. If a second tool call fires while the first permission is pending, the response goes to whoever reads `responseCh` first — which may be the wrong goroutine.

### 3. Workflow Phase Transitions: Auto-Advance is Fragile

**Flow**: Execute → Verify → Ship auto-advance via `PhaseResultMsg` handler.

**Problem**: The `handlePhaseResult` handler (in `app_workflow.go`) must correctly:
1. Parse the phase result
2. Transition to the next phase
3. Reset state for the new phase

But `PhaseResultMsg` carries both success and error cases in the same struct, and the handler must handle all transitions. If any phase handler returns an error or nil result, the workflow gets stuck.

### 4. Diff Screen: No Incoming Content Path

**Flow**: 
- `ScreenDiff` is created in `app_update.go:656-660` when `DiffScreenMsg` arrives
- The diff model receives the content

**Problem**: `DiffScreenMsg` is defined but nothing in the codebase actually SENDS it. The diff screen can be created (via the handler) but never receives content. It always shows "Loading diff..."

### 5. Discuss Phase → REPL: Streaming Disconnect

**Flow**: During Discuss, the workflow engine streams LLM responses via `StreamChunkMsg`.

**Problem**: `StreamChunkMsg` is handled in `app_update.go:515-522`:
```go
case StreamChunkMsg:
    if m.workflowRunning && m.currentPhase != types.PhaseDiscuss {
        m.pendingStreamChunks = append(m.pendingStreamChunks, msg.Chunk)
        return m, nil
    }
    if m.replModel != nil {
        m.replModel.AppendStreamChunk(msg.Chunk)
    }
```

During Discuss phase, chunks go to `replModel.AppendStreamChunk()`. But `AppendStreamChunk()` only appends to `m.streamContent` — it doesn't build segments, doesn't handle thinking blocks, and doesn't render incrementally. The streaming content during Discuss is buffered but not rendered in real-time.

### 6. Settings → Config Hot-Reload: Incomplete Field Sync

**Flow**: Config hot-reload sends `ConfigReloadMsg`.

**Problem**: The handler at `app_update.go:678-691` only syncs some fields:
```go
m.config.UI = msg.Config.UI
m.config.Permissions = msg.Config.Permissions
m.config.Features = msg.Config.Features
m.config.Ledger = msg.Config.Ledger
```

Missing: `Provider`, `Model`, `Ghost`, `Agents` config sections. A hot-reload only partially updates the config.

---

## Screen State Ownership Issues

### AppState fields that multiple screens write to:

| Field | Writers | Problem |
|-------|---------|---------|
| `m.streaming` | repl.go, app_update.go | Set from multiple places without coordination |
| `m.thinking` | repl.go, app_update.go | Same as above |
| `m.workflowRunning` | app_workflow.go, app_update.go | Could be set out of order |
| `m.currentPhase` | setWorkflowPhase, handlePhaseResult | Phase transitions rely on this being in sync |
| `m.toastText` | app.go, app_workflow.go, backup.go | backup.go writes from goroutine |

### Screens that read AppState but don't own it:

| Screen | Reads | Problem |
|--------|-------|---------|
| PlanModel | `m.sessionID`, `m.workflowEngine` | Set by resume, first-run, etc. |
| ExecuteModel | same | Same |
| VerifyModel | same | Same |
| ShipModel | same | Same |

All workflow screens depend on `sessionID` being correctly propagated from whatever screen set it last (resume, first-run, or init). If any path forgets to propagate it, the workflow screens write to the wrong session directory.

---

## Recommended Fixes

### Fix 1: ModelSelector Return Path
Add a `ModelSelectedMsg` handler in `app_update.go` that updates `m.activeModel`, `m.activeProvider`, and calls `m.replModel.SetProvider(...)`.

### Fix 2: Permission Channel Correlation
Either:
- Add a `RequestID` field to `PermissionRequest` and `PermissionResponse`, with a map of pending requests
- Or make `responseCh` buffered with capacity equal to `PermissionChannelBuffer`

### Fix 3: Diff Screen Content
Create a `ShowDiff(diff, title)` command somewhere in the codebase (e.g., in the rollback package or ship phase) that emits `DiffScreenMsg`.

### Fix 4: Discuss Streaming
During Discuss, `AppendStreamChunk()` should also handle thinking segments and render incrementally, matching the behavior of `handleStreamMsg()`.

### Fix 5: Config Hot-Reload Completeness
Sync all config sections, not just UI/Permissions/Features/Ledger.
