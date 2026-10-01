# M31A State Checkpoints & Fault Recovery

This document describes the crash recovery scanner, two-phase checkpoint protocol, 15 canonical failure classifications, differential replanning engine, and fail-closed corruption semantics in M31 Autonomous (M31A).

## Core Recovery Principle

> **"Never fake success. Preserved work must have cryptographic evidence; corrupted state fails closed."**

Autonomous software engineering involves complex, multi-step code transformations prone to compiler errors, process interrupts, power loss, and host crashes. M31A ensures state is recoverable without blind re-execution or silent corruption.

---

## Two-Phase Checkpoint Commit Protocol

Checkpoints are captured at task boundaries or after critical verification gates using a strict **two-phase commit protocol** (`CheckpointManager`):

```
Phase 1: External Artifact Staging
  ├── Write verification logs, patches, and reports to staging/<checkpoint_id>/
  ├── Flush bytes to storage with tokio::fs::File::sync_all
  └── Compute and validate content-addressed SHA-256 digests against manifest
         ↓
Phase 2: Atomic SQLite Transaction
  ├── BEGIN TRANSACTION;
  ├── INSERT INTO checkpoints (id, mission_id, manifest_json, manifest_hash, ...);
  ├── INSERT INTO checkpoint_artifacts (...);
  ├── COMMIT;
  └── On Commit Success: Promote staged artifacts to authoritative FsArtifactStore
```

### Crash Invariant
If a crash or power failure occurs during Phase 1, the SQLite transaction has not executed; the staged files remain harmless orphans. If a crash occurs after Phase 2, the checkpoint is fully committed and authoritative.

---

## The 15 Canonical Failure Classifications

Errors detected during task execution or verification are deterministically categorized into 15 structured failure classes (`FailureClassification`):

| Failure Class | Retryable? | Description | Canonical Recovery Action |
|:---|:---:|:---|:---|
| **`Transient`** | Yes | Temporary network drop, connection reset, or upstream 503. | Exponential backoff retry with jitter. |
| **`Timeout`** | Yes | Process or model invocation exceeded step time limit. | Re-attempt with increased timeout or smaller chunk. |
| **`Permission`** | **No (0 Budget)** | Attempted access to out-of-workspace or restricted file. | Non-retryable; route to policy coordinator or halt. |
| **`Policy`** | **No (0 Budget)** | Invocation rejected by active security policy or veto. | Non-retryable; halt or pause for interactive approval. |
| **`Environment`** | No | Missing system dependency (e.g. `cargo` binary not found). | Halt mission; notify operator via doctor diagnostics. |
| **`Dependency`** | Yes | Crate or package resolution failure (`Cargo.lock` conflict). | Trigger dependency resolution task. |
| **`Compilation`** | Yes | Rust/compiler syntax error or mismatched type (`E0308`). | Pass compiler diagnostics to `Diagnostician` agent. |
| **`Test`** | Yes | Unit or integration test assertion failure. | Trigger targeted repair task in `Implementer` agent. |
| **`ToolContract`** | Yes | Model provided invalid JSON arguments violating tool schema. | Re-prompt model with JSON schema validation errors. |
| **`Model`** | Yes | Model provider returned error, rate limit, or refusal. | Fall back to alternative configured provider model. |
| **`Context`** | No | Prompt context exceeded maximum model window limit. | Evict lower-priority context; compact history. |
| **`ResourceLimit`** | No | Child process killed by kernel OOM or cgroup memory limit. | Non-retryable under current limits; pause or halt. |
| **`RepositoryState`** | Yes | Unexpected working tree drift or merge conflict. | Re-capture baseline; synchronize git worktree. |
| **`Architecture`** | Yes | Cycle detected in task DAG or repeated cyclic oscillation. | Trigger `DifferentialReplanEngine` to restructure DAG. |
| **`Unknown`** | No | Unhandled kernel panic or unexpected trap code. | Halt mission fail-closed; emit diagnostic dump. |

### Strict Zero-Budget Rule for Security Failures
Per decision **D-06**, `Permission` and `Policy` failures strictly evaluate to a retry budget of `0`. Security violations are never automatically retried, eliminating brute-force bypass attempts.

---

## Differential DAG Replanning Engine

When a task fails with a retryable error or architectural invalidation, the runtime does not discard previously completed work. The `DifferentialReplanEngine` executes a differential reconciliation:

1. **Preservation of Verified Work**: Tasks in `TaskState::Succeeded` whose semantic inputs and repository baselines have not changed remain untouched. Their verified success state is carried forward into revision $N+1$.
2. **Targeted Invalidation**: Only the failed task, its downstream dependents, or invalidated nodes are superseded.
3. **Revision Tracking**: A new `TaskGraph` revision is materialized in SQLite with incremental revision numbers ($rev_1 \to rev_2$).
4. **Durable Audit Trail**: Every replan transaction is persisted in the `recovery_attempts` table.

---

## Startup Crash Recovery Scanner

When the M31A runtime daemon starts up, the `StartupCrashRecoveryScanner` scans SQLite for missions in unfinished states (`in_progress`, `running`). It evaluates the recovery state into 4 strict classifications:

| Recovery Classification | Is Safe to Resume? | Description |
|:---|:---:|:---|
| **`SafeToResume`** | **Yes** | Checkpoint manifest valid, all artifacts present in store, repo workspace clean. |
| **`NeedsRepair`** | No (Auto-Repairable) | Checkpoint valid, but workspace files need restoration to baseline commit. |
| **`Ambiguous`** | **No (Halt/Escalate)** | Unexplained repo drift or conflicting job outcomes. Never auto-promoted. |
| **`Corrupt`** | **No (Halt/Rollback)** | Missing required artifacts, truncated SQLite WAL, or invalid schema hash. |

### Fail-Closed Recovery Guarantee
In accordance with requirement `SEC-01`, `Ambiguous` and `Corrupt` states are **never auto-promoted to success or resumed automatically**. The runtime halts execution and prompts for administrative investigation.
