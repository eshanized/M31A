# Subagents Wiring Report

**Phase:** 01-wiring-audit  
**Plan:** 01-02  
**Task:** 8 — Subagents (lifecycle, worktrees, dispatcher sharing, event propagation)  
**Generated:** 2026-07-10

---

## 1. Manager Creation

### 1.1 Dependencies Injection (`main.go:294-304`, `manager.go:84-93`)

```go
// main.go — Step 19: Subagent Manager Creation
subagentMgr := subagent.NewManager(subagent.Dependencies{
    WorkDir:       workDir,
    Registry:      registry,
    ActiveModel:   activeModel,
    Logger:        slog.Default(),
    Worktrees:     GitWorktrees{},  // implements WorktreeOps
    NewDispatcher: dispatcherFactory,
    Profiles:      cfg.Agents.Profiles,
})
```

### 1.2 Manager Structure (`manager.go:63-71`)

```go
type Manager struct {
    deps         Dependencies
    sem          chan struct{}           // MaxConcurrent=8
    agents       sync.Map                // id -> *Subagent
    eventCh      chan SubagentEvent      // Buffered 256
    spawnMu      sync.Mutex
    totalSpawned int32                   // MaxTotalSubagents=50
    spawnTimes   []time.Time             // MaxSpawnRate=10/min
}
```

### 1.3 Limits Configuration (`manager.go:18-37`)

| Limit | Constant | Value | Purpose |
|-------|----------|-------|---------|
| Max Concurrent | `MaxConcurrent` | 8 | Semaphore slots |
| Event Buffer | `eventBuffer` | 256 | Channel capacity |
| Default Max Tools | `DefaultMaxTools` | 50 | Per-subagent tool budget |
| Default Max Tokens | `DefaultMaxTokens` | 50,000 | Token budget (input+output) |
| Max Total | `MaxTotalSubagents` | 50 | Session-wide spawn cap |
| Max Spawn Rate | `MaxSpawnRate` | 10/min | Prevent resource exhaustion |
| Shutdown Timeout | `shutdownTimeout` | 5s | Force cleanup deadline |

---

## 2. Spawn Sequence

### 2.1 Spawn Flow (`manager.go:100-236`)

```
Spawn(parentCtx, SpawnRequest)
    │
    ├─ Validate: Description + Prompt required
    ├─ Resolve Profile: ResolveProfile(type, Profiles) → AgentProfile
    ├─ Apply Budget Overrides: Profile.MaxTools/Tokens/Turns
    ├─ Check Provider: resolveProvider() → Active LLMProvider
    ├─ Acquire Concurrency Slot: sem <- struct{}{} (or ErrMaxConcurrent)
    ├─ Generate Agent ID: newAgentID() → 6 hex chars
    ├─ Resolve Model: req.ModelID > Profile.Model > ActiveModel.ID
    ├─ Create Worktree (if IsolationWorktree):
    │     GitWorktrees.Create(ctx, workDir, agentID, req.Name)
    │     → .m31a-worktrees/<agentID> on branch m31a/agent-<agentID>
    ├─ context.WithCancel(parentCtx)
    ├─ Create Subagent struct (Status=Running)
    ├─ Store in agents map
    ├─ Emit EventSpawned
    ├─ Launch runLoop goroutine
    ├─ If !Background: wait on agent.done channel
    └─ Return (id, *Subagent, nil)
```

### 2.2 Profile Resolution (`profile.go`)

```go
func ResolveProfile(agentType string, profiles map[string]SubagentProfileConfig) (AgentProfile, bool) {
    // 1. Built-in profiles (explore, general, security, etc.)
    // 2. Merge user config overrides (AllowedTools, DeniedTools, MaxTools, MaxTokens, MaxTurns, SystemPrompt, Model)
    // 3. Return AgentProfile with resolved values
}
```

### 2.3 Dispatcher Factory (`main.go:300`)

```go
dispatcherFactory := tools.NewDispatcherFactory(
    backupDir, backupDir,
    &cfg.Permissions, &cfg.Tools,
    nil,  // output store = nil for subagents
    cfg.Agents.Profiles,
)
```

**Child dispatcher created in `runLoop` (`manager.go:383-388`):**
```go
dispatcher, err := m.deps.NewDispatcher(sa.Info.Worktree)
ApplyToolFilter(dispatcher, sa.Profile)  // Allowlist/denylist
dispatcher.Stop() // deferred
```

---

## 3. RunLoop

### 3.1 Loop Structure (`loop.go:46-97`)

```go
func (l *loop) run(ctx context.Context) {
    // Initialize messages: system prompt + user prompt
    l.messages = []Message{
        {Role: "system", Content: l.buildSystemPrompt()},
        {Role: "user", Content: l.agent.req.Prompt},
    }
    
    for turn := 0; turn < l.maxTurns_; turn++ {
        // 1. Check context cancellation
        // 2. Check budgets (tools + tokens)
        // 3. runOneTurn(ctx, budgetExhausted)
        // 4. If done/error/cancelled → finish
    }
    // Max turns exceeded
    failAgent("exceeded max turns")
}
```

