# Speculative Evaluation and Scoring Model Specification

**Subsystem:** `m31a-agent-engine::speculative`  
**Policy Version:** `1.0.0`  
**Status:** Canonical Implementation

---

## 1. Candidate Hypothesis Generation

Speculative candidate generation produces materially different technical strategies tailored to the engineering domain rather than cosmetic variations.

### 1. Performance and Optimization Objectives
When an objective involves performance, latency, throughput, or efficiency (`optimi`, `perf`, `speed`, `fast`), `generate_diverse_hypotheses` generates three distinct hypotheses:

- **Approach A (Algorithmic & Complexity Optimization):**  
  Redesign algorithmic inner loop and data access patterns to reduce computational complexity ($O(N^2) \to O(N)$) and eliminate redundant traversals.
- **Approach B (Zero-Copy & Memory Allocation Elimination):**  
  Replace heavy heap allocations, clones, and owned data structures with borrowed references, contiguous arena storage, and zero-copy slicing.
- **Approach C (Parallel Execution & Concurrent Pipeline):**  
  Decompose independent computational workloads across parallel worker threads using bounded channels and work-stealing concurrency.

### 2. General Architecture & Defect Objectives
When an objective involves features, bug fixes, or architecture:

- **Approach A (Direct Targeted Implementation):**  
  Minimal, focused modifications directly addressing the core objective with high locality and minimal blast radius.
- **Approach B (Decoupled Modular Architecture):**  
  Extract clean interface boundaries and isolate functional components to maximize testability and eliminate tight coupling.
- **Approach C (Defensive & Fault-Tolerant Implementation):**  
  Robust state validation, defensive bounds checking, explicit fallback paths, and clear error telemetry.

---

## 2. Hard Gate Requirements

A candidate MUST satisfy all four hard gates to be considered eligible. If any gate fails, $\text{Composite Score} = 0.0$ and the candidate is rejected/failed:

```
[Candidate Branch]
       │
       ▼
[Gate 1: Implementation Check] ──(No changes / empty patch)──> [Rejected: NoImplementation] (Score: 0.0)
       │ (Files modified)
       ▼
[Gate 2: Build & Typecheck]    ──(Compile errors)────────────> [Failed: BuildFailed] (Score: 0.0)
       │ (Clean build)
       ▼
[Gate 3: Test Verification]    ──(Test failure / timeout)────> [Failed: TestsFailed] (Score: 0.0)
       │ (Tests pass)
       ▼
[Gate 4: Codebase Safety]      ──(SafetyGateVerdict::Blocked)─> [Failed: SecurityBlocked] (Score: 0.0)
       │ (Verdict != Blocked)
       ▼
[Eligible Candidate] ───> [Deterministic Scorer: Score > 0.0]
```

### Gate Failure Taxonomy

```rust
pub enum SpeculativeFailureReason {
    None,
    NoImplementation,
    PatchParseError(String),
    PatchApplicationFailed(String),
    BuildFailed(String),
    TestsFailed(String),
    SecurityBlocked(String),
    Timeout,
    Cancelled,
    ExecutionError(String),
}
```

---

## 3. Scoring Breakdown Formula

Candidates that successfully clear all hard gates receive a composite score between `0.0` and `100.0` based on the following breakdown:

### 1. Correctness (Weight: 50.0 points)
- Clean compilation in project build system: **30.0 points**
- Successful passage of project test suite: **20.0 points**
- Total: **50.0 points**

### 2. Safety & Blast Radius (Weight: 25.0 points)
Calculated from the AST blast radius analysis:
$$\text{Blast Component} = \text{clamp}(100.0 - \text{blast\_radius\_score}, 0.0, 100.0) \times 0.25$$
- If `SafetyGateVerdict::Warning`, penalized by 10 points: $\max(\text{Blast Component} - 10.0, 0.0)$.
- If clean (`SafetyGateVerdict::Allow`), full `Blast Component` awarded.

### 3. Performance (Weight: 20.0 points)
- Benchmarking measured and outcome improved: **20.0 points**
- Benchmarking not measured (neutral standard baseline): **10.0 points**
- Benchmarking degraded: **0.0 points**

### 4. Maintainability & Locality (Weight: 15.0 points)
Rewards focused, concise solutions over sprawling diffs:
$$\text{Diff Penalty} = \text{clamp}(\text{modified\_files} \times 0.5, 0.0, 10.0)$$
$$\text{Maintainability Score} = \text{clamp}(15.0 - \text{Diff Penalty}, 5.0, 15.0)$$

---

## 4. Evidence Structure Schemas

Every evaluated candidate captures rich empirical evidence for verification:

```rust
pub struct CandidateEvaluation {
    pub branch_id: String,
    pub name: String,
    pub hypothesis_prompt: String,
    pub state: SpeculativeCandidateState,
    pub failure_reason: SpeculativeFailureReason,
    pub hard_gates_passed: bool,
    pub build_passed: bool,
    pub tests_passed: bool,
    pub execution_duration_ms: u64,
    pub test_duration_ms: u64,
    pub safety_verdict: String,
    pub blast_radius_score: f64,
    pub modified_files: Vec<String>,
    pub git_commit_sha: Option<String>,
    pub composite_score: f64,
    pub score_breakdown: ScoreBreakdown,
    pub build_evidence: Option<BuildEvidence>,
    pub test_evidence: Option<TestEvidence>,
    pub security_evidence: Option<SecurityEvidence>,
    pub benchmark_evidence: Option<BenchmarkEvidence>,
    pub patch_content: String,
    pub summary: String,
}
```
