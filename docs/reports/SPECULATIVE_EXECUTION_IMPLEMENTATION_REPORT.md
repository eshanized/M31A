# Speculative Execution Implementation Report

**Milestone:** Foundation Milestone — Genuine Speculative Execution  
**Subsystems:** `m31a-agent-engine`, `m31a-tools`, `m31a-verify`, `m31a-ast`, `m31a-storage`  
**Date:** November 2026  
**Status:** Completed & Fully Verified

---

## 1. Executive Summary

This implementation report documents the complete overhaul of `SpeculativeArena` in `crates/m31a-agent-engine/src/speculative.rs`. Previously, the speculative arena contained architectural flaws where candidate branches were generated with `patch_content: None`, allowing unchanged workspaces to be declared as winners.

The completed milestone transforms `SpeculativeArena` into an active, multi-candidate execution and evaluation engine that:
1. Disallows unchanged or empty candidates from passing verification or winning competitions.
2. Generates genuinely diverse engineering hypotheses (algorithmic, zero-copy, parallel) tailored to task objectives.
3. Executes candidates in isolated Git worktrees or filtered ephemeral sandboxes.
4. Enforces four sequential hard gates (Meaningful implementation, Build check, Test suites, and Codebase safety).
5. Implements a deterministic composite scoring policy (`SCORING_POLICY_VERSION = "1.0.0"`).
6. Executes two-phase atomic promotion with dirty workspace preflight checks, path containment verification, and automated rollback upon post-integration verification failure.

---

## 2. Key Code Modifications

### 1. Hard Gate 1: Elimination of Empty Candidate Exploit
In `crates/m31a-agent-engine/src/speculative.rs`:
```rust
let implementation_valid = !modified_files.is_empty() && failure_reason == SpeculativeFailureReason::None;
if !implementation_valid && failure_reason == SpeculativeFailureReason::None {
    failure_reason = SpeculativeFailureReason::NoImplementation;
}
```
If a candidate introduced no changes, it is marked `SpeculativeCandidateState::Rejected`, given `composite_score = 0.0`, and precluded from promotion.

### 2. Hypothesis Diversification
Implemented `SpeculativeArena::generate_diverse_hypotheses`, automatically detecting task intent (e.g. performance vs architectural) and generating distinct, structured candidate approaches.

### 3. Candidate Execution Abstraction
Introduced the `CandidateExecutor` trait and `CandidateExecutionContext`, allowing real `AgentEngine` or test executors to run within isolated sandboxes without circular crate dependencies.

### 4. Deterministic Scoring Model
Structured `ScoreBreakdown` capturing:
- Correctness (50 points)
- Safety & blast radius (25 points)
- Performance (20 points)
- Maintainability & locality (15 points)

### 5. Safe Workspace Promotion & Rollback
Implemented two-phase promotion:
- Verified primary workspace is clean using `WorktreeManager::is_dirty`.
- Validated patch paths using `WorktreeManager::validate_patch_paths`.
- Applied patch to primary repository.
- Re-ran verification (`AutonomousVerifier::verify_check` and `verify_tests`).
- If post-verification failed, triggered automatic `git checkout -- .` and `git clean -fd` rollback.

---

## 3. Test Verification Results

All tests pass 100%:

| Test Target | Tests | Status |
| :--- | :---: | :---: |
| `crates/m31a-agent-engine/src/speculative.rs` (Unit tests) | 7 | **PASS** |
| `crates/m31a-agent-engine/tests/speculative_and_workflow_e2e_test.rs` | 2 | **PASS** |
| `crates/m31a-agent-engine/tests/agent_lifecycle_and_integration_test.rs` | 9 | **PASS** |
| `crates/m31a-agent-engine` (Total crate test suite) | 91 | **PASS** |

### Verified Test Cases:
- `test_empty_candidate_is_rejected_and_cannot_win`: Verifies empty candidates receive `0.0` score and `SpeculativeFailureReason::NoImplementation`.
- `test_failing_tests_gate_rejection`: Verifies candidates with broken tests fail Hard Gate 3.
- `test_candidate_executor_invoked_in_sandbox`: Verifies candidate executor writes files inside isolated sandbox.
- `test_valid_patch_candidate_evaluated_and_selected`: Verifies working candidate passes all gates and is selected as winner.
- `test_promotion_requires_approval_leaves_pending`: Verifies human approval requirements leave workspace unmodified until confirmed.
- `test_genuine_speculative_execution_compete_and_promote`: End-to-end multi-candidate competition in a real Cargo Git repo, verifying empty candidate rejection, syntax error candidate rejection, working candidate selection, clean promotion, and post-promotion test passage.
