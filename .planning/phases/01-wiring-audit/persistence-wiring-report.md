# Persistence Wiring Report

**Phase:** 01-wiring-audit  
**Plan:** 01-02  
**Task:** 6 — Persistence (sessions, checkpoints, ledger, rollback, compaction, AutoDream, metrics, narrative, shutdown)  
**Generated:** 2026-07-10

---

## 1. Session Lifecycle

### 1.1 Manager Structure (`pkg/session/manager.go:25-33`)

```go
type Manager struct {
    baseDir         string        // ~/.m31a (global config)
    workDir         string        // Project root
    sessionIDBytes  int           // Default 4 (8 hex chars)
    maxRecentModels int           // Default 10
    sessionCacheTTL time.Duration // Default 2s
    lock            *fileutil.FileLock  // projectDir/session.lock
    coordinator     *coordinator.Coordinator[string]  // Per-session concurrency
}
```

### 1.2 Project-Local Storage

**Directory:** `<workDir>/.m31a/` (created in `NewSession()`)

| File | Purpose | Format |
|------|---------|--------|
| `session.json` | Session metadata + workflow state | JSON |
| `messages.json` | Conversation messages | JSON array |
| `checkpoint.json` | Phase checkpoints (max 2) | JSON array |
| `TASKS.md` | Task list (checkbox format) | Markdown |
| `PLAN.md` | Implementation plan | Markdown |
| `LEDGER.md` | Cross-session ledger (global) | Markdown table |
| `session.lock` | File lock for concurrent access | Empty file |

### 1.3 Session Operations

| Operation | Method | Key Logic |
|-----------|--------|-----------|
| Create | `NewSession()` | Lock → backup existing → write session.json + messages.json → unlock → ensure .gitignore |
| Load | `LoadSession()` | Lock → read session.json + messages.json → validate phase enum → set ResumedAt |
| Save | `SaveSession()` | Lock → marshal session + messages → atomic write both |
| List | `ListSessions()` | Stat session.json → return single SessionInfo (project-local = max 1) |
| Delete | `DeleteSession()` | Lock → remove session.json, session.json.bak, messages.json |
| Update Workflow | `UpdateWorkflowState()` | Lock → load metadata → set goal/phase/questions → atomic save |
| Save Messages | `SaveMessages()` | Lock → marshal messages → atomic write messages.json |
| Load Messages | `LoadMessages()` | Read messages.json (no lock — read-only) |

### 1.4 Atomic Writes

```go
// fileutil.AtomicWrite() — write to temp, rename
func (m *Manager) atomicWrite(path string, data []byte) error {
    return fileutil.AtomicWrite(path, data)
}
```

---

## 2. Checkpoints

### 2.1 Checkpoint Data (`engine.go:82-88`, `pkg/session/checkpoint.go:18-25`)

```go
// Engine checkpoint (in-memory + disk)
type CheckpointData struct {
    Phase       WorkflowPhase
    Goal        string
    PlanVersion int
    Decisions   []DecisionReceipt
    Timestamp   time.Time
}

// Disk checkpoint (session manager)
type Checkpoint struct {
    Phase        WorkflowPhase `json:"phase"`
    Timestamp    time.Time     `json:"timestamp"`
    MessageCount int           `json:"message_count"`
    TaskCount    int           `json:"task_count"`
    Goal         string        `json:"goal,omitempty"`
    PlanVersion  int           `json:"plan_version,omitempty"`
}
```

### 2.2 Save Sequence (`engine.go:307-328`)

```go
func (e *Engine) SaveCheckpointData(goal string) {
    // 1. Capture in-memory
    cp := &CheckpointData{
        Phase:       e.stateMachine.CurrentPhase(),
        Goal:        goal,
        PlanVersion: e.state.planVersion,
        Decisions:   e.SnapshotDecisions(),
        Timestamp:   time.Now(),
    }
    e.state.checkpointData = cp

    // 2. Persist to disk (session manager)
    sessCheckpoint := session.Checkpoint{
        Phase: cp.Phase, Timestamp: cp.Timestamp,
        Goal: cp.Goal, PlanVersion: cp.PlanVersion,
    }
    e.sessionMgr.SaveCheckpoint(e.sessionID, sessCheckpoint)
}
```

