# `m31a-review`

Automated code review engine, static vulnerability analysis, and multi-model consensus lanes for **M31A**.

---

## Overview

`m31a-review` performs automated, in-depth code reviews on Git working tree diffs or branch comparisons (`m31a review --base main`). It aggregates findings across multiple model perspectives (correctness, performance, security, architecture), filters out noise, and presents structured review reports.

---

## User-Facing Capabilities

- **Automated Diff Review (`m31a review`)**: Reviews uncommitted changes before committing or opening a Pull Request.
- **Severity-Ranked Findings**: Categorizes issues into `Critical`, `High`, `Medium`, and `Low` with line numbers and suggested fixes.
- **Multi-Lane Consensus**: Dispatches parallel reviews across specialized models and consolidates findings into high-confidence alerts.

---

## Developer Guide & Architecture

### Key Modules

- [`engine`](src/engine.rs): `ReviewEngine` orchestrating git diff extraction, chunking, prompt dispatch, and synthesis.
- [`lanes`](src/lanes.rs): `MultiLaneReviewDispatcher` and `PrimaryConsolidator` evaluating consensus across multiple model evaluations.
- [`analyzer`](src/analyzer.rs): `StaticDiffAnalyzer` checking basic static invariants prior to LLM submission.
- [`model`](src/model.rs): `ReviewFinding`, `ReviewReport`, `ReviewSeverity`, and `ReviewVerdict`.

---

## Testing

```bash
cargo test -p m31a-review
```
