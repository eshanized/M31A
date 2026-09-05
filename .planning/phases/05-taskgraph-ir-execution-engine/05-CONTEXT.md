# Phase 5: TaskGraph IR & Execution Engine - Context

**Gathered:** 2026-09-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Executable TaskGraph IR with full schema (Objectives, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]) + validated execution state machine (PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED) with wave-based parallel execution via Kahn's algorithm. Covers TASKGRAPH-01 through TASKGRAPH-06.

</domain>

<decisions>
## Implementation Decisions

### TaskGraph IR Schema Completeness
- **D-01:** Create new TaskGraphIR type separate from Task/Plan — **Reversibility:** costly — New type in internal/core/types/planning.go with all TASKGRAPH-01 fields (Objective, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]). Task/Plan remain user-facing; IR is internal executable form.
- **D-02:** TaskGraphIR lives in internal/core/types/planning.go alongside Plan — **Reversibility:** reversible — Keeps planning-related types together. Plan has CompileToIR() method that produces TaskGraphIR from requirements, decisions, tasks.
- **D-03:** Plan.CompileToIR() method on Plan type handles compilation to TaskGraphIR — **Reversibility:** reversible — Plan owns compilation logic. Single method, easy to test. Takes requirements, decisions, tasks and produces fully populated TaskGraphIR.

### Execution State Machine Design
- **D-04:** Replace TaskStatus with new ExecutionState enum matching requirements exactly: PLANNED, READY, RUNNING, WAITING, VERIFYING, COMPLETED, FAILED — **Reversibility:** costly — Clean break from legacy TaskStatus. Update all consumers (runner, TUI, events). ExecutionState type owns transition validation.
- **D-05:** ExecutionState type with TransitionTo(next) method that validates and returns error on invalid transitions — **Reversibility:** reversible — State machine logic encapsulated in type. ExecutionState.TransitionTo(VERIFYING) returns error if current != WAITING. Self-documenting, testable.
- **D-06:** FAILED state carries RecoveryAction enum (RETRY, REPAIR, REPLAN, ESCALATE); Runner interprets and transitions accordingly — **Reversibility:** reversible — Decouples state from recovery logic. FAILED is terminal for current attempt; RecoveryAction encodes next step. Runner reads action and transitions.

### Checkpoint Gate Integration
- **D-07:** Checkpoints as TaskGraphIR.Checkpoints[] with type (ONE_WAY_DOOR/REVIEW); Runner pauses wave, emits CheckpointRequested event, waits for CheckpointApproved event — **Reversibility:** reversible — Checkpoints part of IR. Runner checks before executing wave; emits event via EventStore, blocks until approval event. TUI handles approval UI.
- **D-08:** Hybrid EventStore + in-memory channel for approval flow: EventStore for durability (CheckpointRequested, CheckpointApproved), Runner uses channel for fast resume; TUI writes to both — **Reversibility:** reversible — Follows Phase 4 D-36 (WritePending persists before blocking). EventStore captures decision for audit/replay. In-memory channel avoids polling. TUI writes event + sends MsgEmitter. Runner reads event on startup for recovery.
- **D-09:** Checkpoint per task via TaskGraphIR.Checkpoints[]; each has type, required_approvers, evidence_refs. Config only sets defaults — **Reversibility:** reversible — Maximum flexibility. Each checkpoint in IR specifies exactly what's needed. Config provides defaults for tasks without explicit checkpoints. IR is authoritative.

### Failure Recovery Strategies
- **D-10:** RecoveryAction enum on FAILED state (as chosen). Runner reads action: RETRY→reset to READY; REPAIR→apply patch then RETRY; REPLAN→request new TaskGraphIR from Planner; ESCALATE→emit event, pause for human — **Reversibility:** reversible — Direct mapping. FAILED state carries RecoveryAction. Runner interprets and transitions. EventStore records each recovery attempt.
- **D-11:** Task tracks retry_count; RETRY allowed if retry_count < MaxRetries. REPAIR: Runner calls LLM (via provider) with failure context to generate patch; applies patch via tools; then RETRY — **Reversibility:** reversible — Retry count per task. MaxRetries from Runner config. REPAIR uses provider to generate fix from error + code context. Patch applied via FileEdit tool. Then retry.
- **D-12:** REPLAN: Runner emits ReplanRequested event with failure context; Planner agent (Phase 6) creates new TaskGraphIR; Runner swaps IR and restarts from PLANNED. ESCALATE: emits EscalationRequested event; TUI shows S16 Failure/Recovery; human chooses action — **Reversibility:** reversible — Event-driven. REPLAN triggers planner agent. ESCALATE shows S16 screen with RETRY/REPAIR/REPLAN/STOP options. Human decision becomes new RecoveryAction.