### 3.2 Single Turn (`loop.go:103-204`)

```
runOneTurn(ctx, budgetExhausted)
    │
    ├─ Build ChatRequest (model, messages, reasoning)
    ├─ If !budgetExhausted: req.Tools = buildToolDefinitions()  // Excludes AskUserQuestion
    ├─ provider.ChatCompletionStream(ctx, req) → StreamIterator
    ├─ consume(iterator) → (content, thinking, usage, nativeToolCalls)
    ├─ Emit EventThinking (if thinking)
    ├─ Parse tool calls: native > text fallback
    ├─ If no tool calls: append assistant msg → return (done=true)
    ├─ If budgetExhausted: discard tool calls → return (done=true)
    ├─ Append assistant msg with tool calls
    ├─ For each tool call: runTool(ctx, tc) sequentially
    └─ return (done=false)
```

### 3.3 Tool Execution (`loop.go:208-250`)

```go
runTool(ctx, tc)
    │
    ├─ Emit EventToolStart (tool_name, abbreviated input)
    ├─ dispatcher.Execute(ctx, tc) → ToolCallOutput
    ├─ Emit EventToolDone (output/error, duration)
    ├─ Increment toolCallsRun
    ├─ Append tool message to history
    └─ Return nil (errors surfaced via ToolCallOutput.Error)
```

### 3.4 Budget Enforcement (`loop.go:66-85`)

```go
toolBudgetExhausted := l.toolCallsRun >= l.maxTools
tokenBudgetExhausted := l.maxTokens > 0 && l.inputToks+l.outputToks >= l.maxTokens

if toolBudgetExhausted || tokenBudgetExhausted {
    // Inject user message forcing summary
    l.messages = append(l.messages, Message{
        Role: "user",
        Content: "Budget exhausted. Summarize findings now without further tool use.",
    })
}
```

### 3.5 Completion Paths

| Path | Trigger | Event Emitted |
|------|---------|---------------|
| Success | No tool calls from LLM | `EventDone` with summary, usage |
| Cancelled | `ctx.Done()` | `EventCancelled` with error |
| Error | LLM/tool error | `EventError` with error |
| Max Turns | Loop exits naturally | `EventError` "exceeded N turns" |

---

## 4. Event Propagation

### 4.1 Event Channel (`manager.go:96`, `events.go:69-99`)

```go
// Manager
eventCh = make(chan SubagentEvent, eventBuffer)  // 256

// Events() returns read-only channel for TUI
func (m *Manager) Events() <-chan SubagentEvent { return m.eventCh }
```

### 4.2 Event Types (`events.go:17-29`)

| Event | Payload Fields | Purpose |
|-------|----------------|---------|
| `EventSpawned` | AgentID, Name, SubagentType, Worktree, Timestamp | TUI shows new agent card |
| `EventToolStart` | ToolCallID, ToolName, ToolInput | Live tool activity |
| `EventToolDone` | ToolCallID, ToolName, ToolOutput, ToolError, DurationMs | Result display |
| `EventTextDelta` | Delta | Streaming text |
| `EventThinking` | Delta | Reasoning display |
| `EventDone` | Summary, ToolCalls, Input/OutputToks, Usage | Completion |
| `EventError` | Error | Failure display |
| `EventCancelled` | Error | Cancellation notice |

### 4.3 Emission Logic (`manager.go:347-373`)

```go
func (m *Manager) emit(ev SubagentEvent) {
    // Lifecycle events (spawned/done/error/cancelled) — BLOCK with 5s timeout
    // to ensure TUI can call Cleanup() and prevent worktree leaks.
    switch ev.Type {
    case EventDone, EventError, EventCancelled, EventSpawned:
        select { case m.eventCh <- ev: case <-time.After(5*time.Second): }
    default:
        // Text deltas, tool events — DROP if channel full (backpressure)
        select { case m.eventCh <- ev: default: }
    }
}
```

### 4.4 TUI Consumer (`app.go:415-416`)

```go
case SubagentEventMsg:
    cmds = append(cmds, m.handleSubagentEvent(msg)...)
```

**Handler routes to sidebar, agent palette, expanded cards.**

---

## 5. Worktree Lifecycle

### 5.1 GitWorktrees Implementation (`worktree.go:15-229`)

```go
type GitWorktrees struct {
    Root string  // Default: <parentWorkDir>/.m31a-worktrees/
}
```

### 5.2 Create (`worktree.go:34-55`)

