# M31A Execution Integrity Baseline Audit Report

**Report Version:** 1.0.0  
**Audit Date:** 2026-11-06  
**Auditor:** M31A Autonomous Engineering Team  
**Scope:** `m31a-agent-engine`, `m31a-tools`, `m31a-storage`, `m31a-core`, `m31a-verify`, `m31a-cli`, `m31a-tui`, `m31a-app-server`  
**Status:** Complete — Baseline Established

---

## 1. Executive Summary

A comprehensive source audit of M31A's speculative execution and workflow orchestration architecture was conducted across the 39-crate workspace. The audit identified 11 critical and high-priority defects centered on execution integrity:
1. Tasks could succeed without a genuine child agent executing or producing code changes.
2. The speculative arena in CLI and workflows was called without a real model-backed `CandidateExecutor`.
3. Candidate promotion contained dangerous broad destructive rollbacks (`git clean -fd`).
4. Scheduling was partially decoupled from durable SQLite worker leasing and dependency completion checks.
5. Inability to verify workspace cleanliness was treated as clean.

The findings have been categorized into a formal remediation matrix below.

---

## 2. Remediation Matrix

| Finding ID | Severity | Affected Module | Source File & Function | Root Cause | Required Correction | Target Status |
|---|---|---|---|---|---|---|
| **F-01** | **P0** | `m31a-agent-engine` | `workflow.rs` / `execute_task` | Swallowing child spawn errors and using unisolated compiler check on unchanged code when runtime is `None` | Require runtime; fail closed on missing/failed child agent; enforce non-empty diff | **Resolved (Remediated & Verified)** |
| **F-02** | **P0** | `m31a-agent-engine`, `m31a-cli` | `workflow.rs`, `speculate.rs` | `SpeculativeArena::compete_with_options` called with `executor: None` in production paths | Implement production `AgentEngineCandidateExecutor` and wire into workflow and CLI | **Resolved (Remediated & Verified)** |
| **F-03** | **P0** | `m31a-agent-engine` | `speculative.rs` / `compete_with_options` | Post-integration failure invokes `git checkout -- .` and `git clean -fd`, destroying user files | Implement transactional rollback restricted to exact applied patch hunks | **Resolved (Remediated & Verified)** |
| **F-04** | **P0** | `m31a-agent-engine` | `speculative.rs` | `mgr.is_dirty(...).unwrap_or(false)` treats error inspecting git status as clean workspace | Fail closed: error checking dirty state blocks promotion | **Resolved (Remediated & Verified)** |
| **F-05** | **P0** | `m31a-agent-engine` | `workflow.rs` | `verify_check(...).unwrap_or(true)` treats verification error as success | Fail closed: change default to `false` | **Resolved (Remediated & Verified)** |
| **F-06** | **P0** | `m31a-core`, `m31a-agent-engine` | `workflow.rs` | Fragile substring matching (`task.id.contains("speculate")`) | Introduce typed `WorkflowTaskKind` and migrate storage schema | **Resolved (Remediated & Verified)** |
| **F-07** | **P1** | `m31a-cli` | `workflow.rs` | `WorkflowOrchestrator::new(storage, None, None, None, None)` initializes without execution runtime | Reuse application provider/tool/runtime initialization | **Resolved (Remediated & Verified)** |
| **F-08** | **P1** | `m31a-agent-engine`, `m31a-storage` | `workflow.rs`, `workflow_store.rs` | Claiming operated without checking upstream dependencies | Subquery dependency check in `claim_ready_task` | **Resolved (Remediated & Verified)** |
| **F-09** | **P1** | `m31a-storage` | `workflow_store.rs` / `claim_ready_task` | No `attempt_id` / fencing token; stale workers can overwrite newer attempts | Add `attempt_id` to `workflow_tasks`, increment on claim, fence updates | **Resolved (Remediated & Verified)** |
| **F-10** | **P1** | `m31a-agent-engine` | `workflow.rs` / `resume_workflow` | Re-executes completed tasks when resuming an interrupted workflow | Skip completed tasks; reconstruct DAG of remaining incomplete tasks | **Resolved (Remediated & Verified)** |
| **F-11** | **P1** | `m31a-agent-engine` | `speculative.rs` / `generate_diverse_hypotheses` | Keyword-only hardcoded approaches based on presence of "perf" | Context-aware candidate hypothesis generation with structured fallback | **Resolved (Remediated & Verified)** |

---

## 3. Remediation Verification

All eleven findings have been fully remediated and verified under the [M31A Execution Integrity Contract](../architecture/EXECUTION_INTEGRITY_CONTRACT.md).
Detailed subsystem implementation, migration, and test results are documented in the [Execution Integrity Remediation Report](EXECUTION_INTEGRITY_REMEDIATION_REPORT.md).

- **Migration 16:** Applied cleanly, adding `task_kind`, `attempt_id`, and `execution_result_json` to `workflow_tasks`.
- **Unit & Integration Tests:** 100% passing across `m31a-core`, `m31a-storage`, `m31a-agent-engine`, and `m31a-cli`.
- **E2E Suite:** Validated multi-candidate speculative competition, hard gating, transactional rollback, full workflow lifecycle with `LocalWireServer`, and fail-closed behavior without runtime.
- **Compiler & Linter:** `cargo fmt`, `cargo check --workspace --all-targets`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings` passing cleanly.
