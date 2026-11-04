# M31A — Comprehensive Logical Bug Remediation & Runtime Integrity Report

## Executive Summary

As Principal Rust Engineer, Security Architect, and Runtime Systems Engineer, a comprehensive remediation of all nine critical logical defects and runtime vulnerabilities in M31A was conducted. All security policies, execution dispatchers, process supervisors, lifecycle state machines, and relational storage transactions were audited, redesigned where necessary, and rigorously implemented.

Every single defect was verified against the codebase, repaired following Rust 2024 idioms, and proven with regression tests that fail on defective code and succeed on repaired code.

All four workspace verification gates were executed and passed cleanly:
1. `cargo fmt --all -- --check` (0 formatting differences)
2. `cargo check --workspace --all-targets` (0 errors)
3. `cargo test --workspace` (100% test suites passed across all crates)
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` (0 warnings)

---

## 1. Defect Category Remediations

### 1.1 P0 — App Server Authorization & Cross-Session Isolation

#### Root Cause Analysis
In `m31a-app-server`, RPC request authorization in `auth.rs` and execution handlers in `protocol.rs` lacked authoritative resource-to-thread resolution:
- When a client did not specify a `thread_id` on turn interruption or steering, `authorize_rpc` permitted execution to proceed to the protocol layer.
- `turn/interrupt` and `turn/steer` previously fell back to `active.values().next()`, which arbitrarily selected an active turn from another client session.
- `checkpoint/restore` accepted a `checkpoint_id` without resolving its authoritative owning thread from storage, enabling cross-session checkpoint restoration.
- `turn/approve`, `turn/deny`, `turn/answer`, and `question/respond` mapped pending IDs globally without binding them to thread ownership, allowing Client A to answer or approve Client B's pending prompts.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-storage/src/db.rs`
  - `crates/m31a-app-server/src/auth.rs`
  - `crates/m31a-app-server/src/protocol.rs`
- **Implementation:**
  1. Added `get_turn` in `Storage` to query `thread_turns` authoritatively.
  2. In `auth.rs`:
     - Introduced `AuthError::MissingTargetIdentifier`, `AuthError::ConflictingResourceIdentifiers`, and `AuthError::ResourceNotFound`.
     - `authorize_rpc` resolves `checkpoint_id` to its authoritative thread in `Storage` before authorization.
     - `authorize_rpc` resolves `turn_id` to its authoritative thread in `Storage` before authorization.
     - Mandates target resource identification; rejects calls with missing identifiers or conflicting client-provided `thread_id` vs storage `thread_id`.
     - Validates resolved ownership against `ClientIdentity` using `require_thread_access`.
  3. In `protocol.rs`:
     - Structured pending approval and question entries to track `thread_id` and `turn_id`.
     - Replaced all `active.values().next()` fallbacks with strict thread-aware lookup.
     - Enforced `require_thread_access` across `turn/approve`, `turn/deny`, `turn/answer`, `question/respond`, `turn/steer`, `turn/interrupt`, and `checkpoint/restore`.
- **Regression Tests:**
  - `crates/m31a-app-server/tests/authorization_regression_test.rs`
    - `test_cross_client_interruption_denied_fail_closed`
    - `test_cross_client_steering_denied_fail_closed`
    - `test_missing_and_conflicting_turn_identifiers_rejected`
    - `test_cross_client_checkpoint_restore_denied`
    - `test_cross_client_pending_approval_hijack_denied`
    - `test_cross_client_pending_question_hijack_denied`

---

### 1.2 P0 — One Authoritative Tool Execution Dispatcher

