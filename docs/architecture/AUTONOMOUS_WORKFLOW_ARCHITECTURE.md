# Autonomous Workflow Orchestration Architecture

**System:** M31A Autonomous Engineering Platform  
**Subsystem:** Workflow Orchestration, Directed Acyclic Graph (DAG) Planning, Wave Scheduling, and Lease Recovery  
**Primary Modules:** `m31a-agent-engine::workflow`, `m31a-agent-engine::waves`, `m31a-storage::workflow_store`, `m31a-core::workflow`  
**Version:** 1.0.0 (Foundation Milestone)

---

## 1. Mission and Core Subsystems

The Autonomous Workflow Orchestration subsystem enables M31A to execute substantial software engineering objectives across multiple hours through structured task decomposition, topological wave concurrency, distributed worker leasing, and durable state persistence.

Rather than running unstructured LLM loops, M31A coordinates execution through five interconnected architectural layers:

```
┌────────────────────────────────────────────────────────────────────────┐
│                   CLI / TUI / App Server JSON-RPC                      │
│       (m31a workflow run/status/inspect, Screen 5, RPC endpoints)      │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        WorkflowOrchestrator                            │
│    - Workflow Lifecycle Management (Created -> Running -> Completed)   │
│    - DAG Plan Generation & Replanning Revisions                        │
│    - Wave Execution & Post-Wave Hard Verification                      │
│    - Speculative Arena Stage Invocation                                │
└─────────────┬─────────────────────┬──────────────────────┬─────────────┘
              │                     │                      │
              ▼                     ▼                      ▼
┌───────────────────────┐ ┌───────────────────┐ ┌────────────────────────┐
│     WaveScheduler     │ │EngineAgentRuntime │ │     WorkflowStore      │
│ - Topological sort    │ │ - Isolated child  │ │ - SQLite Migration 15  │
│ - Write-path conflict │ │   agents          │ │ - Worker lease claiming│
│   partitioning        │ │ - Worktrees       │ │ - Stale reconciliation │
│ - Concurrency bounds  │ │ - Tool surface    │ │ - Content-addressed   │
│ - Verification gates  │ │ - Depth control   │ │   SHA-256 artifacts    │
└───────────────────────┘ └───────────────────┘ └────────────────────────┘
```

---

## 2. DAG Plan Generation and Wave Partitioning

### 1. Architectural Plan Decomposition & Typed Task Kinds
When an engineering objective is received:
1. `WorkflowOrchestrator::generate_plan` inspects project structure, languages, build files, and configuration.
2. Identifies whether speculative execution is beneficial (for optimization, performance, refactoring, or algorithmic redesign).
3. Produces a `WorkflowPlan` consisting of discrete, verifiable `WorkflowTask` nodes typed with `WorkflowTaskKind`:
   - `Analysis`: Read-only structural inspection and baseline verification.
   - `Implementation`: Source modification by genuine child agents in isolated Git worktrees.
   - `Speculative`: Parallel candidate generation, execution, and hard-gated competition.
   - `Integration`: Patch validation and atomic commit into the primary workspace.
   - `Verification`: Native test suite, compiler checks, and blast-radius safety gating.
   - `Delivery`: Structured report generation and artifact finalization.

### 2. Topological Validation (Kahn's Algorithm)
Before execution, `WorkflowPlan::validate` performs strict acyclicity checking:
- In-degree calculation for all task dependency edges.
- Kahn's algorithm cycle detection: detects any circular dependencies and rejects unexecutable DAGs.
- Validates that all declared dependency IDs correspond to known tasks.
- Verifies uniqueness of task identifiers.

### 3. File Write-Path Conflict Partitioning
Tasks that depend on earlier tasks must run in later waves. Furthermore, tasks that are topologically independent but declare overlapping file write paths (`declared_write_paths`) must not execute concurrently in shared workspaces:
- `WorkflowPlan::partition_into_waves` groups independent tasks into execution waves ($W_1, W_2, \dots, W_n$).
- Two independent tasks with conflicting write paths are partitioned into sequential waves to prevent race conditions and concurrent workspace corruption.

---

## 3. Worker Leasing, Dependency Fencing, and Stale Task Recovery

