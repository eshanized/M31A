# Phase 5: TaskGraph IR & Execution Engine - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-09-05
**Phase:** 5-TaskGraph IR & Execution Engine
**Areas discussed:** TaskGraph IR Schema Completeness, Execution State Machine Design, Checkpoint Gate Integration, Failure Recovery Strategies, Wave Execution & Parallelism Control, TaskGraph Persistence & EventStore Events

---

## TaskGraph IR Schema Completeness

| Option | Description | Selected |
|--------|-------------|----------|
| Extend existing Task/TaskGraph types in-place | Add missing fields (Objectives, Constraints, Preconditions, VerificationSteps, RiskLevel, Checkpoints) directly to Task/TaskGraph structs. Simpler, single source of truth. | |
| Create new TaskGraphIR type separate from Task/Plan | New type in planning.go or execution.go with all IR fields. Task/Plan stay as user-facing; IR is internal executable form. Cleaner separation but more types to maintain. | ✓ |
| Extend TaskGraph with IR fields; keep Task as-is | TaskGraph gets Objectives, Requirements[], Constraints[], Decisions[], Preconditions[], VerificationSteps[], RiskLevel, Checkpoints[]; Task stays minimal. IR fields at graph level, tasks stay focused. | |

**User's choice:** Create new TaskGraphIR type separate from Task/Plan
**Notes:** User prefers clean separation between user-facing types (Task, Plan) and internal executable IR. The IR is an implementation detail for the execution engine.

---

## TaskGraph IR Schema Completeness (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| In internal/core/types/execution.go (new file) | Follows six-plane alignment — execution plane owns executable IR. Plan (planning plane) compiles to TaskGraphIR. Clear ownership boundary. | |
| In internal/core/types/planning.go alongside Plan | Keeps planning-related types together. Plan has CompileToIR() method. Simpler imports, but blurs plane boundary. | ✓ |
| In internal/engine/workflow/ir.go (internal to engine) | IR is an engine implementation detail. Not exposed to TUI or other layers. Maximum encapsulation but less reusable. | |

**User's choice:** In internal/core/types/planning.go alongside Plan
**Notes:** Keeping planning-related types together simplifies imports. Plan.CompileToIR() method provides clear compilation path.

---

## TaskGraph IR Schema Completeness (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Plan.CompileToIR() method on Plan type | Plan owns the compilation logic. Takes requirements, decisions, tasks and produces TaskGraphIR. Single method, easy to test. | ✓ |
| Standalone compiler: CompilePlanToIR(plan) in separate file | Separates compilation logic from domain types. Easier to swap compilation strategies. Follows Phase 1 pattern of keeping types pure. | |
| Planner agent (Phase 6) produces TaskGraphIR directly | Plan is human-facing; TaskGraphIR is agent-produced executable artifact. Compilation happens during planning phase, not as a method call. | |

**User's choice:** Plan.CompileToIR() method on Plan type
**Notes:** Plan owns compilation. Single method, testable. Clear ownership.

---

## TaskGraph IR Schema Completeness (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| All TASKGRAPH-01 fields: Objective, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[] | Full IR schema in one type. TaskGraphIR.Tasks contains enriched TaskIR with preconditions, verification steps, checkpoints per task. | ✓ |
| Graph-level fields only: Objective, Requirements[], Constraints[], Decisions[], Preconditions[], VerificationSteps[], RiskLevel, Checkpoints[]; Tasks[] references existing Task type | Keeps Task minimal. IR adds graph-level context (preconditions, verification, checkpoints at graph level). Tasks stay as-is. | |
| Minimal delta: add only Preconditions[], VerificationSteps[], RiskLevel, Checkpoints[] to TaskGraph; enrich Task with Preconditions, VerificationSteps | Extends TaskGraph rather than new type. Simpler migration but less clean separation of concerns. | |

**User's choice:** All TASKGRAPH-01 fields: Objective, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]
**Notes:** Full IR schema matches requirements exactly. TaskGraphIR is the complete executable representation.

---

## Execution State Machine Design