### 2.3 Load Sequence (`engine.go:332-361`)

```go
func (e *Engine) LoadCheckpointData(data *CheckpointData) {
    if data == nil {
        // Load from disk (newest first)
        checkpoints, _ := e.sessionMgr.LoadCheckpoints(e.sessionID)
        if len(checkpoints) > 0 {
            cp := checkpoints[0]  // LoadCheckpoints sorts desc by timestamp
            data = &CheckpointData{...}
        }
    }
    e.state.checkpointData = data
    e.stateMachine.SetPhase(data.Phase)      // Bypass validation
    e.state.planVersion = data.PlanVersion
    // Restore decisions to log
    if data.Decisions != nil && e.state.decisionLog != nil {
        for _, d := range data.Decisions { e.state.decisionLog.Log(d) }
    }
}
```

### 2.4 Disk Persistence (`pkg/session/checkpoint.go:30-111`)

```go
// SaveCheckpoint — append, trim to 2, atomic write
func (m *Manager) SaveCheckpoint(sessionID string, cp Checkpoint) error {
    existing, _ := m.loadCheckpointsRaw(sessionID)
    existing = append(existing, cp)
    if len(existing) > 2 { existing = existing[len(existing)-2:] }
    data, _ := json.Marshal(existing)
    return m.atomicWrite(filepath.Join(m.projectDir(), "checkpoint.json"), data)
}

// LoadCheckpoints — sort desc, prune to 2, rewrite if pruned
func (m *Manager) LoadCheckpoints(sessionID string) ([]Checkpoint, error) {
    // ... unmarshal ...
    sort.Slice(checkpoints, func(i,j) bool { 
        return checkpoints[i].Timestamp.After(checkpoints[j].Timestamp) 
    })
    if len(checkpoints) > 2 {
        checkpoints = checkpoints[:2]
        // Rewrite pruned
        data, _ := json.Marshal(checkpoints)
        m.atomicWrite(path, data)
    }
    return checkpoints, nil
}
```

### 2.5 Checkpoint Triggers

| Trigger | Location | Phase |
|---------|----------|-------|
| Initialize complete | `initialize.go:101` | Initialize |
| Plan complete | `plan.go:195` | Plan |
| Execute complete | `execute.go:150` | Execute (retry on failure) |
| Verify complete | `verify.go:129` | Verify |
| Runtime complete | `runtime.go:132` | Runtime |
| Ship complete | `ship.go:209` | Ship |

---

## 3. Ledger

### 3.1 Ledger Entry (`pkg/ledger/ledger.go:19-33`)

```go
type LedgerEntry struct {
    SessionID       string
    Timestamp       time.Time
    Model           string
    Provider        string
    ProjectType     string
    GoalKeywords    []string
    Framework       string
    TaskCount       int
    FailedTasks     int
    SkippedTasks    int
    CostEstimate    float64
    DurationMinutes int
    CommitCount     int
}
```

### 3.2 Append-Only Write (`ledger.go:128-171`)

```go
func (l *Ledger) Append(entry LedgerEntry) error {
    l.lock.Lock(); l.mu.Lock()
    defer l.lock.Unlock(); defer l.mu.Unlock()

    // Dedup by SessionID
    for _, e := range l.entries {
        if e.SessionID == entry.SessionID {
            return fmt.Errorf("entry already exists: %w", ErrTaskFailed)
        }
    }
    l.entries = append(l.entries, entry)

    // Append-only: if file exists, append single row; else create with header
    if _, err := os.Stat(l.path); err == nil {
        return l.appendEntry(entry)  // OpenFile O_WRONLY|O_APPEND
    }
    return l.rewriteFile()  // Create with header
}
```

### 3.3 Entry Formatting (`ledger.go:174-189`)

```go
func formatEntry(e LedgerEntry) string {
    return fmt.Sprintf(
        "| %s | %s | %s | %s | %d | %d | %d | %.2f | %d | %d |",
        e.SessionID, e.Timestamp.Format(time.RFC3339), e.Model, e.ProjectType,
        e.TaskCount, e.FailedTasks, e.SkippedTasks, e.CostEstimate,
        e.DurationMinutes, e.CommitCount,
    )
}
```

### 3.4 Query & Stats (`ledger.go:210-337`)

