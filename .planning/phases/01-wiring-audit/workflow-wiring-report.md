# Workflow Engine Wiring Report

**Phase:** 01-wiring-audit  
**Plan:** 01-02  
**Task:** 1 — Workflow Engine (7 phases, transitions, events, checkpoints)  
**Generated:** 2026-07-10

---

## 1. Phase Registration & State Machine

### 1.1 Valid Transitions Matrix

| From Phase | Valid To Phases | Source |
|------------|-----------------|--------|
| `Idle` | `Initialize` | `state_machine.go:29` |
| `Initialize` | `Discuss`, `Execute`, `Idle` | `state_machine.go:30` |
| `Discuss` | `Plan`, `Execute`, `Idle` | `state_machine.go:31` |
| `Plan` | `Execute`, `Plan`, `Discuss`, `Idle` | `state_machine.go:32` |
| `Execute` | `Verify`, `Ship`, `Idle` | `state_machine.go:33` |
| `Verify` | `Runtime`, `Ship`, `Execute`, `Idle` | `state_machine.go:34` |
| `Runtime` | `Ship`, `Execute`, `Idle` | `state_machine.go:35` |
| `Ship` | `Idle` | `state_machine.go:36` |

### 1.2 State Machine Implementation (`internal/workflow/state_machine.go`)

```go
// NewStateMachine initializes at PhaseIdle with the transition graph
func NewStateMachine() *StateMachine {
    return &StateMachine{
        currentPhase: m31types.PhaseIdle,
        history:      []m31types.WorkflowPhase{m31types.PhaseIdle},
        validTransitions: map[m31types.WorkflowPhase][]m31types.WorkflowPhase{...}
    }
}

// Transition() validates and applies phase changes
// - Mutex-protected (state_machine.go:60)
// - Checks from/current phase matches (state_machine.go:63-65)
// - Validates against validTransitions map (state_machine.go:67-81)
// - Plan↔Discuss oscillation guard (max 3 cycles) (state_machine.go:84-93)
// - Appends to history (state_machine.go:96)
```

### 1.3 Phase Registration in Engine

**Location:** `internal/workflow/engine.go:696` (switch statement in `RunPhase`)

| Phase Constant | Handler Method | Entry Point |
|----------------|----------------|-------------|
| `PhaseInitialize` | `runInitialize` | `engine.go:697` |
| `PhaseDiscuss` | `runDiscuss` | `engine.go:699` |
| `PhasePlan` | `runPlan` | `engine.go:701` |
| `PhaseExecute` | `runExecute` | `engine.go:703` |
| `PhaseVerify` | `runVerify` | `engine.go:705` |
| `PhaseRuntime` | `runRuntime` | `engine.go:707` |
| `PhaseShip` | `runShip` | `engine.go:709` |

---

## 2. Transition Validation & PhaseCoordinator

### 2.1 Engine.Transition() → PhaseCoordinator.CoordinateTransition()

**File:** `internal/workflow/engine.go:732`

```go
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
    e.state.transitionMu.Lock()
    defer e.state.transitionMu.Unlock()

    // Delegate to StateMachine for validation
    if err := e.stateMachine.Transition(from, to); err != nil {
        return err
    }

    // Delegate side effects to PhaseCoordinator
    return e.phaseCoordinator.CoordinateTransition(ctx, from, to)
}
```

### 2.2 PhaseCoordinator Responsibilities (`internal/workflow/phase_coordinator.go`)

| Method | Purpose | Key Actions |
|--------|---------|-------------|
| `PrePhaseSetup()` | Pre-execution setup | Budget check, batch approval revocation, proactive compaction (`phase_coordinator.go:61-88`) |
| `PostPhaseExecution()` | Metrics recording | Duration, cost, LLM usage (`phase_coordinator.go:91-111`) |
| `CoordinateTransition()` | Transition side effects | Emit start msg, save checkpoint, write STATE.md, emit complete msg (`phase_coordinator.go:115-157`) |

### 2.3 Checkpoint Save/Load Sequence

**Save (Engine.SaveCheckpointData):** `engine.go:307-328`
```go
cp := &CheckpointData{
    Phase:       e.stateMachine.CurrentPhase(),
    Goal:        goal,
    PlanVersion: e.state.planVersion,
    Decisions:   e.SnapshotDecisions(),  // decision.Logger.Flush()
    Timestamp:   time.Now(),
}
e.state.checkpointData = cp
e.sessionMgr.SaveCheckpoint(e.sessionID, sessCheckpoint)  // → pkg/session/checkpoint.go:30
```

