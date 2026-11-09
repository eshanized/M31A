# `m31a-review`

> Automated AI and static Git diff code reviewer, security vulnerability scanner, and review report generator for M31A.

---

## Why Does This Crate Exist?

Code review is a critical gate in software development. When an autonomous coding agent makes dozens of changes across multiple crates, a developer needs a quick, trustworthy second opinion before merging.

`m31a-review` provides automated Git diff analysis:
- Inspects uncommitted working tree changes or diffs against a base branch (`m31a review --base main`).
- Detects security vulnerabilities (hardcoded credentials, unsafe blocks, SQL injections, command injection vectors).
- Analyzes code health (error escalation, resource leaks, missing tests, breaking API changes).
- Computes risk levels (`Low`, `Medium`, `High`, `Critical`) and delivers a concrete verdict (`Approve`, `Comment`, `RequestChanges`).

---

## Core Concepts & How It Works

### 1. Dual-Engine Review Pipeline
`m31a-review` pairs deterministic static diff analysis with semantic model intelligence:
- **`StaticDiffAnalyzer`**: Runs fast pattern matching and AST heuristics to catch obvious anti-patterns (e.g., `.unwrap()` in critical paths, exposed API tokens, unclosed file descriptors).
- **LLM Review Lanes (`lanes.rs`)**: Feeds diffs to specialized review lanes (Security, Performance, Correctness, Architecture) using NVIDIA NIM models for deep semantic reasoning.

### 2. Risk Level & Verdict Scoring (`model.rs`)
Based on finding severities:
- `Critical` or `High` findings $\to$ `ReviewVerdict::RequestChanges`
- `Medium` findings $\to$ `ReviewVerdict::Comment`
- Only `Low` or clean diffs $\to$ `ReviewVerdict::Approve`

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/engine.rs`](src/engine.rs) | `ReviewEngine` | Primary review coordinator executing analysis over diff streams. |
| [`src/analyzer.rs`](src/analyzer.rs) | `StaticDiffAnalyzer` | Static rule engine analyzing hunks for vulnerabilities and bugs. |
| [`src/model.rs`](src/model.rs) | `ReviewReport`, `Finding`, `ReviewVerdict`, `RiskLevel` | Data structures representing findings, line ranges, and scores. |
| [`src/formatter.rs`](src/formatter.rs) | `ReviewFormatter` | Pretty-prints review findings to terminal ANSI or Markdown summaries. |
| [`src/lanes.rs`](src/lanes.rs) | `ReviewLaneManager` | Coordinates specialized multi-lane semantic review agents. |

---

## Practical Usage Example

```rust
use m31a_review::engine::ReviewEngine;
use m31a_review::model::ReviewVerdict;

fn main() -> anyhow::Result<()> {
    let diff_text = r#"
diff --git a/src/auth.rs b/src/auth.rs
index abc..def 100644
--- a/src/auth.rs
+++ b/src/auth.rs
@@ -10,3 +10,4 @@
+let api_key = "nvapi-1234567890abcdef";
"#;

    let report = ReviewEngine::analyze(diff_text);
    println!("Verdict: {:?}", report.verdict);
    println!("Risk Level: {:?}", report.risk_level);

    for finding in report.findings {
        println!("[{:?}] {}: {}", finding.severity, finding.rule_id, finding.message);
    }

    assert_eq!(report.verdict, ReviewVerdict::RequestChanges);
    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-review
```
