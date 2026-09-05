# Phase 5: TaskGraph IR & Execution Engine - Research

**Researched:** 2026-09-05
**Domain:** Executable TaskGraph IR with validated state machine and wave-based parallel execution
**Confidence:** HIGH

## Summary

Phase 5 implements the executable TaskGraph Intermediate Representation (IR) and its execution engine — the core orchestration layer that transforms planning artifacts into verified, resumable engineering work. The phase addresses six requirements (TASKGRAPH-01 through TASKGRAPH-06) covering: the complete TaskGraph IR schema with all required fields; a validated execution state machine (PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED) with explicit transition validation; failure recovery strategies (RETRY, REPAIR, REPLAN, ESCALATE) with bounded retries and LLM-assisted patch generation; Kahn's algorithm for topological sorting with wave-based parallel execution; checkpoint gates for ONE_WAY_DOOR decisions requiring human approval; and full task state persistence at every transition for exact resume capability.

**Primary recommendation:** Extend the existing `TaskRunner` in `internal/engine/taskrunner/runner.go` and `internal/core/types/planning.go` to implement the complete TaskGraph IR with ExecutionState enum, validated transitions, EventStore integration for durability, checkpoint gates, and recovery strategies. The existing Kahn's algorithm implementation provides the scheduling foundation; the work is primarily extending the data model, adding state machine validation, and wiring EventStore projections for recovery.

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Create new TaskGraphIR type separate from Task/Plan — New type in `internal/core/types/planning.go` with all TASKGRAPH-01 fields (Objective, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]). Task/Plan remain user-facing; IR is internal executable form.
- **D-02:** TaskGraphIR lives in `internal/core/types/planning.go` alongside Plan — Keeps planning-related types together. Plan has `CompileToIR()` method that produces TaskGraphIR from requirements, decisions, tasks.
- **D-03:** Plan.CompileToIR() method on Plan type handles compilation to TaskGraphIR — Plan owns compilation logic. Single method, easy to test. Takes requirements, decisions, tasks and produces fully populated TaskGraphIR.
- **D-04:** Replace TaskStatus with new ExecutionState enum matching requirements exactly: PLANNED, READY, RUNNING, WAITING, VERIFYING, COMPLETED, FAILED — Clean break from legacy TaskStatus. Update all consumers (runner, TUI, events). ExecutionState type owns transition validation.
- **D-05:** ExecutionState type with TransitionTo(next) method that validates and returns error on invalid transitions — State machine logic encapsulated in type. ExecutionState.TransitionTo(VERIFYING) returns error if current != WAITING. Self-documenting, testable.
- **D-06:** FAILED state carries RecoveryAction enum (RETRY, REPAIR, REPLAN, ESCALATE); Runner interprets and transitions accordingly — Decouples state from recovery logic. FAILED is terminal for current attempt; RecoveryAction encodes next step. Runner reads action and transitions.
- **D-07:** Checkpoints as TaskGraphIR.Checkpoints[] with type (ONE_WAY_DOOR/REVIEW); Runner pauses wave, emits CheckpointRequested event, waits for CheckpointApproved event — Checkpoints part of IR. Runner checks before executing wave; emits event via EventStore, blocks until approval event. TUI handles approval UI.
- **D-08:** Hybrid EventStore + in-memory channel for approval flow: EventStore for durability (CheckpointRequested, CheckpointApproved), Runner uses channel for fast resume; TUI writes to both — Follows Phase 4 D-36 (WritePending persists before blocking). EventStore captures decision for audit/replay. In-memory channel avoids polling. TUI writes event + sends MsgEmitter. Runner reads event on startup for recovery.
- **D-09:** Checkpoint per task via TaskGraphIR.Checkpoints[]; each has type, required_approvers, evidence_refs. Config only sets defaults — Maximum flexibility. Each checkpoint in IR specifies exactly what's needed. Config provides defaults for tasks without explicit checkpoints. IR is authoritative.
- **D-10:** RecoveryAction enum on FAILED state (as chosen). Runner reads action: RETRY→reset to READY; REPAIR→apply patch then RETRY; REPLAN→request new TaskGraphIR from Planner; ESCALATE→emit event, pause for human — Direct mapping. FAILED state carries RecoveryAction. Runner interprets and transitions. EventStore records each recovery attempt.
- **D-11:** Task tracks retry_count; RETRY allowed if retry_count < MaxRetries. REPAIR: Runner calls LLM (via provider) with failure context to generate patch; applies patch via tools; then RETRY — Retry count per task. MaxRetries from Runner config. REPAIR uses provider to generate fix from error + code context. Patch applied via FileEdit tool. Then retry.
- **D-12:** REPLAN: Runner emits ReplanRequested event with failure context; Planner agent (Phase 6) creates new TaskGraphIR; Runner swaps IR and restarts from PLANNED. ESCALATE: emits EscalationRequested event; TUI shows S16 Failure/Recovery; human chooses action — Event-driven. REPLAN triggers planner agent. ESCALATE shows S16 screen with RETRY/REPAIR/REPLAN/STOP options. Human decision becomes new RecoveryAction.
- **D-13:** Config: `[execution] max_parallel_tasks = 4` (default), `task_timeout = 30m`, `wave_timeout = 0` (no limit). Runner reads from config.toml layered loading — Follows Phase 1 config pattern. max_parallel_tasks replaces DefaultMaxParallelTasks. task_timeout already exists. wave_timeout adds safety for stuck waves.
- **D-14:** Optional mutex/lock names in TaskGraphIR; tasks with same lock name run sequentially even in same wave — For resources not captured by dependencies (e.g., same file, same DB). Task.LockNames[] = ["db-migrate", "config-write"]. Runner acquires lock before task. More realistic.
- **D-15:** Wave completes when all tasks in group reach terminal state (COMPLETED/FAILED/SKIPPED). Partial failure: failed tasks block dependents (set to SKIPPED), wave continues for independent tasks. Next wave only starts when current wave fully resolved — Current runner.go behavior. Failed task -> dependents SKIPPED. Wave boundary = synchronization point. Simple, predictable.
- **D-16:** Fine-grained events per transition: TaskGraphCreated, TaskGraphValidated, TaskPlanned, TaskReady, TaskRunning, TaskWaiting, TaskVerifying, TaskCompleted, TaskFailed, TaskRetried, TaskRepaired, TaskReplanned, TaskEscalated, CheckpointRequested, CheckpointApproved, CheckpointDenied — One event per state transition. Maximum traceability. EventStore becomes complete audit log. Aligns with Phase 1 event vocabulary.
- **D-17:** Runner calls EventStore.Append() on every ExecutionState transition via TransitionTo(). Event includes task_id, from_state, to_state, timestamp, run_id, attempt_number. Projections rebuild exact state — TransitionTo() emits event. Runner subscribes to its own events for recovery. On resume, replays events to reconstruct TaskGraphIR execution state. Follows Phase 1 pattern.
- **D-18:** TaskGraphIR in EventStore as TaskGraphCreated event. Checkpoint projections every N events (Phase 1 D-13). Resume: load latest checkpoint + replay events since checkpoint — Phase 1 D-13/D-14: checkpoint every N events with full state. Resume = checkpoint + incremental replay. Scales to large TaskGraphs. Most robust.