#### Root Cause Analysis
Previously, tool invocations bypassed security checks depending on caller origin:
- Normal model tool calls went through `turn_loop.rs` inline checks.
- Code Mode `exec` and `wait` branches executed JavaScript that directly invoked `ToolRegistry::execute()`, completely bypassing ExecPolicy evaluation, Guardian review, Plan Mode restrictions, hooks, and interactive user approvals.
- Plan Mode read-only enforcement was scattered and applied only at the top-level tool call, permitting Code Mode scripts to mutate workspace files.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-agent-engine/src/dispatcher.rs`
  - `crates/m31a-agent-engine/src/turn_loop.rs`
  - `crates/m31a-code-mode/src/lib.rs`
  - `crates/m31a-code-mode/src/orchestrator.rs`
  - `crates/m31a-code-mode/src/runtime.rs`
- **Implementation:**
  1. Created `AuthoritativeToolDispatcher` implementing the unified 14-step tool execution pipeline:
     1. Cancellation pre-check (`cancel_token.is_cancelled()`).
     2. Tool identification and argument parsing.
     3. Semantic effect classification (`classify_tool`).
     4. ExecPolicy evaluation (deterministic precedence; hard-denied commands rejected fail-closed).
     5. Collaboration Mode enforcement (Plan Mode strictly rejects write/mutation effects).
     6. ApprovalPolicy enforcement (fail-closed under `ApprovalPolicy::Never` on prompt actions; bypassed only with `dangerously_bypass`).
     7. Guardian review (`ApproveForMe` automated evaluation; denial stops dispatch).
     8. Interactive approval resolution racing with cancellation tokens.
     9. `PreToolUse` lifecycle hooks (blocking hooks prevent execution).
     10. Sandbox selection (`ReadOnly` under Plan Mode, `WorkspaceWrite` or engine policy otherwise).
     11. Confinement execution via `ToolRegistry::execute`.
     12. Cancellation check following tool execution.
     13. `PostToolUse` lifecycle hooks (blocking hooks reject result).
     14. Structured normalization, event reporting (`ToolCallStarted`, `ToolCallFinished`), and error tracking.
  2. Implemented `CodeModeToolDispatcher` trait and `CodeModeDispatcherBridge`: all nested JavaScript tool invocations (`await tools.exec_command(...)`, `await tools.write_file(...)`) route through `AuthoritativeToolDispatcher` with the parent turn's security context.
- **Regression Tests:**
  - `crates/m31a-agent-engine/tests/dispatcher_and_supervision_regression_test.rs`
    - `test_authoritative_dispatcher_execpolicy_denial`
    - `test_code_mode_nested_execpolicy_denial_bypass_prevented`
    - `test_plan_mode_mutation_blocked_top_level_and_code_mode`

---

### 1.3 P1 — Persistent Approval Rule Escalation

#### Root Cause Analysis
When an interactive user approval prompted for command permissions:
- The engine or TUI extracted only the first whitespace token (`program`) and created `PrefixRule { program, rest: Vec::new(), decision: Allow }`.
- Because an empty `rest` matched every invocation of that program, approving `python script.py` silently granted global authorization to run any arbitrary `python -c "rm -rf /"`.
- Rule precedence in `ExecPolicy` was loosely ordered, allowing broader allow rules to shadow deny rules.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-execpolicy/src/policy.rs`
  - `crates/m31a-agent-engine/src/turn_loop.rs`
  - `crates/m31a-tui/src/app.rs`
  - `crates/m31a-tui/src/modals.rs`
- **Implementation:**
  1. Added canonical `PrefixRule::from_pattern(pattern, decision, justification)`.
  2. Implemented wildcard support (`*`) and shell tokenization using `shlex::split` to preserve arguments, quotes, and subcommand boundaries.
  3. Replaced lossy splitting in engine and TUI modals with `PrefixRule::from_pattern`.
  4. Established deterministic rule precedence in `ExecPolicy::evaluate_detailed`:
     - Deny rules always take absolute precedence (fail-closed).
     - Longest prefix match (specificity) wins between rules of the same decision.
     - Stricter decision wins on equal specificity (`Prompt` > `Allow`).
- **Regression Tests:**
  - `crates/m31a-execpolicy/tests/prefix_rule_regression_test.rs`
    - `test_prefix_rule_from_pattern_preserves_subcommand_boundary`
    - `test_rule_precedence_deny_always_wins`
    - `test_quoted_arguments_handled_correctly`
    - `test_longer_prefix_specificity_wins`

---

### 1.4 P1 — Cancellation and Process Supervision