| Option | Description | Selected |
|--------|-------------|----------|
| Replace TaskStatus with new ExecutionState enum matching requirements exactly | Clean break. PLANNED, READY, RUNNING, WAITING, VERIFYING, COMPLETED, FAILED. Update all consumers (runner, TUI, events). | ✓ |
| Map current to required: Pending→PLANNED/READY, Running→RUNNING, Done→COMPLETED, Failed→FAILED; add WAITING, VERIFYING | Minimal change. Keep existing statuses, add WAITING and VERIFYING. Map semantically. Less disruption to existing code. | |
| Dual state system: TaskStatus (legacy) + ExecutionState (new IR) | TaskGraphIR uses ExecutionState for validated transitions. Legacy Task/Plan keep TaskStatus. Bridge at compilation. Clean separation. | |

**User's choice:** Replace TaskStatus with new ExecutionState enum matching requirements exactly
**Notes:** Clean break avoids confusion between legacy and new state machines. All consumers updated together.

---

## Execution State Machine Design (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Transition validation in Runner: method ExecuteGroup validates current→next before state change | Runner owns execution flow. Validates PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED. Centralized, testable. | |
| ExecutionState type with TransitionTo(next) method that validates and returns error on invalid | State machine logic encapsulated in the type itself. ExecutionState.TransitionTo(VERIFYING) returns error if current != WAITING. Self-documenting. | ✓ |
| Separate StateMachine type (like Phase 1 workflow engine) that both Runner and TaskGraphIR use | Reusable state machine. Phase 1 engine.go has StateMachine for workflow phases. Same pattern for task execution. Shared validation logic. | |

**User's choice:** ExecutionState type with TransitionTo(next) method that validates and returns error on invalid
**Notes:** Encapsulation in the type itself is self-documenting and testable. TransitionTo(VERIFYING) fails if current != WAITING.

---

## Execution State Machine Design (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| FAILED is terminal; recovery actions create new TaskGraphIR with reset state (REPLAN) or new execution attempt (RETRY) | FAILED = end of current execution. RETRY re-executes same TaskGraphIR from READY. REPAIR patches then RETRY. REPLAN creates new TaskGraphIR. ESCALATE pauses for human. | |
| FAILED has sub-states: FAILED_RETRYABLE, FAILED_REPAIRABLE, FAILED_REPLANNABLE, FAILED_ESCALATED | FAILED isn't terminal; sub-state encodes recovery path. TransitionTo allows FAILED→READY for RETRY, FAILED→RUNNING for REPAIR (after patch). More granular. | |
| Separate RecoveryAction enum; FAILED state carries RecoveryAction; Runner interprets and transitions accordingly | ExecutionState.FAILED has RecoveryAction field (RETRY/REPAIR/REPLAN/ESCALATE). Runner reads it and decides next transition. Decouples state from recovery logic. | ✓ |

**User's choice:** Separate RecoveryAction enum; FAILED state carries RecoveryAction; Runner interprets and transitions accordingly
**Notes:** Decouples state from recovery logic. FAILED is terminal for current attempt; RecoveryAction encodes next step.

---

## Checkpoint Gate Integration

| Option | Description | Selected |
|--------|-------------|----------|
| Checkpoints as TaskGraphIR.Checkpoints[] with type (ONE_WAY_DOOR/REVIEW); Runner pauses wave, emits CheckpointRequested event, waits for CheckpointApproved event | Checkpoints are part of IR. Runner checks before executing wave; if checkpoint exists, emits event via EventStore, blocks until approval event. TUI handles approval UI. | ✓ |
| Checkpoints separate from TaskGraphIR; configured in execution policy; Runner evaluates at wave boundaries | Checkpoints not in IR. Execution policy defines which tasks/waves need approval. Runner consults policy at wave boundaries. More flexible, less IR bloat. | |
| Checkpoints embedded in Task preconditions; Task.Preconditions includes CheckpointRequired{type, evidence}; Runner evaluates before task starts | Fine-grained per-task checkpoints. Precondition system naturally handles 'must approve before run'. Aligns with Preconditions[] in IR. | |

