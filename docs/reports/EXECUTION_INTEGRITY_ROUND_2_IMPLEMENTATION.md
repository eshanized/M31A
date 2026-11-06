# M31A Execution Integrity Remediation — Round 2 Implementation Report

**Project:** M31A Autonomous Engineering Platform  
**Target:** 39-Crate Rust 2024 Workspace  
**Status:** Complete Implementation  
**Milestone:** Execution Integrity & Orchestration Closure (Round 2)

---

## 1. Executive Summary

This report documents the implementation details for the Round 2 remediation of M31A's speculative execution and autonomous workflow orchestration subsystems.

All 11 identified defects (D-01 through D-11) have been addressed directly within the existing architecture without introducing duplicate runtimes, second schedulers, or first-party auth layers.

---

## 2. Remediated Defects & Source Changes

### D-01: Durable Task Claiming & Worker Leasing
- **Issue**: Production execution dispatched tasks directly without durable worker claims or lease management.
- **Remediation**:
  - Implemented `claim_task` and `claim_ready_task` in `crates/m31a-storage/src/workflow_store.rs` requiring positive lease durations (`lease_duration_ms > 0`), checking that all upstream dependencies have `status = 'completed'`, preventing write-path conflicts with concurrent non-isolated tasks, atomically incrementing `attempt_id`, and generating transactional `TaskClaimed` events.
  - Implemented `TaskHeartbeatGuard` in `crates/m31a-agent-engine/src/workflow.rs`, spawning a ticker that renews task leases at `lease_duration_ms / 3` intervals and cancels task execution if renewal fails or is fenced.
  - Allowed claiming tasks in `('ready', 'retry_pending', 'pending')` whose dependencies are satisfied, and automated transition of downstream `pending` tasks to `ready` upon upstream completion.

### D-02: Stale-Worker Fencing & Attempt ID Persistence
- **Issue**: Persistence passed `None` for worker ID and attempt ID, bypassing optimistic concurrency fencing.
- **Remediation**:
  - `WorkflowOrchestrator::execute_task` now passes `Some(claimed_task.attempt_id)` and `Some(&worker_id)` to `update_task_execution_result`.
  - In `crates/m31a-storage/src/workflow_store.rs`, `update_task_execution_result` verifies `attempt_id = ? AND worker_id = ? AND (lease_expires IS NULL OR lease_expires > now)`. If 0 rows are affected, it returns an explicit `Err(Fencing violation or stale lease)`.

### D-03: Zero Silent Persistence Errors
- **Issue**: Result persistence errors were discarded with `let _ =`.
- **Remediation**:
  - Replaced all `let _ = store.update_task_execution_result(...)` and `let _ = store.record_event(...)` calls with explicit `?` propagation in `WorkflowOrchestrator::execute_task`. If persistence fails, the task and wave fail closed.

### D-04: Authoritative Integration & Approval Lifecycle
- **Issue**: Direct-agent and speculative promotion used disconnected integration paths and broad destructive git cleanups.
- **Remediation**:
  - Implemented `WorkflowIntegrationService` in `crates/m31a-agent-engine/src/integration.rs` with `FileSnapshot` byte tracking.
  - Both `WorkflowOrchestrator::execute_task` (for `Integration` tasks) and `SpeculativeArena::compete_with_options` (for winner promotion) route exclusively through `WorkflowIntegrationService`.
  - Enforced approval gating (`require_approval_for_integration`).
  - Implemented exact per-file byte rollback: restoring original bytes for modified files and unlinking created files on post-integration compiler check or test failures. Broad destructive cleanups (`git clean -fd`, `git checkout -- .`) are strictly eliminated.

### D-05: Real Cancellation Terminating Child Agent Trees
- **Issue**: Workflow cancellation updated SQLite status but left child LLM loops and OS process trees running in the background.
- **Remediation**:
  - Refactored `WorkflowOrchestrator::cancel_workflow` to be `async` and await `rt.terminate_tree(&agent_id).await` for all spawned agents registered in `active_agent_ids`.
  - Signalled the workflow-level `CancellationToken`, which immediately aborts child task executions and heartbeat tickers.
  - Updated callers in `crates/m31a-app-server/src/protocol.rs` and `crates/m31a-cli/src/workflow.rs` with `.await?`.

