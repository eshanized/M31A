# Tools Dispatcher Wiring Report

**Phase:** 01-wiring-audit  
**Plan:** 01-02  
**Task:** 3 — Tools Dispatcher (18 tools, permissions, rate limits, concurrency, output store)  
**Generated:** 2026-07-10

---

## 1. Tool Registration

### 1.1 Registration Sequence (`internal/tools/defaults.go:60-121`)

```go
func DefaultDispatcher(workDir, backupDir, sessionsDir string, cfg *config.PermissionsConfig, toolsCfg *config.ToolsConfig) (*Dispatcher, error) {
    d := NewDispatcher(cfg)
    // ... output store init, persistent permissions load ...
    
    // 18 tools registered in order:
    d.Register(NewBash(...))           // 1
    d.Register(NewFileRead(...))       // 2
    d.Register(NewFileWrite(...))      // 3
    d.Register(NewEdit(...))           // 4
    d.Register(NewTodoWrite(...))      // 5
    d.Register(NewTodoRead(...))       // 6
    d.Register(NewWebFetch(...))       // 7
    d.Register(NewAskUserQuestion(...))// 8
    d.Register(NewGlob(...))           // 9
    d.Register(NewGrep(...))           // 10
    d.Register(NewFileList(...))       // 11
    d.Register(NewFileDelete(...))     // 12
    d.Register(NewFileMove(...))       // 13
    d.Register(NewCodeMap(...))        // 14
    d.Register(NewCodeComplexity(...)) // 15
    d.Register(NewDevServer(...))      // 16
    d.Register(NewHTTPCheck(...))      // 17
    d.Register(NewWebSearch(...))      // 18
}
```

### 1.2 Tool Registration Table

| # | Tool Name | Risk Level | Constructor | Key Config |
|---|-----------|------------|-------------|------------|
| 1 | `Bash` | Dangerous | `NewBash(workDir, timeout, blockedCmds, obfuscation)` | `BashMaxTimeoutSecs`, `AdditionalBlockedCommands` |
| 2 | `FileRead` | Safe | `NewFileRead(workDir)` | `MaxGlobResults`, `SkipDirs` |
| 3 | `FileWrite` | Destructive | `NewFileWrite(workDir, backupDir)` | `MaxBackupsPerFile` |
| 4 | `Edit` | Destructive | `NewEdit(workDir, backupDir)` | `FuzzyThreshold`, `MinLinesForFuzzy` |
| 5 | `TodoWrite` | Safe | `NewTodoWrite(sessionsDir, "")` | — |
| 6 | `TodoRead` | Safe | `NewTodoRead(sessionsDir, "")` | — |
| 7 | `WebFetch` | Safe | `NewWebFetch(sessionsDir, false, retries, delay)` | `WebfetchMaxRetries`, `WebfetchRetryDelayMs` |
| 8 | `AskUser` | Safe | `NewAskUserQuestion(reqCh, respCh, pendingQ)` | — |
| 9 | `Glob` | Safe | `NewGlob(workDir)` | `MaxGlobResults` |
| 10 | `Grep` | Safe | `NewGrep(workDir)` | `MaxGrepResults` |
| 11 | `FileList` | Safe | `NewFileList(workDir)` | `SkipDirs` |
| 12 | `FileDelete` | Destructive | `NewFileDelete(workDir, backupDir)` | `MaxBackupsPerFile` |
| 13 | `FileMove` | Destructive | `NewFileMove(workDir, backupDir)` | `MaxBackupsPerFile` |
| 14 | `CodeMap` | Safe | `NewCodeMap(workDir)` | — |
| 15 | `CodeComplexity` | Safe | `NewCodeComplexity(workDir, nil)` | — |
| 16 | `DevServer` | Dangerous | `NewDevServer(workDir)` | `MaxConcurrent` |
| 17 | `HTTPCheck` | Safe | `NewHTTPCheck()` | — |
| 18 | `WebSearch` | Safe | `NewWebSearch(baseURL)` | `WebSearchBaseURL`, `WebSearchEnabled` |

### 1.3 Interface Compliance (`internal/tools/interface.go`)

All tools implement `types.Tool` (compile-time checked via `types.Tool` interface):
```go
type Tool interface {
    Name() string
    Description() string
    RiskLevel() types.RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
    ParameterSchema() string  // JSON Schema (optional, via SchemaProvider)
}
```

**Verification:** Each tool's constructor registers the concrete type; `Dispatcher.Register()` stores `types.Tool` interface — Go enforces implementation.

---

## 2. Permission System

### 2.1 Permission Decision Flow (`dispatcher.go:426-456`)

