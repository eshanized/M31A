# `m31a-patch`

> High-performance multi-file patch engine, OpenAI Codex patch specification parser, two-phase atomic file transactions, and 4-tier fuzzy hunk matcher for M31A.

---

## Why Does This Crate Exist?

Standard unified diff tools (`patch`, `git apply`) are brittle when used by AI agents. A single mismatched space, an indented line, or a carriage return shift will cause standard patch tools to fail with `patch does not apply`. When that happens, naive agents fall into frustrating loops attempting to re-read and re-generate the entire file from scratch, wasting thousands of tokens.

`m31a-patch` was purpose-built to solve this problem:
- Full implementation of the **OpenAI Codex multi-file patch specification** (`*** Begin Patch`, `*** Add File:`, `*** Delete File:`, `*** Update File:`, `*** Move to:`, `@@ context`, `*** End Patch`).
- Features a **4-tier fuzzy match engine** (`seek_sequence`) that tolerates indentation shifts, whitespace variations, and Unicode typographic quirks.
- Enforces **two-phase atomic file transactions**: all hunks across all files are validated and staged in memory first; if even one hunk fails to resolve, the entire transaction is aborted, leaving the working tree 100% pristine.

---

## Core Concepts & How It Works

### 1. 4-Tier Fuzzy Seek Sequence (`fuzzy_matcher.rs`)
When searching for context lines to anchor an update hunk, `FuzzyMatcher` tests matching tiers in increasing order of tolerance:
1. **Tier 1 (Exact Match)**: Byte-for-byte character equality.
2. **Tier 2 (Right-Strip Whitespace)**: Ignores trailing spaces, tabs, and CRLF vs LF mismatches.
3. **Tier 3 (Full Trim)**: Trims leading and trailing indentation shifts, matching pure line semantics.
4. **Tier 4 (Unicode Typographic Normalization)**: Converts smart quotes (`"`, `"`), em/en dashes (`—`, `–`), and non-breaking spaces into ASCII equivalents.

### 2. Two-Phase Atomic Commits (`transaction.rs`)
Mutating files on disk during patch parsing is dangerous. If hunk #5 fails after hunks #1-4 have already overwritten files, the codebase is left corrupted.  
`PatchTransaction` stages all file changes in shadow buffers. Only when all hunks across all files pass verification are changes committed atomically to disk.

### 3. Streaming Grammar Parser (`v4a.rs`, `streaming_parser.rs`)
Parses patches as they stream over the wire, allowing the TUI and diff viewers to render syntax-colored additions and deletions before generation finishes.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/v4a.rs`](src/v4a.rs) | `PatchParser`, `PatchAction`, `FilePatch` | Parses OpenAI Codex multi-file patch grammar. |
| [`src/transaction.rs`](src/transaction.rs) | `PatchTransaction`, `TransactionStats` | Coordinates atomic staging, rollback buffers, and disk commits. |
| [`src/fuzzy_matcher.rs`](src/fuzzy_matcher.rs) | `FuzzyMatcher`, `MatchLocation`, `MatchTier` | 4-tier resilient hunk anchoring engine. |
| [`src/unified_diff.rs`](src/unified_diff.rs) | `UnifiedDiffParser` | Parses traditional unidiffs (`diff -u`) for backward compatibility. |

---

## Practical Usage Example

```rust
use m31a_patch::transaction::PatchTransaction;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let patch_text = r#"*** Begin Patch
*** Update File: src/main.rs
@@ fn main() {
-    println!("Hello World");
+    println!("Hello from M31A!");
 }
*** End Patch"#;

    let workspace_root = Path::new(".");
    let mut tx = PatchTransaction::new(workspace_root);

    // 1. Stage and verify patch hunks
    tx.stage_patch(patch_text)?;

    // 2. Inspect transaction statistics before applying
    let stats = tx.stats();
    println!("Files to modify: {}", stats.files_modified);
    println!("Total lines added: {}", stats.lines_added);
    println!("Total lines removed: {}", stats.lines_deleted);

    // 3. Atomically commit changes to disk
    tx.commit()?;
    println!("Patch committed successfully!");

    Ok(())
}
```

---

## CLI Integration

Apply patches directly from the command line:

```bash
# Apply a structured patch file to the current workspace
m31a apply-patch patch.txt

# Inspect and apply pending agent diffs
m31a apply
```

---

## Testing & Verification

```bash
cargo test -p m31a-patch
```
