# `m31a-diff`

> In-memory turn diff tracking, syntactic diff computation, side-by-side terminal rendering, and code churn analytics for M31A.

---

## Why Does This Crate Exist?

When an agent edits 15 files across a single turn, checking what changed by repeatedly spawning `git diff` subprocesses is slow, disk-bound, and fails entirely if the workspace is not a Git repository.

`m31a-diff` provides an **In-Memory Turn Diff Tracker** (inspired by OpenAI Codex's `TurnDiffTracker`):
- Captures file mutations (added, modified, deleted, renamed) directly in memory during patch staging without querying the filesystem or spawning Git.
- Computes character-level, word-level, and line-level diffs using the `similar` crate.
- Renders unified and side-by-side colorized diffs with line numbers and syntax styling for the TUI (`Ctrl+D`).
- Calculates code churn metrics (additions, deletions, net line delta) in real time.

---

## Core Concepts & How It Works

### 1. In-Memory Turn Tracking (`turn_tracker.rs`)
`TurnDiffTracker` tracks file mutations across an agent turn:
- Stores `original_content` and `current_content` in memory.
- Can generate unified diffs on demand without touching disk.
- Records revision numbers and timestamps for each mutation.

### 2. High-Performance Diff Engine (`engine.rs`)
Powered by Myers' diff algorithm via `similar`:
- Generates `DiffStats` (files changed, insertions, deletions).
- Marks unchanged context lines, additions (`+`), and removals (`-`).

### 3. Terminal Diff Viewers (`render.rs`, `view.rs`)
Formats diffs for terminal display:
- Line numbers for old and new files.
- Red/green background highlighting.
- Word-level intra-line change emphasis.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/turn_tracker.rs`](src/turn_tracker.rs) | `TurnDiffTracker`, `TrackedFileEntry`, `FileMutationKind` | In-memory tracker recording deltas without spawning Git. |
| [`src/engine.rs`](src/engine.rs) | `DiffEngine`, `DiffStats`, `DiffLine` | Myers diff algorithm wrapper and hunk generator. |
| [`src/render.rs`](src/render.rs) | `DiffRenderer` | ANSI colorized terminal formatter for unified and split diffs. |
| [`src/churn.rs`](src/churn.rs) | `ChurnMetrics` | Aggregated line churn and file modification ratios. |

---

## Practical Usage Example

```rust
use m31a_diff::turn_tracker::TurnDiffTracker;

fn main() -> anyhow::Result<()> {
    let mut tracker = TurnDiffTracker::new();

    // 1. Record an in-memory modification during an agent turn
    tracker.record_modify(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "pub fn add(a: i32, b: i32) -> i32 {\n    // Added overflow check\n    a.saturating_add(b)\n}",
    );

    // 2. Generate unified diff text instantly without Git
    let unified_diff = tracker.generate_unified_diff()?;
    println!("Generated Diff:\n{}", unified_diff);

    // 3. Inspect churn statistics
    let stats = tracker.stats();
    println!("Files modified: {}, Lines added: {}, Lines deleted: {}", 
        stats.files_modified, stats.lines_added, stats.lines_deleted);

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-diff
```
