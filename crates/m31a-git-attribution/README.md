# `m31a-git-attribution`

> Standardized Git commit trailer generation, AI co-authorship attribution (`Co-authored-by`), and code provenance tracking for M31A.

---

## Why Does This Crate Exist?

When AI agents create Git commits, transparency and regulatory compliance require clear attribution:
- Which human developer directed the agent?
- Which AI model synthesized the patch (`M31A-Model: nvidia/nemotron-3.5-lightning-30b-a3b`)?
- What was the unique session thread ID (`M31A-Session: thread_01...`) to enable audit replays?
- Did the commit adhere to GitHub's standard `Co-authored-by` convention?

`m31a-git-attribution` automates Git commit metadata injection, appending standard trailers to commit messages cleanly and maintaining cryptographic provenance.

---

## Core Concepts & How It Works

### 1. Standardized Git Commit Trailers (`CommitTrailerBuilder`)
Appends RFC-compliant trailers to commit messages without corrupting the commit subject or body:
```text
Refactor auth tokens to use Ed25519 signatures

Implemented cryptographic key verification and added regression unit tests.

Co-authored-by: M31A <m31a@local>
M31A-Session: 0194e43b-821b-7a2e-8e6f-123456789abc
M31A-Model: nvidia/nemotron-3.5-lightning-30b-a3b
```

### 2. Git2 Repository Integration
Interacts directly with the local `.git` repository via `git2` to read branch tips, inspect HEAD commits, and compute blame attribution.

---

## Key Modules & Types

| Struct / Enum | Description |
| :--- | :--- |
| `CommitTrailerBuilder` | Builder configuring agent author name, email, session ID, and model slug. |
| `GitAttributionTracker` | Analyzes commit blame and tracks lines authored by AI vs human contributors. |

---

## Practical Usage Example

```rust
use m31a_git_attribution::CommitTrailerBuilder;

fn main() {
    let message = "Implement zero-copy JSON parsing with simd-json";

    let formatted_commit = CommitTrailerBuilder::new()
        .with_session_id("session_0194a")
        .with_model_id("nvidia/nemotron-3.5-lightning-30b-a3b")
        .format_commit_message(message);

    println!("Formatted Commit Message:\n{}", formatted_commit);
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-git-attribution
```
