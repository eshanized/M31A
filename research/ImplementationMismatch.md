# Implementation Mismatch Report

**Date:** 26 June 2026
**Scope:** Divergences between EFIE specification documents and actual implementation

---

## Executive Summary

The EFIE implementation contains **3 critical algorithmic bugs**, **3 structural divergences**, **2 missing features**, and **8 dead code instances**. Most critically, PageRank computes importance in the **wrong direction** (measures "who depends on me" instead of "who I depend on"), which fundamentally invalidates the algorithm's claim to identify architecturally important files.

---

## 1. Critical Algorithmic Divergences

### 1.1 PageRank Direction is Inverted

**Specification** (EFIE_RESEARCH.md:945):
> $PR(v_i) = \frac{1-d}{N} + d \sum_{v_j \in \text{Imports}(v_i)} \frac{PR(v_j)}{|\text{Imports}(v_j)|}$

The spec defines PageRank on the **forward** import graph: importance flows from a file to the files it imports.

**Implementation** (centrality.go:27-35):
```go
for _, importedBy := range node.ImportedBy {
    inDegree := len(g.Nodes[importedBy].ImportedBy)
    if inDegree > 0 {
        rank += g.PageRank[importedBy] / float64(inDegree)
    }
}
```

The code distributes importance to files that **import** the current file (via `ImportedBy`). This is the **transpose** of what the spec describes.

**Impact:** Files with many importers (leaf files that are widely used) get high PageRank, while files that import many important files (architectural hubs) get low PageRank. This is the **opposite** of what the algorithm claims to measure.

### 1.2 Dangling Node Detection is Inverted

**Specification** (EFIE_RESEARCH.md:943):
> Dangling nodes (with no outgoing edges) distribute their rank equally to all nodes.

**Implementation** (centrality.go:38-42):
```go
if len(node.ImportedBy) == 0 {
    danglingSum += g.PageRank[nodeID]
}
```

The code checks `ImportedBy` instead of `Imports`. A file with no importers is treated as dangling, when it should be a file with no imports.

**Impact:** Combined with bug 1.1, this creates a double inversion that makes PageRank computation mathematically incorrect.

### 1.3 Degree Centrality Includes ExternalNode

**Specification** (EFIE_RESEARCH.md:748):
> Degree centrality measures the number of import relationships a file participates in.

**Implementation** (efie.go:75):
```go
efie.FileCentrality[fileID] = float64(len(node.Imports)+len(node.ImportedBy)) / float64(len(efie.Graph.Nodes))
```

The denominator includes `ExternalNode` ("__external__"), which is a synthetic sentinel. This skews all centrality values downward.

**Impact:** All centrality values are systematically biased. Files connected to `ExternalNode` appear less central than they actually are.

---

## 2. Structural Divergences

### 2.1 Scoring Function Component Ranges

| Component | Spec Range | Actual Range | Issue |
|-----------|-----------|-------------|-------|
| Graph Centrality | [0, 10] | [0, 10] | OK |
| Direct Relevance | 0 or 35 | 0 or 35 | OK |
| Import Proximity | [0, 20] | [0, +∞) | **UNBOUNDED** — can exceed 20 |
| Symbol Match | [0, 15] | [0, 15] | OK |
| Community Coherence | {0, 5, 10} | {0, 5, 10} | OK |

**Issue:** Import proximity score is capped by the `score += 10.0` line per target file, but there is no upper bound check. A file importing 10 targets gets 100 points, completely dominating the 35-point direct relevance score.

**Spec says** (EFIE_ALGORITHM_V2.md:1281):
> The maximum proximity score is capped at 20.0 (matching the 20% weight)

**Implementation** (scorer.go:49-67): No cap is applied.

### 2.2 Community Detection Hash Function

**Specification** (EFIE_ALGORITHM_V2.md:507):
> Initial assignment: $c_i \leftarrow h(v_i) \mod K$ where $h$ is a hash function

**Implementation** (community.go:19):
```go
communities[nodeID] = int(hashString(nodeID) % uint64(numCommunities))
```

The `hashString` function (community.go:347-358) uses FNV-1a with XOR folding:
```go
h = h.wrapping_mul(0x01000193).wrapping_add(b as u64)
```

**Issue:** XOR folding reduces hash quality, causing predictable collisions. For small community counts (K=5-10), certain file names will always map to the same community, biasing Louvain initialization.