**User's choice:** Checkpoints as TaskGraphIR.Checkpoints[] with type (ONE_WAY_DOOR/REVIEW); Runner pauses wave, emits CheckpointRequested event, waits for CheckpointApproved event
**Notes:** Checkpoints are part of the IR. Runner checks before wave execution. Follows Phase 4 checkpoint patterns.

---

## Checkpoint Gate Integration (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| EventStore events: CheckpointRequested (with task, risk, evidence) → TUI shows S13 Checkpoint screen → user approves → CheckpointApproved event → Runner resumes | Follows Phase 4 D-36: WritePending persists before blocking. EventStore is source of truth. TUI subscribes to events. Approval writes CheckpointApproved event. Runner polls/waits for event. | |
| Runner blocks on channel; TUI sends approval via tea.Msg through MsgEmitter; no EventStore round-trip for approval | Faster, no EventStore latency. But breaks event-driven architecture. Phase 1: all durable state = events. Approval decision must be in event log for audit/replay. | |
| Hybrid: EventStore for durability (CheckpointRequested, CheckpointApproved), Runner uses in-memory channel for fast resume; TUI writes to both | Best of both. EventStore captures decision for audit/replay. In-memory channel avoids polling. TUI writes event + sends MsgEmitter. Runner reads event on startup for recovery. | ✓ |

**User's choice:** Hybrid: EventStore for durability (CheckpointRequested, CheckpointApproved), Runner uses in-memory channel for fast resume; TUI writes to both
**Notes:** Best of both worlds. EventStore for audit/replay, in-memory channel for fast resume without polling.

---

## Checkpoint Gate Integration (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Two types: ONE_WAY_DOOR (irreversible, requires explicit approval) and REVIEW (reversible, info only). Config: [execution.checkpoints] auto_approve_review = true | Matches CONTEXT_M31A.md §31. ONE_WAY_DOOR = destructive ops (git push, db migrate). REVIEW = high-risk but reversible. Config controls default behavior. | |
| Three types: ONE_WAY_DOOR, REVIEW, AUTOMATIC (policy-based, no human). Risk classification from TOOLS-03 drives default checkpoint type per task | TOOLS-03 risk classes (high/medium/low) map to checkpoint types. High risk → ONE_WAY_DOOR. Medium → REVIEW. Low → AUTOMATIC. Configurable overrides. | |
| Checkpoint per task via TaskGraphIR.Checkpoints[]; each has type, required_approvers, evidence_refs. Config only sets defaults | Maximum flexibility. Each checkpoint in IR specifies exactly what's needed. Config provides defaults for tasks without explicit checkpoints. IR is authoritative. | ✓ |

**User's choice:** Checkpoint per task via TaskGraphIR.Checkpoints[]; each has type, required_approvers, evidence_refs. Config only sets defaults
**Notes:** Maximum flexibility. IR is authoritative. Config only provides defaults.

---

## Failure Recovery Strategies

| Option | Description | Selected |
|--------|-------------|----------|
| RecoveryAction enum on FAILED state (as chosen). Runner reads action: RETRY→reset to READY; REPAIR→apply patch then RETRY; REPLAN→request new TaskGraphIR from Planner; ESCALATE→emit event, pause for human | Direct mapping. FAILED state carries RecoveryAction. Runner interprets and transitions. Simple, explicit. EventStore records each recovery attempt. | ✓ |
| RecoveryHandler interface per strategy; Runner delegates to handler; each handler returns next ExecutionState | Extensible. New recovery strategies = new handlers. RETRYHandler, REPAIRHandler, REPLANHandler, ESCALATEHandler. Runner just calls handler. More flexible for future. | |
| Recovery as separate phase in TaskGraphIR execution: FAILED → RECOVERY_PLANNING → RECOVERY_EXECUTION → back to READY/RUNNING | Recovery is a first-class execution phase. Allows complex recovery workflows (diagnose → patch → verify → retry). More states but clearer semantics. | |

