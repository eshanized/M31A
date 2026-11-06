# Speculative Parallel Reality Architecture

**System:** M31A Autonomous Engineering Platform  
**Subsystem:** Speculative Candidate Execution & Multi-Reality Arena  
**Primary Modules:** `m31a-agent-engine::speculative`, `m31a-verify`, `m31a-storage::worktree`, `m31a-ast`  
**Version:** 1.0.0 (Foundation Milestone)

---

## 1. Overview and Mission

The Speculative Parallel Reality engine transforms M31A from a sequential single-turn coding assistant into an engineering platform capable of exploring multiple competing hypotheses concurrently.

When given a complex engineering objective—such as optimizing performance, refactoring architecture, or addressing non-trivial defects—relying on a single generated solution risks regression, architectural debt, or dead ends. The Speculative Arena generates diverse, task-specific approaches, executes candidate implementations in fully isolated sandboxes, evaluates each candidate using hard verification gates and real empirical evidence, and safely promotes the winning solution into the primary workspace.

```
                           [Engineering Objective]
                                      │
                                      ▼
                        [Diverse Hypothesis Generator]
                       (Algorithmic, Zero-Copy, Parallel)
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
  [Candidate 1 Sandbox]       [Candidate 2 Sandbox]       [Candidate 3 Sandbox]
  (Git Worktree / Tempdir)    (Git Worktree / Tempdir)    (Git Worktree / Tempdir)
          │                           │                           │
  [Real Agent Execution]      [Real Agent Execution]      [Real Agent Execution]
          │                           │                           │
  [Hard Gate 1: Non-Empty]    [Hard Gate 1: Non-Empty]    [Hard Gate 1: Non-Empty]
  [Hard Gate 2: Build Check]  [Hard Gate 2: Build Check]  [Hard Gate 2: Build Check]
  [Hard Gate 3: Test Suite]   [Hard Gate 3: Test Suite]   [Hard Gate 3: Test Suite]
  [Hard Gate 4: Code Safety]  [Hard Gate 4: Code Safety]  [Hard Gate 4: Code Safety]
          │                           │                           │
    Score: 0.0 (Rejected)       Score: 0.0 (Failed)        Score: 88.5 (Eligible)
          └───────────────────────────┼───────────────────────────┘
                                      │
                                      ▼
                       [Deterministic Composite Scorer]
                           (Select Winning Branch)
                                      │
                                      ▼
                        [Two-Phase Safe Promotion]
                    - Preflight Dirty Workspace Check
                    - Strict Patch Path Containment
                    - Post-Integration Build/Test Gate
                    - Automatic Rollback on Failure
                                      │
                                      ▼
                         [Verified Primary Workspace]
```

---

## 2. Hard Gating Pipeline

A fundamental design requirement is that unverified, broken, or unchanged candidates can **never** be selected as winners. Previous versions suffered from a vulnerability where unchanged workspaces were awarded victories because no build errors were found. The new architecture enforces four mandatory sequential hard gates:

### Gate 1: Meaningful Implementation Validation
- **Requirement:** A candidate must introduce actual, non-empty workspace modifications (`!modified_files.is_empty()`).
- **Enforcement:** If no files were modified or the patch content is empty, the candidate is immediately rejected with `SpeculativeFailureReason::NoImplementation`.
- **Score:** Composite score is fixed to `0.0`. State is set to `SpeculativeCandidateState::Rejected`. Downstream build, test, and safety stages are skipped.

### Gate 2: Build & Typecheck Verification
- **Requirement:** The candidate's modifications must compile cleanly in the project's native build system.
- **Enforcement:** Executes `AutonomousVerifier::verify_check` within the candidate's isolated sandbox.
- **Failure:** If compiler errors occur, failure reason is set to `SpeculativeFailureReason::BuildFailed(diagnostics)` and the candidate is marked `SpeculativeCandidateState::Failed` with score `0.0`.

### Gate 3: Test Suite Verification
- **Requirement:** Existing regression tests and any newly added acceptance tests must pass completely.
- **Enforcement:** Executes `AutonomousVerifier::verify_tests` within the sandbox.
- **Failure:** Any test failure, timeout, or runtime crash results in `SpeculativeFailureReason::TestsFailed(details)` with score `0.0`.

