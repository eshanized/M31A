# M31A Execution Integrity Round 2 — Baseline Defect Audit

**Audit Date:** 2026-11-06  
**Auditor:** M31A Autonomous Engineering Team  
**Scope:** `m31a-agent-engine`, `m31a-storage`, `m31a-core`, `m31a-cli`, `m31a-app-server`, `m31a-tui`  
**Classification:** Baseline Audit & Defect Catalog  

---

## 1. Executive Summary

A comprehensive source inspection across the 39-crate M31A workspace identified 11 specific architectural defects that allow tasks or workflows to report completion falsely, bypass durable leasing/fencing, drop cancellation signals, execute synthetic analysis/delivery tasks, award points for unmeasured performance benchmarks, or corrupt workspaces upon failed rollback.

---

## 2. Defect Classification & Matrix

| Finding ID | Priority | Module | Source File & Location | Root Cause & Runtime Consequence | Required Acceptance Test |
| :--- | :---: | :--- | :--- | :--- | :--- |
| **D-01** | **P0** | `agent-engine` | `workflow.rs` (L478) | `execute_workflow` directly calls `WaveScheduler::execute_workflow_waves`, completely bypassing `WorkflowStore::claim_ready_task`. Tasks run without durable worker claims or leases. | `test_durable_scheduler_claims_tasks_with_leases_before_execution` |
| **D-02** | **P0** | `agent-engine` | `workflow.rs` (L977) | `update_task_execution_result` is invoked with `None, None` for `expected_attempt_id` and `expected_worker_id`, completely bypassing optimistic concurrency fencing. A stale worker can overwrite newer attempts. | `test_stale_worker_commit_rejected_by_fencing` |
| **D-03** | **P0** | `agent-engine` | `workflow.rs` (L885, L925, L974) | Database persistence results for artifacts and execution results are discarded via `let _ = self.store().*`. SQLite write failures leave tasks marked completed without durable evidence. | `test_artifact_and_result_persistence_failure_fails_task_closed` |
| **D-04** | **P0** | `agent-engine` | `workflow.rs`, `speculative.rs`, `agents.rs` | Separate, duplicated integration logic across `agents.rs`, `speculative.rs`, and `workflow.rs`. No shared snapshotting, approval gating, or unified rollback. | `test_unified_integration_service_exact_rollback_and_approval_gate` |
| **D-05** | **P0** | `agent-engine` | `workflow.rs` (L1053) | `cancel_workflow` only updates database state without signalling `CancellationToken`s or invoking `terminate_tree` on active child agents. Subagents continue executing indefinitely. | `test_workflow_cancellation_terminates_active_agent_tree` |
| **D-06** | **P0** | `core`, `agent-engine` | `core::workflow.rs` (L378), `agent-engine::workflow.rs` (L967) | `TaskExecutionResult` equates `tests_passed` to `build_passed` and hardcodes `safety_passed = true` without executing test or safety suites. Synthetic boolean masking. | `test_task_execution_result_reflects_separate_build_test_safety_evidence` |
| **D-07** | **P1** | `agent-engine` | `workflow.rs` (L879, L919) | Analysis task returns `{ "status": "analysis_completed" }` without analyzing code; Delivery task returns `{ "status": "delivery_ready" }` without consolidating actual changes and test evidence. | `test_analysis_and_delivery_tasks_produce_substantive_artifacts` |
| **D-08** | **P1** | `agent-engine` | `workflow.rs` (L460) | Workflow configuration fields (`max_concurrency`, `task_timeout_seconds`, `verify_post_wave`, `require_approval_for_integration`) are partially ignored or not enforced during scheduling. | `test_workflow_execution_config_strictly_enforced` |
| **D-09** | **P1** | `agent-engine` | `speculative.rs` (L1014, L1375) | Candidate execution timeout is not enforced on `exec.execute(&ctx)`; comparative benchmark awards 10.0 points when `NotMeasured` instead of 0.0. `git status` splits on whitespace. | `test_speculative_timeout_and_zero_score_for_unmeasured_benchmarks` |
| **D-10** | **P1** | `app-server`, `cli` | `app-server::protocol.rs` (L1979) | App Server JSON-RPC `speculative/evaluations` calls `SpeculativeArena::compete` with `executor: None`, while CLI uses `AgentEngineCandidateExecutor`. | `test_app_server_speculative_evaluations_uses_production_executor` |
| **D-11** | **P0/P1** | `agent-engine` | `workflow.rs` (L1015) | Interrupted workflow recovery was not tested against mid-task process failure. Re-execution of completed tasks was not prevented by durable DAG verification. | `test_crash_recovery_resumes_incomplete_tasks_without_replaying_completed` |

---

## 3. Remediation Order & Dependencies

1. **Stage 1 (P0):** Durable Workflow Scheduler & Atomic Claims (`WorkflowStore::claim_ready_task`, write-path conflict checks, active heartbeat guard).
2. **Stage 2 (P0):** Fenced Task Execution Context & Mandatory Persistence (`TaskExecutionContext`, strict Cas on attempt & worker, bubble all DB errors).
3. **Stage 3 (P0):** Unified Authoritative Integration & Approval Service (`WorkflowIntegrationService`, approval gating, exact file snapshot & rollback).
4. **Stage 4 (P0):** Real Workflow Cancellation & Agent Tree Termination (Token propagation, `terminate_tree` invocation).
5. **Stage 5 (P1):** Enforced Workflow Configuration (`max_concurrency` semaphore, `task_timeout_seconds` deadline, post-wave verifier).
6. **Stage 6 (P0):** Typed Verification Evidence & Substantive Tasks (Analysis AST/module discovery, delivery report consolidation, `VerificationStatus` enums).
7. **Stage 7 (P1):** Speculative Execution Hardening (Timeout cancellation, `NotMeasured` scoring policy v2.0.0, robust porcelain parsing).
8. **Stage 8 (P0/P1):** Crash Recovery & Nonterminal Resume (Mid-task kill recovery, attempt increment, fence old workers).
9. **Stage 9 & 10 (P1):** App Server, CLI, and TUI Supervision (Production candidate executor in App Server, live workflow status in TUI).
10. **Stage 11 & 12 (Mandatory):** Comprehensive Verification & Deliverables.
