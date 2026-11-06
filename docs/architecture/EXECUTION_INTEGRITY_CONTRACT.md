# M31A Execution Integrity Contract

**Document Version:** 1.0.0  
**Classification:** Core System Architecture Specification  
**Status:** Authoritative  
**Target Architecture:** Rust 2024 / M31A 39-crate Workspace

---

## 1. Governing Invariant

> **M31A may report an engineering task as completed only when the requested work was genuinely executed, its actual result was captured, its changes were safely integrated, and its required verification evidence was successfully persisted.**

Under no circumstances may successful process creation, unchanged workspace compilation, an agent's conversational claim, or an uninspected wait request be interpreted as task completion.

---

## 2. Core Execution Lifecycle Contracts

### 2.1 Workflow Execution Contract

1. **State Machine Integrity:**
   - A workflow must transition strictly through valid states:  
     `Created` → `Analyzing` → `Planning` → `PlanReady` → `Running` → `Verifying` → `Completed` | `Failed` | `Cancelled`.
   - Transitions to terminal states (`Completed`, `Failed`, `Cancelled`) are permanent.
   - A workflow may enter `Completed` **if and only if**:
     - All tasks in the plan revision have reached `WorkflowTaskStatus::Completed`.
     - All write-path modifications have been verified and integrated into the primary workspace.
     - The final post-integration verification suite (`verify_check`, `verify_tests`, `verify_safety`) passes cleanly on the primary workspace.
     - Final delivery artifacts are content-addressed (SHA-256) and persisted in storage.
     - No unhandled errors, unexpired active worker leases, or pending approvals remain.

2. **Runtime Prerequisite Check:**
   - Execution of implementation or speculative workflows requires an initialized and verified `ProviderClient` and `EngineAgentRuntime`.
   - If the provider or runtime is unavailable or misconfigured, the workflow must fail immediately with a typed error (`WorkflowRuntimeUnavailable`). It must never silently downgrade to a compiler check against an unchanged workspace.

### 2.2 Task Execution Contract

Each task in a workflow is identified by a typed `WorkflowTaskKind`:
- `Analysis`: Read-only workspace inspection and architecture discovery.
- `Implementation`: Source-code modification by a genuine child agent.
- `Speculative`: Parallel competing candidate generation, execution, and evaluation.
- `Integration`: Controlled application and verification of candidate patches.
- `Verification`: Comprehensive build, test, and blast-radius safety gating.
- `Delivery`: Final reporting and artifact consolidation.

1. **Task State Transitions:**
   - Valid lifecycle: `Pending` → `Ready` → `Claimed` → `Running` → `Verifying` → `Completed` | `Failed` | `RetryPending` | `NeedsApproval` | `Cancelled`.
   - A task cannot become `Ready` until all its declared dependencies in `dependencies` have reached `Completed`.
   - A task cannot enter `Completed` without a persisted `TaskExecutionResult` containing:
     - `task_id` and `workflow_id`.
     - `attempt_id` and `worker_id`.
     - `agent_id` (for agent-backed tasks).
     - Non-empty `modified_files` and `patch_content` (for implementation tasks).
     - Concrete verification evidence (exit codes, diagnostics, duration).

2. **Strict Failure Closedness:**
   - If a child agent fails to spawn → Task fails (`SpawnFailed`).
   - If a child agent execution times out → Task fails (`Timeout`).
   - If a child agent terminates without code changes on an implementation task → Task fails (`NoImplementation`).
   - If a verification gate fails or returns an error → Task fails (`VerificationFailed`).
   - If a worker lease expires before completion → Task is marked `RetryPending` or `Failed` by reconciliation; the stale worker is fenced from committing results.

### 2.3 Agent Runtime Execution Contract

1. **Child Agent Lifecycle:**
   - Child agents are spawned using `EngineAgentRuntime::spawn_agent` in an isolated Git worktree.
   - Spawning creates a dedicated `AgentEngine` with its own sandbox policy, tool registry, and turn loop.
   - The orchestrator waits on the child agent via `EngineAgentRuntime::wait_agents` with an explicit timeout.
   - The outcome is strictly inspected:
     - `AgentRunStatus::Completed` or `AgentRunStatus::IntegrationPending`: Valid completion.
     - `AgentRunStatus::Failed(err)`: Explicit execution failure.
     - `AgentRunStatus::Cancelled`: Explicit cancellation.
     - `AgentRunStatus::Running` after deadline: Timeout failure; child agent is cancelled via `CancellationToken`.

2. **Child Worktree Isolation & Cleanup:**
   - Every child agent requiring write access executes inside an isolated Git worktree allocated by `WorktreeManager`.
   - Primary workspace files are never directly exposed to concurrent child writes.
   - Worktrees containing diagnostic evidence or pending changes are preserved until evaluation and integration complete.
   - Once integrated or definitively rejected, worktrees are pruned cleanly using RAII drop guards or explicit cleanup.