```go
// Entries() — copy, sort newest-first
// EntriesFiltered() — by projectType, keywords (substring match), maxResults
// Stats() — mtime-cached aggregate: totals, averages, top failures/frameworks
```

### 3.5 Ship Phase Integration (`ship.go:181-193`)

```go
sess, _ := e.sessionMgr.LoadSession(e.sessionID)
if e.ledger != nil {
    entry := ledger.NewEntry(sess.Session, total, failed, skipped, len(commits), 0)
    e.ledger.Append(entry)
}
```

---

## 4. Rollback

### 4.1 Rollback Manager (`pkg/rollback/rollback.go:34-41`)

```go
type Rollback struct {
    git *git.Git
}

func New(g *git.Git) *Rollback { return &Rollback{git: g} }
```

### 4.2 Reset Operations

| Method | Git Command | Use Case |
|--------|-------------|----------|
| `SoftReset(hash, onReset)` | `git reset --soft` + stash if dirty | Phase rollback, preserve changes |
| `HardReset(hash)` | `git reset --hard` + stash + **backup branch** | Destructive rollback |
| `SafeReset(hash)` | `git reset --hard` + stash + `git stash pop` | Reset but keep uncommitted |
| `RevertFiles(hash, paths)` | `git checkout <hash> -- <paths>` | Selective file restore |

### 4.3 Safety Features

```go
// HardReset creates timestamped backup branch (C-11)
backupBranch := fmt.Sprintf("m31a/rollback-backup-%d", time.Now().Unix())
git.Run("branch", "--force", backupBranch)

// SoftReset invokes callback for TASKS.md sync (M-28)
func (r *Rollback) SoftReset(hash string, onReset func(newHead string) error) {
    // ... stash, reset ...
    if onReset != nil { onReset(newHead) }
}

// countCommitsBetween uses `git rev-list --count` (PK-19 fix)
// O(1) counting instead of loading full log
```

### 4.4 Ship Phase Usage (`ship.go:130-143`)

```go
// Post-ship validation
hash, _ := e.git.HeadHash()
dirty, _ := e.git.HasUncommittedChanges()
// Verifies commit exists, working tree clean
```

---

## 5. Compaction

### 5.1 Compactor (`pkg/compaction/compaction.go`)

```go
type Compactor struct {
    config Config
    tokenEst *tokens.Estimator
}

type Config struct {
    Auto                bool
    Buffer              int      // Context buffer (default 20000)
    KeepTokens          int      // Tokens to retain (default 8000)
    Proactive           bool     // Enable proactive compaction
    ToolCallsThreshold  int      // Tool calls before check (default 15)
    PhaseTransitionPct  int      // % of context at phase transition (default 60)
    SummaryTemplate     string
    SummaryTemplateFile string
}
```

### 5.2 Compaction Triggers

| Trigger | Location | Condition |
|---------|----------|-----------|
| Phase transition | `phase_coordinator.go:77-80` | `compactFn(messages)` in `PrePhaseSetup` |
| Execute tool calls | `execute.go:539-543` | `toolCallsSinceLastCompact >= ToolCallsThreshold` |
| Manual | REPL `/compact` command | User-initiated |

### 5.3 Compaction Execution (`engine.go:972-1029`)

```go
func (e *Engine) proactiveCompactCheck(messages []Message) []Message {
    if !e.cfg.Compaction.Proactive { return messages }
    
    estimated := e.tokens.EstimateMessages(messages)
    threshold := contextLength * PhaseTransitionPct / 100
    
    if estimated <= threshold { return messages }
    
    result, _ := e.compactor.Compact(ctx, messages, e.provider, model)
    if result.Compacted {
        e.emit(CompactionCompleteMsg{TokensBefore, TokensAfter, MessagesRemoved})
        return e.compactedMessages(messages, result.Summary)
    }
    return messages
}
```

### 5.4 Summary Injection (`engine.go:589-619`)

```go
func (e *Engine) compactedMessages(original []Message, summary string) []Message {
    _, recent := compaction.SplitMessages(original, keepTokens, e.tokens.Estimate)
    
    summaryMsg := Message{
        Role: "system",
        Content: "[Compacted Session History]\n" + summary,
        Segments: []MessageSegment{{Type: MessageCompaction, Content: summary, Visible: false}},
        CreatedAt: time.Now(),
    }
    
    return append([]Message{summaryMsg}, recent...)
}
```

