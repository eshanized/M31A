# M31A Round 3: Execution Integrity Implementation Report

**Status:** Implementation Complete & Fully Verified  
**Date:** 2026-11-06  
**Author:** Principal Rust Engineer (Execution Integrity Team)  
**Workspace:** Rust 2024 (39 crates)  
**Verification Result:** 100% Workspace Pass (`cargo fmt`, `cargo clippy -D warnings`, `cargo test`)  

---

## 1. Executive Summary

This report documents the implementation details and engineering changes executed during **Round 3: Workflow Execution Integrity Closure** for the M31A workspace.

Round 3 closes all execution-integrity defects identified in the baseline audit (`docs/reports/ROUND_3_BASELINE_AUDIT.md`), fully establishing the Governing Invariant:
> *M31A may report a workflow task as completed only when its actual execution has succeeded, all required acceptance criteria have been independently verified, all mandatory evidence has been persisted successfully, and any required integration has been authorized, applied, and verified.*

All changes strictly adhere to M31A's non-negotiable engineering constraints:
- Preserved existing 39-crate modular architecture.
- Local-first execution and security boundaries intact (zero external auth/passwords introduced).
- Absolute preservation of user data (zero broad destructive `git clean -fd` or `git checkout -- .` commands).
- Idiomatic Rust 2024, strict types, fail-closed error handling, zero synthetic success flags.

---

## 2. Stage-by-Stage Implementation Narrative

### 2.1 Stage 0: Source Baseline & Execution Audit
- Thoroughly audited all 39 crates, focusing on `m31a-agent-engine`, `m31a-storage`, `m31a-core`, `m31a-app-server`, `m31a-cli`, `m31a-tui`, `m31a-review`, and `m31a-verify`.
- Generated `docs/reports/ROUND_3_BASELINE_AUDIT.md`, documenting the actual execution topology, 8 major defect areas, and the direct remediation dependency graph.

### 2.2 Stage 1: Unified Integration & Human Approval Lifecycle
- **Deduplicated Integration Paths:**
  - In `crates/m31a-agent-engine/src/agents.rs`, refactored `EngineAgentRuntime::integrate_agent` to extract the agent worktree diff and delegate directly to `WorkflowIntegrationService::integrate`. Independent and competing patch application logic in `agents.rs` was removed.
  - In `crates/m31a-agent-engine/src/integration.rs`, unified all integration entrypoints into `WorkflowIntegrationService`.
- **Elimination of Synthetic Auto-Approvals:**
  - Removed `is_approved: !config.require_approval_for_integration` in `crates/m31a-agent-engine/src/workflow.rs`.
  - When approval is required, execution pauses in `WorkflowApprovalStatus::AwaitingApproval`, persisting the request to durable storage. Tasks cannot complete while awaiting approval.
- **Durable Approval Storage (Migration 17):**
  - Added SQLite Migration 17 (`workflow_approvals_and_lock_leases`) in `crates/m31a-storage/src/migrations.rs`.
  - Created `workflow_approvals` table with fields: `id`, `workflow_id`, `task_id`, `attempt_id`, `change_set_hash`, `status`, `requested_at`, `decided_at`, `decision_by`, `reason`, `diff_content`.
  - Implemented store methods in `crates/m31a-storage/src/workflow_store.rs`:
    - `create_approval_request()`
    - `record_approval_decision()`
    - `get_approval_request()`
    - `list_approvals()`
    - `is_patch_approved(task_id, attempt_id, change_set_hash)`
- **Change-Set SHA-256 Hash Binding:**
  - Added cryptographic SHA-256 hashing over canonical sorted paths and content in `crates/m31a-agent-engine/src/integration.rs`.
  - Approval decisions are permanently bound to the change-set hash and attempt counter, preventing approval hijacking across modified diffs or superseded attempts.
- **Commit Integrity & Attribution:**
  - In `WorkflowIntegrationService::integrate`, git commit failures are no longer swallowed with `warn!`. Non-zero commit results now trigger atomic rollback and fail closed with `IntegrationError::GitError`.