**User's choice:** RecoveryAction enum on FAILED state (as chosen). Runner reads action: RETRY→reset to READY; REPAIR→apply patch then RETRY; REPLAN→request new TaskGraphIR from Planner; ESCALATE→emit event, pause for human
**Notes:** Direct mapping. Simple, explicit. EventStore records each recovery attempt.

---

## Failure Recovery Strategies (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Task tracks retry_count; RETRY allowed if retry_count < MaxRetries. REPAIR: Runner calls LLM (via provider) with failure context to generate patch; applies patch via tools; then RETRY | Retry count per task. MaxRetries from Runner config. REPAIR uses provider to generate fix from error + code context. Patch applied via FileEdit tool. Then retry. | ✓ |
| RETRY: Runner re-executes task with same input (idempotent tasks). REPAIR: Separate RepairAgent (Phase 6) produces patch; Runner applies; then retry. Decouples repair from execution | REPAIR is agent-driven. Phase 6 Debugger/Implementer creates patch. Runner just applies. Cleaner separation. RETRY is simple re-execution. | |
| RETRY with exponential backoff (already in runner.go). REPAIR: Runner emits TaskFailed event with error context; TUI/headless triggers repair workflow; patch applied externally; Runner notified via event to retry | Event-driven repair. Runner doesn't call LLM directly. Repair happens via external workflow (TUI or headless command). Runner waits for TaskRepaired event then retries. Follows Phase 1 event architecture. | |

**User's choice:** Task tracks retry_count; RETRY allowed if retry_count < MaxRetries. REPAIR: Runner calls LLM (via provider) with failure context to generate patch; applies patch via tools; then RETRY
**Notes:** Runner directly calls provider for REPAIR. Simpler than event-driven external workflow. Retry count per task.

---

## Failure Recovery Strategies (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| REPLAN: Runner emits ReplanRequested event with failure context; Planner agent (Phase 6) creates new TaskGraphIR; Runner swaps IR and restarts from PLANNED. ESCALATE: emits EscalationRequested event; TUI shows S16 Failure/Recovery; human chooses action | Event-driven. REPLAN triggers planner agent. ESCALATE shows S16 screen with RETRY/REPAIR/REPLAN/STOP options. Human decision becomes new RecoveryAction. | ✓ |
| REPLAN: Runner calls Planner directly (synchronous) to generate new TaskGraphIR; continues execution. ESCALATE: Runner pauses, writes EscalationPending event; external process (CLI/TUI) resolves; Runner polls for resolution | Synchronous replan (simpler). ESCALATE uses event for persistence but blocks synchronously. Less event churn but tighter coupling. | |
| REPLAN and ESCALATE both use RecoveryAction. REPLAN creates new TaskGraph with same objective; ESCALATE sets action=ESCALATE, Runner waits for human to update task RecoveryAction via EventStore | Unified recovery model. All recovery goes through RecoveryAction on FAILED. Human edits event/state to change action. Runner re-reads and acts. Maximum flexibility. | |

**User's choice:** REPLAN: Runner emits ReplanRequested event with failure context; Planner agent (Phase 6) creates new TaskGraphIR; Runner swaps IR and restarts from PLANNED. ESCALATE: emits EscalationRequested event; TUI shows S16 Failure/Recovery; human chooses action
**Notes:** Event-driven. REPLAN triggers Phase 6 Planner agent. ESCALATE uses S16 screen for human decision.

---

## Wave Execution & Parallelism Control

| Option | Description | Selected |
|--------|-------------|----------|
| Config: [execution] max_parallel_tasks = 4 (default), task_timeout = 30m, wave_timeout = 0 (no limit). Runner reads from config.toml layered loading | Follows Phase 1 config pattern. max_parallel_tasks replaces DefaultMaxParallelTasks. task_timeout already exists. wave_timeout adds safety for stuck waves. | ✓ |
| Per-wave parallelism: TaskGraphIR.Waves[] each has MaxParallel; overrides global config. Allows CPU-heavy wave=1, I/O wave=8 | Wave-level control in IR. More granular. Compiler (Plan.CompileToIR) sets per-wave parallelism based on task categories. Config is fallback. | |
| Dynamic parallelism: Runner monitors system resources (CPU, memory) and adjusts semaphore at runtime; config sets min/max bounds | Adaptive. Runner uses golang.org/x/sys to detect resources. Semaphore grows/shrinks within bounds. Complex but maximizes throughput. | |

