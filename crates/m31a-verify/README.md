# `m31a-verify`

Spec-driven verification, autonomous visual TDD, compiler checking, and regression detection for **M31A**.

---

## Overview

`m31a-verify` acts as the quality and verification engine of M31A. It validates workspace changes across compilers, linters, and test suites, executes headless Chromium Visual TDD checks (`verify_visual_state`), audits semantic blast-radius safety, and formulates targeted self-healing guidance when failures occur.

---

## User-Facing Capabilities

- **Autonomous Verification (`m31a verify` or `/verify`)**: Runs compiler checks (`cargo check`, `tsc`, `pytest`) and tests without requiring manual developer oversight.
- **Autonomous Visual TDD (`/visual <url>`)**: Inspects live web frontends using headless Chromium CDP to verify DOM elements, layout styling, and zero console/network errors.
- **Self-Healing Guidance**: Generates precise diagnostic context and error snippets when builds or tests fail, enabling the agent loop to self-correct automatically.

---

## Developer Guide & Architecture

### Key Modules

- [`visual_tdd`](src/visual_tdd.rs): Visual TDD assertions over live web frontends via `m31a-browser`.
- [`runners`](src/runners.rs): Test suite and compiler runner orchestration across Rust, TypeScript, Python, and Go ecosystems.
- [`plan_checker`](src/plan_checker.rs): 8-dimension pre-execution plan analysis detecting risks before code generation starts.
- [`safety`](src/safety.rs): Semantic blast-radius calculation and call-graph impact scoring.
- [`self_heal`](src/self_heal.rs): Synthesizes targeted self-healing prompts from compiler error diagnostics.

---

## Testing

```bash
cargo test -p m31a-verify
```