### 2.3 Stage 2 & 3: Durable Scheduler, Cancellation, and Fenced Execution
- **Immediate SQLite Transactions:**
  - In `crates/m31a-storage/src/workflow_store.rs:721`, changed `conn.transaction()` to `conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)`. This prevents concurrent worker claims from interleaving before lock upgrade.
- **Predicate & Status Consistency:**
  - Aligned candidate selection query with the UPDATE predicate in `claim_next_task` to strictly match `status IN ('ready', 'retry_pending')`.
- **Checked Arithmetic & Fail-Closed Parsing:**
  - Replaced unchecked additions with `checked_add` in timestamp calculations and attempt counter increments (`current_attempt.checked_add(1)`).
  - Replaced `unwrap_or_default()` in path conflict JSON parsing with strict error handling (`StorageError::SerializationError`), failing closed on malformed records.
- **Workflow State Verification on Lease Renewal:**
  - `renew_task_lease` now validates that the parent workflow is in an active state (`Running` or `Verifying`).
- **Fenced Result Persistence:**
  - `save_task_execution_result` requires matching `worker_id`, `current_attempt`, and unexpired `lease_expires_at`. Stale workers attempting to commit are rejected with `StorageError::FencingTokenMismatch`.
- **Cancellation Propagation & Tree Termination:**
  - Integrated `CancellationToken` checkpoints across all execution loops in `crates/m31a-agent-engine/src/workflow.rs`.
  - Registered spawned agents into `active_agent_ids` immediately before invocation to eliminate registration races.
  - Implemented recursive `terminate_tree(&agent_id)` in `crates/m31a-agent-engine/src/agents.rs`, sending termination signals to running child processes and closing agent communication channels.

### 2.4 Stage 4 & 5: Verification Integrity & 12-Point Completion Gate
- **Elimination of Synthetic Verification States:**
  - Removed hardcoded `AnalysisDimensionStatus::Passed` values in `WorkflowOrchestrator`. Tasks must execute empirical checks via `AutonomousVerifier` or fail closed.
  - Workspaces without supported build manifests (`Cargo.toml`, `package.json`, etc.) fail closed with `VerificationIncomplete`.
- **Mandatory Persistence Propagation:**
  - Replaced all ignored `let _ = self.store.save_artifact(...)` patterns with `?` error propagation in `WorkflowOrchestrator::execute_task`.
- **12-Point Final Workflow Completion Gate:**
  - Implemented the 12-point gate in `WorkflowOrchestrator::execute_workflow`. Before declaring `WorkflowStatus::Completed`, the orchestrator verifies:
    1. Zero active/pending tasks remaining.
    2. All tasks in terminal `Completed` state.
    3. Empty error ledger.
    4. Explicit approval verification for all integrated tasks.
    5. No failed or rejected integration outcomes.
    6. Non-zero empirical verification checks passed.
    7. Zero critical review findings.
    8. Mandatory artifacts persisted to disk and database.
    9. Valid commit SHA recorded for git-managed changes.
    10. Bounded filesystem containment.
    11. Root cancellation token untouched.
    12. All workflow path locks released.

### 2.5 Stage 6: File-Backed Crash Recovery Architecture & Test
- Created `crates/m31a-agent-engine/tests/workflow_crash_recovery_test.rs`.
- Validated true file-backed SQLite database persistence across process crashes:
  - Instance A claims a task, writes partial state, and simulates an abrupt crash (dropping in-memory runtime).
  - Instance B starts up, connects to the same file-backed SQLite database, runs `reconcile_stale_tasks`, transitions the abandoned task to `RetryPending`, re-claims it with a new `worker_id` and incremented attempt, and drives the workflow through the 12-point gate to `Completed`.
  - Verified fencing rejection when Instance A attempts late result commits.