To ensure robust local execution that survives daemon crashes, process restarts, or unexpected worker failures, M31A implements a lightweight distributed leasing model atop SQLite with Migration 16 optimistic concurrency:

```
[Task: Ready] ──(claim_ready_task with deps check)──> [Task: Claimed] (lease_expires = now + 60s, attempt++)
                                                              │
                                                      (task begins run)
                                                              │
                                                              ▼
                                                      [Task: Running]
                                                              │
                      ┌───────────────────────────────────────┴───────────────────────────────────────┐
                      │ (worker crash / unrenewed)                                                    │ (normal execution)
                      ▼                                                                               ▼
          [reconcile_stale_tasks]                                                             [Task: Verifying]
             (lease_expires < now)                                                                    │
                      │                                                                               ▼
      ┌───────────────┴───────────────┐                                                       [Task: Completed]
      ▼                               ▼                                                    (with TaskExecutionResult)
 [retry_count < max]         [retry_count >= max]
[Task: RetryPending]           [Task: Failed]
 (rescheduled for pickup)
```

1. **Dependency-Gated Atomic Claiming:**  
   `WorkflowStore::claim_ready_task` uses an immediate SQLite transaction to locate the highest-priority, lowest-wave `ready` or `retry_pending` task. A SQL subquery guarantees that a task is claimed **only when all its declared upstream dependencies have reached `status = 'completed'`**. Claiming atomically assigns `worker_id`, increments `attempt_id`, sets `status = 'claimed'`, and establishes an epoch lease expiration timestamp (`lease_expires_at_epoch_ms`).

2. **Optimistic Concurrency & Worker Fencing:**  
   `WorkflowStore::update_task_execution_result` verifies `expected_attempt_id` and `expected_worker_id` before committing results. If a worker lease expired and was reassigned or failed, any late result commit is rejected, preventing zombie tasks from corrupting state.

3. **Heartbeat & Lease Renewal:**  
   Active workers periodically call `WorkflowStore::renew_task_lease` during execution, extending the lease boundary.

4. **Stale Task Reconciliation:**  
   `WorkflowStore::reconcile_stale_tasks` queries for tasks where `lease_expires_at_epoch_ms < now_epoch_ms` in `claimed`, `running`, or `verifying` states:
   - If `retry_count < max_retries`: increments `retry_count`, clears `worker_id`, and reschedules task to `WorkflowTaskStatus::RetryPending`.
   - If `retry_count >= max_retries`: marks task `WorkflowTaskStatus::Failed` with descriptive error details.

---

## 4. Append-Only Audit Trail and Content-Addressed Artifacts

### 1. Audit Event Ledger
Every state transition, agent assignment, tool invocation, verification result, and human approval decision is durably recorded in the `workflow_events` table:
- Append-only schema: entries are never updated or deleted.
- Fields: `id`, `workflow_id`, `task_id`, `agent_id`, `event_type`, `timestamp_epoch_ms`, `correlation_id`, `payload_json`, `schema_version`.
- Allows full forensic replay, debugging, and audit reporting.

### 2. Content-Addressed Artifact Storage
Outputs produced by workflow tasks (such as analysis reports, AST blast-radius scans, compiler diagnostics, speculative comparison reports, and patch diffs) are stored immutably:
- `WorkflowStore::store_artifact_file` computes the SHA-256 hash of the content.
- Files are saved atomically via temp files in `.m31a/artifacts/<workflow_id>/<sha256_prefix>_<name>`.
- Metadata is recorded in the `workflow_artifacts` database table, linking artifacts to their originating task and workflow.

---

## 5. Crash Resilience and Resume Idempotence

M31A workflows survive unexpected process termination without data corruption or loss of progress:
1. When `WorkflowOrchestrator::resume_workflow` is invoked:
   - Queries `WorkflowStore` for existing workflow metadata and plan revision.
   - Reconciles any stale task leases left by crashed workers.
   - Identifies remaining uncompleted waves and tasks.
   - Resumes wave scheduling from the exact point of interruption without re-executing already verified and completed tasks.
2. If all tasks in a resumed workflow are already `Completed`, the orchestrator verifies workspace integrity and returns `WorkflowState::Completed` idempotently.