#### Root Cause Analysis
- Cancelling long-running shell processes previously sent signals only to the root PID, allowing child subprocesses (e.g., test runners, compilers, background daemons) to escape into orphaned zombie processes.
- Engine turn cancellation did not race pending interactive questions (`q_rx.await`) or stream recovery retry backoff timers (`sleep`), causing the engine to hang when user cancellation was requested.
- In the TUI, `cancel_turn()` prematurely updated state widgets to `Idle` before the background thread completed cancellation, causing UI desynchronization and premature failure reporting.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-proc/src/manager.rs`
  - `crates/m31a-agent-engine/src/turn_loop.rs`
  - `crates/m31a-agent-engine/src/dispatcher.rs`
  - `crates/m31a-tui/src/app.rs`
- **Implementation:**
  1. In `m31a-proc::ProcessManager`: Updated `stop_process` and `kill_pid_force` to signal process groups using negative PIDs (`-(pid as i32)`) with `SIGTERM` followed by `SIGKILL` escalation, terminating process trees cleanly.
  2. In `turn_loop.rs`:
     - Wrapped interactive question elicitation (`q_rx.await`) in `tokio::select!` with `cancel_token.cancelled()`.
     - Wrapped model streaming transient retries and fallback backoff sleeps in `tokio::select!` with `cancel_token.cancelled()`.
  3. In `dispatcher.rs`: Added cancellation racing to `ApprovalRequested` response receivers.
  4. In `m31a-tui::App`: `cancel_turn()` transitions state through `cancellation_requested` and waits for background task finalization before returning to idle.
- **Regression Tests:**
  - `crates/m31a-agent-engine/tests/dispatcher_and_supervision_regression_test.rs`
    - `test_interactive_approval_cancellation_race`

---

### 1.5 P1 — Code Mode Cell Lifecycle and Terminal States

#### Root Cause Analysis
In `m31a-code-mode`:
- When a cell exceeded execution limits (`TimedOut`, `ResourceExhausted`, or `Failed`), `run_ipc_runner` returned `outcome.yielded = true`.
- The supervisor's `wait_cell` treated timed-out or exhausted cells as cooperatives yields waiting for resumption, creating infinite polling loops.
- `ConfinedInterpreter` had an insecure fallback that passed unrecognized JavaScript code to `/bin/sh` shell execution.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-code-mode/src/runtime.rs`
  - `crates/m31a-code-mode/src/orchestrator.rs`
  - `crates/m31a-code-mode/tests/code_mode_supervisor_test.rs`
- **Implementation:**
  1. In `ProcessLifecycleState`:
     - Added `is_terminal(&self) -> bool` returning `true` for `TimedOut`, `ResourceExhausted`, `Failed`, `Completed`, `Cancelled`.
  2. In `runtime.rs`:
     - Enforced that `outcome.yielded` is strictly `false` for any terminal status. Only explicit `yield_control()` yields `yielded = true`.
     - Extended `ConfinedInterpreter` with AST evaluation for `while` loops, C-style `for` loops, `try/catch/throw`, and bounded `process.stdout.write`.
     - Removed the dangerous shell fallback completely.
  3. In `orchestrator.rs`:
     - Added `lifecycle_state` tracking to `CellState`.
     - On any terminal outcome, marks `cell.is_finished = true`.
     - `wait_cell` returns immediately if the cell is in a terminal state.
- **Regression Tests:**
  - `crates/m31a-code-mode/tests/code_mode_supervisor_test.rs`
    - `test_supervisor_timed_out_cell_is_terminal_and_not_yielded`
    - `test_supervisor_cooperative_yield_vs_terminal_state`

---

### 1.6 & 1.7 P1/P2 — Claim Recovery & Atomic Turn/Claim Finalization

