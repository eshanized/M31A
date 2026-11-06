# M31A Workflow Execution Integrity Architecture

**Status:** Authoritative Architectural Specification  
**Scope:** M31A Modular Rust 2024 Workspace (39 crates)  
**Governing Invariant:** Fail-Closed Execution Integrity  

---

## 1. Governing Invariant & Core Tenets

M31A enforces a strict, fail-closed execution integrity invariant across all autonomous workflows:

> **Governing Invariant:**  
> M31A may report a workflow task as completed only when its actual execution has succeeded, all required acceptance criteria have been independently verified, all mandatory evidence has been persisted successfully, and any required integration has been authorized, applied, and verified.

Under this governing invariant, a workflow must **fail closed** (reporting `Failed` or `Cancelled`) whenever:
- Execution fails, times out, or errors out.
- A worker loses its lease or fails to renew it within the lease duration.
- A stale or superseded worker attempts to commit task results.
- A required human approval is missing, rejected, or tampered with.
- An integration preflight, patch application, build, test, review, or git commit fails.
- Workspace containment or path safety invariants are violated (e.g., symlink escape or path traversal).
- Required verification checks cannot run (e.g., unsupported project manifest).
- Mandatory evidence (artifacts, reports, execution logs) fails to persist.
- Workflow cancellation has been requested.
- Crash recovery cannot independently establish the integrity of previously completed work.

---

## 2. End-to-End Execution Topology

M31A's workflow execution topology connects operator interfaces down through orchestration, durable storage, wave scheduling, isolated worktree execution, and the authoritative integration service:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           Operator & Supervision Layer                          │
│     CLI (`m31a workflow`)  │  App Server JSON-RPC  │  Interactive TUI Terminal   │
└───────────────────────────────────────┬─────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                   WorkflowOrchestrator (m31a-agent-engine)                      │
│ - Workflow State Machine (Pending → Planning → Executing → Verifying → Complete)│
│ - CancellationToken & Tree-wide Agent Termination                               │
│ - Autonomous Wave Scheduler & Plan DAG Execution                                │
│ - 12-Point Final Workflow Completion Gate                                       │
└───────────────┬───────────────────────┬─────────────────────────┬───────────────┘
                │                       │                         │
     Durable Claims & Leases            │ Task Wave Concurrency   │ Speculative Arena
                │                       │                         │
                ▼                       ▼                         ▼
┌───────────────────────────┐ ┌──────────────────┐ ┌──────────────────────────────┐
│  WorkflowStore (Storage)  │ │   WaveScheduler  │ │       SpeculativeArena       │
│ - SQLite (IMMEDIATE tx)   │ │ - JoinSet tasks  │ │ - Parallel Hypotheses        │
│ - Task Claims & Fencing   │ │ - Wave barriers  │ │ - Isolated Worktree Evaluator│
│ - Lease Heartbeats        │ │ - Conflict check │ │ - Porcelain -z Git Parser    │
│ - Durable Approval Engine │ └────────┬─────────┘ └──────────────┬───────────────┘
└───────────────────────────┘          │                          │
                                       ▼                          │
                        ┌──────────────────────────────┐          │
                        │ EngineAgentRuntime (Isolated)│          │
                        │ - Per-task Git Worktrees     │          │
                        │ - Sandboxed Tool Execution   │          │
                        │ - File Modification Capture  │          │
                        └──────────────┬───────────────┘          │
                                       │                          │
                                       │ Raw Patch & Diff         │ Promoted Winner
                                       ▼                          ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│               Authoritative WorkflowIntegrationService (Single Path)             │
│ - Preflight Containment & Path Traversal Prevention                             │
│ - Change-Set SHA-256 Hash Calculation & Validation                              │
│ - Durable Human Approval State Verification                                     │
│ - Per-File Pre-Integration Snapshots & Atomic Rollback                          │
│ - Unified Patch Application to Primary Workspace                                │
│ - Verification Engine Gate (Build, Test, Clippy, Safety)                        │
│ - Review Engine Gate (Critical Finding Block)                                   │
│ - Git Attribution Commit & Atomic Error Handling                                │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Durable Task Lifecycle & Lease Fencing

### 3.1 Task Lifecycle State Machine

Tasks transition through explicit, durable states:

```
Pending ──► Ready ──► Running ──► Verifying ──► Completed
              ▲          │             │
              │          ▼             ▼
              └── RetryPending     Failed / Cancelled
```