**Load (Engine.LoadCheckpointData):** `engine.go:332-361`
```go
// 1. Load from disk if no in-memory checkpoint
checkpoints, _ := e.sessionMgr.LoadCheckpoints(e.sessionID)
cp := checkpoints[0]  // newest-first (LoadCheckpoints sorts desc)
// 2. Restore state
e.state.checkpointData = data
e.stateMachine.SetPhase(data.Phase)           // StateMachine.SetPhase()
e.state.planVersion = data.PlanVersion
// 3. Restore decisions to log
for _, d := range data.Decisions { e.state.decisionLog.Log(d) }
```

---

## 3. Event Emission Flow

### 3.1 Event Types Emitted (`internal/workflow/engine.go`)

| Event | Emission Point | Payload |
|-------|----------------|---------|
| `PhaseStarted` | `PhaseTransitionStartMsg` via `PhaseCoordinator.CoordinateTransition:117` | From, To, Context |
| `PhaseCompleted` | `PhaseTransitionCompleteMsg` via `PhaseCoordinator.CoordinateTransition:150` | From, To, Success |
| `PhaseFailed` | Same as above, Success=false | Error message |

### 3.2 Message Routing to TUI

```
Engine.emit() → MsgEmitter.Emit() → narrativeEmitter (intercepts) → narrativeBridge → emitterCh (chan tea.Msg) → TUI Update() via drainEmitterCmd()
```

**Key wiring:**
- `engine.SetMsgEmitter()` called from TUI `initWorkflowEngine()` (`app.go:487`)
- `narrativeEmitter` wraps raw channel, classifies messages (narrative/grouped/hidden/expanded)
- `drainEmitterCmd()` / `drainMultipleCmd()` / `drainAdaptiveCmd()` pull from channel in `Update()`

---

## 4. Context Propagation & Cancellation

### 4.1 Context Flow

```
Engine.RunPhase(ctx, phase, goal)
    ├─ ctx passed to phase handlers (runInitialize, runDiscuss, etc.)
    ├─ ctx passed to LLM streaming: streamLLM(ctx, messages, tools)
    ├─ ctx passed to tool dispatch: dispatcher.Execute(ctx, call)
    ├─ ctx passed to subagent spawn: subagentMgr.Spawn(parentCtx, req)
    └─ ctx.cancel() on Engine.Shutdown() (engine.go:376)
```

### 4.2 Cancellation Points

| Location | Mechanism |
|----------|-----------|
| `engine.go:376` | `e.cancel()` called in `Shutdown()` |
| `engine.go:383` | Wait on `e.done` channel with context timeout |
| `execute.go:101` | Task execution checks `ctx.Err()` before/after |
| `execute.go:253` | `streamLLMWithTools(ctx, ...)` respects context |
| `subagent/manager.go:201` | Subagent context derived from parent `context.WithCancel(parentCtx)` |

---

## 5. Retry Wiring

### 5.1 LLM Stream Retry (`engine.go:1413-1453`)

```go
func (e *Engine) retryChatStream(ctx, req, firstErr) {
    class, reason := retry.ClassifyError(firstErr)  // pkg/retry/classify.go
    if !retry.IsRetryable(class) { return nil, firstErr }

    policy := retry.DefaultPolicy()
    if e.cfg != nil {
        policy = retry.ConfiguredPolicy(
            e.cfg.Features.RetryMaxAttempts,      // default 3
            e.cfg.Features.RetryBaseDelayMs,      // default 1000
            e.cfg.Features.RetryMaxDelayMs,       // default 30000
            e.cfg.Features.RetryBackoffMultiplier, // default 2.0
        )
    }
    // Exponential backoff with jitter via policy.Delay(attempt, nil)
}
```

**Config source:** `config/loader.go:121-124` (FeaturesConfig)

---

## 6. Cost Tracking & Budget Guardrails

### 6.1 CostTracker (`engine.go:114`, `CostTracker` struct)

```go
// Checked in RunPhase() before each phase (engine.go:658-667)
if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
    cost := e.costTracker.TotalCost()
    if cost >= e.cfg.Features.BudgetLimitUSD {
        return PhaseResult{Success: false, Error: "budget limit exceeded"}, err
    }
}

// Updated in PhaseCoordinator.PostPhaseExecution() (phase_coordinator.go:100-102)
if result.Cost > 0 { pc.costTracker.RecordCost(result.Cost) }
```

---

## 7. Compaction Wiring

### 7.1 Proactive Compaction Triggers

| Trigger | Location | Condition |
|---------|----------|-----------|
| Phase transition | `phase_coordinator.go:77-80` | `compactFn(messages)` called in `PrePhaseSetup` |
| Tool calls during Execute | `execute.go:539-543` | `toolCallsSinceLastCompact >= cfg.Compaction.ToolCallsThreshold` (default 15) |

### 7.2 Compaction Execution