**User's choice:** Config: [execution] max_parallel_tasks = 4 (default), task_timeout = 30m, wave_timeout = 0 (no limit). Runner reads from config.toml layered loading
**Notes:** Simple config-based approach. Follows Phase 1 pattern. Per-wave and dynamic can be added later if needed.

---

## Wave Execution & Parallelism Control (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| No coordination within wave; tasks are independent by Kahn's algorithm. Shared resource conflicts prevented by TaskGraphIR dependencies (explicit deps = no concurrent conflicts) | Kahn's algorithm guarantees tasks in same wave have no dependency edges. If they share resources, they should have a dependency. Simpler, correct by construction. | |
| Optional mutex/lock names in TaskGraphIR; tasks with same lock name run sequentially even in same wave | For resources not captured by dependencies (e.g., same file, same DB). Task.LockNames[] = ["db-migrate", "config-write"]. Runner acquires lock before task. More realistic. | ✓ |
| Resource scopes from TOOLS-03 (filesystem.write scope, git.write scope); Runner enforces scope exclusivity per wave | Integrates with Phase 7 permission system. Task declares required scopes. Runner ensures no two tasks in same wave need exclusive same scope. Phase 7 dependency. | |

**User's choice:** Optional mutex/lock names in TaskGraphIR; tasks with same lock name run sequentially even in same wave
**Notes:** For resources not captured by dependencies (same file, DB). Task.LockNames[] allows sequential execution within wave.

---

## Wave Execution & Parallelism Control (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Wave completes when all tasks in group reach terminal state (COMPLETED/FAILED/SKIPPED). Partial failure: failed tasks block dependents (set to SKIPPED), wave continues for independent tasks. Next wave only starts when current wave fully resolved | Current runner.go behavior. Failed task -> dependents SKIPPED. Wave boundary = synchronization point. Simple, predictable. | ✓ |
| Wave has explicit status: RUNNING, COMPLETED, PARTIAL_FAILED. PARTIAL_FAILED = some failed, some done. Runner can retry failed tasks in same wave before advancing (if retry policy allows) | More nuanced. Allows retry within wave before moving on. Wave status visible in TUI. Adds complexity but better UX. | |
| Wave is atomic: if any task fails, entire wave rolls back (via git worktree) and retries or escalates. All-or-nothing per wave. | Strongest guarantee. Uses Phase 8 worktree isolation. Wave failure = worktree reset. Retry whole wave or escalate. Phase 8 dependency but cleanest semantics. | |

**User's choice:** Wave completes when all tasks in group reach terminal state (COMPLETED/FAILED/SKIPPED). Partial failure: failed tasks block dependents (set to SKIPPED), wave continues for independent tasks. Next wave only starts when current wave fully resolved
**Notes:** Current runner.go behavior. Failed task -> dependents SKIPPED. Wave boundary = synchronization point. Simple, predictable.

---

## TaskGraph Persistence & EventStore Events

| Option | Description | Selected |
|--------|-------------|----------|
| Fine-grained events per transition: TaskGraphCreated, TaskGraphValidated, TaskPlanned, TaskReady, TaskRunning, TaskWaiting, TaskVerifying, TaskCompleted, TaskFailed, TaskRetried, TaskRepaired, TaskReplanned, TaskEscalated, CheckpointRequested, CheckpointApproved, CheckpointDenied | One event per state transition. Maximum traceability. EventStore becomes complete audit log. Aligns with Phase 1 event vocabulary (CONTEXT_M31A.md §11). | ✓ |
| Coarse events: TaskGraphCreated, TaskGraphStarted, TaskGraphCompleted, TaskGraphFailed, TaskStateChanged (with from/to state), CheckpointEvent (type=req/approved/denied) | Fewer event types. TaskStateChanged carries from/to ExecutionState. Simpler projections. Less event volume but less granular replay. | |
| Hybrid: coarse for graph-level (Created, Started, Completed, Failed), fine-grained for task-level (TaskStateChanged with from/to), checkpoint events as separate type | Balanced. Graph lifecycle = few events. Task transitions = TaskStateChanged with full context. Checkpoints separate for approval workflow. | |