### 2.3 Bloom Filter Hash Function

**Specification** (EFIE_ALGORITHM_V2.md:1328):
> Uses `murmur3` hash function for Bloom filter

**Implementation** (bloom.go:33-42):
```go
func bloomHash(data []byte, seed uint32) uint64 {
    h := uint64(seed)
    for _, b := range data {
        h = h.wrapping_mul(0x01000193).wrapping_add(uint64(b))
    }
    return h
}
```

This is FNV-1a, not murmur3. While functionally similar, the spec explicitly requires murmur3.

---

## 3. Missing Features

### 3.1 Incremental Build (mtime-based deltas)

**Specification** (EFIE_ALGORITHM_V2.md:1075):
> Phase 1 (Discovery): File walking with mtime-based delta detection...

**Implementation:** No mtime tracking. `Build()` always rebuilds from scratch.

**Impact:** 63% faster rebuild claim (EFIE_RESEARCH.md:1057) is entirely theoretical.

### 3.2 Convergence Monitoring for PageRank

**Specification** (EFIE_ALGORITHM_V2.md:943):
> Iterate until $\|PR^{t+1} - PR^t\|_1 < \epsilon$ or max 20 iterations

**Implementation** (centrality.go:15-43): Fixed 20 iterations, no convergence check.

**Impact:** The 4% convergence guarantee (EFIE_RESEARCH.md:962) is unverified.

---

## 4. Code Duplication

### 4.1 Parsers Duplicated

`internal/codeintel/efie/parsers.go` (452 lines) is an **exact copy** of `internal/codeintel/parser.go` (562 lines, subset).

### 4.2 Import Resolution Duplicated

`internal/codeintel/efie/resolve.go` (243 lines) duplicates logic from `internal/codeintel/graph.go:283-467`.

### 4.3 Identifier Extraction Duplicated

`extractIdentifiers` in `scorer.go:139-184` duplicates `relevance.go:127-179`.

### 4.4 Helper Functions Duplicated

`splitCamelCase`, `isDigit`, `isIdentChar`, `stopWords`, `isStopWord` all duplicated between `scorer.go` and `relevance.go`.

**Total duplicated code:** ~780 lines.

---

## 5. Dead Code

| File:Line | Dead Code | Reason |
|-----------|-----------|--------|
| `wrapper.go:6` | `var _ = strings.TrimSpace` | Unused import hack |
| `index.go:11-14` | `CommunityAdj` | Never used |
| `index.go:315-351` | `SortedImportanceRank` | Never called |
| `graph.go:232-239` | `AllNodes` | Never called |
| `trie.go:89-102` | `FuzzySearch` | Never called |
| `resolve.go:145` | `fileSet` parameter | Accepted but never used |
| `bloom.go:41-64` | Custom `ln()` | Reimplements `math.Log` |
| `efie.go:39` | `MaxFileID` | Only incremented, never read |

---

## 6. Test Coverage Gaps

### 6.1 Untested Critical Paths

| Function | File:Line | Why Critical |
|----------|-----------|-------------|
| `Query()` | efie.go:337 | Main query entry point |
| `adaptiveExpansionQuery()` | query.go:14 | Core query algorithm |
| `bfsQuery()` | query.go:101 | Fallback query strategy |
| `EFIEIndex.Build()` | efie.go:73 | Integration test path |
| `RelevantFiles()` | efie.go:475 | Used by wrapper |
| `EFIEIndexer.*` | wrapper.go | All wrapper methods |
| `resolveImport()` | resolve.go:147 | All 4 resolvers |
| `FuzzySearch()` | trie.go:89 | Dead code |

### 6.2 Broken Benchmarks

| Benchmark | File:Line | Issue |
|-----------|-----------|-------|
| `BenchmarkPageRank` | benchmark_test.go:303 | Passes nil graph |
| `BenchmarkLouvain` | benchmark_test.go:345 | Passes nil graph |

---

## 7. Summary

| Category | Count | Severity |
|----------|-------|----------|
| Critical algorithmic bugs | 3 | HIGH |
| Structural divergences | 3 | MEDIUM |
| Missing features | 2 | MEDIUM |
| Code duplication instances | 4 | LOW |
| Dead code instances | 8 | LOW |
| Untested critical paths | 8 | HIGH |
| Broken benchmarks | 2 | MEDIUM |