```
ensurePermission(ctx, call, tool, input)
    │
    ├─ checkPermission(toolName, input) → (allowed, permContext, ruleError)
    │       │
    │       ├─ 1. Config rules (cfg.Permissions.Rules) — exact tool match, then pattern match
    │       ├─ 2. Agent profile defaults (cfg.Permissions.Agents[active].Rules)
    │       ├─ 3. Persistent permissions (loaded from .m31a/permissions.json)
    │       └─ 4. Risk-level fallback: Dangerous/Destructive → "ask"
    │
    ├─ If allowed by rule → return nil (proceed)
    │
    ├─ If rule action = "ask" → askPermission() / askPermissionWithAgentDefault()
    │
    ├─ Check batch approval → checkBatchApproval(toolName, risk)
    │
    ├─ If risk >= Dangerous & no rule match → askPermissionFallback()
    │
    └─ Return error if denied / prompt failed
```

### 2.2 Permission Rule Structure (`config/types.go`)

```go
type PermissionRule struct {
    Tool    string  // Exact tool name or pattern
    Pattern string  // Regex for tool args (e.g., "rm -rf *")
    Action  string  // "allow" | "deny" | "ask"
}
```

### 2.3 Permission Sources (Priority Order)

| Priority | Source | Storage | Scope |
|----------|--------|---------|-------|
| 1 | Config rules (`cfg.Permissions.Rules`) | `config.toml` | Global/project |
| 2 | Agent profile defaults (`cfg.Permissions.Agents`) | `config.toml` | Per-agent-type |
| 3 | Persistent permissions | `.m31a/permissions.json` | Project, user-granted |
| 4 | Batch approvals | In-memory (`dispatcher.batchApprovals`) | Task-scoped |
| 5 | Risk-level defaults | Hardcoded | Fallback |

### 2.4 Batch Approvals (`dispatcher.go:41-42, 173, 212`)

```go
batchApprovals map[string]BatchApproval  // key: "toolName:riskLevel"

func (d *Dispatcher) checkBatchApproval(toolName string, risk RiskLevel) bool {
    d.batchMu.RLock()
    defer d.batchMu.RUnlock()
    key := toolName + ":" + string(risk)
    _, ok := d.batchApprovals[key]
    return ok
}

// Revoked on phase transition (phase_coordinator.go:73-75)
func (d *Dispatcher) RevokeBatchApprovals() {
    d.batchMu.Lock()
    d.batchApprovals = make(map[string]BatchApproval)
    d.batchMu.Unlock()
}
```

---

## 3. Rate Limiting

### 3.1 Token Bucket Implementation (`dispatcher.go:46-53, 87-132`)

```go
// Normal tools
rateTokens:     make(chan struct{}, ToolRateLimitBurst)     // default 20
rateTicker:     time.NewTicker(time.Second / ToolRateLimitPerSec)  // default 10/sec

// Dangerous/Destructive tools
dangerousRateTokens: make(chan struct{}, DangerousRateLimitBurst)  // default 5
dangerousRateTicker: time.NewTicker(time.Second / DangerousRateLimitPerSec) // default 2/sec
```

**Config mapping:** `config/loader.go:145-148` → `ToolsConfig`

| Parameter | Config Field | Default |
|-----------|--------------|---------|
| Normal burst | `Tools.RateLimitBurst` | 20 |
| Normal rate | `Tools.RateLimitPerSec` | 10 |
| Dangerous burst | `Tools.DangerousRateLimitBurst` | 5 |
| Dangerous rate | `Tools.DangerousRateLimitPerSec` | 2 |

### 3.2 Execution Flow Rate Check (`dispatcher.go:225-250`)

```go
// 1. Concurrency semaphore (acquired FIRST)
select {
case d.concurrencySem <- struct{}{}:
    defer func() { <-d.concurrencySem }()
case <-ctx.Done():
    return ctx.Err()
}

// 2. Normal rate limit
select {
case <-d.rateTokens:
case <-ctx.Done():
    return ctx.Err()
}

// 3. Per-risk-level rate limit (M6)
risk := tool.RiskLevel()
if riskLevelValue(risk) >= riskLevelValue(types.RiskDangerous) {
    select {
    case <-d.dangerousRateTokens:
    case <-ctx.Done():
        return ctx.Err()
    }
}
```

---

## 4. Concurrency Control

### 4.1 Semaphore (`dispatcher.go:54-55, 216-221`)

```go
concurrencySem: make(chan struct{}, MaxConcurrentTools)  // default 8
```

**Config:** `Tools.MaxConcurrent` (default 8, max 32 validated in `loader.go:660-666`)

### 4.2 Tool Execution Parallelism (`execute.go:398-513`)