**User's choice:** Fine-grained events per transition: TaskGraphCreated, TaskGraphValidated, TaskPlanned, TaskReady, TaskRunning, TaskWaiting, TaskVerifying, TaskCompleted, TaskFailed, TaskRetried, TaskRepaired, TaskReplanned, TaskEscalated, CheckpointRequested, CheckpointApproved, CheckpointDenied
**Notes:** Maximum traceability. Complete audit log. Aligns with Phase 1 event vocabulary.

---

## TaskGraph Persistence & EventStore Events (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Runner calls EventStore.Append() on every ExecutionState transition via TransitionTo(). Event includes task_id, from_state, to_state, timestamp, run_id, attempt_number. Projections rebuild exact state | TransitionTo() emits event. Runner subscribes to its own events for recovery. On resume, replays events to reconstruct TaskGraphIR execution state. Follows Phase 1 pattern. | ✓ |
| Runner batches events per wave; writes TaskStateChanged batch at wave boundary. Checkpoint events written immediately. Reduces EventStore writes | Performance optimization. Wave boundary = sync point. Batch writes. But loses exact per-transition timing. TASKGRAPH-06 says 'at each transition' — may not satisfy. | |
| Runner writes to EventStore via callback: OnStateChange(task, from, to) → EventStore.Append(). Caller (TUI/headless) provides EventStore. Decouples Runner from EventStore | Runner doesn't import EventStore. Callback pattern. TUI/headless wires EventStore. More testable. Runner just calls callback on transition. | |

**User's choice:** Runner calls EventStore.Append() on every ExecutionState transition via TransitionTo(). Event includes task_id, from_state, to_state, timestamp, run_id, attempt_number. Projections rebuild exact state
**Notes:** TransitionTo() emits event directly. Runner owns its EventStore dependency. Follows Phase 1 pattern of event-driven state.

---

## TaskGraph Persistence & EventStore Events (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| TaskGraphCreated event stores full TaskGraphIR JSON. On resume: load TaskGraphIR from event, replay TaskStateChanged events to reconstruct per-task state, continue from last incomplete task | Full IR in event. Replay rebuilds exact state. Simple, complete. EventStore is source of truth. Phase 1 projection pattern. | |
| TaskGraphIR stored as projection artifact in .m31a/tasks/<run-id>/taskgraph.json (like Phase 1 projections). Events only for transitions. Resume loads projection + replays recent events | Separation: IR = projection (human-readable, queryable). Events = transitions. Faster resume (load projection + few events). Follows Phase 1 checkpoint/projection pattern. | |
| TaskGraphIR in EventStore as TaskGraphCreated event. Checkpoint projections every N events (Phase 1 D-13). Resume: load latest checkpoint + replay events since checkpoint | Phase 1 D-13/D-14: checkpoint every N events with full state. Resume = checkpoint + incremental replay. Scales to large TaskGraphs. Most robust. | ✓ |

**User's choice:** TaskGraphIR in EventStore as TaskGraphCreated event. Checkpoint projections every N events (Phase 1 D-13). Resume: load latest checkpoint + replay events since checkpoint
**Notes:** Follows Phase 1 checkpoint/projection pattern. Scales to large TaskGraphs. Most robust.

---

## the agent's Discretion

- Exact checkpoint interval (N events) and retention count for TaskGraph checkpoints — tunable via config
- Exact task_timeout and wave_timeout defaults — tune based on observed workloads
- LLM prompt template for REPAIR patch generation — implementation detail
- Lock name conventions for Task.LockNames[] — establish pattern
- Output formatting for TUI S09/S10 screens — projection concern

---

## Deferred Ideas

None — discussion stayed within phase scope.