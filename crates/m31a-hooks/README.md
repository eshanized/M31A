# `m31a-hooks`

> 12-point canonical phase loop lifecycle hooks, deterministic event intercepts, and custom extension execution for M31A.

---

## Why Does This Crate Exist?

In enterprise engineering workflows, developers need deterministic seams to enforce organizational standards:
- Running custom linters or security scanners before an agent begins executing code.
- Auto-formatting code with `prettier` or `rustfmt` immediately after a wave of edits concludes.
- Generating changelog entries or notifying a Slack channel before and after shipping a phase.
- Validating architecture specifications before granting execution permissions.

`m31a-hooks` establishes a **12-Point Canonical Phase Loop Lifecycle Hook** system. Teams can attach shell scripts, executables, or validation pipelines to explicit phase points, guaranteeing that custom policies run deterministically at every phase boundary.

---

## Core Concepts & How It Works

### The 12 Canonical Lifecycle Extension Seams
Hooks execute at 12 precise phase points during an autonomous workflow:

1. `discuss:pre`: Prior to user requirements elicitation.
2. `discuss:post`: After context requirements are finalized.
3. `plan:pre`: Before architectural plan drafting initiates.
4. `plan:post`: After `PLAN.md` passes pre-execution validation.
5. `execute:pre`: Before code mutations begin.
6. `execute:wave:pre`: Before a wave of parallel tasks launches.
7. `execute:wave:post`: After a wave finishes (e.g. running pre-commit formatting).
8. `execute:post`: After all execution waves conclude.
9. `verify:pre`: Before adversarial test verification begins.
10. `verify:post`: After verification succeeds and artifacts are generated.
11. `ship:pre`: Prior to Git branch merging or PR creation.
12. `ship:post`: After the milestone has shipped.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/lifecycle.rs`](src/lifecycle.rs) | `CanonicalPhasePoint`, `LifecycleHookRunner` | Defines the 12 phase points and coordinates execution. |
| [`src/config.rs`](src/config.rs) | `HooksConfig`, `HookEntry` | Loads hook definitions from `.m31a/hooks.toml` or user config. |
| [`src/runtime.rs`](src/runtime.rs) | `HookExecutionResult`, `HookContext` | Captures exit codes, stdout/stderr, and enforces execution timeouts. |

---

## Practical Usage Example

```rust
use m31a_hooks::lifecycle::{CanonicalPhasePoint, LifecycleHookRunner};
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let workspace = Path::new(".");
    let runner = LifecycleHookRunner::load(workspace)?;

    // Dispatch execute:wave:post hook (e.g., auto-formatting)
    let result = runner.dispatch(CanonicalPhasePoint::ExecuteWavePost, workspace).await?;

    if result.all_passed() {
        println!("All post-wave hooks passed successfully!");
    } else {
        eprintln!("Hook failed with exit code: {:?}", result.failure_details());
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-hooks
```