```go
// engine.go:972-1029 proactiveCompactCheck()
if e.compactor != nil && e.cfg.Compaction.Proactive {
    threshold = contextLength * PhaseTransitionPct / 100  // default 60%
    if estimated > threshold {
        result := e.compactor.Compact(ctx, messages, provider, model)
        if result.Compacted {
            e.emit(CompactionCompleteMsg{TokensBefore, TokensAfter, MessagesRemoved})
            return e.compactedMessages(messages, result.Summary)
        }
    }
}
```

---

## 8. CodeIntel Lifecycle

### 8.1 Lazy Build & Invalidation

```go
// engine.go:1168-1184 getCodeIntel()
e.codeIntelMu.Lock()
if e.codeIntelBuilt { return e.codeIntel }
e.codeIntelBuilt = true
idx := codeintel.NewIndexer(e.workDir)
idx.Build(buildCtx)  // 30s timeout
e.codeIntel = idx

// Invalidate between Execute groups (execute.go:124-129)
if groupToolCalls > 0 {
    e.codeIntelMu.Lock()
    e.codeIntel = nil
    e.codeIntelBuilt = false
    e.codeIntelMu.Unlock()
}
```

---

## 9. Decision Log

### 9.1 Decision Logger Integration

```go
// engine.go:278-298
func (e *Engine) LogDecision(r decision.DecisionReceipt) {
    if e.state.decisionLog != nil {
        e.state.decisionLog.Log(r)
    }
}

// Flush on Close() (engine.go:364-368)
func (e *Engine) Close() {
    if e.state.decisionLog != nil {
        e.state.decisionLog.Close()
    }
}
```

**Checkpoint includes decisions:** `engine.go:308-313` — `e.SnapshotDecisions()` captured in `CheckpointData.Decisions`

---

## 10. Cross-References to Other Systems

| Connection | Engine Side | Consumer Side |
|------------|-------------|---------------|
| Tools → Execute | `engine.dispatcher.Execute()` | `internal/tools/dispatcher.go:212` |
| Provider → Phases | `e.provider.ChatCompletionStream()` | `internal/provider/*/client.go` |
| Session → Checkpoints | `e.sessionMgr.SaveCheckpoint()` | `pkg/session/checkpoint.go:30` |
| Session → State | `e.sessionMgr.SaveState()` | `pkg/session/manager.go:301` |
| Ledger → Ship | `e.ledger.Append()` | `pkg/ledger/ledger.go:128` |
| Rollback → Ship | `e.git.ResetHard()` | `pkg/rollback/rollback.go:147` |
| Metrics → All phases | `e.collector.RecordPhaseDuration()` | `pkg/metrics/collector.go` |

---

## 11. Call Graph: PhaseCoordinator

```
Engine.RunPhase()
  └─ PhaseCoordinator.PrePhaseSetup()
       ├─ Budget check (costTracker)
       ├─ Revoke batch approvals (dispatcher)
       ├─ Proactive compaction (compactor)
       └─ Record phase transition (collector)
  ├─ Engine.run<Phase>()
  └─ PhaseCoordinator.PostPhaseExecution()
       ├─ Record duration (collector)
       ├─ Record cost (costTracker)
       └─ Record LLM usage (collector)
  └─ Engine.Transition()
       └─ PhaseCoordinator.CoordinateTransition()
            ├─ Emit PhaseTransitionStartMsg
            ├─ Save checkpoint (sessionMgr)
            ├─ Write STATE.md (sessionMgr)
            └─ Emit PhaseTransitionCompleteMsg
```

---

## 12. Summary: Verified Wiring

✅ **All 7 phases registered** with handlers in `engine.go:696` switch  
✅ **StateMachine.validTransitions** contains all required edges (Idle→Initialize, Initialize→Discuss/Execute, Discuss→Plan/Execute, Plan→Execute/Plan/Discuss, Execute→Verify/Ship, Verify→Runtime/Ship/Execute, Runtime→Ship/Execute, Ship→Idle)  
✅ **Mutex protection** on transitions (`state.transitionMu`)  
✅ **Plan↔Discuss oscillation guard** (max 3 cycles)  
✅ **Events emitted** at phase boundaries via `PhaseCoordinator` → `MsgEmitter` → TUI  
✅ **Checkpoints saved** at every transition with Phase, Goal, PlanVersion, Decisions, Timestamp  
✅ **Context propagation** through all phase handlers, LLM streams, tool dispatch, subagents  
✅ **Retry policy** configurable via `config.Features.Retry*`  
✅ **Budget guardrail** checked before each phase  
✅ **Proactive compaction** at phase transitions and during Execute (tool call threshold)  
✅ **CodeIntel** lazy-built per session, invalidated between Execute groups  
✅ **Decision log** flushed on engine Close(), included in checkpoints