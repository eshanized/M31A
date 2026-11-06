# M31A Round 3: Execution Integrity Baseline Audit

**Date:** 2026-11-06  
**Auditor:** Principal Rust Engineer (Execution Integrity Team)  
**Status:** Baseline Completed — Direct Remediation Roadmap Established  
**Workspace:** Rust 2024 (39 crates)

---

## 1. Executive Summary

This report documents the comprehensive source-level audit of M31A's workflow execution topology, storage engine, verification gates, integration lifecycle, speculative execution engine, cancellation propagation, and operator interfaces (TUI, CLI, App Server).

While Round 1 and Round 2 established initial foundations (task claims, worker leases, attempt tracking, per-file snapshots, typed task kinds), the audit confirms critical execution-integrity gaps:
- **Duplicated integration paths:** `EngineAgentRuntime::integrate_agent` bypasses `WorkflowIntegrationService`.
- **Synthetic approval bypass:** Production code substitutes `is_approved: !config.require_approval_for_integration` for human authorization.
- **Silent commit failure:** A failed `git commit` inside `WorkflowIntegrationService` returns `success: true`.
- **Incomplete cancellation and leak race:** Task timeout drops futures without terminating child agent trees or waiting for child process exit.
- **Scheduler and claim transaction flaws:** Non-immediate SQLite transactions, candidate vs update predicate mismatches (`pending` selected but rejected by update), malformed write-path fail-open parsing, and unchecked arithmetic overflow.
- **Fabricated verification evidence:** Task dimensions (build, tests, safety, review) are hardcoded to `Passed` without executing empirical checks.
- **Ignored mandatory persistence:** Artifact storage calls discard results with `let _ = ...`.
- **Unverified rollback:** `execute_exact_rollback` returns `()` and ignores restoration errors.
- **Supervision and interface gaps:** TUI workflow screen is inert; App Server lacks `workflow/approve` and `workflow/reject`; speculative candidate executor construction is duplicated across entrypoints.

---

## 2. Actual Execution Topology

```
                  ┌────────────────────────────────────────┐
                  │ Operator Interfaces: CLI / App Server  │
                  │              / TUI                     │
                  └──────────────────┬─────────────────────┘
                                     │
                                     ▼
                  ┌────────────────────────────────────────┐
                  │         WorkflowOrchestrator           │
                  │   (m31a-agent-engine::workflow)        │
                  └──────┬───────────┬──────────────┬──────┘
                         │           │              │
           Durable Claims│           │DAG Waves     │Speculative Hypotheses
                         ▼           ▼              ▼
                 ┌──────────────┐ ┌──────────────┐ ┌──────────────────────┐
                 │WorkflowStore │ │WaveScheduler │ │   SpeculativeArena   │
                 │(m31a-storage)│ │ (join_set)   │ │ (m31a-agent-engine)  │
                 └──────────────┘ └──────┬───────┘ └──────────┬───────────┘
                                         │                    │
                                         ▼                    ▼
                                ┌─────────────────┐  ┌─────────────────┐
                                │EngineAgentRun-  │  │AgentEngine-     │
                                │time (Worktrees) │  │CandidateExecutor│
                                └────────┬────────┘  └────────┬────────┘
                                         │                    │
                                         │ (Bypasses SVC!)    │
                                         ▼                    ▼
                               ┌────────────────────────────────────────┐
                               │       WorkflowIntegrationService       │
                               │ - Pre-check & Approval Gating          │
                               │ - Snapshots & Exact Rollback           │
                               │ - AutonomousVerifier (Check/Test/Safe) │
                               │ - Git Attribution Commit               │
                               └────────────────────────────────────────┘
```

---

## 3. Remaining Defects with Precise Source References

### 3.1 P0 — Unified Integration & Human Approval Lifecycle
1. **Duplicated Integration Paths (`crates/m31a-agent-engine/src/agents.rs:1147–1358`):**
   `EngineAgentRuntime::integrate_agent` performs independent path validation, git preflight, review analysis, isolated testing, and git commit via `mgr.apply_and_commit_worktree_diff`. In `crates/m31a-agent-engine/src/workflow.rs:1101`, implementation tasks invoke `rt.integrate_agent` rather than delegating to `WorkflowIntegrationService`.
2. **Auto-Approval Bypass (`crates/m31a-agent-engine/src/workflow.rs:998`):**
   `is_approved: !config.require_approval_for_integration` directly violates the governing invariant by auto-approving integration requests when approval is configured false, without human authorization.
3. **No Durable Approval State Machine (`crates/m31a-storage/src/migrations.rs:384–494`):**
   There is no `workflow_approvals` table. Approval requests and decisions are not durably tracked, bound to change-set hashes, or associated with specific attempt IDs.
4. **Commit Failure Swallowed (`crates/m31a-agent-engine/src/integration.rs:425–429, 441–452`):**
   When `commit_integrated_changes` fails, it logs `warn!` and returns `success: true` with `commit_sha: None`, misrepresenting an uncommitted patch as a completed integration.