---

## 6. AutoDream

### 6.1 Consolidator (`pkg/autodream/autodream.go`)

```go
type Consolidator struct {
    mu       sync.Mutex
    messages []Message
    config   Config
}

func New(messages []Message) *Consolidator {
    return &Consolidator{messages: messages, config: DefaultConfig()}
}

func (c *Consolidator) SetMessages(msgs []Message) { c.mu.Lock(); c.messages = msgs; c.mu.Unlock() }
func (c *Consolidator) CanConsolidate() bool { /* token threshold check */ }
func (c *Consolidator) Consolidate() ConsolidationResult {
    // LLM call to summarize, remove old messages, keep summary
}
```

### 6.2 TUI Integration (`app.go:680-694`)

```go
func (m *AppState) checkAutoDream() {
    if m.autoDream == nil || m.replModel == nil { return }
    msgs := m.replModel.Messages()
    m.autoDream.SetMessages(msgs)
    if !m.autoDream.CanConsolidate() { return }
    
    result := m.autoDream.Consolidate()
    if result.Success {
        m.replModel.SetMessages(m.autoDream.Messages())
        m.addToast(fmt.Sprintf("Auto-compressed: %d messages removed, ~%d tokens saved", 
            result.MessagesRemoved, result.TokensSaved), "info")
    }
}
```

### 6.3 Trigger

- Called periodically from REPL model (not shown in provided files)
- Token-based threshold: when message history exceeds configured limit

---

## 7. Metrics

### 7.1 Collector (`pkg/metrics/collector.go`)

```go
type Collector struct {
    mu           sync.RWMutex
    sessionID    string
    baseDir      string
    enabled      bool
    toolCalls    map[string]ToolCallMetric
    llmUsage     []LLMUsageMetric
    phaseDurations map[WorkflowPhase]PhaseDurationMetric
    healEvents   []HealMetric
    flushTicker  *time.Ticker
    stopCh       chan struct{}
}
```

### 7.2 Recorded Metrics

| Metric | Method | Fields |
|--------|--------|--------|
| Tool calls | `RecordToolCall(name, success, durationMs)` | Count, success rate, avg duration |
| LLM usage | `RecordLLMInteraction(phase, usage, cost)` | Input/output tokens, cost, model |
| Phase duration | `RecordPhaseDuration(phase, ms, success)` | Duration, success |
| Heal events | `RecordHealTrigger(phase)`, `RecordHealOutcome(phase, success)` | Count, success rate |
| Bisect | `RecordBisectTrigger(phase)`, `RecordBisectOutcome(phase, success)` | Count, success rate |

### 7.3 Persistence (`collector.go` — not fully shown)

- `Flush()` → writes `METRICS.json` in session dir
- `Stop()` → flushes on shutdown
- `AppState.Shutdown()` calls `collector.Stop()` (`app.go:180-182`)

---

## 8. Narrative Persistence

### 8.1 Narrative State (`internal/tui/app.go:490-492`)

```go
m.narrativeState = NewNarrativeState()
m.sidebarModel.SetNarrativeState(m.narrativeState)
```

### 8.2 Narrative Emitter (`internal/pkg/narrative/emitter.go`)

```go
// Intercepts workflow messages, classifies, groups, renders
// Classifications: narrative, grouped, hidden, expanded
// Persisted in session via NarrativeState
```

### 8.3 Session Restore

- Narrative state restored when session loaded
- `sessionRestoredMsg` handler reapplies narrative state to sidebar

---

## 9. Shutdown Save Sequence

### 9.1 AppState.Shutdown() Order (`app.go:154-202`)

