# `m31a-verify`

> Autonomous verification engine, 8-dimension plan checking, adversarial goal-backward verification, Visual TDD, and automated self-healing loops for M31A.

---

## Why Does This Crate Exist?

Writing code is only 20% of engineering; 80% is verification, testing, and debugging. If an autonomous agent writes code without rigorous verification, it will produce silent regressions, broken contracts, and brittle software.

`m31a-verify` turns M31A from a naive code generator into an uncompromising autonomous verification engineer:
- **8-Dimension Pre-Execution Plan Checking**: Analyzes architectural plans *before* touching files, identifying circular dependencies, missing error handlers, and security blindspots.
- **Adversarial Goal-Backward Verification**: Instead of asking "did my code run?", it works backward from the user's objective to find missing artifacts, failing edge-case assertions, and incomplete goals.
- **Visual TDD Engine (`visual_tdd.rs`)**: Automates headless Chromium CDP sessions to assert live DOM presence, computed CSS styles, and zero console/network errors.
- **Targeted Self-Healing (`self_heal.rs`)**: When builds or tests fail, it parses compiler error diagnostics and formulates targeted corrective patches for the agent loop.

---

## Core Concepts & How It Works

### 1. 8-Dimension Plan Checker (`plan_checker.rs`)
Evaluates implementation plans across:
1. *Completeness*: Are all files and requirements addressed?
2. *Feasibility*: Does the current toolchain support the chosen approach?
3. *Safety & Blast Radius*: Does the change break upstream public APIs?
4. *Testability*: Are verifiable assertion mechanisms included?
5. *Security Invariants*: Are credentials and permissions protected?
6. *Performance Impact*: Are known algorithmic bottlenecks introduced?
7. *Rollback Strategy*: Can changes be cleanly reverted?
8. *Dependency Order*: Are DAG tasks scheduled without deadlocks?

### 2. Autonomous Visual TDD (`visual_tdd.rs`)
Communicates with [`m31a-browser`](../m31a-browser) to test web frontends:
- Evaluates DOM element presence and visibility.
- Checks exact text and attribute matches.
- Enforces zero unhandled browser console exceptions and zero HTTP 4xx/5xx network errors.
- Captures screenshot artifacts and formats actionable regression feedback.

### 3. Automated Test Suite Runners & Diagnostic Parsing (`diagnostics.rs`, `runners.rs`)
Dispatches test suites across Rust (`cargo test`), Node.js (`npm test`, `vitest`, `jest`), and Python (`pytest`), parsing compiler errors and stack traces into structured `Diagnostic` items.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/plan_checker.rs`](src/plan_checker.rs) | `PlanChecker`, `PlanCheckReport`, `IssueSeverity` | 8-dimension pre-execution architectural plan validation. |
| [`src/goal_backward.rs`](src/goal_backward.rs) | `GoalBackwardVerifier`, `GoalBackwardReport` | Adversarial objective verification working backwards from goals. |
| [`src/visual_tdd.rs`](src/visual_tdd.rs) | `VisualTddEngine`, `VisualAssertion`, `VisualReport` | Headless Chromium UI validation, style checks, and screenshots. |
| [`src/self_heal.rs`](src/self_heal.rs) | `SelfHealer`, `HealingPatch` | Synthesizes targeted corrective instructions from compiler diagnostics. |
| [`src/safety.rs`](src/safety.rs) | `BlastRadiusGate`, `SafetyReport` | Evaluates AST symbol blast radius and data-flow taint invariants. |
| [`src/diagnostics.rs`](src/diagnostics.rs) | `DiagnosticParser`, `Diagnostic`, `DiagnosticSeverity` | Extracts compiler, linter, and runtime error traces. |

---

## Practical Usage Example

```rust
use m31a_verify::diagnostics::DiagnosticParser;
use m31a_verify::plan_checker::{PlanChecker, PlanModel, TaskModel};

fn main() -> anyhow::Result<()> {
    // 1. Analyze an architectural plan before execution
    let plan = PlanModel {
        id: "plan_01".into(),
        objective: "Migrate auth tokens to Ed25519 signatures".into(),
        tasks: vec![
            TaskModel {
                id: "task_1".into(),
                description: "Update Cargo.toml with ed25519-dalek".into(),
                dependencies: vec![],
            },
            TaskModel {
                id: "task_2".into(),
                description: "Implement token signing in src/token.rs".into(),
                dependencies: vec!["task_1".into()],
            },
        ],
    };

    let report = PlanChecker::evaluate(&plan);
    println!("Plan Feasibility Score: {}/100", report.score);
    println!("Issues found: {}", report.issues.len());

    // 2. Parse raw compiler diagnostics
    let rustc_error = "error[E0425]: cannot find value `token` in this scope\n --> src/main.rs:14:5";
    let diagnostics = DiagnosticParser::parse_rustc(rustc_error);
    for diag in diagnostics {
        println!("[{:?}] {}: {}", diag.severity, diag.file, diag.message);
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-verify
```
