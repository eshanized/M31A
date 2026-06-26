# EFIE Evaluation Report

**Date:** 26 June 2026
**Machine:** linux/amd64, 8 CPUs, Go 1.26.4
**Methodology:** 5 runs per benchmark (quick mode), synthetic codebases 100-10,000 files

---

## Executive Summary

EFIE implements a novel codebase exploration algorithm combining PageRank, Louvain community detection, betweenness centrality, Trie indexing, and adaptive expansion. This report presents empirical evidence for all quantitative claims in the manuscript.

**Critical Finding:** The current implementation does NOT achieve the claimed performance advantages. EFIE is significantly slower than the baseline for both build and query operations on synthetic codebases. The manuscript requires major revision.

---

## Claim Verification Results

| Claim | Status | Measured | Evidence |
|-------|--------|----------|----------|
| C1: Build completes in ~2.5s for 10K files | **REFUTED** | 5679ms | Wall-clock benchmark, n=5 |
| C2: Incremental rebuilds in ~0.9s | **UNVERIFIED** | N/A | Incremental build not implemented |
| C3: 5-20x faster relevance queries | **REFUTED** | 0.02x (50x slower) | Head-to-head comparison, n=5 |
| C4: 16% additional memory overhead | **REFUTED** | -45% (EFIE uses less) | Memory profiling, n=5 |
| C5: O(N × F / P + E × log V) build | **VERIFIED** | R²=0.9671 | Linear regression, 6 sizes |
| C6: O(S × B) query time | **VERIFIED** | R²=0.9990 | Linear regression, 6 sizes |
| C7: Betweenness within 10% | **UNVERIFIED** | N/A | No exact baseline implemented |
| C8: PageRank converges in 20 iterations | **UNVERIFIED** | N/A | Convergence not measured |
| C9: Bloom filter 1.2 bytes/element | **UNVERIFIED** | N/A | Memory not profiled per-element |

---

## Detailed Results

### Build Time (ms)

| Files | EFIE | Original | Speedup | EFIE Heap (MB) | Orig Heap (MB) |
|-------|------|----------|---------|----------------|----------------|
| 100 | 11.0±1.9 | 3.3±0.8 | 0.3x | 2.0±0.1 | 1.6±0.4 |
| 500 | 56.1±3.9 | 12.6±0.5 | 0.2x | 3.0±0.1 | 3.1±0.6 |
| 1,000 | 150.4±3.7 | 32.4±4.3 | 0.2x | 4.4±0.1 | 6.8±1.0 |
| 2,000 | 386.1±20.4 | 56.2±6.5 | 0.1x | 7.7±0.6 | 11.1±0.7 |
| 5,000 | 1550.4±60.4 | 143.2±3.7 | 0.1x | 17.4±0.7 | 27.5±1.9 |
| 10,000 | 5679.2±201.3 | 293.6±28.2 | 0.1x | 30.0±2.7 | 54.7±2.9 |

### Query Time (ms)

| Files | EFIE | Original | Speedup |
|-------|------|----------|---------|
| 100 | 0.2±0.1 | 0.0±0.0 | 0.0x |
| 500 | 1.0±0.1 | 0.0±0.0 | 0.0x |
| 1,000 | 1.9±0.2 | 0.0±0.0 | 0.0x |
| 2,000 | 3.7±0.2 | 0.1±0.0 | 0.0x |
| 5,000 | 11.0±1.8 | 0.2±0.1 | 0.0x |
| 10,000 | 17.8±0.4 | 0.3±0.1 | 0.0x |

### Empirical Complexity

| Operation | Slope (ms/file) | Intercept | R² | Interpretation |
|-----------|----------------|-----------|-----|----------------|
| EFIE build | 0.5091 | -378.2 | 0.9671 | Linear O(N) |
| Orig build | 0.0284 | 1.2 | 0.9938 | Linear O(N) |
| EFIE query | 0.001906 | -0.2 | 0.9990 | Linear O(N) |
| Orig query | 0.000017 | 0.0 | 0.8581 | Near-constant |

### Community Detection

All synthetic codebases detected **0 communities**. This indicates:
1. The synthetic codebases lack sufficient import structure
2. The Louvain algorithm requires denser graphs to find communities
3. The algorithm's community detection component may not be activated on sparse graphs

---

## Analysis

### Why EFIE is Slower

1. **Overhead of precomputation**: PageRank (20 iterations), Louvain, and betweenness centrality are computed at build time, adding ~5 seconds for 10K files
2. **Adaptive expansion overhead**: The max-heap BFS with community seed expansion is more expensive than simple BFS for sparse graphs
3. **Trie construction**: Building the Trie for symbol search adds overhead not present in the original linear scan
4. **No communities detected**: Without community structure, the community boost component provides no benefit

### Why EFIE Uses Less Memory

The original backend stores the full import graph in memory with string-based adjacency lists. EFIE uses more compact data structures (Bloom filters, Trie) that are more memory-efficient for large codebases.

### When Would EFIE Be Faster?

EFIE's advantages would manifest on:
1. **Dense codebases** with strong community structure (many internal imports)
2. **Complex queries** requiring relevance scoring across many files
3. **Large codebases** where the precomputed importance scores avoid rescoring all files
4. **Repeated queries** where the build-time investment pays off

---

## Recommendations

1. **Remove unverified claims** from the manuscript
2. **Mark remaining claims as "Expected" or "Hypothesized"** based on theoretical analysis
3. **Add benchmark results** showing actual measured performance
4. **Test on real codebases** with natural community structure
5. **Profile the implementation** to identify optimization opportunities
6. **Consider alternative evaluation metrics** (e.g., relevance quality, not just speed)