- **Pending:** Task created, awaiting prerequisite tasks in the workflow dependency DAG.
- **Ready:** Prerequisite dependencies satisfied; eligible for worker claim.
- **Running:** Claimed by a worker instance holding a valid lease; executing agent actions.
- **Verifying:** Execution finished; post-task verification checks in flight.
- **Completed:** Task execution, verification, evidence persistence, and required integration succeeded.
- **RetryPending:** Task failed transiently or worker lease expired; scheduled for backoff retry.
- **Failed / Cancelled:** Task terminated permanently due to fatal error, unrecoverable invariant violation, or operator cancellation.

### 3.2 Immediate SQLite Transactions

To prevent race conditions during concurrent worker claims, `WorkflowStore::claim_next_task` uses SQLite `IMMEDIATE` transaction behavior:

```rust
let tx = conn
    .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
    .map_err(|e| StorageError::LockError(format!("Failed to begin immediate claim transaction: {e}")))?;
```

`IMMEDIATE` transactions acquire a reserved lock on the SQLite database immediately at transaction start, preventing multiple workers from selecting the same ready candidate before upgrading to write locks.

### 3.3 Lease Fencing & Stale Worker Rejection

Each task claim grants an exclusive lease:
- **`worker_id`:** Unique identifier of the claiming worker node or process.
- **`current_attempt`:** Monotonically increasing attempt counter.
- **`lease_expires_at`:** Unix timestamp marking lease expiry (computed using checked arithmetic).

When persisting task results, `WorkflowStore::save_task_execution_result` verifies the fenced invariant:

```sql
UPDATE workflow_tasks
SET status = ?1, ...
WHERE id = ?2 
  AND worker_id = ?3 
  AND current_attempt = ?4 
  AND lease_expires_at > ?5
```

If the lease has expired or another worker claimed the task (incrementing `current_attempt`), the update matches 0 rows and fails closed with `StorageError::FencingTokenMismatch`. Stale workers can never overwrite state or commit results.

### 3.4 Active Lease Heartbeat Protocol

While a task is running, the worker runs a background heartbeat task that periodically calls:

```rust
store.renew_task_lease(&task_id, &worker_id, attempt, lease_duration_ms)?;
```

The store validates that:
1. The task exists and matches `worker_id` and `current_attempt`.
2. The workflow is in an active state (`Running` or `Verifying`).
3. The lease has not already expired past recovery tolerance.

Upon renewal, `lease_expires_at` is extended by `lease_duration_ms` using checked arithmetic.

### 3.5 Stale Lease Reconciliation

During orchestrator startup or recovery passes, `WorkflowStore::reconcile_stale_tasks(now)` identifies running tasks whose leases expired:
- Tasks with attempts remaining are transitioned to `RetryPending` with backoff.
- Tasks that have exhausted retries transition to `Failed`.
- Active path locks associated with expired leases are pruned from the `workflow_locks` table.
- Every reconciliation emits a durable recovery event in `workflow_events`.

---

## 4. Cancellation Architecture & Tree-Wide Termination

### 4.1 Cooperative Cancellation Tokens

Cancellation is mediated by `tokio_util::sync::CancellationToken`:
- A root cancellation token is instantiated in `WorkflowOrchestrator` per workflow.
- Child tokens are scoped to individual task waves and tasks.
- Cooperative cancellation checkpoints are positioned before claiming, during tool execution, prior to verification, and before patch integration.

### 4.2 Hierarchical Agent Tree Termination

When a workflow or task is cancelled (or times out):
1. The orchestrator triggers the cancellation token.
2. The orchestrator invokes `runtime.cancel_workflow(&workflow_id)`.
3. `cancel_workflow` looks up all active agents registered for that workflow in `active_agent_ids`.
4. For each agent, `runtime.terminate_tree(&agent_id)` recursively terminates the agent and all descendants:
   - Closes bidirectional agent communication channels.
   - Marks agent status as `Terminated`.
   - Sends termination signals (SIGTERM, followed by SIGKILL) to all child OS processes spawned by sandboxed tools.
   - Cleans up temporary worktree mounts.

### 4.3 Safe Integration Cancellation

If cancellation occurs while an integration is in flight:
- `WorkflowIntegrationService` catches the cancellation signal.
- The primary workspace lock prevents concurrent modifications.
- If pre-integration snapshotting completed, the service triggers `execute_exact_rollback`, restoring all modified, created, or deleted files to their exact pre-integration states.
- The integration status is set to `IntegrationOutcome::Cancelled` and no git commit is created.

---

## 5. Verification Integrity & 12-Point Completion Gate

### 5.1 Elimination of Synthetic Verification

Previous implementations exhibited "synthetic passes" where task analysis stages or verification fields were initialized to `Passed` without empirical proof. Round 3 eliminates all synthetic states:
- Verification checks must execute against actual build systems (`cargo check`, `cargo test`, `npm test`, `pytest`, etc.).
- If no supported manifest is present in the workspace, verification **fails closed** with `VerificationIncomplete`.
- Static analysis and security scanning (via `m31a-review` and `m31a-verify`) must run and report zero critical findings.