### D-06: Strict Workflow Configuration Enforcement
- **Issue**: `max_concurrency`, `verify_post_wave`, and `task_timeout_seconds` were partially ignored.
- **Remediation**:
  - Bounded concurrency with a `tokio::sync::Semaphore` initialized from `config.max_concurrency.clamp(1, 16)`.
  - Passed `config.verify_post_wave` to `WaveScheduler::execute_workflow_waves`.
  - Enforced `config.task_timeout_seconds` using `tokio::time::timeout` wrapping each task's execution future.

### D-07: Verifiable Execution Evidence & Typed Statuses
- **Issue**: Boolean fields like `build_passed` were set to true without real verification runs.
- **Remediation**:
  - Extended `TaskExecutionResult` in `crates/m31a-core/src/workflow.rs` with typed `VerificationStatus` (`Passed`, `Failed`, `NotExecuted`, `NotApplicable`, `TimedOut`, `Blocked`, `Pending`) for `build_status`, `tests_status`, `safety_status`, and `review_status`, plus `verification_duration_ms` and `exit_code`.
  - Populated actual status values from real `AutonomousVerifier` checks and test executions.

### D-08: Substantive Analysis and Delivery Tasks
- **Issue**: Analysis and Delivery tasks were empty stubs that marked themselves completed without inspecting code or consolidating reports.
- **Remediation**:
  - `Analysis`: Discovers workspace packages, parses manifest dependencies, counts crates, runs baseline compiler checks via `AutonomousVerifier::verify_check`, and writes a structured `analysis_report.json` artifact.
  - `Delivery`: Collects modified files, artifact manifests, git HEAD commit hash, and task execution outcomes across all completed waves into a structured `delivery_report.json` artifact.

### D-09: Idempotent Crash-Resilient Recovery
- **Issue**: Resuming workflows repeated completed tasks or reset attempt counters.
- **Remediation**:
  - Added an idempotency guard at the entry of `execute_task`: if a task already has `status == Completed`, it returns immediately with success and existing metadata.
  - `resume_workflow` invokes `reconcile_stale_tasks` to convert expired claims into `RetryPending`, then dispatches execution. Reclaimed tasks receive incremented attempt IDs while completed tasks remain untouched.

### D-10: Speculative Execution Hardening
- **Issue**: Candidate promotion bypassed git porcelain parsing, scoring policy allowed unearned points for missing benchmarks, and execution lacked timeout cancellation.
- **Remediation**:
  - Implemented `parse_porcelain_path` handling indexed, untracked, rename, and quoted formats.
  - Upgraded scoring policy version to `2.0.0` with `0.0` points for unmeasured performance benchmarks.
  - Added candidate timeout enforcement with `tokio::time::timeout` and token cancellation.

### D-11: App Server & TUI Supervision Wiring
- **Issue**: App server endpoints dispatched un-executed speculative evaluations without runtime wiring.
- **Remediation**:
  - Wired `speculative/evaluations` in `crates/m31a-app-server/src/protocol.rs` to construct an `AgentEngineCandidateExecutor` using the server's runtime, provider client, tools registry, storage, and execution policy.
  - Verified TUI workflow screens in `crates/m31a-tui/src/screens.rs` render live workflow wave progress, task counts, and plan revisions.

---

## 3. Summary of Modified Source Files

| Crate | Path | Changes |
|---|---|---|
| `m31a-core` | `src/workflow.rs` | Added `VerificationStatus`, `TaskExecutionContext`; extended `TaskExecutionResult` |
| `m31a-storage` | `src/workflow_store.rs` | Hardened `claim_task`, `renew_task_lease`, `reconcile_stale_tasks`, `update_task_execution_result` |
| `m31a-agent-engine` | `Cargo.toml` | Added `sha2` dependency |
| `m31a-agent-engine` | `src/lib.rs` | Exported `integration` module |
| `m31a-agent-engine` | `src/integration.rs` | **New file**: `WorkflowIntegrationService`, `FileSnapshot`, approval gating, per-file rollback |
| `m31a-agent-engine` | `src/speculative.rs` | Policy `2.0.0`, `parse_porcelain_path`, candidate timeout, `WorkflowIntegrationService` promotion |
| `m31a-agent-engine` | `src/workflow.rs` | Heartbeat guard, active agent registration, `terminate_tree` cancellation, substantive Analysis/Delivery |
| `m31a-agent-engine` | `tests/speculative_and_workflow_e2e_test.rs` | Added 4 new integration tests covering fencing, recovery, approval, cancellation |
| `m31a-app-server` | `src/protocol.rs` | Async `cancel_workflow`, real `AgentEngineCandidateExecutor` wiring |
| `m31a-cli` | `src/workflow.rs` | Async `cancel_workflow` invocation |