### Gate 4: Codebase Safety & AST Blast-Radius Audit
- **Requirement:** Candidate code must not introduce dangerous data flows, security taint violations, or excessive blast radius.
- **Enforcement:** `CodebaseSafetyGate::audit_workspace_changes` inspects modified AST nodes, symbol definitions, cross-file references, and source-to-sink dataflow taints.
- **Failure:** Any `SafetyGateVerdict::Blocked` finding triggers `SpeculativeFailureReason::SecurityBlocked(summary)` with score `0.0`.

---

## 3. Workspace Isolation and Concurrency

Candidates must never write concurrently into the same shared workspace or interfere with each other.

### 1. Git Worktree Isolation
When the source workspace is located inside a Git repository:
- `WorktreeManager::create_worktree` spawns an independent, lightweight Git worktree at `.m31a/worktrees/<branch_id>`.
- The worktree is mapped to a dedicated Git branch `m31a-spec-<branch_id>`.
- Git pointer files (`.git`) are recognized and respected; bubblewrap mounts protect parent repositories while granting full read-write execution inside the worktree root.
- The `target/` build directory is isolated per candidate, ensuring no cross-process file locking collisions during parallel `cargo` invocations.

### 2. Ephemeral Sandbox Copy Fallback
If the workspace is not a Git repository:
- Ephemeral filtered copy directory is created in `std::env::temp_dir()`.
- Excludes `.git`, `target`, `node_modules`, `dist`, and hidden metadata.
- Managed by `SpeculativeSandboxGuard`, ensuring RAII cleanup upon candidate completion unless explicitly flagged for evidence retention.

### 3. Concurrency Bounds
- Orchestrated via `tokio::task::JoinSet` bounded by an `Arc<tokio::sync::Semaphore>`.
- Configurable `max_concurrency` (default: 4, clamped between 1 and 16).
- Independent cancellation tokens (`CancellationToken`) per candidate prevent cascading task aborts.
- Candidate failures are isolated; one candidate crashing or timing out does not terminate competing candidates.

---

## 4. Deterministic Composite Scoring Model

Candidates passing all four hard gates are ranked according to a deterministic composite scoring function (`SCORING_POLICY_VERSION = "1.0.0"`):

$$\text{Composite Score} = S_{\text{correctness}} + S_{\text{safety}} + S_{\text{perf}} + S_{\text{maintainability}}$$

| Component | Max Points | Evaluation Basis |
| :--- | :---: | :--- |
| **Correctness** | 50.0 | Passing build (30.0 pts) + passing full test suite (20.0 pts). |
| **Safety** | 25.0 | Inverse blast radius score: $(100 - \text{blast\_radius}) \times 0.25$. Penalized by 10 pts if safety warnings exist. |
| **Performance** | 20.0 | Measured latency/throughput improvement via `PerformanceProfiler`. 20.0 pts for measured improvements, 10.0 pts for neutral/unmeasured standard baselines. |
| **Maintainability** | 15.0 | High locality reward: $15.0 - (\text{modified\_files} \times 0.5)$, clamped between 5.0 and 15.0 pts. |

If any hard gate fails, $\text{Composite Score} = 0.0$ strictly.

---

## 5. Two-Phase Safe Promotion and Rollback

Promoting code changes into the user's primary repository requires rigorous transactional guards:

1. **Preflight Clean-Workspace Verification:**  
   Before any patch is applied, `WorktreeManager::is_dirty` checks the primary workspace. If uncommitted changes exist, promotion is blocked immediately to prevent overwriting user work.

2. **Patch Path Containment Verification:**  
   `WorktreeManager::validate_patch_paths` inspects every modified file path. Any directory traversal (`..`), root paths (`/`), or protected directories (`.git`, `.m31a`, `.ssh`, `.env`) are rejected before disk modification.

3. **Application:**  
   - If in M31A tool format (`*** Begin Patch`), applied via `m31a_tools::patch::apply_patch_to_workspace`.
   - If in unified git diff format, applied via atomic `git apply --ignore-whitespace`.

4. **Post-Integration Verification Gate:**  
   Immediately following patch application, `AutonomousVerifier::verify_check` and `AutonomousVerifier::verify_tests` execute on the primary workspace.

5. **Automated Rollback:**  
   If post-integration build or tests fail:
   - `git checkout -- .` and `git clean -fd` restore the primary workspace to its exact pre-integration state.
   - `rollback_executed` is set to `true` in `SpeculativeIntegrationReport`.
   - The failure details are recorded in the audit trail.