### the agent's Discretion
- Exact checkpoint interval (N events) and retention count for TaskGraph checkpoints — tunable via config
- Exact task_timeout and wave_timeout defaults — tune based on observed workloads
- LLM prompt template for REPAIR patch generation — implementation detail
- Lock name conventions for Task.LockNames[] — establish pattern
- Output formatting for TUI S09/S10 screens — projection concern

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TASKGRAPH-01 | TaskGraph IR contains Objectives, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[] | TaskGraphIR type design in Architecture Patterns; CompileToIR() method on Plan |
| TASKGRAPH-02 | Execution state machine enforces PLANNED → READY → RUNNING → WAITING → VERIFYING → COMPLETED/FAILED with validated transitions | ExecutionState enum with TransitionTo() validation; state machine diagram |
| TASKGRAPH-03 | Failure states support RETRY (bounded), REPAIR (patch + rerun), REPLAN (new TaskGraph), ESCALATE (human) | RecoveryAction enum on FAILED state; Runner interpretation logic; REPAIR via provider |
| TASKGRAPH-04 | Kahn's algorithm for topological sort; wave-based parallel execution of independent tasks | Existing TaskRunner.Schedule() implementation; ExecuteGroup with semaphore |
| TASKGRAPH-05 | Checkpoint gates block execution until human approval for ONE_WAY_DOOR decisions | Checkpoint struct in TaskGraphIR; Runner checkpoint handling; EventStore + channel hybrid |
| TASKGRAPH-06 | Task state (objective, files, dependencies, acceptance, verification) persisted at each transition for exact resume | EventStore append on every TransitionTo(); projection rebuild; checkpoint every N events |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| TaskGraph IR compilation | Engineering Plane (Plan type) | — | Plan owns compilation logic; produces internal executable IR |
| Execution state machine | Execution Plane (Runner) | Assurance Plane (Verification) | Runner owns state transitions; Verification consumes terminal states |
| Kahn's scheduling / wave execution | Execution Plane (Runner) | — | Runner executes Schedule() and ExecuteGroup() |
| Checkpoint gates | Execution Plane (Runner) | Interaction Plane (TUI S13) | Runner emits events/blocks; TUI renders approval UI |
| Failure recovery (RETRY/REPAIR/REPLAN/ESCALATE) | Execution Plane (Runner) | Intelligence Plane (Provider for REPAIR) | Runner orchestrates; Provider generates patches for REPAIR |
| EventStore persistence | Memory Plane (EventStore) | Execution Plane (Runner) | Runner emits events; EventStore provides durability |
| Task state resume | Memory Plane (EventStore projections) | Execution Plane (Runner) | Projections rebuild state; Runner reinitializes from projection |
| Config (execution section) | Engineering Plane (Config loader) | — | Layered TOML loading follows Phase 1 pattern |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `github.com/google/uuid` | v1.6.0 | UUID generation for TaskGraphIR, tasks, checkpoints, events | Standard Go UUID library; already used in codebase |
| `modernc.org/sqlite` | v1.32.2 | SQLite driver with WAL mode for EventStore | Pure Go, no CGO, WAL support; already used in Phase 1 |
| `github.com/BurntSushi/toml` | v1.4.0 | TOML config parsing for [execution] section | Standard Go TOML library; already used in config loader |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `github.com/fsnotify/fsnotify` | v1.7.0 | Config file watching for hot-reload | Already in config loader; reuse for execution config changes |
| `golang.org/x/sync` | v0.7.0 | errgroup, semaphore for bounded parallelism | Runner already uses semaphore pattern; extend for lock names |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom state machine | `github.com/looplab/fsm` | External dependency; current D-05 design (TransitionTo method on type) is simpler, more testable, zero-dep |
| Redis for checkpoint coordination | In-memory channel + EventStore | Redis adds infrastructure dependency; hybrid approach (D-08) meets requirements |
| Separate workflow engine (Temporal) | Custom Runner | Temporal overkill for single-process agent; Runner already implements Kahn's algorithm |

**Installation:**
```bash
go get github.com/google/uuid@v1.6.0
go get modernc.org/sqlite@v1.32.2
go get github.com/BurntSushi/toml@v1.4.0
go get github.com/fsnotify/fsnotify@v1.7.0
go get golang.org/x/sync@v0.7.0
```

**Version verification:** All libraries verified via `go list -m` against go.mod — versions match existing usage in codebase.

## Package Legitimacy Audit