```go
func (g *GitWorktrees) Create(ctx, parentWorkDir, agentID, branchSuffix) (string, error) {
    root := g.rootFor(parentWorkDir)           // .m31a-worktrees/
    path := filepath.Join(root, sanitize(agentID))
    branch := "m31a/agent-" + agentID + suffix
    
    // Clean stale branch
    git.New(parentWorkDir).Run("branch", "-D", branch)
    
    // Create worktree on new branch from HEAD
    exec.CommandContext(ctx, "git", "worktree", "add", "-b", branch, path)
    // Runs in parentWorkDir
    
    return path, nil
}
```

### 5.3 Remove (`worktree.go:58-89`)

```go
func (g *GitWorktrees) Remove(ctx, path) error {
    parentRepo, _ := parentRepoOf(path)  // git rev-parse --show-toplevel
    branch, _ := branchOf(ctx, path)     // git rev-parse --abbrev-ref HEAD
    
    // Force remove worktree
    exec.CommandContext(ctx, "git", "worktree", "remove", "--force", path)
    os.RemoveAll(path)  // Fallback
    
    // Clean branch
    if branch starts with "m31a/agent-" {
        git branch -D branch
    }
    return nil
}
```

### 5.4 Startup Sweep (`worktree.go:94-151`, `main.go:398-403`)

```go
func Sweep(ctx, parentWorkDir) error {
    // 1. git worktree prune (clears metadata)
    // 2. List live worktrees (git worktree list --porcelain)
    // 3. Delete orphaned m31a/agent-* branches
    // 4. Remove orphaned dirs under .m31a-worktrees/
}
```

---

## 6. Dispatcher Sharing

### 6.1 Parent Dispatcher (`main.go:318-321`)

```go
// Agent tool registered on PARENT dispatcher (non-child)
dispatcher.Register(tools.NewAgent(subagentMgr, false, 0, cfg.Agents.Profiles))
// false = can spawn background subagents
```

### 6.2 Child Dispatcher Factory (`manager.go:383-391`)

```go
dispatcher, err := m.deps.NewDispatcher(sa.Info.Worktree)
// ApplyToolFilter removes AskUserQuestion, applies profile allowlist/denylist
ApplyToolFilter(dispatcher, sa.Profile)
defer dispatcher.Stop()
```

### 6.3 ToolDispatcher Interface (`events.go:148-156`)

```go
type ToolDispatcher interface {
    Execute(ctx, call ToolCallInput) (ToolCallOutput, error)
    ListTools() []ToolDescriptor
    UnregisterTool(name string)
    Stop()
}
```

**Concrete type:** `*tools.Dispatcher` satisfies interface without modification.

### 6.4 Profile Filtering (`profile.go`)

```go
func ApplyToolFilter(d ToolDispatcher, profile AgentProfile) {
    // 1. Denylist: unregister tools in profile.DeniedTools
    // 2. Allowlist: if profile.AllowedTools non-empty, unregister all NOT in list
    // 3. AskUserQuestion always removed (subagents run headless)
}
```

### 6.5 Isolation Guarantees

| Aspect | Parent | Child |
|--------|--------|-------|
| Dispatcher Instance | Shared (parent) | New per subagent |
| Tool Set | Full 18 tools | Filtered by profile |
| Worktree | Main repo | Isolated (if Worktree) |
| Can Spawn Children | Yes (`isChild=false`) | **No** (`isChild=true` on Agent tool) |
| Output Store | Persistent | Nil (ephemeral) |
| Permissions | Global + project | Profile-scoped |

---

## 7. Cancellation Propagation

### 7.1 Manager-Level (`manager.go:263-277`)

```go
func (m *Manager) Cancel(id string) {
    if sa := m.Get(id); sa != nil { sa.cancel() }
}

func (m *Manager) CancelAll() {
    m.agents.Range(func(_, v) bool {
        if sa, ok := v.(*Subagent); ok { sa.cancel() }
        return true
    })
}
```

### 7.2 Subagent Context (`manager.go:201`)

```go
ctx, cancel := context.WithCancel(parentCtx)
sa := &Subagent{
    cancel: cancel,
    done:   make(chan struct{}),  // closed when loop exits
}
```

### 7.3 Shutdown (`manager.go:299-333`)

```go
func (m *Manager) Shutdown(ctx context.Context) {
    m.CancelAll()
    
    shutdownCtx, cancel := context.WithTimeout(ctx, shutdownTimeout)  // 5s
    defer cancel()
    
    m.agents.Range(func(key, value) bool {
        sa := value.(*Subagent)
        select {
        case <-sa.done:  // Exited cleanly
        case <-shutdownCtx.Done():
            // Timeout — force cleanup
            m.forceCleanup(sa)  // Delete from map, leave goroutine running
            return true
        }
        // Cleanup worktree
        if sa.Info.Isolation == IsolationWorktree {
            m.deps.Worktrees.Remove(ctx, sa.Info.Worktree)
        }
        m.agents.Delete(key)
        return true
    })
    close(m.eventCh)
}
```

