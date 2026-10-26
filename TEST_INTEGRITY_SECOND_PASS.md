# M31A — Second-Pass Test Integrity Overhaul Report
## Elimination of Remaining Fake Tests via Production-Path Verification & Adversarial Falsification

**Author:** Principal Test Engineer & Independent Quality Auditor  
**Date:** 2026-10-26  
**Repository:** `github.com/eshanized/M31A`  
**Crate:** `m31a` v0.1.12 (Single-crate Rust architecture)  
**Authoritative Baseline:** Commit `73e6ff519b8e057a5c6bc28c3f402145e2ccf9e2` (`master`)  

---

## 1. Executive Summary

Following a preliminary audit that resolved surface-level defects, a rigorous second-pass test integrity overhaul was conducted on the authoritative checkout of M31A. The primary standard for this overhaul was: **test the behavior, not the appearance of correctness**.

### Key Second-Pass Discoveries & Remediations
1. **The TOML to Markdown Loader Divergence:** During Phase 8 / Issue #65, prompt contracts and user-defined slash commands were migrated from TOML frontmatter to standard Markdown frontmatter with YAML data (`*.md`). However, multiple test suites (`tests/remediation_runtime_model_promptos.rs`, `tests/security_p0_prompt_trust.rs`, `tests/workflow_prompts.rs`, and `tests/user_defined_commands.rs`) continued writing `.toml` files. Because runtime loaders (`collect_prompt_files`, `collect_command_files`) strictly scan for `.md` files, 23 integration tests were either failing outright or vacuously passing while loading 0 files. All test fixtures were upgraded to canonical Markdown frontmatter, converting failing/vacuous tests into strict behavioral assertions.
2. **Architecture Authority Tests That Only Proved Symbols Existed:** In `tests/architecture_change_authority.rs` and `tests/architecture_planning_authority.rs`, several tests purported to enforce architectural uniqueness, planning seams, and completion gates, but merely asserted `let _ = impl_;` or tautological self-comparisons (`assert_eq!(p1.id, p1.clone().id)` and `classify_scope(1, 5) == direct_classify(1, 5)`). These were converted to true behavioral verifications with `TypeId` identity checks, SQLite state assertions, concurrency surface reservation conflict tests, and completion gate evaluation against unpersisted tasks.
3. **Unexamined Asynchronous Outcomes & Fake Successes (`let _ = ...`):** In `tests/genesis_discovery.rs`, `tests/p3_agent_execution_test.rs`, `tests/checkpoint_authority_test.rs`, `tests/agentic_core_coordination_and_dispatch.rs`, and `tests/scheduler_state_machine_and_cancellation.rs`, critical asynchronous state machine operations (`next_turn`, `run_step_loop`, `step`, `find_ready_work`) had their outcomes discarded via `let _ = ...`. In `p3_agent_execution_test.rs`, this hid the fact that the runner properly rejects unverified completion attempts for implementer roles with `AgentOutcome::StepLimitExceeded`. In `scheduler_state_machine_and_cancellation.rs`, 5 occurrences were replaced with rigorous checks on `ready_tasks`, candidate titles, and `blocked_tasks_count`.
4. **Conditional Test Assertions:** In `tests/intent_decomposition_and_goal_analysis.rs`, assertions were wrapped inside `if state.unknowns.is_empty()`, allowing them to pass silently if the condition was false. These were converted to unconditional assertions, and 7 unexamined `let _ = state.add_unknown(...)` calls were upgraded to `.expect("evidence-backed unknown must be added")` to ensure duplicate or fabricated unknowns fail closed.
5. **Outdated Technical Debt in Architectural Boundaries:** In `tests/architecture_process_authority.rs`, an outdated exception permitting process spawning in `"src/runtime.rs"` was removed from `approved_boundaries`, closing an architecture loophole and enforcing that `src/runtime.rs` contains zero process spawn sites.