> **Required** whenever this phase installs external packages. Run the Package Legitimacy Gate protocol before completing this section.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `github.com/google/uuid` | Go module proxy | 10+ yrs | 100M+/wk | github.com/google/uuid | OK | Approved (already in go.mod) |
| `modernc.org/sqlite` | Go module proxy | 5+ yrs | 10M+/wk | github.com/modernc/sqlite | OK | Approved (already in go.mod) |
| `github.com/BurntSushi/toml` | Go module proxy | 10+ yrs | 50M+/wk | github.com/BurntSushi/toml | OK | Approved (already in go.mod) |
| `github.com/fsnotify/fsnotify` | Go module proxy | 10+ yrs | 100M+/wk | github.com/fsnotify/fsnotify | OK | Approved (already in go.mod) |
| `golang.org/x/sync` | Go module proxy | 5+ yrs | 200M+/wk | golang.org/x/sync | OK | Approved (already in go.mod) |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All packages already present in go.mod and verified via `go list -m`. No new external dependencies required for Phase 5.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            TASKGRAPH IR COMPILATION                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  Plan (user-facing)                                                         │
│    │                                                                        │
│    ├── Objective: string                                                    │
│    ├── Requirements: []uuid.UUID                                            │
│    ├── Tasks: []Task (with Dependencies, Files, AcceptanceCriteria)        │
│    ├── RiskLevel: RiskLevel                                                 │
│    └── CompileToIR(requirements, decisions) → TaskGraphIR                  │
│                                                                             │
│         ▼                                                                   │
│                                                                             │
│  TaskGraphIR (internal executable)                                          │
│    ├── ID: uuid.UUID                                                        │
│    ├── PlanID: uuid.UUID                                                    │
│    ├── Objective: string                                                    │
│    ├── Requirements: []uuid.UUID                                            │
│    ├── Constraints: []string                                                │
│    ├── Decisions: []uuid.UUID                                               │
│    ├── Tasks: []TaskIR                                                      │
│    ├── Dependencies: []TaskDependency                                       │
│    ├── Preconditions: []Precondition                                        │
│    ├── AcceptanceCriteria: []string                                         │
│    ├── VerificationSteps: []VerificationStep                                │
│    ├── RiskLevel: RiskLevel                                                 │
│    ├── Checkpoints: []Checkpoint                                            │
│    └── Waves: []Wave (computed from Schedule)                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              EXECUTION ENGINE                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Runner (internal/engine/taskrunner/runner.go)                              │
│    ├── Schedule() → [][]int (waves via Kahn's algorithm)  [VERIFIED: runner.go:76-146] │
│    ├── ExecuteGroup(ctx, wave, ExecuteFunc) → error                        │
│    │     ├── Bounded parallelism via semaphore (max_parallel_tasks)        │
│    │     ├── Per-task timeout (task_timeout)                               │
│    │     ├── Retry with exponential backoff (MaxRetries)                  │
│    │     ├── Lock acquisition for Task.LockNames[]                        │
│    │     └── Checkpoint gate before wave execution                         │
│    │                                                                       │
│    ├── EventStore integration:                                             │
│    │     ├── TransitionTo() emits Event on every state change             │
│    │     ├── CheckpointRequested / CheckpointApproved events               │
│    │     └── Recovery: replay events from last projection checkpoint       │
│    │                                                                       │
│    └── Failure Recovery:                                                   │
│          ├── FAILED + RecoveryAction.RETRY    → reset to READY             │
│          ├── FAILED + RecoveryAction.REPAIR   → LLM patch → RETRY          │
│          ├── FAILED + RecoveryAction.REPLAN   → emit ReplanRequested       │
│          └── FAILED + RecoveryAction.ESCALATE → emit EscalationRequested   │
│                                                                             │
│  State Machine (ExecutionState):                                           │
│    PLANNED → READY → RUNNING → WAITING → VERIFYING → COMPLETED             │
│                              ↘ FAILED (with RecoveryAction)                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
            ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
            │  EventStore │ │   Provider  │ │    Tools    │
            │  (SQLite)   │ │  (REPAIR)   │ │ (FileEdit)  │
            └─────────────┘ └─────────────┘ └─────────────┘
                    │               │
                    ▼               ▼
            ┌─────────────────────────────┐
            │   TUI (S09/S10/S13/S16)     │
            │   Projection via MsgEmitter │
            └─────────────────────────────┘
```

### Recommended Project Structure

```
internal/
├── core/
│   └── types/
│       ├── planning.go          # TaskGraphIR, TaskIR, Checkpoint, ExecutionState, RecoveryAction, CompileToIR()
│       ├── types.go             # RiskLevel (existing), add ExecutionState, RecoveryAction, CheckpointType
│       └── event.go             # Add new event types per D-16
├── engine/
│   └── taskrunner/
│       ├── runner.go            # Extended Runner with ExecutionState, checkpoints, recovery, EventStore
│       ├── scheduler.go         # Extracted Schedule() and wave logic (optional separation)
│       ├── recovery.go          # Recovery strategies: RETRY, REPAIR, REPLAN, ESCALATE
│       ├── checkpoint.go        # Checkpoint gate logic, approval channel
│       └── state_machine.go     # ExecutionState.TransitionTo() validation
├── memory/
│   └── eventstore/
│       ├── eventstore.go        # Existing; add Append() for new event types
│       ├── projection.go        # Add TaskGraphExecutionProjection for resume
│       └── query.go             # Existing
└── integrations/
    └── provider/
        └── registry.go          # Existing; GetProviderForRequest for REPAIR
```

### Pattern 1: ExecutionState with Validated Transitions
**What:** ExecutionState is a typed enum with a `TransitionTo(next ExecutionState) error` method that enforces the valid state graph. No arbitrary transitions allowed.

**When to use:** All task state mutations in Runner, TUI projections, and recovery logic.

**Example:**
```go
// Source: internal/core/types/planning.go (new)
type ExecutionState string

const (
    ExecutionStatePlanned    ExecutionState = "PLANNED"
    ExecutionStateReady      ExecutionState = "READY"
    ExecutionStateRunning    ExecutionState = "RUNNING"
    ExecutionStateWaiting    ExecutionState = "WAITING"
    ExecutionStateVerifying  ExecutionState = "VERIFYING"
    ExecutionStateCompleted  ExecutionState = "COMPLETED"
    ExecutionStateFailed     ExecutionState = "FAILED"
)

var validTransitions = map[ExecutionState][]ExecutionState{
    ExecutionStatePlanned:   {ExecutionStateReady},
    ExecutionStateReady:     {ExecutionStateRunning, ExecutionStateWaiting},
    ExecutionStateRunning:   {ExecutionStateWaiting, ExecutionStateVerifying, ExecutionStateFailed},
    ExecutionStateWaiting:   {ExecutionStateRunning, ExecutionStateVerifying, ExecutionStateFailed},
    ExecutionStateVerifying: {ExecutionStateCompleted, ExecutionStateFailed},
    ExecutionStateCompleted: {},  // terminal
    ExecutionStateFailed:    {ExecutionStateReady},  // only via RecoveryAction
}

func (s ExecutionState) TransitionTo(next ExecutionState) error {
    allowed, ok := validTransitions[s]
    if !ok {
        return fmt.Errorf("unknown state %s", s)
    }
    for _, a := range allowed {
        if a == next {
            return nil
        }
    }
    return fmt.Errorf("invalid transition: %s → %s", s, next)
}

func (s ExecutionState) IsTerminal() bool {
    return s == ExecutionStateCompleted || s == ExecutionStateFailed
}
```

### Pattern 2: TaskGraphIR with CompileToIR()
**What:** Plan.CompileToIR() compiles user-facing Plan + Requirements + Decisions into executable TaskGraphIR with all TASKGRAPH-01 fields, computes waves via Kahn's algorithm, and injects checkpoints.

**When to use:** Plan phase output → Execute phase input.

**Example:**
```go
// Source: internal/core/types/planning.go (new)
type TaskIR struct {
    ID                 int
    Description        string
    Action             string
    Category           string
    Files              []string
    AcceptanceCriteria []string
    VerificationSteps  []VerificationStep
    Dependencies       []int
    Preconditions      []Precondition
    LockNames          []string
    RiskLevel          RiskLevel
    ExecutionState     ExecutionState
    RetryCount         int
    MaxRetries         int
    RecoveryAction     RecoveryAction
    CheckpointIDs      []string
}

type Checkpoint struct {
    ID                string
    Type              CheckpointType // ONE_WAY_DOOR, REVIEW
    TaskIDs           []int
    RequiredApprovers int
    EvidenceRefs      []string
    Description       string
}

type CheckpointType string
const (
    CheckpointTypeOneWayDoor CheckpointType = "ONE_WAY_DOOR"
    CheckpointTypeReview     CheckpointType = "REVIEW"
)

type TaskGraphIR struct {
    ID                uuid.UUID
    PlanID            uuid.UUID
    Objective         string
    Requirements      []uuid.UUID
    Constraints       []string
    Decisions         []uuid.UUID
    Tasks             []TaskIR
    Dependencies      []TaskDependency
    Preconditions     []Precondition
    AcceptanceCriteria []string
    VerificationSteps []VerificationStep
    RiskLevel         RiskLevel
    Checkpoints       []Checkpoint
    Waves             []Wave
}

type Wave struct {
    TaskIDs []int
    Index   int
}

func (p *Plan) CompileToIR(requirements []Requirement, decisions []Decision) (*TaskGraphIR, error) {
    // 1. Map Plan.Tasks → TaskIR with ExecutionState=PLANNED
    // 2. Run Kahn's algorithm (reuse runner.Schedule logic) to compute Waves
    // 3. Inject Checkpoints from decisions (ONE_WAY_DOOR) and config defaults
    // 4. Validate: no circular deps, all refs resolve, checkpoints reference valid tasks
    // 5. Return populated TaskGraphIR
}
```

### Pattern 3: Hybrid Checkpoint Approval (EventStore + Channel)
**What:** Runner pauses before executing a wave containing ONE_WAY_DOOR checkpoints. Emits CheckpointRequested event to EventStore (durable). Blocks on in-memory channel. TUI renders S13, user approves → TUI writes CheckpointApproved event to EventStore AND sends value on channel. Runner unblocks, continues.

**When to use:** All ONE_WAY_DOOR checkpoint gates.

**Example:**
```go
// Source: internal/engine/taskrunner/checkpoint.go (new)
type CheckpointGate struct {
    EventStore *eventstore.SQLiteEventStore
    ApproveCh  chan CheckpointApproval // in-memory, per-run
}

type CheckpointApproval struct {
    CheckpointID string
    Approved     bool
    Approver     string
    EvidenceRefs []string
}

func (g *CheckpointGate) RequestApproval(ctx context.Context, cp Checkpoint, runID uuid.UUID) (CheckpointApproval, error) {
    // 1. Emit CheckpointRequested event to EventStore
    evt := types.Event{
        Type:      types.EventCheckpointRequested,
        RunID:     &runID,
        Payload:   json.Marshal(CheckpointRequestedPayload{Checkpoint: cp, RunID: runID}),
    }
    if err := g.EventStore.Append(ctx, evt); err != nil {
        return CheckpointApproval{}, err
    }

    // 2. Block on channel (with context cancellation for timeout)
    select {
    case approval := <-g.ApproveCh:
        // 3. Emit CheckpointApproved/Denied event
        approvedEvt := types.Event{
            Type:    types.EventCheckpointResolved,
            RunID:   &runID,
            Payload: json.Marshal(CheckpointResolvedPayload{CheckpointID: cp.ID, Approved: approval.Approved}),
        }
        g.EventStore.Append(ctx, approvedEvt)
        return approval, nil
    case <-ctx.Done():
        return CheckpointApproval{}, ctx.Err()
    }
}
```

### Anti-Patterns to Avoid
- **Direct TaskStatus mutation:** Never mutate `task.Status` directly. Always use `ExecutionState.TransitionTo()` which validates and emits events.
- **Skipping EventStore on transition:** Every state change must append to EventStore for recovery. In-memory only loses durability.
- **REPAIR without provider:** Don't hardcode patch logic. Use provider registry (Phase 2) for LLM-generated patches.
- **Checkpoint polling:** Don't poll EventStore for approval. Use in-memory channel for fast resume (D-08).
- **Wave execution without lock acquisition:** Tasks with same LockNames must serialize even within wave.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Topological sort / wave computation | Custom DFS/recursion | Kahn's algorithm (existing in runner.go:76-146) | Handles cycles, O(V+E), already tested |
| State machine validation | Ad-hoc if/else chains | ExecutionState.TransitionTo() method | Centralized, testable, self-documenting |
| Event persistence | Custom JSONL files | SQLite EventStore (Phase 1) | WAL mode, range queries, projections, checkpoints |
| Checkpoint approval flow | Polling loop on DB | Hybrid EventStore + channel (D-08) | Fast resume + durability; follows Phase 4 D-36 |
| REPAIR patch generation | String manipulation | LLM via Provider interface (Phase 2) | Handles complex edits, uses existing streaming/resilience |
| Config for execution params | Hardcoded constants | Layered TOML config (Phase 1 loader) | Global→workspace→project→env; hot reload via fsnotify |
| Task state resume | Recompute from Plan | EventStore projections + checkpoint replay | Exact state reconstruction; handles partial waves |

**Key insight:** The existing TaskRunner already implements Kahn's algorithm, bounded parallelism, retry with backoff, and dependency checking. Phase 5 is primarily about: (1) replacing TaskStatus with ExecutionState + validation, (2) adding TaskGraphIR as the compilation target, (3) wiring EventStore for every transition, (4) implementing checkpoint gates and recovery strategies, (5) adding lock names for resource serialization.

## Runtime State Inventory

> Phase 5 is a greenfield implementation (new types, new execution engine). No rename/refactor of existing runtime state. However, the new engine will CREATE runtime state that must be tracked:

| Category | Items Created by Phase 5 | Action Required |
|----------|--------------------------|------------------|
| Stored data | EventStore events: TaskGraphCreated, TaskStateChanged (per transition), CheckpointRequested, CheckpointApproved/Denied, TaskRetried, TaskRepaired, TaskReplanned, TaskEscalated | EventStore schema supports all; projections rebuild state |
| Live service config | Runner in-memory: approval channels per run, lock semaphores per lock name | Channels created per run; locks released on task completion |
| OS-registered state | None | — |
| Secrets/env vars | None (Provider API keys handled by Phase 1 keychain) | — |
| Build artifacts | None | — |

**Nothing found in category:** OS-registered state, Secrets/env vars, Build artifacts — verified by codebase inspection (no OS registration, secrets in keychain, Go binary only).

## Common Pitfalls

### Pitfall 1: State Machine Validation Bypass (PITFALLS.md #3)
**What goes wrong:** Direct state mutation bypasses TransitionTo(), allowing invalid transitions (e.g., PLANNED → COMPLETED). Recovery/checkpoint load restores invalid state.
**Why it happens:** State treated as data field not validated transition system. Multiple code paths mutate status.
**How to avoid:** Make ExecutionState a private type with TransitionTo() as ONLY mutation path. Runner, TUI projections, recovery ALL use TransitionTo(). Validate on EventStore replay.
**Warning signs:** Direct `task.Status = ...` assignments; SetState() methods without validation.

### Pitfall 2: Incomplete Task State Restoration (PITFALLS.md #6)
**What goes wrong:** Checkpoint saves phase/plan but not task ExecutionState, retry_count, acceptance results. Resume re-executes completed tasks.
**Why it happens:** Task state treated as derivable from Plan; actually has independent lifecycle (heals, status, acceptance).
**How to avoid:** TaskGraphIR includes full task state. EventStore projections capture every TransitionTo(). Resume loads projection + replays events since checkpoint.
**Warning signs:** Checkpoint struct missing task array; resume path doesn't reinitialize Runner with restored states.

### Pitfall 3: Checkpoint Race Condition
**What goes wrong:** Multiple checkpoint requests for same wave; approval channel receives wrong approval; TUI shows stale checkpoint.
**Why it happens:** Single approval channel shared across checkpoints; no correlation ID.
**How to avoid:** Per-checkpoint channel or correlation ID in approval struct. Runner waits on specific checkpoint ID. TUI includes checkpoint ID in approval event.
**Warning signs:** Single `chan bool` for approvals; approval struct missing CheckpointID.

### Pitfall 4: REPAIR Infinite Loop
**What goes wrong:** REPAIR generates patch that fails same way; retry_count not incremented; infinite RETRY/REPAIR cycle.
**Why it happens:** REPAIR doesn't increment retry_count; no max repair attempts distinct from max retries.
**How to avoid:** Track repair_count separately. MaxRepairs config (default 1). After failed repair, escalate to REPLAN/ESCALATE.
**Warning signs:** retry_count used for both RETRY and REPAIR; no repair attempt limit.

### Pitfall 5: Lock Name Deadlock
**What goes wrong:** Tasks A and B both need locks ["db", "config"] but acquire in different order → deadlock.
**Why it happens:** Lock acquisition order not enforced.
**How to avoid:** Sort LockNames alphabetically before acquisition. Document convention: locks acquired in sorted order.
**Warning signs:** LockNames acquired in task-defined order; no sorting.

### Pitfall 6: Wave Timeout Starvation
**What goes wrong:** wave_timeout=0 (no limit) but one task hangs → entire wave blocks → dependent waves never start.
**Why it happens:** No per-task timeout enforcement within wave; task_timeout exists but wave_timeout=0 defaults to infinite.
**How to avoid:** Enforce task_timeout strictly. wave_timeout as safety net. Default task_timeout=30m (matches BashTimeout).
**Warning signs:** wave_timeout=0 with no task_timeout enforcement; context cancellation not propagated to task fn.

## Code Examples

Verified patterns from official sources and existing codebase:

### ExecutionState with TransitionTo() Validation
```go
// Source: Internal design per CONTEXT.md D-04, D-05
type ExecutionState string

const (
    ExecutionStatePlanned   ExecutionState = "PLANNED"
    ExecutionStateReady     ExecutionState = "READY"
    ExecutionStateRunning   ExecutionState = "RUNNING"
    ExecutionStateWaiting   ExecutionState = "WAITING"
    ExecutionStateVerifying ExecutionState = "VERIFYING"
    ExecutionStateCompleted ExecutionState = "COMPLETED"
    ExecutionStateFailed    ExecutionState = "FAILED"
)

func (s ExecutionState) TransitionTo(next ExecutionState) error {
    valid := map[ExecutionState][]ExecutionState{
        ExecutionStatePlanned:   {ExecutionStateReady},
        ExecutionStateReady:     {ExecutionStateRunning, ExecutionStateWaiting},
        ExecutionStateRunning:   {ExecutionStateWaiting, ExecutionStateVerifying, ExecutionStateFailed},
        ExecutionStateWaiting:   {ExecutionStateRunning, ExecutionStateVerifying, ExecutionStateFailed},
        ExecutionStateVerifying: {ExecutionStateCompleted, ExecutionStateFailed},
        ExecutionStateCompleted: {},
        ExecutionStateFailed:    {ExecutionStateReady}, // only via RecoveryAction
    }
    for _, allowed := range valid[s] {
        if allowed == next {
            return nil
        }
    }
    return fmt.Errorf("invalid transition %s → %s", s, next)
}
```

### Kahn's Algorithm (Existing - Verified)
```go
// Source: internal/engine/taskrunner/runner.go:76-146 [VERIFIED: runner.go:76-146]
func (r *Runner) Schedule() ([][]int, error) {
    // ... builds adjacency list, in-degree, dependents map ...
    // Kahn's algorithm:
    var groups [][]int
    var queue []int
    for _, t := range r.tasks {
        if inDegree[t.ID] == 0 {
            queue = append(queue, t.ID)
        }
    }
    processed := 0
    for len(queue) > 0 {
        groups = append(groups, queue)
        processed += len(queue)
        var nextQueue []int
        for _, id := range queue {
            for _, depID := range dependents[id] {
                inDegree[depID]--
                if inDegree[depID] == 0 {
                    nextQueue = append(nextQueue, depID)
                }
            }
        }
        queue = nextQueue
    }
    if processed < n {
        return nil, ErrCircularDependency
    }
    return groups, nil
}
```

### ExecuteGroup with Bounded Parallelism (Existing - Verified)
```go
// Source: internal/engine/taskrunner/runner.go:148-303 [VERIFIED: runner.go:148-303]
func (r *Runner) ExecuteGroup(ctx context.Context, group []int, fn ExecuteFunc) error {
    // ... pre-filter ready tasks ...
    sem := make(chan struct{}, r.maxParallel())
    for _, rt := range ready {
        wg.Add(1)
        sem <- struct{}{}
        go func() {
            defer wg.Done()
            defer func() { <-sem }()
            // ... retry loop with backoff ...
            r.mu.Lock()
            r.status[task.ID] = types.StatusRunning  // → becomes ExecutionStateRunning
            r.mu.Unlock()
            // ... execute with timeout ...
        }()
    }
    wg.Wait()
    return nil
}
```

### EventStore Append (Existing - Verified)
```go
// Source: internal/memory/eventstore/append.go (pattern)
func (s *SQLiteEventStore) Append(ctx context.Context, evt types.Event) error {
    payload, _ := json.Marshal(evt.Payload)
    metadata, _ := json.Marshal(evt.Metadata)
    _, err := s.db.ExecContext(ctx, `
        INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
        VALUES (?, ?, ?, ?, ?, ?, ?)
    `, evt.ID.String(), evt.Type, evt.Timestamp.UnixNano(),
        nullString(evt.RunID.String()), nullString(evt.SessionID.String()),
        payload, metadata)
    return err
}
```

### REPAIR Strategy via Provider (New Pattern)
```go
// Source: CONTEXT.md D-11, Phase 2 provider registry
func (r *Runner) attemptRepair(ctx context.Context, task TaskIR, failure FailureContext) (TaskResult, error) {
    provider, _, err := r.ProviderRegistry.GetProviderForRequest(
        types.ChatRequest{},
        r.Config.Provider.FallbackMode,
        r.Config.Provider.FallbackPriority,
        r.Config.Provider.HealthCheckTimeoutSecs,
    )
    if err != nil {
        return TaskResult{}, err
    }

    prompt := buildRepairPrompt(task, failure)
    resp, err := provider.ChatCompletion(ctx, types.ChatRequest{
        Model:    r.Config.Model.Default,
        Messages: []types.Message{{Role: "user", Content: prompt}},
    })
    if err != nil {
        return TaskResult{}, err
    }

    patch := extractPatch(resp.Content)
    // Apply via FileEdit tool (Phase 7)
    result := r.ToolsDispatcher.Execute(ctx, types.ToolInput{
        Name: "edit_file",
        Params: map[string]any{"path": task.Files[0], "patch": patch},
    })
    return result, nil
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Markdown plan as executable | TaskGraph IR (strongly typed) | Phase 5 | Eliminates planner/executor interpretation drift |
| TaskStatus enum (pending/running/done/failed) | ExecutionState with validated transitions | Phase 5 | Prevents invalid states; enables recovery |
| Single retry loop | RETRY/REPAIR/REPLAN/ESCALATE strategies | Phase 5 | Bounded, auditable, escalates to human when needed |
| In-memory task state | EventStore + projections + checkpoints | Phase 1 + 5 | Exact resume; crash recovery; audit trail |
| No checkpoint gates | ONE_WAY_DOOR/REVIEW checkpoints in IR | Phase 5 | Human control over irreversible actions |
| No resource locks | Task.LockNames[] for mutex serialization | Phase 5 | Prevents concurrent writes to same file/DB |

**Deprecated/outdated:**
- `TaskStatus` enum in `types.go` — replaced by `ExecutionState` (D-04)
- `TaskGraphStatus` enum — replaced by TaskGraphIR execution state derived from task states
- `runner.go` Status field using `TaskStatus` — migrated to `ExecutionState`

## Assumptions Log

> List all claims tagged `[ASSUMED]` in this research. The planner and discuss-phase use this section to identify decisions that need user confirmation before execution.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `max_parallel_tasks = 4` default is optimal for typical workloads | Standard Stack, Architecture Patterns | Too low = underutilization; too high = resource contention |
| A2 | `task_timeout = 30m` default matches BashTimeout and is sufficient | Architecture Patterns, Config | Too short = false failures; too long = stuck waves |
| A3 | REPAIR via LLM patch generation works reliably for common failure modes | Recovery Strategies | If unreliable, REPAIR becomes no-op; falls back to REPLAN/ESCALATE |
| A4 | Lock name convention (alphabetical acquisition) prevents deadlocks | Common Pitfalls #5 | Deadlocks stall execution; require manual intervention |
| A5 | Checkpoint interval of 100 events (from config default) balances durability/performance | D-18, Config | Too frequent = overhead; too sparse = replay time |
| A6 | Phase 6 Planner agent will exist to handle REPLAN strategy | D-12 | REPLAN blocks until Phase 6 complete; may need stub |

## Open Questions

1. **Wave timeout default**
   - What we know: D-13 sets `wave_timeout = 0` (no limit). task_timeout = 30m.
   - What's unclear: Should wave_timeout have a non-zero default as safety net?
   - Recommendation: Keep 0 default; add monitoring/alerting in TUI S09 for stuck waves. Revisit after workload observation.

2. **REPAIR prompt template**
   - What we know: D-11 says "LLM prompt template for REPAIR patch generation — implementation detail"
   - What's unclear: Exact prompt structure, context inclusion (error, code, acceptance criteria)
   - Recommendation: Design prompt in implementation; include task objective, failure error, affected files, acceptance criteria. Mark as agent's discretion.

3. **Checkpoint approval TUI integration**
   - What we know: D-07, D-08 define event flow. S13 Checkpoint screen in UI-SPEC.md.
   - What's unclear: Exact MsgEmitter message types for CheckpointRequested/Approved.
   - Recommendation: Define `CheckpointRequestedMsg` and `CheckpointApprovedMsg` in TUI types (Phase 10/11). Runner emits via MsgEmitter channel.

4. **MaxRepairs config distinct from MaxRetries**
   - What we know: D-11 mentions retry_count for RETRY. REPAIR uses provider then RETRY.
   - What's unclear: Should REPAIR have separate attempt limit?
   - Recommendation: Add `MaxRepairs` config (default 1). Track repair_count on TaskIR. After exhaust, escalate.

## Environment Availability

> Phase 5 has no external dependencies beyond the project's own codebase and Go toolchain. All required libraries are in go.mod.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Go toolchain | Build | ✓ | 1.26.5 (per go.mod) | — |
| SQLite (modernc.org/sqlite) | EventStore | ✓ | v1.32.2 | — |
| NVIDIA API key | REPAIR strategy (provider) | ✓/✗ | — | REPAIR skipped; fallback to RETRY/REPLAN/ESCALATE |

**Missing dependencies with no fallback:** None — REPAIR degrades gracefully to other recovery actions.
**Missing dependencies with fallback:** NVIDIA API key → REPAIR unavailable; Runner uses RETRY/REPLAN/ESCALATE.

## Validation Architecture

> Included because `workflow.nyquist_validation` is not explicitly `false` in `.planning/config.json`.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` + `github.com/stretchr/testify` (assert) |
| Config file | None — see Wave 0 |
| Quick run command | `go test ./internal/engine/taskrunner/... -count=1` |
| Full suite command | `make test` (race-enabled with coverage) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TASKGRAPH-01 | TaskGraphIR compiles with all fields | Unit | `go test ./internal/core/types/... -run TestTaskGraphIR -count=1` | ❌ Wave 0 |
| TASKGRAPH-01 | Plan.CompileToIR() produces valid IR | Unit | `go test ./internal/core/types/... -run TestCompileToIR -count=1` | ❌ Wave 0 |
| TASKGRAPH-02 | ExecutionState.TransitionTo() validates | Unit | `go test ./internal/core/types/... -run TestExecutionState -count=1` | ❌ Wave 0 |
| TASKGRAPH-02 | Invalid transition returns error | Unit | `go test ./internal/core/types/... -run TestInvalidTransition -count=1` | ❌ Wave 0 |
| TASKGRAPH-03 | RETRY resets to READY, increments count | Unit | `go test ./internal/engine/taskrunner/... -run TestRetry -count=1` | ❌ Wave 0 |
| TASKGRAPH-03 | REPAIR calls provider, applies patch | Integration | `go test ./internal/engine/taskrunner/... -run TestRepair -count=1` | ❌ Wave 0 |
| TASKGRAPH-03 | REPLAN emits ReplanRequested event | Unit | `go test ./internal/engine/taskrunner/... -run TestReplan -count=1` | ❌ Wave 0 |
| TASKGRAPH-03 | ESCALATE emits EscalationRequested event | Unit | `go test ./internal/engine/taskrunner/... -run TestEscalate -count=1` | ❌ Wave 0 |
| TASKGRAPH-04 | Schedule() produces valid topological waves | Unit | `go test ./internal/engine/taskrunner/... -run TestSchedule -count=1` | ✅ (existing) |
| TASKGRAPH-04 | ExecuteGroup respects max_parallel_tasks | Unit | `go test ./internal/engine/taskrunner/... -run TestParallelism -count=1` | ❌ Wave 0 |
| TASKGRAPH-04 | LockNames serialize same-lock tasks | Unit | `go test ./internal/engine/taskrunner/... -run TestLockNames -count=1` | ❌ Wave 0 |
| TASKGRAPH-05 | Checkpoint gate blocks wave until approval | Integration | `go test ./internal/engine/taskrunner/... -run TestCheckpointGate -count=1` | ❌ Wave 0 |
| TASKGRAPH-05 | CheckpointApproved event persisted | Integration | `go test ./internal/memory/eventstore/... -run TestCheckpointEvents -count=1` | ❌ Wave 0 |
| TASKGRAPH-06 | EventStore captures every TransitionTo | Integration | `go test ./internal/engine/taskrunner/... -run TestEventEmission -count=1` | ❌ Wave 0 |
| TASKGRAPH-06 | Resume reconstructs exact task states | Integration | `go test ./internal/engine/taskrunner/... -run TestResume -count=1` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test ./internal/engine/taskrunner/... ./internal/core/types/... -count=1`
- **Per wave merge:** `make test` (full suite with race detector)
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/core/types/planning_test.go` — TaskGraphIR, CompileToIR, ExecutionState, RecoveryAction tests
- [ ] `internal/engine/taskrunner/runner_test.go` — Extended Runner tests (ExecutionState, checkpoints, recovery, locks, events)
- [ ] `internal/engine/taskrunner/recovery_test.go` — RETRY, REPAIR, REPLAN, ESCALATE strategy tests
- [ ] `internal/engine/taskrunner/checkpoint_test.go` — Checkpoint gate, approval channel tests
- [ ] `internal/memory/eventstore/projection_test.go` — TaskGraphExecutionProjection for resume
- [ ] Framework: testify already in go.mod; no additional installs needed

*(If no gaps: "None — existing test infrastructure covers all phase requirements")*

## Security Domain

> Required when `security_enforcement` is enabled (absent = enabled).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | — |
| V3 Session Management | No | — |
| V4 Access Control | Yes | Capability-based permissions (Phase 7); checkpoint approval required for ONE_WAY_DOOR |
| V5 Input Validation | Yes | `ValidateTaskFiles()` on all file paths; ExecutionState.TransitionTo() validates transitions |
| V6 Cryptography | No | — (EventStore uses SQLite; no custom crypto) |
| V7 Error Handling | Yes | Structured errors via `internal/core/errors`; no panic; recovery bounded |
| V8 Logging | Yes | EventStore = immutable audit log; no secrets in events |
| V9 Communication | No | — (local process only) |
| V10 HTTP Security | No | — |
| V11 Business Logic | Yes | State machine prevents invalid transitions; recovery bounded by config |
| V12 File/Resources | Yes | `ValidateTaskFiles()` with `EvalSymlinks` on workspace root + target |
| V13 API Security | No | — |
| V14 Configuration | Yes | Layered TOML; API keys in keychain not config; execution config validated |
| V15 Supply Chain | Yes | Provider legitimacy gate (Phase 4 DEPEND); no new deps in Phase 5 |

### Known Threat Patterns for Go Task Execution Engine

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal in task.Files | Tampering | Central `ValidateTaskFiles()` with `EvalSymlinks` on both paths (Phase 7 TOOLS-04) |
| Shell injection in REPAIR patch | Tampering/Execution | Patches applied via `edit_file` tool (not shell); dispatcher validates |
| Unbounded resource consumption | DoS | `max_parallel_tasks`, `task_timeout`, `wave_timeout` config limits |
| Checkpoint bypass | Spoofing/Tampering | EventStore durability + TransitionTo validation; approval required for ONE_WAY_DOOR |
| Cross-run state corruption | Tampering | RunID in every event; projections scoped by RunID; session ID validation on resume |
| REPAIR patch malicious content | Tampering | Patch validated by `edit_file` tool; applied in isolated worktree (Phase 8) |

## Sources

### Primary (HIGH confidence)
- `internal/engine/taskrunner/runner.go:76-146` — Kahn's algorithm Schedule() implementation [VERIFIED]
- `internal/engine/taskrunner/runner.go:148-303` — ExecuteGroup with semaphore, retry, timeout [VERIFIED]
- `internal/core/types/planning.go:84-197` — Plan, Task, TaskGraph, Requirement, Decision types [VERIFIED]
- `internal/core/types/types.go:9-16` — RiskLevel enum [VERIFIED]
- `internal/core/types/event.go:23-77` — EventType vocabulary [VERIFIED]
- `internal/memory/eventstore/eventstore.go` — SQLite WAL mode, schema, Append pattern [VERIFIED]
- `internal/memory/eventstore/projection.go` — ProjectionManager, checkpoint/replay [VERIFIED]
- `internal/core/config/loader.go` — Layered TOML config loading pattern [VERIFIED]
- `internal/integrations/provider/registry.go` — GetProviderForRequest for REPAIR [VERIFIED]
- `.planning/CONTEXT_M31A.md` §8.5, §10, §11, §31 — Canonical TaskGraph IR, state machine, events, checkpoints [CITED]
- `.planning/REQUIREMENTS.md` — TASKGRAPH-01 through TASKGRAPH-06 [CITED]
- `.planning/ROADMAP.md` — Phase 5 success criteria [CITED]
- `.planning/phases/05-taskgraph-ir-execution-engine/05-CONTEXT.md` — All D-01 through D-18 decisions [CITED]

### Secondary (MEDIUM confidence)
- `.planning/research/PITFALLS.md` — Pitfalls 1, 3, 6, 9, 12, 13, 16 relevant to Phase 5 [CITED]

### Tertiary (LOW confidence)
- None — all critical findings verified against codebase or canonical docs

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — All libraries already in go.mod, verified via `go list -m`
- Architecture: HIGH — Decisions locked in CONTEXT.md; existing codebase provides 80% of implementation
- Pitfalls: HIGH — PITFALLS.md documents exact failure modes; mitigations designed into D-04 through D-18

**Research date:** 2026-09-05
**Valid until:** 2026-10-05 (30 days — stable Go ecosystem, locked decisions)