---

## 3. Speculative Execution Contract

### 3.1 Candidate Implementation & Diversity

1. **Hypothesis Formulation:**
   - Candidate hypotheses must represent distinct, task-relevant engineering strategies (e.g. algorithmic complexity vs zero-copy structures vs parallel pipeline).
   - Each candidate receives a distinct `branch_id`, an isolated workspace sandbox, and explicit acceptance criteria.

2. **Genuine Candidate Execution:**
   - Every candidate is executed via a production `CandidateExecutor` (`AgentEngineCandidateExecutor`) backed by `EngineAgentRuntime`.
   - The executor runs the agent inside the candidate's isolated worktree, captures modified files, generates a unified diff patch, and records execution metrics.
   - Candidates that fail to produce non-empty diffs are rejected at Gate 1 with `NoImplementation` and receive a score of `0.0`.

### 3.2 Sequential Hard Verification Gates

Candidates must sequentially pass five mandatory gates:

| Gate | Requirement | Failure Behavior |
|---|---|---|
| **Gate 1: Implementation** | $\ge 1$ modified file, valid patch containment, no directory traversal | State `Rejected`, score `0.0` |
| **Gate 2: Compilation** | `verify_check` succeeds in sandbox | State `Failed`, score `0.0` |
| **Gate 3: Test Suite** | `verify_tests` succeeds in sandbox | State `Failed`, score `0.0` |
| **Gate 4: Code Safety** | `CodebaseSafetyGate::audit_workspace_changes` passes (blast radius $\le 85.0$, no critical taints) | State `Rejected`, score `0.0` |
| **Gate 5: Performance** | Baseline vs candidate comparative benchmark execution (if requested) | Score scaled; unmeasured marked `NotMeasured` |

### 3.3 Safe Transactional Promotion & Rollback

1. **Preflight Cleanliness Verification:**
   - The primary workspace must be verified clean before promotion (`git status --porcelain`).
   - If the cleanliness check encounters an error, promotion is **blocked**. Inability to inspect is never treated as clean.
2. **Path & Hunk Validation:**
   - All patch hunks are validated against directory traversal (`..`) and protected workspace targets.
3. **Transactional Application:**
   - Patch is applied cleanly to the primary working tree.
   - Post-integration verification (`verify_check` and `verify_tests`) is executed immediately on the primary workspace.
4. **Deterministic Rollback:**
   - If post-integration verification fails, rollback reverts **only** the exact files modified by the applied patch.
   - Broad destructive shell commands (`git checkout -- .`, `git clean -fd`) are **strictly prohibited**.
   - Original uncommitted user files and untracked artifacts outside the patch scope remain untouched.

---

## 4. Durable Scheduling & Worker Leasing Contract

### 4.1 Atomic Task Claiming

1. **SQLite Immediate Transaction:**
   - `claim_ready_task` executes inside an immediate SQLite transaction.
   - Selects tasks with `status IN ('ready', 'retry_pending')` whose declared dependencies are all in `completed` status.
   - Ordering: `wave_index ASC, priority DESC, id ASC`.
2. **Fencing Token (`attempt_id`):**
   - Each claim increments the task's `attempt_id`.
   - The lease is granted to `worker_id` until `lease_expires_at_epoch_ms`.
   - State updates must match both `worker_id` and `attempt_id`. A stale worker whose lease expired cannot overwrite a newer worker's state.

### 4.2 Lease Heartbeats & Reconciliation

1. **Heartbeat:**
   - Running tasks renew their lease via `renew_task_lease` periodically during active execution.
2. **Reconciliation:**
   - If `lease_expires_at_epoch_ms < now`, the task lease has expired.
   - `reconcile_stale_tasks` transitions the task to `RetryPending` if `retry_count < max_retries`, or `Failed` if the retry budget is exhausted.

---

## 5. Recovery & Cancellation Contract

### 5.1 Idempotent Resume

When `resume_workflow` is invoked:
1. Reconcile stale leases.
2. Identify completed tasks: completed tasks are **never re-executed**.
3. Identify active valid leases: tasks with active leases are not duplicated.
4. Reconstruct the remaining DAG of pending, ready, and retryable tasks.
5. Resume execution strictly from the incomplete wave.

### 5.2 Real Cancellation Propagation

When `cancel_workflow` is invoked:
1. Workflow state transitions to `Cancelled`.
2. Active cancellation tokens (`CancellationToken`) trigger child agent termination.
3. In-flight task outcomes are reconciled; unfinished tasks enter `Cancelled`.
4. Child worktrees are cleaned up safely.
5. No patches are applied or promoted after cancellation.
