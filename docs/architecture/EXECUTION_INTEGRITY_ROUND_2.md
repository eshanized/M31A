# M31A Execution Integrity Architecture — Round 2

**Document Version:** 2.0.0  
**Status:** Authoritative Architectural Specification  
**Classification:** Core Execution Subsystems & Invariant Contract  
**Target Architecture:** Rust 2024 / M31A 39-Crate Workspace  

---

## 1. Executive Mission & Governing Invariant

M31A is an autonomous engineering platform designed to reliably execute substantial engineering objectives.
The primary governing invariant for execution integrity is:

> **M31A may report an engineering task as completed only when its actual execution has succeeded, all required acceptance criteria have been satisfied, the resulting evidence has been persisted successfully, and any required integration has been safely authorized and completed.**

A workflow must never report completion when any required execution, verification, persistence, approval, or integration operation has failed.

---

## 2. Real vs. Intended Execution Topology

### 2.1 The Actual Baseline Execution Topology (Audited Flaws)

The previous implementation left severe disconnects between storage primitives and active execution:

```
[CLI / TUI / App Server]
          │
          ▼
[WorkflowOrchestrator::execute_workflow]
          │
          ├────────────────────────────────────────┐
          │ (BYPASSES Durable Claiming & Leasing!) │
          ▼                                        ▼
[WaveScheduler::execute_workflow_waves]     [WorkflowStore]
  (In-memory task slice dispatch)           (Leasing & dependency checks
          │                                  sat dormant in store)
          ▼                                        │
[WorkflowOrchestrator::execute_task]               │
  - Marks task 'running' without worker lease      │
  - Executes tool loop / child agent               │
  - Runs AutonomousVerifier::verify_check          │
  - Calls update_task_execution_result:            │
      attempt_id = None, worker_id = None ─────────┘
      (BYPASSES attempt/worker fencing!)
  - Errors swallowed via `let _ =`!
```

### 2.2 The Authoritative Remediated Execution Topology

Under the Round 2 architecture, `WorkflowStore` and the authoritative durable scheduler govern all task lifecycles through unbreakable fencing and heartbeats:

```
[CLI / TUI / App Server]
          │
          ▼
[WorkflowOrchestrator] ──(CancellationToken root)
          │
          ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   Durable Workflow Scheduler Loop                      │
│                                                                        │
│  1. Check Workflow State & Root Cancellation                           │
│  2. Reconcile Stale Tasks (expired leases -> retry_pending / failed)  │
│  3. Claim Ready Task (SQLite IMMEDIATE transaction):                  │
│     - Verify workflow is active ('running')                            │
│     - Verify all declared dependencies are 'completed'                 │
│     - Verify no conflicting declared write paths with active tasks     │
│     - Assign worker_id, increment attempt_id, set lease_expires       │
│     - Atomically transition to 'claimed' and record TaskClaimed event  │
│  4. Dispatch Task Execution with TaskExecutionContext:                 │
│     - Spawn active lease heartbeat loop                                │
│     - Execute task via typed WorkflowTaskKind handler                  │
│     - If child agent: track child agent_id in workflow tree            │
│  5. Verification & Safety Gating:                                      │
│     - Separate build, test, and codebase safety audit evidence         │
│  6. Authoritative Unified Integration Service:                         │
│     - Enforce require_approval_for_integration (suspend if unapproved) │
│     - Pre-integration snapshot & strict path containment               │
│     - Atomic patch application with exact per-file rollback on failure │
│  7. Fenced Task Result Commit:                                         │
│     - UPDATE workflow_tasks WHERE attempt_id = :att AND worker_id = :w │
│     - Validates lease_expires > now; rejects stale commits             │
│     - Fails closed on database errors (no swallowed Result!)           │
│  8. Stop Heartbeat & Release Concurrency Permit                        │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     Workflow Final Completion Gate                     │
│  - Verify all plan tasks are in terminal 'completed' status            │
│  - Verify every required artifact exists and matches SHA-256           │
│  - Execute full workspace verification (verify_check + verify_tests)   │
│  - Atomic transition to 'completed' with delivery summary artifact     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Subsystem Invariant Ownership Mapping

Each operational responsibility has exactly one authoritative owner:

| Responsibility | Authoritative Subsystem & Owner | Invariant & Enforcement Mechanism |
| :--- | :--- | :--- |
| **Workflow Lifecycle** | `m31a-agent-engine::workflow::WorkflowOrchestrator` | State machine: `Created` → `Planning` → `PlanReady` → `Running` → `Completed` \| `Failed` \| `Cancelled`. Terminal states are immutable. |
| **Task Claiming & Leasing** | `m31a-storage::workflow_store::WorkflowStore` | Atomic SQLite transaction with subquery dependency check, write-path exclusion, attempt increment, and lease expiration. |
| **Attempt Identity & Fencing** | `m31a-storage::workflow_store::WorkflowStore` | Strict CAS: `WHERE workflow_id = ? AND id = ? AND attempt_id = ? AND worker_id = ? AND lease_expires > ?`. Affected rows == 0 fails closed. |
| **Lease Heartbeat & Renewal** | `m31a-agent-engine::workflow::TaskHeartbeatGuard` | Background ticker renewing lease every $T_{\text{lease}} / 3$. If renewal fails or attempt is fenced, triggers immediate task cancellation. |
| **Agent Tree & Cancellation** | `m31a-agent-engine::agents::EngineAgentRuntime` | Root `CancellationToken` hierarchy. Calling `cancel_workflow` signals tokens and executes `terminate_tree(&agent_id)` on all active child agents. |
| **Unified Integration & Approval** | `m31a-agent-engine::integration::WorkflowIntegrationService` | Single authority for direct agents, speculative candidates, and workflow tasks. Checks approval gate, records file snapshot, exact per-file rollback. |
| **Task Evidence & Verification** | `m31a-core::workflow::TaskExecutionResult` | Explicit typed dimensions: `build_status`, `tests_status`, `safety_status`, `review_status`, duration, exit codes. No synthetic boolean masking. |
| **Artifact Persistence** | `m31a-storage::workflow_store::WorkflowStore` | Content-addressed SHA-256 storage in `artifacts/<wf_id>/`. Storage errors bubble up and fail the task. |
| **Crash Recovery** | `m31a-agent-engine::workflow::WorkflowOrchestrator::resume_workflow` | Reconciles stale leases, verifies completed tasks have valid results, re-schedules incomplete tasks with new attempt IDs without re-running completed work. |
| **Supervision & Notifications** | `m31a-app-server` & `m31a-tui` | Event-driven updates matching persisted `workflow_events` ledger without blocking render loops. |

---

## 4. Unified Integration and Approval Service Contract

All code integrations into the primary workspace must pass through `WorkflowIntegrationService`:

1. **Approval Gate:**
   If `require_approval_for_integration` is true, integration transitions to `NeedsApproval` / `IntegrationPending` with change-set hash and suspends until human approval is recorded in SQLite.
2. **Preflight Cleanliness:**
   `is_dirty` checks workspace. Any uncommitted changes or git inspection errors fail closed immediately.
3. **Pre-Integration Snapshot:**
   Every file to be touched is recorded with its pre-existing content SHA-256 (or marked non-existent if created).
4. **Patch Application & Post-Check:**
   Applies unified patch with `--ignore-whitespace`. Runs `AutonomousVerifier::verify_check` and `verify_tests`.
5. **Exact Transactional Rollback:**
   If verification fails, restores exactly the recorded pre-integration snapshot (re-writing original contents for modified files, deleting created files). Broad destructive commands (`git clean -fd`, `git checkout -- .`) are strictly prohibited.
