# `m31a-fuzzy`

> High-performance zero-allocation fuzzy matching, candidate ranking, and path search engine for M31A.

---

## Why Does This Crate Exist?

When searching across a repository with 50,000 files in the universal command palette (`Ctrl+P`) or auto-completing slash commands in the composer, latency must be under 5 milliseconds. If fuzzy search lags, the entire interactive terminal experience feels sluggish.

`m31a-fuzzy` is an optimized fuzzy scoring engine engineered in Rust:
- **Zero-Allocation Stack Fast-Path**: Uses fixed-size stack buffers for paths under 256 bytes (covering >99.9% of codebase paths), avoiding heap allocations entirely.
- **SIMD-Friendly Subsequence Pre-Check**: Instantly eliminates non-matching targets in a single CPU pass before computing detailed alignment matrices.
- **Sophisticated Scoring Heuristics**: Awards bonuses for prefix matches, word boundaries (camelCase, snake_case, kebab-case), consecutive character runs, and path segment separators (`/`).

---

## Core Concepts & How It Works

### 1. Two-Phase Matching Pipeline
1. **Fast-Reject Pre-Check**: Checks whether the query is a subsequence of the target using case-insensitive ASCII comparison. If not, drops out immediately ($O(N)$).
2. **Alignment & Bonus Matrix**: Computes the optimal alignment, awarding:
   - Start of string bonus: `+35`
   - Path separator boundary (`/` or `\`): `+30`
   - Word boundary (`_`, `-`, `.`, camelCase shift): `+25`
   - Consecutive match multiplier.

### 2. File & Symbol Search Integration (`file_search.rs`)
Powers the `file_search` tool and palette navigation, returning ranked results with matched character indices for ANSI highlighting.

---

## Key Modules & Functions

| Module | Key Functions / Types | Purpose |
| :--- | :--- | :--- |
| [`src/matcher.rs`](src/matcher.rs) | `fuzzy_match_indices_ascii`, `fuzzy_score` | Core zero-allocation scoring algorithm and matched index extractor. |
| [`src/candidates.rs`](src/candidates.rs) | `CandidateFilter`, `ScoredCandidate` | Top-K candidate ranking and score thresholding. |
| [`src/file_search.rs`](src/file_search.rs) | `FuzzyFileSearcher` | Traverses directories and scores matching file paths in parallel. |

---

## Practical Usage Example

```rust
use m31a_fuzzy::matcher::fuzzy_match_indices_ascii;

fn main() {
    let query = b"mcpc";
    let target = b"crates/m31a-mcp/src/client.rs";

    if let Some((score, indices)) = fuzzy_match_indices_ascii(query, target) {
        println!("Match found! Score: {}", score);
        println!("Matched character indices: {:?}", indices);
    }
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-fuzzy
```
