# `m31a-fuzzy`

High-performance fuzzy matching, scoring, and candidate search engine for **M31A**.

---

## Overview

`m31a-fuzzy` implements optimized fuzzy string matching and ranking algorithms. It drives the universal command palette (`Ctrl+P`), workspace file scanning (`@file`), model search (`Ctrl+M`), and prompt history filtering with zero external C dependencies.

---

## User-Facing Capabilities

- **Universal Command Palette (`Ctrl+P`)**: Blazing-fast interactive search across commands, tools, open files, and recent prompts.
- **Typo-Tolerant Matching**: Scores matches by contiguous runs, prefix matches, word boundaries, and camelCase/snake_case transitions.

---

## Developer Guide & Architecture

### Key Modules

- [`matcher`](src/matcher.rs): `fuzzy_match(pattern, text) -> Option<i32>` returning match score, and `fuzzy_match_indices()` returning exact matched character spans for UI highlighting.
- [`file_search`](src/file_search.rs): `WorkspaceScanner` providing asynchronous, gitignore-aware directory traversal.
- [`candidates`](src/candidates.rs): Generic `Candidate` abstraction wrapping heterogeneous search items.

---

## Testing

```bash
cargo test -p m31a-fuzzy
```