```go
maxToolConcurrency := 4  // default
if e.cfg != nil && e.cfg.Tools.MaxToolConcurrency > 0 {
    maxToolConcurrency = e.cfg.Tools.MaxToolConcurrency
}

sem := make(chan struct{}, maxToolConcurrency)
for i, tc := range toolCalls {
    wg.Add(1)
    go func(idx int, call ToolCall) {
        defer wg.Done()
        sem <- struct{}{}  // acquire
        defer func() { <-sem }()  // release
        // ... execute tool ...
    }(i, tc)
}
wg.Wait()
```

**Note:** Two concurrency layers:
1. **Dispatcher-level** (global): `MaxConcurrentTools` (default 8) — limits total concurrent tool executions across all tasks
2. **Task-level** (per-task): `MaxToolConcurrency` (default 4) — limits parallel tool calls within a single LLM response

---

## 5. Output Store

### 5.1 Bounding Configuration (`dispatcher.go:58-59, 304-310`)

```go
outputStore *OutputStore

// Config (config/loader.go:142-143, 150-151)
OutputMaxLines:       types.DefaultOutputMaxLines   // 1000
OutputMaxBytes:       types.DefaultOutputMaxBytes   // 100KB
OutputRetentionDays:  7
```

### 5.2 Bound() on Results (`dispatcher.go:304-310`)

```go
if d.outputStore != nil && err == nil {
    bounded, _, wasBounded := d.outputStore.Bound(output)
    if wasBounded {
        output = bounded
        truncated = true
    }
}
```

**Implementation:** `internal/tools/output_store.go` — truncates to max lines/bytes, stores full output on disk with TTL cleanup.

### 5.3 Cleanup on Startup (`defaults.go:30`)

```go
_, _ = store.Cleanup(OutputRetentionDays * 24 * time.Hour)
```

---

## 6. Metrics Collection

### 6.1 Collector Attachment (`dispatcher.go:188-202, 296-298`)

```go
func (d *Dispatcher) SetCollector(c *metrics.Collector) {
    d.mu.Lock()
    d.collector = c
    d.mu.Unlock()
    // Propagate to tools that support it (e.g., Edit for strategy tracking)
    for _, tool := range d.tools {
        if edit, ok := tool.(*Edit); ok {
            edit.SetCollector(c)
        }
    }
}

// In Execute():
if d.collector != nil {
    d.collector.RecordToolCall(call.Name, err == nil, elapsed)
}
```

### 6.2 Metrics Recorded (`pkg/metrics/collector.go`)

| Metric | Method | Fields |
|--------|--------|--------|
| Tool calls | `RecordToolCall(name, success, durationMs)` | Count, success rate, latency |
| LLM usage | `RecordLLMInteraction(phase, usage, cost)` | Tokens, cost per phase |
| Phase duration | `RecordPhaseDuration(phase, ms, success)` | Duration, outcome |
| Heal events | `RecordHealTrigger/Outcome(phase, success)` | Self-heal statistics |
| Bisect events | `RecordBisectTrigger/Outcome(phase, success)` | Bisect statistics |

---

## 7. AskUser Tool & Channel Routing

### 7.1 Channel Structure (`dispatcher.go:29-31, 348-354`)

```go
questionReqCh:    chan QuestionRequest      // buffered (QuestionChannelBuffer)
questionRespCh:   chan QuestionResponse     // buffered
pendingQuestions: sync.Map  // map[int64]chan QuestionResponse
```

### 7.2 Per-Request Routing (`dispatcher.go:458-480`)

```go
// Dispatcher.Execute() → AskUser.Execute() → sends QuestionRequest to questionReqCh
// TUI questionListenerCmd reads from questionReqCh → emits QuestionRequestMsg
// User answers → TUI sends QuestionResponse → Dispatcher.RespondQuestion(requestID, answer)
// RespondQuestion routes to per-request channel (pendingQuestions[requestID])
// Falls back to shared questionRespCh if no per-request channel
```

**Fixes H-2:** Prevents cross-caller response routing when multiple AskUser calls in flight.

---

## 8. Subagent Dispatcher Factory

### 8.1 Factory Creation (`main.go:300`)

```go
dispatcherFactory := tools.NewDispatcherFactory(
    backupDir, backupDir, 
    &cfg.Permissions, &cfg.Tools, 
    nil,  // output store = nil for subagents
    cfg.Agents.Profiles,
)
```

### 8.2 Profile-Based Filtering (`internal/tools/subagent/manager.go:390-391`)

```go
dispatcher, err := m.deps.NewDispatcher(sa.Info.Worktree)
// ApplyToolFilter removes tools not in profile.AllowedTools / adds profile.DeniedTools
ApplyToolFilter(dispatcher, sa.Profile)
```

**Profile config:** `config/types.go` — `SubagentProfileConfig{AllowedTools, DeniedTools, MaxTools, MaxTokens, MaxTurns, SystemPrompt, Model}`