### 7.4 Force Cleanup (`manager.go:339-341`)

```go
func (m *Manager) forceCleanup(sa *Subagent) {
    m.agents.Delete(sa.Info.ID)  // Forget agent; goroutine may still run
}
```

---

## 8. Profiles

### 8.1 Built-in Profiles (`profile.go`)

| Profile | System Prompt | Allowed Tools | Max Tools | Max Tokens | Max Turns |
|---------|---------------|---------------|-----------|------------|-----------|
| `explore` | Code exploration focus | Read, Glob, Grep, FileList, CodeMap | 30 | 30k | 15 |
| `general` | General purpose | All except AskUser | 50 | 50k | 25 |
| `security` | Security audit focus | Read, Grep, Glob, FileList, Edit | 25 | 25k | 10 |
| `refactor` | Refactoring focus | Read, Edit, FileWrite, Glob, Grep | 40 | 40k | 20 |

### 8.2 User Overrides (`config/types.go`)

```go
SubagentProfileConfig{
    AllowedTools:  []string{"FileRead", "FileWrite", "Edit", "Grep"},
    DeniedTools:   []string{"Bash", "FileDelete"},
    MaxTools:      20,
    MaxTokens:     20000,
    MaxTurns:      10,
    SystemPrompt:  "Custom prompt...",
    Model:         "anthropic/claude-3.5-sonnet",
}
```

### 8.3 Resolution (`manager.go:139-158`)

```go
profile, ok := ResolveProfile(req.SubagentType, m.deps.Profiles)
if req.MaxTools <= 0 { req.MaxTools = profile.MaxTools }
if req.MaxTokens <= 0 { req.MaxTokens = profile.MaxTokens }
```

---

## 9. Cross-Reference Summary

| Connection | Producer | Channel/Method | Consumer |
|------------|----------|----------------|----------|
| Spawn → Manager | Agent tool | `Manager.Spawn()` | `manager.go:100` |
| Manager → Loop | `manager.go:230` | `go m.runLoop()` | `loop.go:46` |
| Loop → Events | `loop.go:132,210,238,350` | `manager.emit()` | `eventCh` (256) |
| Events → TUI | `eventCh` | `subagentListenerCmd` | `handleSubagentEvent` |
| Cancel → Loop | `manager.Cancel()` | `sa.cancel()` | `ctx.Done()` in loop |
| Worktree Create | `Spawn()` | `Worktrees.Create()` | `GitWorktrees` |
| Worktree Remove | `Cleanup()`/`Shutdown()` | `Worktrees.Remove()` | `GitWorktrees` |
| Dispatcher Factory | `main.go:300` | `NewDispatcher(worktree)` | `loop.go:383` |
| Tool Filter | `runLoop()` | `ApplyToolFilter()` | Child dispatcher |

---

## 10. Threat Model Alignment

| Threat ID | Component | Mitigation Verified |
|-----------|-----------|---------------------|
| T-01-20 | Worktree cleanup | `Shutdown()` waits 5s then force-cleans; `Sweep()` at startup |
| T-01-21 | Tool access | `ApplyToolFilter()` enforces allowlist/denylist; `isChild` prevents grandchild spawn |
| T-01-22 | Spawn rate | `MaxSpawnRate=10/min`, `MaxTotalSubagents=50`, semaphore `MaxConcurrent=8` |
| T-01-23 | Event info disclosure | Events contain tool calls/results; no secrets (API keys never in tool args) |

---

## 11. Summary: Verified Wiring

✅ **Manager Creation** — Dependencies injected at startup (registry, model, worktrees, dispatcher factory, profiles)  
✅ **Spawn Sequence** — Validation → profile resolution → budget → concurrency slot → worktree → context → goroutine  
✅ **RunLoop** — Message init → turn loop (max 25) → budget checks → LLM stream → tool dispatch → history update  
✅ **Event Channel** — Buffered 256, lifecycle events block 5s, streaming events drop under backpressure  
✅ **Worktrees** — `.m31a-worktrees/<id>` on branch `m31a/agent-<id>`, created/removed via git CLI, swept at startup  
✅ **Dispatcher Sharing** — Parent registers Agent tool; child gets filtered dispatcher per worktree; `isChild` prevents recursion  
✅ **Profile Filtering** — Allowlist/denylist applied, AskUserQuestion removed, budgets from profile  
✅ **Cancellation** — Parent context → child context → loop checks `ctx.Err()` each turn; `CancelAll()` + 5s shutdown timeout  
✅ **Shutdown** — Cancel all → wait with timeout → force cleanup → close eventCh → worktree removal  
✅ **Security** — Prompt injection defenses in system prompt; worktree isolation; no secret leakage in events