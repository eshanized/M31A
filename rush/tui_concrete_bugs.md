# M31A TUI Concrete Bugs

> Verified bugs with exact file:line references and reproduction evidence.

---

## BUG-01: ModelSelector selection does nothing

**File**: `internal/tui/modelselector.go:199-206`
**Handler**: `internal/tui/app_update.go:502-503`

When user presses Enter in the model selector, it emits:
```go
return m, func() tea.Msg {
    return AppMsg{
        ModelSelected: &ModelSelectedMsg{...},
    }
}
```

But `app_update.go:502` calls `handleAppMsg(msg)` which doesn't handle `ModelSelectedMsg`. The active model is never updated. The entire model selector feature is non-functional.

**Reproduction**: Press `ctrl+p` → select a model → press Enter → nothing happens.

---

## BUG-02: backupCurrentSession() mutates state from goroutine

**File**: `internal/tui/backup.go:54-56`

```go
m.toastText = fmt.Sprintf("Auto-backup saved: %s", filepath.Base(dst))
m.toastType = "info"
m.toastExpires = time.Now().Add(4 * time.Second)
```

Called from `app_workflow.go` during Ship phase execution (which runs in a goroutine via `RunPhaseCmd`). Mutates `AppState.toastText`, `toastType`, `toastExpires` directly from a goroutine, violating Bubble Tea's single-threaded Update() contract.

**Fix**: Return a `ToastMsg` instead of mutating state directly.

---

## BUG-03: Sidebar cache written from goroutine without sync

**File**: `internal/tui/sidebar.go:140-146`

```go
func (m *SidebarModel) refreshCmd() tea.Cmd {
    return func() tea.Msg {  // ← runs in goroutine
        if !m.lastStatusFetch.IsZero() && time.Since(m.lastStatusFetch) < sidebarStatusCacheTTL {
            return SidebarRefreshMsg{Statuses: m.gitStatusCache, Err: m.gitStatusCacheErr}
        }
        statuses, err := m.git.StatusPorcelain()
        m.gitStatusCache = statuses      // ← write in goroutine
        m.gitStatusCacheErr = err        // ← write in goroutine
        m.lastStatusFetch = time.Now()   // ← write in goroutine
        return SidebarRefreshMsg{Statuses: statuses, Err: err}
    }
}
```

`View()` reads `m.statuses` and `m.loading`. The goroutine writes to `m.gitStatusCache*` fields. While `SidebarRefreshMsg` is delivered to Update() (which is safe), the cache fields are written before the message is sent. If `View()` runs between the cache write and the message delivery, it reads stale data.

**Severity**: Low in practice (race window is tiny), but technically a data race.

---

## BUG-04: StreamChunkMsg lost during non-discuss workflow phases

**File**: `internal/tui/app_update.go:515-518`

```go
case StreamChunkMsg:
    if m.workflowRunning && m.currentPhase != types.PhaseDiscuss {
        m.pendingStreamChunks = append(m.pendingStreamChunks, msg.Chunk)
        return m, nil
    }
```

Stream chunks from workflow phases other than Discuss are buffered in `m.pendingStreamChunks`. But no code ever reads from this slice after the phase completes. `flushPendingStreamChunks()` only appends to messages, but it's never called after the streaming phase ends.

**Impact**: Streaming content from Execute/Verify phases is silently lost.

---

## BUG-05: OptimizedMsg never handled

**File**: `internal/tui/types.go:174-177`
**Expected handler**: `app_update.go`

`OptimizedMsg` is defined as a message type but has no handler in `app_update.go`. Any code that emits this message (e.g., `/optimize` command, Plan screen "O" key) will have its result silently dropped.

---

## BUG-06: renderQuickActions() is dead code

**File**: `internal/tui/repl_quickactions.go:8-48`

The function `renderQuickActions()` renders 4 action tiles (Settings, Models, Sessions, Help) but is never called from `repl_view.go` or anywhere else. The entire quick actions feature is unimplemented.

---

## BUG-07: Hardcoded colors ignore theme

**File**: `internal/tui/repl_view.go:100`
```go
Foreground(lipgloss.Color("#000000"))  // fallback banner
```

**File**: `internal/tui/repl_view.go:313`
```go
Background(lipgloss.Color("#2A2A2A"))  // welcome input box
```

Both use hardcoded dark-theme colors that will look wrong on light themes.

---

## BUG-08: Brittle planningDir recalculation in SetSessionID

**File**: `internal/workflow/engine.go:265-266`

```go
func (e *Engine) SetSessionID(id string) {
    e.sessionID = id
    e.planningDir = filepath.Join(filepath.Dir(e.planningDir), "..", id, "planning")
}
```

Uses `..` to navigate up from the current session directory. This assumes `planningDir` is always at depth 3 from sessions root. If the path structure ever changes (e.g., nested sessions), this produces an incorrect path.

**Fix**: Use `filepath.Join(sessionsRoot, id, "planning")` where `sessionsRoot` is stored as a field.

---

## BUG-09: Empty tool parameter schemas sent to LLM

**File**: `internal/workflow/engine.go:400-406`

```go
defs = append(defs, provider.ToolDefinition{
    Name:        tool.Name(),
    Description: tool.Description(),
    Parameters:  "{}",
})
```

All tools report `"{}"` as their parameter schema. The LLM has no idea what parameters each tool accepts, leading to malformed tool calls.

**Impact**: Every tool call requires the LLM to guess parameters, increasing self-heal attempts.

---

## BUG-10: permissionResponseCh is unbuffered — potential deadlock

**File**: `internal/tools/dispatcher.go:38`

```go
responseCh: make(chan PermissionResponse),  // UNBUFFERED
```

If the TUI sends a permission response but the tool goroutine hasn't read it yet (e.g., another tool call fired first), the TUI's send blocks. Since Bubble Tea's Update() is single-threaded, this blocks the entire UI.

**Fix**: Make `responseCh` buffered: `make(chan PermissionResponse, PermissionChannelBuffer)`

---

## BUG-11: Duplicate TickMsg type definitions

**File**: `internal/tui/streaming.go:54-56` and `internal/tui/repl.go:268`

Two `TickMsg` types exist. The REPL handles `TickMsg` at line 268, and `StreamTickCmd()` emits the one from `streaming.go`. Since they're in the same package, they're actually the same type. This is confusing but not a bug.

---

## BUG-12: Config hot-reload only syncs partial fields

**File**: `internal/tui/app_update.go:678-681`

```go
m.config.UI = msg.Config.UI
m.config.Permissions = msg.Config.Permissions
m.config.Features = msg.Config.Features
m.config.Ledger = msg.Config.Ledger
```

Missing sync of: `Provider`, `Model`, `Ghost`, `Agents` config sections. Hot-reloading only partially updates the running configuration.

---

## Summary

| Bug | Severity | Category | Fix Effort |
|-----|----------|----------|------------|
| BUG-01 | High | Broken Feature | Medium |
| BUG-02 | Critical | Race Condition | Easy |
| BUG-03 | Low | Race Condition | Easy |
| BUG-04 | High | Lost Data | Medium |
| BUG-05 | Medium | Unhandled Message | Easy |
| BUG-06 | Low | Dead Code | Easy |
| BUG-07 | Low | Theme Bug | Easy |
| BUG-08 | Medium | Fragile Code | Medium |
| BUG-09 | High | LLM Quality | Medium |
| BUG-10 | Critical | Deadlock Risk | Easy |
| BUG-11 | Info | Code Smell | N/A |
| BUG-12 | Medium | Incomplete Feature | Easy |