### Wave Execution & Parallelism Control
- **D-13:** Config: [execution] max_parallel_tasks = 4 (default), task_timeout = 30m, wave_timeout = 0 (no limit). Runner reads from config.toml layered loading — **Reversibility:** reversible — Follows Phase 1 config pattern. max_parallel_tasks replaces DefaultMaxParallelTasks. task_timeout already exists. wave_timeout adds safety for stuck waves.
- **D-14:** Optional mutex/lock names in TaskGraphIR; tasks with same lock name run sequentially even in same wave — **Reversibility:** reversible — For resources not captured by dependencies (e.g., same file, same DB). Task.LockNames[] = ["db-migrate", "config-write"]. Runner acquires lock before task. More realistic.
- **D-15:** Wave completes when all tasks in group reach terminal state (COMPLETED/FAILED/SKIPPED). Partial failure: failed tasks block dependents (set to SKIPPED), wave continues for independent tasks. Next wave only starts when current wave fully resolved — **Reversibility:** reversible — Current runner.go behavior. Failed task -> dependents SKIPPED. Wave boundary = synchronization point. Simple, predictable.

### TaskGraph Persistence & EventStore Events
- **D-16:** Fine-grained events per transition: TaskGraphCreated, TaskGraphValidated, TaskPlanned, TaskReady, TaskRunning, TaskWaiting, TaskVerifying, TaskCompleted, TaskFailed, TaskRetried, TaskRepaired, TaskReplanned, TaskEscalated, CheckpointRequested, CheckpointApproved, CheckpointDenied — **Reversibility:** costly — One event per state transition. Maximum traceability. EventStore becomes complete audit log. Aligns with Phase 1 event vocabulary (CONTEXT_M31A.md §11).
- **D-17:** Runner calls EventStore.Append() on every ExecutionState transition via TransitionTo(). Event includes task_id, from_state, to_state, timestamp, run_id, attempt_number. Projections rebuild exact state — **Reversibility:** reversible — TransitionTo() emits event. Runner subscribes to its own events for recovery. On resume, replays events to reconstruct TaskGraphIR execution state. Follows Phase 1 pattern.
- **D-18:** TaskGraphIR in EventStore as TaskGraphCreated event. Checkpoint projections every N events (Phase 1 D-13). Resume: load latest checkpoint + replay events since checkpoint — **Reversibility:** costly — Phase 1 D-13/D-14: checkpoint every N events with full state. Resume = checkpoint + incremental replay. Scales to large TaskGraphs. Most robust.

### the agent's Discretion
- Exact checkpoint interval (N events) and retention count for TaskGraph checkpoints — tunable via config
- Exact task_timeout and wave_timeout defaults — tune based on observed workloads
- LLM prompt template for REPAIR patch generation — implementation detail
- Lock name conventions for Task.LockNames[] — establish pattern
- Output formatting for TUI S09/S10 screens — projection concern

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Requirements
- `.planning/CONTEXT_M31A.md` §8.5 — TaskGraph IR schema (Objectives, Requirements, Constraints, Decisions, Tasks, Dependencies, Preconditions, AcceptanceCriteria, VerificationSteps, RiskLevel, Checkpoints)
- `.planning/CONTEXT_M31A.md` §10 — Execution state machine (PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED, failure states, recovery actions)
- `.planning/CONTEXT_M31A.md` §11 — Event model (minimum event vocabulary including TaskCreated, TaskReady, AgentStarted, ToolCallRequested, CheckpointRequested, CheckpointApproved, TaskCompleted, TaskFailed)
- `.planning/CONTEXT_M31A.md` §31 — Checkpoint classification (ONE_WAY_DOOR vs SAFE_AUTOMATION, human approval requirement)
- `.planning/REQUIREMENTS.md` — TASKGRAPH-01 through TASKGRAPH-06 (6 requirements)
- `.planning/ROADMAP.md` — Phase 5 success criteria (5 criteria)

### Prior Phase Decisions (binding)
- `.planning/phases/01-foundation-domain-model-event-store/01-CONTEXT.md` — EventStore patterns, schema_versioning, projection checkpoints, Event types, SQLite WAL mode
- `.planning/phases/02-llm-provider-abstraction/02-CONTEXT.md` — Provider interface, streaming, model profiles, BaseClient retry/resilience patterns
- `.planning/phases/03-code-intelligence-graph/03-CONTEXT.md` — CodeIntel query API (Define/References/Upstream/Downstream/Impact), Kahn's algorithm precedent in impact analysis
- `.planning/phases/04-intelligence-features/04-CONTEXT.md` — Checkpoint patterns (D-34, D-35, D-36), evidence-first patterns, 3-level confidence enum

### Current Implementation (Reusable Assets)
- `internal/engine/taskrunner/runner.go` — Kahn's algorithm implementation, wave-based ExecuteGroup with semaphore, TaskStatus enum, retry logic with exponential backoff
- `internal/core/types/planning.go` — Task, TaskGraph, Plan, Requirement, Decision types (need extension for IR)
- `internal/core/types/types.go` — RiskLevel, TaskStatus, DefaultMaxParallelTasks, BashTimeout constants
- `internal/core/config/loader.go` — Layered TOML config loading (global → workspace → project → env)
- `internal/integrations/provider/registry.go` — Provider registry for LLM calls (needed for REPAIR)
- `internal/memory/eventstore/` — EventStore interface and SQLite implementation (Append, QueryByType/range, subscriptions)