### 5.2 The 12-Point Final Workflow Completion Gate

Before any workflow status transitions to `WorkflowStatus::Completed` in `WorkflowOrchestrator::execute_workflow`, it must pass all 12 points of the Authoritative Completion Gate:

1. **Active Tasks Zero:** Zero tasks remain in `Pending`, `Ready`, `Running`, `Verifying`, or `RetryPending` state.
2. **Terminal Success State:** Every task in the workflow plan is in `WorkflowTaskStatus::Completed`.
3. **Empty Error Ledger:** Workflow error ledger is empty; no unhandled task execution errors exist.
4. **Integration Authorization:** If `require_approval_for_integration` is enabled, every integration has an approved, non-stale `WorkflowApprovalRequest` matching the change-set hash.
5. **No Failed Integrations:** No task integration outcomes recorded as `IntegrationFailed` or `Rejected`.
6. **Empirical Verification Pass:** All verification dimensions (build, test, safety, clippy) passed with non-zero executed checks.
7. **Clean Review Status:** Zero critical or blocking findings recorded in the review ledger.
8. **Artifact Persistence:** All mandatory delivery artifacts (`delivery_report.json`, plan snapshots, task logs) exist on disk and in durable storage.
9. **Git Attribution Integrity:** In git-managed workspaces, all applied patches have a durable commit SHA recorded in the task result.
10. **Workspace Containment:** No file modifications occurred outside the bounded primary workspace.
11. **Cancellation Untriggered:** The root cancellation token was never signalled.
12. **Lock Release:** All path locks held in `workflow_locks` have been released.

If any of the 12 points fails, the workflow transitions to `WorkflowStatus::Failed`.

---

## 6. File-Backed Crash Recovery Architecture

Crash recovery ensures that an M31A instance can crash (SIGKILL, power loss, OOM) at any arbitrary instruction and a secondary instance can cleanly recover:

```
[Instance A (Running)] ──── Crash / Kill ────► [Dead]
                                                  │
                                                  ▼
[Instance B (Cold Start)] ──► Open File-Backed SQLite Database
                                 │
                                 ├─► Reconcile Stale Tasks (expired leases → RetryPending)
                                 ├─► Clear Abandoned Path Locks
                                 ├─► Inspect Persisted Task Fencing Tokens
                                 ├─► Rehydrate Workflow Context & Plan DAG
                                 └─► Resume Execution from Safe Ready Tasks
```

### 6.1 State Reconciliation Protocol
1. **Database Reconnect:** Instance B initializes `WorkflowStore` pointing to the durable file-backed SQLite database.
2. **Migration Consistency:** Migrations (1 through 17) verify schema integrity and foreign keys.
3. **Stale Lease Sweep:** `reconcile_stale_tasks(current_time)` identifies tasks held by Instance A whose leases expired.
4. **Fencing Rejection:** Any in-flight attempts by Instance A that wake up late are rejected by the fencing token predicate `current_attempt = attempt AND lease_expires_at > now`.
5. **Orchestrator Resumption:** Instance B loads the workflow, finds the `RetryPending` tasks, re-claims them with a new `worker_id` and incremented `current_attempt`, and executes them to completion.

---

## 7. Speculative Execution Hardening

### 7.1 Hypothesis Generation & Isolation

`SpeculativeArena` generates multiple competing hypotheses for complex tasks:
- Each candidate executes in a dedicated git worktree isolated from the primary repository.
- Candidates operate under isolated resource governance and sandbox policies.

### 7.2 Porcelain -z Git Diff Parsing

To guarantee path safety with special filenames (spaces, quotes, UTF-8, newlines), candidate diff collection uses `git status --porcelain=v1 -z` and `git diff -z`:
- Fields are NUL-delimited (`\0`).
- Renamed and copied files (`R  `, `C  `) properly parse both original and destination paths.
- Filename quoting artifacts are eliminated.

### 7.3 Unified Candidate Executor Factory

`m31a-agent-engine::create_candidate_executor` provides a single authoritative factory used across CLI, App Server, and Orchestrator:
- Enforces uniform sandbox containment.
- Injects standard LLM provider routing and tool sets.
- Guarantees identical execution policies across all candidate paths.

### 7.4 Authoritative Winner Promotion

When a winning speculative hypothesis is selected:
- The candidate diff is extracted from the isolated worktree.
- The candidate diff is submitted directly to `WorkflowIntegrationService::integrate`.
- The primary workspace is **never** touched directly by `SpeculativeArena`.
- Winner promotion undergoes the exact same preflight, human approval, review gate, verification check, and snapshot/rollback cycle as standard workflow tasks.