### 3.2 P0 — Cancellation, Agent Trees, and Fenced Execution
1. **Cancellation Dropping Futures Without Process Termination (`crates/m31a-agent-engine/src/workflow.rs:1284–1303`):**
   `tokio::select!` drops `execution_fut` upon task timeout or cancellation token signal. The underlying child agent and its child processes continue executing.
2. **Agent Registration Cancellation Race (`crates/m31a-agent-engine/src/workflow.rs:1072–1081`):**
   `spawn_agent` is called before registering the child agent in `active_agent_ids`. If cancellation occurs during or immediately after spawn, `cancel_workflow` misses the child agent.
3. **Missing Process Tree Termination in Child Tasks (`crates/m31a-agent-engine/src/agents.rs:1067–1076`):**
   While `terminate_tree` closes agent channels, external tool processes (such as long-running compiler checks or test commands) can survive if not explicitly SIGKILLed/reaped.

### 3.3 P0 — Durable Scheduler & Storage Correctness
1. **Deferred Transactions in Task Claims (`crates/m31a-storage/src/workflow_store.rs:721`):**
   `conn.transaction()` uses `DEFERRED` locking, allowing concurrent workers to race before the transaction upgrades to write. It must use `Immediate` transaction behavior.
2. **Status Predicate Inconsistency (`crates/m31a-storage/src/workflow_store.rs:777, 793, 854`):**
   Candidate selection query includes `t.status IN ('ready', 'retry_pending', 'pending')`, but the UPDATE query only matches `status IN ('ready', 'retry_pending')`. Candidate selection and update must be consistent.
3. **Fail-Open Path Conflict Parsing (`crates/m31a-storage/src/workflow_store.rs:833`):**
   `serde_json::from_str(&write_paths_json).unwrap_or_default()` treats malformed JSON as an empty set, failing open instead of failing closed.
4. **Integer Overflow Vulnerability (`crates/m31a-storage/src/workflow_store.rs:843, 723`):**
   `current_attempt + 1` and `now + lease_duration_ms` lack checked arithmetic.
5. **Lease Renewal on Terminal Workflows (`crates/m31a-storage/src/workflow_store.rs:916–935`):**
   `renew_task_lease` does not verify that the workflow is in an active state (`Running` or `Verifying`).
6. **Stale Task Reconciliation Silently Discards Errors (`crates/m31a-storage/src/workflow_store.rs:962`):**
   `filter_map(|r| r.ok())` swallows SQLite row errors during stale lease reconciliation, and reconciliation does not emit durable audit recovery events.

### 3.4 P0 — Verification Evidence & Completion Integrity
1. **Synthetic Evidence Fabrication (`crates/m31a-agent-engine/src/workflow.rs:846–847, 948–951, 1113–1116`):**
   Verification dimensions (build, tests, safety, review) are assigned `VerificationStatus::Passed` without executing empirical verifiers.
2. **Incomplete Final Completion Gate (`crates/m31a-agent-engine/src/workflow.rs:598–606`):**
   The final gate executes only `verify_check`. It does not execute tests or safety audits, does not verify artifact existence or content SHA-256 hashes, and does not verify the absence of active/pending tasks.

### 3.5 P0 — Artifact Persistence & Verified Rollback
1. **Ignored Mandatory Artifact Persistence (`crates/m31a-agent-engine/src/workflow.rs:618, 860, 927, 938`):**
   All `store_artifact_file` invocations use `let _ = ...`. An I/O error or SQLite write failure is silently ignored.
2. **Incomplete Rollback Reporting (`crates/m31a-agent-engine/src/integration.rs:521–527`):**
   `execute_exact_rollback` returns `()` and swallows per-file restoration errors with `error!`. It does not report a structured `RollbackResult` or detect concurrent file tampering.

### 3.6 P1 — Speculative Execution Hardening
1. **Line-Oriented Git Porcelain Parsing (`crates/m31a-agent-engine/src/speculative.rs:258–275, 398`):**
   `parse_porcelain_path` splits on lines and whitespace, failing on filenames with spaces, unicode characters, or quoted paths. It must use `git status --porcelain=v1 -z` with NUL delimiter handling.
2. **Duplicated Executor Factory (`crates/m31a-agent-engine/src/workflow.rs:883–900`, `crates/m31a-cli/src/speculate.rs:120–137`, `crates/m31a-app-server/src/protocol.rs:1995–2005`):**
   Candidate executor construction is duplicated across three crates with divergent fallback behaviors.

### 3.7 P1 — Operator Supervision (TUI, CLI, App Server)
1. **Inert TUI Workflow Screen (`crates/m31a-tui/src/screens.rs:540–670`, `terminal.rs:3486`):**
   The Agents screen renders a read-only table without selection state, task DAG inspection, or actions for approval, rejection, cancellation, or resumption.
2. **Missing App Server RPC Methods (`crates/m31a-app-server/src/protocol.rs:1789–1935`):**
   `workflow/approve` and `workflow/reject` RPC methods do not exist.