### 8.3 Child Dispatcher Restrictions

| Restriction | Implementation |
|-------------|----------------|
| Cannot spawn grandchildren | `Agent` tool registered with `isChild=true` on parent dispatcher only |
| Tool allowlist/denylist | `ApplyToolFilter()` removes unallowed tools from child dispatcher |
| Separate output store | `nil` passed to factory — subagents don't persist tool output |
| Separate permissions | Child dispatcher gets own permission config from profile |

---

## 9. TodoWrite Callback → Sidebar

### 9.1 Wiring (`app.go:502-537`)

```go
func (m *AppState) wireTodoWriteCallback() {
    m.dispatcher.SetTodoWriteCallback(func(items []tools.TodoItem) {
        sidebarItems := make([]SidebarTodoItem, len(items))
        for i, item := range items {
            sidebarItems[i] = SidebarTodoItem{
                Content:  item.Content,
                Status:   item.Status,
                Priority: item.Priority,
                Source:   "llm",
            }
        }
        msg := SidebarTodoUpdateMsg{Items: sidebarItems}
        // Bounded retry to emitterCh (max 3 attempts, 10ms backoff)
        for attempt := 0; attempt <= maxRetries; attempt++ {
            select {
            case m.emitterCh <- msg: return
            default:
                if attempt < maxRetries { time.Sleep(retryBackoff) }
            }
        }
        globalDropCounter.Add(1)
    })
}
```

### 9.2 Sync from TaskRunner (`execute.go:144-146, 171-173`)

```go
// After each execution group
if syncErr := e.dispatcher.SyncTodoFromTasks(runner.Tasks()); syncErr != nil {
    e.logger.Warn("todo sync after group failed", "error", syncErr)
}
// Final sync
if syncErr := e.dispatcher.SyncTodoFromTasks(updatedTasks); syncErr != nil {
    e.logger.Warn("final todo sync failed", "error", syncErr)
}
```

---

## 10. Dispatcher Stop & Cleanup

### 10.1 Stop() (`dispatcher.go:396-412`)

```go
func (d *Dispatcher) Stop() {
    d.stopOnce.Do(func() {
        close(d.rateDone)
        d.rateTicker.Stop()
        close(d.dangerousRateDone)
        d.dangerousRateTicker.Stop()
        // Drain stale responses from shared channels
        for {
            select {
            case <-d.responseCh:
            default:
                return
            }
        }
    })
}
```

**Called from:** `AppState.Shutdown()` (`app.go:192-194`)

---

## 11. Execution Flow Summary

```
Engine.executeTaskWithTools()
    └─ buildExecuteContext() → messages
    └─ streamLLMWithTools() → content, toolCalls
    └─ For each toolCall (parallel, bounded by MaxToolConcurrency):
         └─ Dispatcher.Execute(ctx, call)
              ├─ Concurrency semaphore (MaxConcurrentTools)
              ├─ Rate limit: normal bucket (RateLimitBurst/PerSec)
              ├─ Rate limit: dangerous bucket (if RiskLevel >= Dangerous)
              ├─ Tool lookup (dispatcher.tools map)
              ├─ Permission check:
              │     ├─ Config rules
              │     ├─ Agent profile defaults
              │     ├─ Persistent permissions
              │     ├─ Batch approvals
              │     └─ Risk fallback → AskUser
              ├─ Input validation (JSON, param normalization)
              ├─ tool.Execute(ctx, input)
              ├─ Output bounding (OutputStore.Bound)
              ├─ Metrics: RecordToolCall(name, success, duration)
              └─ Return ToolResult
    └─ Feed ALL tool results (success + errors) back to LLM
    └─ Quality gate check
    └─ Git commit (scoped to task.Files)
```

---

## 12. Summary: Verified Wiring

✅ **18 tools registered** in `defaults.go` with correct risk levels and config  
✅ **All implement `types.Tool`** interface (compile-time verified)  
✅ **Permission decision tree** — config rules → agent defaults → persistent → batch → risk fallback  
✅ **Batch approvals** task-scoped, revoked on phase transitions  
✅ **Rate limiting** — two token buckets (normal + dangerous) with config-driven refill rates  
✅ **Concurrency** — global semaphore (8) + per-task parallelism (4)  
✅ **Output store** — bounds output (1000 lines/100KB), TTL cleanup (7 days)  
✅ **Metrics** — every tool call recorded with success, duration; propagated to Edit tool  
✅ **AskUser routing** — per-request channels via sync.Map prevents cross-talk  
✅ **Subagent factory** — creates profile-filtered dispatchers, no Agent tool in children  
✅ **TodoWrite → Sidebar** — callback wired, synced from TaskRunner after each group  
✅ **Stop()** — sync.Once, stops tickers, drains channels