### 2.6 Stage 7: Speculative Execution Hardening
- **Porcelain -z NUL-Delimited Parser:**
  - In `crates/m31a-agent-engine/src/speculative.rs`, replaced line-based git status parsing with `parse_porcelain_z` using `\0` delimiters.
  - Fully handles filenames with spaces, unicode, quotes, and git renames (`R  `), capturing both old and new paths.
- **Unified Candidate Executor Factory:**
  - Added `m31a_agent_engine::create_candidate_executor` in `crates/m31a-agent-engine/src/lib.rs`.
  - Replaced ad-hoc executor constructors in `crates/m31a-cli/src/speculate.rs` and `crates/m31a-agent-engine/src/speculative.rs` with the shared factory.

### 2.7 Stage 8: Operator Supervision (App Server, CLI, TUI)
- **App Server JSON-RPC Protocol (`crates/m31a-app-server/src/protocol.rs`):**
  - Added `workflow/approvals` handler (lists pending approval requests).
  - Added `workflow/approve` handler (records operator approval).
  - Added `workflow/reject` handler (records operator rejection with reason).
  - Added comprehensive RPC tests in `crates/m31a-app-server/tests/authorization_regression_test.rs`.
- **CLI Commands (`crates/m31a-cli/src/workflow.rs`):**
  - Added `m31a workflow approve <ID> --reason "..."` command.
  - Added `m31a workflow reject <ID> --reason "..."` command.
  - Added CLI integration tests in `crates/m31a-cli/tests/cli_integration_test.rs`.
- **Interactive TUI Enhancements (`crates/m31a-tui/src/terminal.rs`, `screens.rs`):**
  - Added visual selection cursor `▶` for active workflow tasks.
  - Added operator hotkey handling: `a` (approve), `r` (reject), `c` (cancel), `u` (refresh), `Enter`/`i` (inspect).

---

## 3. Affected Crates & Source Diffs Summary

| Crate | Key Files Modified | Primary Changes |
| :--- | :--- | :--- |
| `m31a-core` | `workflow.rs`, `lib.rs` | `WorkflowApprovalStatus`, `WorkflowApprovalRequest`, enriched `TaskExecutionContext`. |
| `m31a-storage` | `migrations.rs`, `workflow_store.rs`, `worktree.rs` | Migration 17, `workflow_approvals` table, `Immediate` transactions, checked arithmetic, lease fencing, durable approval queries, ignore `.m31a` in `is_dirty`. |
| `m31a-agent-engine` | `integration.rs`, `agents.rs`, `workflow.rs`, `speculative.rs`, `lib.rs` | Authoritative `WorkflowIntegrationService`, unified diff delegation, 12-point completion gate, cancellation token propagation, `parse_porcelain_z`, shared candidate executor factory. |
| `m31a-app-server` | `protocol.rs`, `resource_validator.rs`, `authorization_regression_test.rs` | JSON-RPC endpoints: `workflow/approvals`, `workflow/approve`, `workflow/reject`. |
| `m31a-cli` | `workflow.rs`, `speculate.rs`, `cli_integration_test.rs` | CLI subcommands: `approve`, `reject`, shared speculative factory integration. |
| `m31a-tui` | `screens.rs`, `terminal.rs` | Visual task selector `▶`, approval/rejection/cancellation keybindings. |

---

## 4. Verification & Clean Code Quality

The entire workspace was verified following implementation:
- **Code Formatting:** `cargo fmt --all -- --check` passed with 0 diffs.
- **Linter Check:** `cargo clippy --workspace --all-targets --all-features -- -D warnings` passed with 0 warnings.
- **Unit and Integration Tests:** `cargo test --workspace` passed with 100% success across all 39 crates.
- **Dedicated Round 3 Test Suites:**
  - `crates/m31a-agent-engine/tests/workflow_execution_integrity_round3_test.rs` (12 tests passed)
  - `crates/m31a-agent-engine/tests/workflow_crash_recovery_test.rs` (passed)
  - `crates/m31a-app-server/tests/authorization_regression_test.rs` (passed)
  - `crates/m31a-cli/tests/cli_integration_test.rs` (passed)