3. **Missing Workflow Notifications (`crates/m31a-app-server/src/protocol.rs`):**
   Workflow lifecycle events and approval requests are not emitted over the multiplexer/SSE notification broadcast stream.

---

## 4. Reusable Existing Components

- **`m31a_agent_engine::integration::WorkflowIntegrationService`:** Existing framework for pre-check, dirty check, patch application, and verification.
- **`m31a_verify::AutonomousVerifier`:** `verify_check`, `verify_tests`, `verify_safety`.
- **`m31a_verify::safety::CodebaseSafetyGate`:** Change set AST and symbol blast-radius analysis.
- **`m31a_review::ReviewEngine`:** Multi-pattern static code review with severity findings.
- **`m31a_storage::WorktreeManager`:** Isolated branch management, clean diff export, preflight checking.
- **`m31a_storage::migrations::MigrationEngine`:** Transactional forward migrations.
- **`m31a_app_server::connection_multiplexer::ConnectionMultiplexer`:** Real-time client notification broadcasting.

---

## 5. Required Changes and Affected Crates

| Subsystem | Crates | Scope of Changes |
| :--- | :--- | :--- |
| **Unified Integration** | `m31a-agent-engine`, `m31a-tools` | Refactor `EngineAgentRuntime::integrate_agent` to delegate to `WorkflowIntegrationService`. Make `WorkflowIntegrationService` the sole authority. |
| **Durable Approvals** | `m31a-storage`, `m31a-core`, `m31a-agent-engine` | Migration 17 for `workflow_approvals`. Implement approval lifecycle state machine with change-set hash and attempt binding. |
| **Cancellation & Fencing** | `m31a-agent-engine`, `m31a-core` | `TaskExecutionContext` with cancellation propagation, pre-spawn registration, process tree termination, and fenced result updates. |
| **Durable Scheduler** | `m31a-storage`, `m31a-agent-engine` | `claim_task_internal` immediate transactions, consistent predicates, checked arithmetic, fail-closed metadata parsing, deterministic reconciliation. |
| **Verification Evidence** | `m31a-agent-engine`, `m31a-verify` | Genuine empirical evidence capture for each task kind. Comprehensive 12-point final completion gate. |
| **Artifacts & Rollback** | `m31a-agent-engine`, `m31a-storage` | Structured `RollbackResult` with SHA-256 verification. Mandatory artifact write enforcement. |
| **Crash Recovery** | `m31a-agent-engine`, `m31a-storage` | Real process-restart crash recovery acceptance test using a file-backed SQLite database. |
| **Speculative Hardening** | `m31a-agent-engine`, `m31a-cli`, `m31a-app-server` | NUL-delimited `git status -z` parser. Shared production candidate executor factory. |
| **Interfaces (TUI/CLI/RPC)**| `m31a-tui`, `m31a-cli`, `m31a-app-server` | Interactive TUI workflow supervision. `workflow/approve`, `workflow/reject` RPC methods and CLI subcommands. Progress notifications. |

---

## 6. Implementation Dependency Graph

```
Stage 0: Baseline Audit (Done)
   │
   ▼
Stage 1: Unified Integration & Human Approval (P0)
   ├── Migration 17: workflow_approvals table
   ├── WorkflowIntegrationService as sole integration authority
   └── EngineAgentRuntime::integrate_agent adapter
   │
   ▼
Stage 2: Cancellation, Agent Trees & Fenced Execution (P0)
   ├── Hierarchical cancellation token propagation
   ├── Pre-spawn active agent registration
   └── Fenced task completion with lease verification
   │
   ▼
Stage 3: Durable Scheduler & Storage Correctness (P0)
   ├── Atomic immediate SQLite task claims
   ├── Lease renewal with active workflow validation
   └── Transactional stale task recovery with event emission
   │
   ▼
Stage 4: Verification Evidence & Completion Gate (P0)
   ├── Independent empirical verification dimensions
   └── 12-point final workflow completion gate
   │
   ▼
Stage 5: Artifact Persistence & Verified Rollback (P0)
   ├── Structured RollbackResult with SHA-256 verification
   └── Mandatory artifact persistence error enforcement
   │
   ▼
Stage 6: Genuine Crash Recovery (P0)
   └── File-backed SQLite active-task process restart test
   │
   ▼
Stage 7: Speculative Execution Hardening (P1)
   ├── NUL-delimited git porcelain parsing
   └── Shared candidate executor factory
   │
   ▼
Stage 8: Operator Supervision — TUI, CLI & App Server (P1)
   ├── App Server approval & rejection RPC methods
   ├── Workflow progress notification broadcast
   ├── CLI workflow approve/reject commands
   └── Interactive TUI workflow supervision screen
   │
   ▼
Stage 9: Configuration & State-Machine Enforcement (P0/P1)
   └── Complete validation of WorkflowExecutionConfig
   │
   ▼
Stage 10: Comprehensive Acceptance Testing
   └── Automated test suite across all 11 stages
   │
   ▼
Stage 11: Full Workspace Verification & Documentation
   └── cargo fmt, cargo check, cargo clippy, cargo test
```