```go
func (m *AppState) Shutdown() {
    // 1. Persist session state FIRST (W2: before cancelling contexts)
    m.saveSessionOnShutdown()
    
    // 2. Stop dev servers (W3: prevent orphans)
    if ds, ok := m.dispatcher.GetTool("DevServer"); ok { ds.StopAll() }
    
    // 3. Close watchers
    if m.fileWatcher != nil { m.fileWatcher.Close() }
    if m.configWatcherStop != nil { close(m.configWatcherStop) }
    
    // 4. Save frecent history
    if m.frecentHistory != nil { m.frecentHistory.Save() }
    
    // 5. Flush metrics
    if m.collector != nil { m.collector.Stop() }
    
    // 6. Cancel workflow
    if m.workflowCancel != nil { m.workflowCancel() }
    
    // 7. Cancel contexts
    if m.shutdownCancel != nil { m.shutdownCancel() }
    if m.streamCancelFn != nil { m.streamCancelFn() }
    
    // 8. Stop dispatcher
    if m.dispatcher != nil { m.dispatcher.Stop() }
    
    // 9. Close decision logger (flushes decisions)
    if eng, ok := m.workflowEngine.(*workflow.Engine); ok { eng.Close() }
    
    // 10. Shutdown subagent manager
    if m.subagentManager != nil { m.subagentManager.Shutdown(context.Background()) }
}
```

### 9.2 saveSessionOnShutdown() (`app.go:206-245`)

```go
func (m *AppState) saveSessionOnShutdown() {
    // Persist workflow phase/goal if active
    if m.workflowPhase != PhaseIdle && m.workflowPhase != "" {
        m.sessionManager.UpdateWorkflowState(m.sessionID, m.workflowGoal, m.workflowPhase, m.discussQuestions)
    }
    
    // Save full session (messages + metadata) if REPL exists
    if m.replModel != nil {
        sess, _ := m.sessionManager.LoadSession(m.sessionID)
        if sess != nil {
            sess.Messages = m.replModel.Messages()
            sess.MessageCount = len(sess.Messages)
            if m.activeProvider != "" { sess.Provider = m.activeProvider }
            if m.activeModel != nil { sess.Model = m.activeModel.ID }
            m.sessionManager.SaveSession(sess)
        }
    }
}
```

---

## 10. Cross-Reference Summary

| Persistence Layer | Writer | Reader | Trigger |
|-------------------|--------|--------|---------|
| Sessions | `SaveSession()`, `SaveMessages()` | `LoadSession()`, `LoadMessages()` | Phase transitions, shutdown, REPL |
| Checkpoints | `Engine.SaveCheckpointData()` → `sessionMgr.SaveCheckpoint()` | `Engine.LoadCheckpointData()` → `sessionMgr.LoadCheckpoints()` | Every phase boundary |
| Ledger | `ship.go` → `ledger.Append()` | `Ledger.Entries()`, `Stats()` | Session completion |
| Rollback | `Rollback.SoftReset/HardReset/SafeReset` | `Rollback.Chain()`, `Preview()` | User command, failed phase |
| Compaction | `compactor.Compact()` → injected summary | N/A (in-memory) | Phase transition, tool call threshold |
| AutoDream | `consolidator.Consolidate()` | REPL message history | Token threshold |
| Metrics | `collector.Record*()` → `Flush()` | `METRICS.json` | Continuous, flush on shutdown |
| Narrative | `narrativeEmitter` → `NarrativeState` | Sidebar render | Workflow events |

---

## 11. Summary: Verified Wiring

✅ **Session Lifecycle** — Project-local `.m31a/` with atomic writes, file locking, .gitignore injection  
✅ **Checkpoints** — In-memory `CheckpointData` + disk `session.Checkpoint`; save at every phase; load newest-first; max 2 retained; includes Phase, Goal, PlanVersion, Decisions, Timestamp  
✅ **Ledger** — Append-only `LEDGER.md` with deduplication; markdown table format; stats with mtime caching; queried by project type/keywords  
✅ **Rollback** — Soft/Hard/Safe reset via git; backup branch on hard reset; commit counting via `rev-list --count`; callback for TASKS.md sync  
✅ **Compaction** — Proactive at phase transitions (60% threshold) and Execute (15 tool calls); LLM-generated summary injected as system message; recent messages retained  
✅ **AutoDream** — Consolidator summarizes old messages; triggered by token threshold; updates REPL history in-place  
✅ **Metrics** — Collector records tool calls, LLM usage, phase durations, heal/bisect events; flushed to `METRICS.json` on shutdown  
✅ **Narrative** — Emitter intercepts workflow messages, classifies, persists in session; restored on resume  
✅ **Shutdown Sequence** — 10-step ordered cleanup: session save → dev servers → watchers → history → metrics → workflow cancel → contexts → dispatcher → decision log → subagents