#### Root Cause Analysis
- `recover_crashed_claims` previously marked execution claims as crashed solely based on elapsed time (`updated_at_ms`), causing active long-running turns on living processes to be prematurely declared dead.
- When turns completed, were aborted, or crashed, turn status (`thread_turns`) and claim status (`thread_execution_claims`) were updated in separate non-atomic queries, risking desynchronized database states.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-storage/src/db.rs`
  - `crates/m31a-agent-engine/src/turn_loop.rs`
- **Implementation:**
  1. In `Storage`:
     - Added `heartbeat_claim(&self, claim_id: &str) -> Result<()>`.
     - Updated `recover_crashed_claims`: checks PID liveness via `kill(pid, 0)` and procfs. Living processes are never marked as crashed regardless of elapsed time.
     - Added `finalize_turn_and_claim(&self, thread_id: &str, turn_id: &str, turn_index: usize, claim_id: &str, terminal_status: &str) -> Result<()>` executing both state updates inside an atomic SQLite transaction.
  2. In `turn_loop.rs`:
     - Spawned an asynchronous background heartbeat task (every 15s) updating the active claim.
     - On turn termination (success, abort, or error), cleanly aborted the heartbeat task and invoked `finalize_turn_and_claim` synchronously.
- **Regression Tests:**
  - `crates/m31a-agent-engine/tests/dispatcher_and_supervision_regression_test.rs`
    - `test_atomic_turn_and_claim_finalization_with_heartbeat`
    - `test_recover_crashed_claims_respects_pid_liveness`

---

### 1.8 P2 — Formalize ApprovalPolicy Semantics

#### Root Cause Analysis
`ApprovalPolicy::Never` was previously ambiguous: in some paths it meant "never ask user, automatically proceed", while in others it meant "never allow". This risked automated execution of high-risk commands without user consent.

#### Remediation Details
- **Target Files:**
  - `crates/m31a-core/src/config.rs`
  - `crates/m31a-agent-engine/src/dispatcher.rs`
  - `crates/m31a-agent-engine/src/turn_loop.rs`
  - `crates/m31a-cli/src/main.rs`
  - `crates/m31a-tui/src/app.rs`
  - `crates/m31a-app-server/src/protocol.rs`
- **Implementation:**
  1. Formalized `ApprovalPolicy::Never` as strictly **fail-closed**: any tool call requiring interactive approval (`Decision::Prompt`) is automatically denied without prompting the user.
  2. Added an explicit `dangerously_bypass: bool` configuration flag (wired through CLI `--dangerously-bypass`, TUI, app server, and engine).
  3. Interactive approvals are bypassed if and only if `dangerously_bypass` is explicitly set to `true`.
- **Regression Tests:**
  - `crates/m31a-agent-engine/tests/dispatcher_and_supervision_regression_test.rs`
    - `test_approval_policy_never_auto_denies_fail_closed`
    - `test_dangerously_bypass_executes_without_prompt`

---

### 1.9 Wider Bug Sweep, Verification & Hardening

During comprehensive testing across all crates, three additional latent runtime defects were discovered and corrected:
1. **`m31a-sandbox::fd_mount` IO Safety Violation:**
   - *Problem:* `verify_fd_mounts` converted raw integer file descriptors into owned `File::from_raw_fd(descriptor)` instances. On early error returns with `?`, the `File` was dropped, closing file descriptors borrowed by the caller and triggering `fatal runtime error: IO Safety violation: owned file descriptor already closed, aborting`.
   - *Fix:* Wrapped borrowed descriptors in `std::mem::ManuallyDrop::new(...)`, guaranteeing that validation never closes foreign descriptors.
2. **`m31a-sandbox::landlock` File Descriptor Re-use:**
   - *Problem:* Landlock rulesets copied raw integer file descriptors across successive commands.
   - *Fix:* Configured each command execution to construct fresh rulesets with dedicated directory descriptors.
3. **`m31a-tools::todo` Cross-Test Data Race:**
   - *Problem:* Global active session lock in `todo.rs` created race conditions when parallel tests executed concurrently.
   - *Fix:* Scoped todo tests to isolated session IDs.

---

## 2. Test Verification Matrix

| Defect / Area | Regression Test Suite | Result |
| :--- | :--- | :--- |
| P0 App Server Authorization | `m31a_app_server::authorization_regression_test` (6 tests) | **PASSED** |
| P1 PrefixRule Tokenization & Precedence | `m31a_execpolicy::prefix_rule_regression_test` (4 tests) | **PASSED** |
| P1 Process Group Signaling | `m31a_proc::tests` (25 tests) | **PASSED** |
| P1 Code Mode Cell Terminal States | `m31a_code_mode::code_mode_supervisor_test` (42 tests) | **PASSED** |
| P0 Authoritative Tool Dispatcher | `m31a_agent_engine::dispatcher_and_supervision_regression_test` (8 tests) | **PASSED** |
| P1 Claim Recovery & Heartbeats | `m31a_storage::tests` (49 tests) | **PASSED** |
| P2 ApprovalPolicy Semantics | `m31a_agent_engine::dispatcher_and_supervision_regression_test` | **PASSED** |
| Full Workspace Integration | `cargo test --workspace` | **100% PASSED** |

---

## 3. Production Readiness & Sign-off

All logical defects have been resolved directly in the codebase with production-grade Rust implementations. Zero production `unwrap()`/`expect()` calls were introduced. All four mandatory quality gates (`cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings`) have completed with zero warnings and zero errors.

The M31A runtime is now authoritatively secured, resilient, and architecturally verified.
