# `m31a-patch`

High-performance multi-file patch engine, Codex patch specification parser, and 4-tier fuzzy hunk matcher for **M31A**.

---

## Overview

`m31a-patch` parses, verifies, and applies multi-file code patches adhering to the OpenAI Codex patch specification (`*** Begin Patch`, `*** Add File:`, `*** Delete File:`, `*** Update File:`, `*** Move to:`, `@@ context`, `*** End Patch`). It features two-phase atomic file transactions and a resilient 4-tier fuzzy match engine.

---

## User-Facing Capabilities

- **Direct Patch Application (`m31a apply-patch <file>`)**: Applies structured multi-file patches directly to the current workspace.
- **Atomic Two-Phase Commits**: Patches are staged and verified before applying; if any hunk fails to apply, all changes are rolled back automatically with zero partial writes.
- **Fuzzy Context Tolerance**: Automatically tolerates whitespace variations, tab/space mismatches, and unicode quotes so patches don't fail unnecessarily.

---

## Developer Guide & Architecture

### 4-Tier Fuzzy Seek Sequence (`seek_sequence`)

When locating contextual hunk anchors, `FuzzyMatcher` tests targets in order:
1. **Tier 1 (Exact Match)**: Byte-for-byte equality.
2. **Tier 2 (Right-Strip Whitespace)**: Trailing spaces and carriage returns ignored.
3. **Tier 3 (Full Trim)**: Leading and trailing indentation ignored.
4. **Tier 4 (Unicode Normalization)**: Converts curly quotes, typographic dashes, and non-breaking spaces into ASCII equivalents.

### Key Modules

- [`v4a`](src/v4a.rs): Parses structured multi-file patch grammar.
- [`transaction`](src/transaction.rs): `PatchTransaction` managing atomic staging, rollback buffers, and `TransactionStats`.
- [`fuzzy_matcher`](src/fuzzy_matcher.rs): `FuzzyMatcher` and `MatchLocation`.
- [`unified_diff`](src/unified_diff.rs): `UnifiedDiffParser` parsing standard unidiffs (`diff -u`).

---

## Testing

```bash
cargo test -p m31a-patch
```