### Research & Risks
- `.planning/research/PITFALLS.md` — Pitfall 3: State machine validation, Pitfall 6: Task recovery, Pitfall 9: Event store durability
- `.planning/codebase/ARCHITECTURE.md` — Current workflow engine, MsgEmitter pattern, single-threaded UI constraint

### Integration Points
- **TaskGraphIR → Runner**: Compiled IR feeds Runner.New() for scheduling and execution
- **Runner → EventStore**: Every ExecutionState transition emits event via TransitionTo()
- **Runner → Provider (Phase 2)**: REPAIR strategy calls LLM via provider for patch generation
- **Runner → TUI (Phase 10/11)**: MsgEmitter sends TaskStartMsg, TaskUpdateMsg, CheckpointRequestedMsg; TUI subscribes
- **Runner → Tools (Phase 7)**: REPAIR applies patches via FileEdit tool; tasks execute via Dispatcher
- **Checkpoints → TUI S13**: CheckpointRequested event triggers S13 Checkpoint screen; approval writes CheckpointApproved event
- **Failure Recovery → TUI S16**: EscalationRequested event triggers S16 Failure/Recovery screen

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **TaskRunner** (`internal/engine/taskrunner/runner.go:26-364`): Kahn's algorithm Schedule(), ExecuteGroup() with bounded parallelism, retry logic, status/results tracking. Extend with ExecutionState, checkpoint handling, EventStore integration.
- **Plan/Task/TaskGraph types** (`internal/core/types/planning.go:84-197`): Plan, Task, TaskGraph, Requirement, Decision with JSON marshaling. Add TaskGraphIR type, CompileToIR() method, ExecutionState enum.
- **EventStore** (`internal/memory/eventstore/`): Append, QueryByType/range, subscriptions. TaskGraphCreated, TaskStateChanged, CheckpointRequested/Approved events land here.
- **Config loader** (`internal/core/config/loader.go`): Layered TOML — new `[execution]` section follows existing section patterns.
- **Provider registry** (`internal/integrations/provider/registry.go`): GetProviderForRequest for REPAIR LLM calls.

### Established Patterns
- **EventStore pattern** (Phase 1): Append-only events, in-memory projections, SQLite durability — TaskGraph execution follows same pattern
- **MsgEmitter pattern** (Architecture): Channel `chan tea.Msg` bridges Runner (goroutine) → TUI (main thread) — Runner emits TaskStartMsg, TaskUpdateMsg, CheckpointRequestedMsg
- **Lazy provider initialization** (Phase 2): Providers registered on first LLM call — REPAIR uses same pattern
- **Project-local sessions** (Phase 1): `.m31a/tasks/<run-id>/` per run — TaskGraph checkpoints follow this pattern
- **Config layering** (Phase 1): Global → workspace → project → env — `[execution]` section follows same pattern

### Integration Points
- **EventStore → Runner projections**: TaskGraphCreated, TaskStateChanged events rebuild execution state on resume
- **Runner → TUI**: S09 Run Dashboard, S10 Task Detail consume Runner state via MsgEmitter
- **Runner → Provider**: REPAIR strategy uses provider for patch generation
- **Runner → Tools Dispatcher**: Task execution via Dispatcher.Execute() (Phase 7)
- **CodeIntel → Runner**: Impact analysis for dependency validation (Phase 3)
- **Checkpoints → TUI**: S13 Checkpoint screen consumes CheckpointRequested events

</code_context>

<specifics>
## Specific Ideas

- TaskGraphIR structure:
  ```go
  type TaskGraphIR struct {
      ID        uuid.UUID
      PlanID    uuid.UUID
      Objective string
      Requirements []uuid.UUID
      Constraints []string
      Decisions   []uuid.UUID
      Tasks       []TaskIR
      Dependencies []TaskDependency
      Preconditions []Precondition
      AcceptanceCriteria []string
      VerificationSteps []VerificationStep
      RiskLevel   RiskLevel
      Checkpoints []Checkpoint
      Waves       []Wave
  }
  ```
- Checkpoint struct:
  ```go
  type Checkpoint struct {
      ID            string
      Type          CheckpointType // ONE_WAY_DOOR, REVIEW
      TaskIDs       []int
      RequiredApprovers int
      EvidenceRefs  []string
      Description   string
  }
  ```
- ExecutionState with TransitionTo():
  ```go
  type ExecutionState string
  const (Planned, Ready, Running, Waiting, Verifying, Completed, Failed ExecutionState = ...)
  func (s ExecutionState) TransitionTo(next ExecutionState) error { /* validate */ }
  ```
- RecoveryAction enum: RETRY, REPAIR, REPLAN, ESCALATE
- Config section:
  ```toml
  [execution]
  max_parallel_tasks = 4
  task_timeout = "30m"
  wave_timeout = "0"
  checkpoint_interval = 100
  ```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---
*Phase: 05-TaskGraph IR & Execution Engine*
*Context gathered: 2026-09-05*