### Overhaul Metrics
- **Total Files Audited:** 184 (116 in `src/`, 68 in `tests/`)
- **Total Tests Remediated in Second Pass:** 38 test functions across 10 test suites
- **Falsification Mutations Tested:** 2 isolated adversarial mutations (`classify_scope` returning `Local` unconditionally; `evaluate_task_completion` failing open on missing DB records)
- **Falsification Result:** Both mutated implementations triggered immediate test failures, verifying that tests detect broken production code.
- **Verification Status:** `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and full `cargo test` passing 100%.

---

## 2. Methodology: Distinguishing Real from Fake Tests

A test was classified as **defective or fake** if it met any of the following criteria:
1. **Tautological Assertions:** Assertions comparing a value to itself, to its own clone, or comparing two calls to the same underlying function (`classify_scope(1, 5) == direct_classify(1, 5)`).
2. **Vacuous Discards (`let _ = ...`):** Calling core production entry points without asserting on the returned `Result`, state change, or side effect.
3. **Format/Extension Mismatches:** Tests writing legacy `.toml` files that runtime loaders skip, resulting in empty collections.
4. **Permissive Conditionals:** Wrapping assertions in `if` blocks that can evaluate to `false` and skip verification without error.
5. **Symbol-Only Architecture Checks:** Tests that only instantiate a type or print its name without testing dispatch, authority, or contract enforcement.

### Falsification Standard
For every critical behavioral contract, an adversarial fault was considered:
- If `classify_scope` is mutated to return `Local` for all task counts, does the test fail? **Yes** (panicked with `left: Local, right: Partial`).
- If `evaluate_task_completion` is mutated to approve unpersisted task records, does the test fail? **Yes** (panicked with `Gate must reject evaluation for unpersisted task record`).

---

## 3. Detailed Audit Findings & Remediations

### 3.1 PromptOS & User Command Markdown Migration Boundary
- **Files:**
  - `tests/remediation_runtime_model_promptos.rs`
  - `tests/security_p0_prompt_trust.rs`
  - `tests/workflow_prompts.rs`
  - `tests/user_defined_commands.rs`
- **Defect:** Runtime loaders (`collect_prompt_files` in `src/prompt/catalog.rs` and `collect_command_files` in `src/interaction/user_commands.rs`) strictly enforce `ext == "md"`. Tests were generating `.toml` files. In `user_defined_commands.rs`, 19 tests failed with `NotFound` because commands were never loaded. In `security_p0_prompt_trust.rs`, `evil.toml` was ignored so guidance was never recorded.
- **Remediation:** Migrated all test fixtures to valid Markdown frontmatter with YAML headers (`*.md`). Implemented `write_command` helper in `tests/user_defined_commands.rs`. Updated `test_phase_s_7_atomic_reload_failure_preserves_valid_commands` to use `FORBIDDEN_NS_MD` with valid frontmatter to test contract ID namespace validation instead of parser failure.
- **Verification:** All 29 tests in `tests/user_defined_commands.rs`, all 10 tests in `tests/workflow_prompts.rs`, and all 4 tests in `tests/security_p0_prompt_trust.rs` pass.

### 3.2 Change Authority Architecture Guards (`tests/architecture_change_authority.rs`)
- **Defects:**
  - `test_canonical_change_proposal_type_is_unique`: Tautological self-comparison `assert_eq!(p1.id, p1.clone().id)`.
  - `test_change_provenance_store_is_unique`: Checked symbol string name via `type_name::<ChangeProvenanceStore>()` without invoking store.
  - `test_change_authority_is_unique_authority`: Discarded three `type_name` calls without testing concurrency surface reservation.
  - `test_completion_gate_remains_canonical`: Only checked `TypeId` of `EvidenceCompletionGate` without testing gate evaluation.
- **Remediation:**
  - Replaced self-comparison with `TypeId` equality between `m31a::change::ChangeProposal` and `m31a::kernel::ChangeProposal`, plus distinct UUID validation.
  - Replaced `type_name` check with behavioral `store.record(&record, None)` and exact retrieval verification from `store.get_proposal(id)`.
  - Implemented behavioral concurrency reservation tests calling `reserve_surface`, verifying conflict rejection (`ConcurrentTaskConflict`) on overlapping surfaces, and releasing with `release_surface`.
  - Added behavioral verification calling `gate.evaluate_task_completion` against in-memory SQLite, proving unpersisted tasks fail closed with error.
- **Verification:** `cargo test --test architecture_change_authority` (12/12 passed).

### 3.3 Planning Authority Architecture Guards (`tests/architecture_planning_authority.rs`)
- **Defects:**
  - `test_plan_service_seam_is_accessible`: Discarded instance with `let _ = impl_;`.
  - `test_classify_scope_accessible_from_canonical_path`: Self-comparison `assert_eq!(classify_scope(1, 5), direct_classify(1, 5));`.
  - `test_plan_quality_metrics_accessible_from_canonical_path`: Only checked `task_count` on an empty plan.
- **Remediation:**
  - Upgraded `test_plan_service_seam_is_accessible` to an async test proving `has_valid_plan(mid)` is initially false, actionable plan request without model provider fails closed with "No model provider ... refusing to fabricate", and read-only request successfully generates initial plan making `has_valid_plan(mid)` return true.
  - Replaced self-comparison in `test_classify_scope_accessible_from_canonical_path` with explicit assertions against `ReplanScope::Local` (counts 0, 1), `Partial` (count 3), and `Full` (count >= 5, or old count 0).
  - Populated candidate plan in `test_plan_quality_metrics_accessible_from_canonical_path` with affected paths and criteria; verified exact metrics and `TypeId` equality.
- **Verification:** `cargo test --test architecture_planning_authority` (12/12 passed).

### 3.4 Socratic Discovery Interview (`tests/genesis_discovery.rs`)
- **Defects:** Lines 82, 101, 121, 140, 145, 150 discarded `session.next_turn().unwrap()` with `let _ = ...`.
- **Remediation:** Captured `DiscoveryTurn` from each turn, asserting expected `turn_number`, non-empty question text, and valid options before submitting responses.
- **Verification:** `cargo test --test genesis_discovery` (7/7 passed).

### 3.5 Agent Execution Step Loop (`tests/p3_agent_execution_test.rs`)
- **Defects:** `let _ = runner.run_step_loop(...).await;` in `test_p3_d_repeated_failing_action_rejected_without_dispatch` and `test_p3_d_consecutive_edit_failures_escalation_warning`.
- **Root Cause:** The test author discarded the outcome because `runner.run_step_loop` returned `AgentOutcome::StepLimitExceeded` rather than `Succeeded`. This occurred because the agent role was `implementer`, which strictly requires verification/completion evidence; when `ModelProposal::Complete` was proposed without evidence, the runner rejected premature completion, consumed a step, and hit the step limit.
- **Remediation:** Asserted exact outcome `AgentOutcome::StepLimitExceeded { limit: 3, consumed: 3 }` (and limit 4), explicitly verifying that the runtime refuses to fake success for unverified implementers.
- **Verification:** `cargo test --test p3_agent_execution_test` (17/17 passed).

### 3.6 Intent Decomposition & Unknown Evidence (`tests/intent_decomposition_and_goal_analysis.rs`)
- **Defects:**
  - Seven occurrences of `let _ = state.add_unknown(...)` discarded errors.
  - `test_intent_context_fragment_is_empty_when_no_state` wrapped assertions in `if state.unknowns.is_empty()`.
  - `setup_git_fixture` discarded git subprocess exit statuses with `let _ = Command::new("git")...status();`.
- **Remediation:**
  - Upgraded all `add_unknown` calls to `.expect("evidence-backed unknown must be added")`, ensuring fabricated or duplicate unknowns are rejected.
  - Converted conditional assertions to unconditional checks on state and fragment text.
  - Replaced discarded git subprocess calls with `.expect().success()`.
- **Verification:** `cargo test --test intent_decomposition_and_goal_analysis` (36/36 passed, 1 live test ignored).

### 3.7 Process Execution Architecture Boundaries (`tests/architecture_process_authority.rs`)
- **Defect:** `approved_boundaries` included `"src/runtime.rs"` under an outdated comment citing technical debt.
- **Remediation:** Removed `"src/runtime.rs"` from `approved_boundaries`. Verified that `src/runtime.rs` contains zero process spawn sites and that `test_runtime_rs_process_execution_contained_to_known_exception` strictly enforces zero occurrences.
- **Verification:** `cargo test --test architecture_process_authority` (3/3 passed).

### 3.8 Checkpoint Authority (`tests/checkpoint_authority_test.rs`)
- **Defect:** Line 260 discarded `controller.step().await.unwrap()` with `let _ = ...`.
- **Remediation:** Asserted `StageOutcome::Advance(_) | StageOutcome::SkipTo(_)`, proving the checkpoint stage executed and transitioned the loop.
- **Verification:** `cargo test --test checkpoint_authority_test` (2/2 passed).

### 3.9 Agentic Core Coordination & Dispatch (`tests/agentic_core_coordination_and_dispatch.rs`)
- **Defect:** Lines 660 and 884 discarded `engine.step(...)` outcomes with `let _ = ...`.
- **Remediation:** Asserted exact turn outcomes (`AgentTurnOutcome::AssistantCommentary` / `AssistantText`), verifying steering message incorporation and mission scope reuse.
- **Verification:** `cargo test --test agentic_core_coordination_and_dispatch` (16/16 passed, 1 live test ignored).

### 3.10 Work Scheduler Engine (`tests/scheduler_state_machine_and_cancellation.rs`)
- **Defect:** Five instances of `let _ = engine.find_ready_work(mission_id).await.unwrap()` discarded readiness results.
- **Remediation:** Asserted `ready_work.ready_tasks.len() == 1`, `ready_work.blocked_tasks_count == 0` (proving pending tasks waiting on unmet dependencies are not counted as blocked until an upstream task terminally fails), and verified exact ready task title (`"Objective for task_1"`, `"Objective for task_a"`, `"Objective for m2_task"`).
- **Verification:** `cargo test --test scheduler_state_machine_and_cancellation` (7/7 passed).

---

## 4. Adversarial Falsification Log

| Mutation Target | File | Deliberate Fault Injected | Expected Test Failure | Observed Test Failure | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `classify_scope` | `src/recovery/replan.rs` | Unconditionally return `ReplanScope::Local` for all task counts | `test_classify_scope_accessible_from_canonical_path` fails on count >= 5 | Panicked at line 256: `assertion left == right failed (left: Local, right: Partial)` | **Falsified & Verified** |
| `evaluate_task_completion` | `src/verification/gate.rs` | Unconditionally return `Ok(CompletionGateDecision::satisfied(...))` when task is missing from DB | `test_completion_gate_remains_canonical` fails closed check | Panicked at line 306: `Gate must reject evaluation for unpersisted task record` | **Falsified & Verified** |

---

## 5. Verification Commands & Output Summary

The standard verification pipeline was executed:

1. **Format Check:**
   ```bash
   cargo fmt --check
   ```
   *Result:* Clean (0 warnings or errors).

2. **Compile Check:**
   ```bash
   cargo check --all-targets
   ```
   *Result:* Clean compilation across all crates, targets, and benchmarks.

3. **Clippy Linters:**
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
   *Result:* 0 warnings, 0 errors.

4. **Test Suite:**
   ```bash
   cargo test
   ```
   *Result:* All unit and integration test targets passed cleanly.
