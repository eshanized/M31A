# Implementation Audit Report

**Date:** 26 June 2026
**Auditor:** Autonomous Research Engineer
**Scope:** Complete EFIE implementation, documentation, and evaluation framework

---

## Executive Summary

The EFIE implementation is **functionally complete** (15 source files, ~3,336 lines of Go) but contains **critical correctness bugs** and **significant discrepancies** between the implementation and the algorithm specification. The evaluation framework exists but is **insufficient for publication** — it uses only synthetic data, lacks relevance quality metrics, and several benchmarks are broken.

**Verdict: NOT PUBLICATION-READY**

---

## 1. Implementation Inventory

### 1.1 EFIE Package (`internal/codeintel/efie/`)

| File | Lines | Status | Issues |
|------|-------|--------|--------|
| `efie.go` | 604 | Complete | Double parse bug, dead code, unused variables, no mutex on Build() |
| `wrapper.go` | 152 | Complete | Unused import hack (`var _ = strings.TrimSpace`) |
| `query.go` | 346 | Complete | BFS includes ExternalNode, over-broad community expansion |
| `scorer.go` | 232 | Complete | Unbounded import proximity, prefix vs exact symbol match |
| `index.go` | 210 | Complete | Dead code (`SortedImportanceRank`, `CommunityAdj`), UTF-8 bug |
| `bloom.go` | 108 | Complete | Custom `ln()` reimplements `math.Log`, weak hash |
| `trie.go` | 162 | Complete | ASCII-only (128 char array), `FuzzySearch` dead code |
| `graph.go` | 195 | Complete | O(N) reverse edge dedup (original uses O(1) set), `AllNodes` dead code |
| `resolve.go` | 243 | Complete | `fileSet` parameter unused, duplicate types, global cache leak |
| `centrality.go` | 186 | Complete | **PageRank distributes to ImportedBy (non-standard)**, **Dangling node detection inverted** |
| `community.go` | 157 | Complete | Modularity gain formula uses `2m` instead of standard `m`, hash collision bias |
| `parsers.go` | 452 | Complete | Entire file is duplicate of `codeintel/parser.go` |
| `rng.go` | 24 | Complete | Clean, no issues |

### 1.2 Original Backend (`internal/codeintel/`)

| File | Lines | Status |
|------|-------|--------|
| `codeintel.go` | 379 | Complete, feature flag `USE_EFIE=true` |
| `parser.go` | 562 | Complete |
| `graph.go` | 467 | Complete |
| `index.go` | 160 | Complete |
| `relevance.go` | 238 | Complete |

### 1.3 Evaluation Framework (`paper/evaluate/`)

| File | Lines | Status | Issues |
|------|-------|--------|--------|
| `generate.go` | 209 | Complete | Synthetic repos lack community structure |
| `benchmark_test.go` | 471 | Broken | `BenchmarkPageRank`/`BenchmarkLouvain` pass nil graphs |
| `evaluate.go` | 504 | Partial | No real repos, no precision/recall, no profiling, no plots |
| `helpers.go` | 82 | Complete | Duplicates `benchQueryMS` |
| `types.go` | 17 | Complete | Minimal |

---

## 2. Critical Bugs

### 2.1 Algorithmic Bugs (HIGH severity)

| # | File:Line | Bug | Impact |
|---|-----------|-----|--------|
| B1 | `centrality.go:27-35` | PageRank distributes to `ImportedBy` instead of `Imports` — non-standard direction | Fundamental: "importance" measures "who depends on me" not "who I depend on" |
| B2 | `centrality.go:38-42` | Dangling node detection inverted: checks `len(ImportedBy)==0` instead of `len(Imports)==0` | PageRank computation is mathematically incorrect |
| B3 | `efie.go:75` | Degree centrality denominator includes ExternalNode | All centrality values skewed |
| B4 | `community.go:19` | Initial community via `hashString` causes collisions, biasing Louvain | Community detection quality degraded |
| B5 | `scorer.go:49-67` | Import proximity score is unbounded — can exceed 20 points, dominating 35-point direct relevance | Scoring function weights meaningless |
| B6 | `query.go:69-79` | BFS includes `ExternalNode` ("__external__") in results | Meaningless results in traversal |
| B7 | `efie.go:279-306` | `buildGraph` re-reads and re-parses all files from disk | 2x I/O waste during build |

### 2.2 Implementation Bugs (MEDIUM severity)

| # | File:Line | Bug | Impact |
|---|-----------|-----|--------|
| B8 | `efie.go:32` | No mutex on `EFIEIndex.Build()` — race on `built` flag | Data race in concurrent use |
| B9 | `bloom.go:41-64` | Custom `ln()` reimplements `math.Log` with potential precision issues | Unnecessary, slower, less accurate |
| B10 | `graph.go:67-71` | Reverse edge deduplication is O(N) scan (original uses O(1) set) | O(N²) for dense graphs |
| B11 | `resolve.go:11` | `fileSet` parameter accepted but never used in any resolver | Dead parameter |
| B12 | `resolve.go:155-156` | Global `goModulePathCache` never evicted | Memory leak in long processes |

### 2.3 Test Coverage Gaps (HIGH severity)

**Completely untested:**
- `Query()` function (the main query entry point)
- `adaptiveExpansionQuery()` (the core algorithm)
- `bfsQuery()`
- `EFIEIndex.Build()` (integration path)
- `EFIEIndex.RelevantFiles()`
- `EFIEIndexer` wrapper (all methods)
- All import resolution functions
- `FuzzySearch()`

**Broken tests:**
- `BenchmarkPageRank` passes nil graph — measures nothing
- `BenchmarkLouvain` passes nil graph — measures nothing

---

## 3. Code Duplication

The entire parser, import resolution, and identifier extraction logic is duplicated between `efie/` and `codeintel/`:

| Component | EFIE | Original | Lines duplicated |
|-----------|------|----------|-----------------|
| Go/TS/Py/Rs parsers | `parsers.go` | `parser.go` | ~450 |
| Import resolution | `resolve.go` | `graph.go:283-467` | ~240 |
| `extractIdentifiers` | `scorer.go:139-184` | `relevance.go:127-179` | ~45 |
| `splitCamelCase` | `scorer.go:186-217` | `relevance.go:183-217` | ~30 |
| `stopWords`/`isStopWord` | `scorer.go:219-232` | `relevance.go:219-232` | ~14 |
| **Total** | | | **~780 lines** |

---

## 4. Documentation vs Implementation

| Spec Feature | Specified | Implemented | Tested |
|-------------|-----------|-------------|--------|
| 5-phase build pipeline | Yes | Yes | No (integration) |
| Deterministic Louvain (seed=42) | Yes | Yes | Yes |
| PageRank (20 iter, d=0.85) | Yes | Yes (wrong direction) | Partial |
| Approx betweenness (stratified) | Yes | Yes | Yes |
| Trie symbol index | Yes | Yes | Yes |
| Bloom filter | Yes | Yes | Yes |
| Adaptive expansion query | Yes | Yes | No |
| 6-component scoring | Yes | Yes (unbounded proximity) | Partial |
| Incremental build (mtime delta) | Yes | **NOT IMPLEMENTED** | N/A |
| Disk cache persistence | Yes | **NOT IMPLEMENTED** | N/A |
| Feature flag `USE_EFIE` | Yes | Yes | Yes |

---

## 5. Summary

| Category | Count |
|----------|-------|
| Source files | 15 |
| Total lines (EFIE) | ~3,336 |
| Total lines (duplicated) | ~780 |
| Critical bugs (algorithmic) | 7 |
| Medium bugs (implementation) | 5 |
| Dead code instances | 6 |
| Untested critical paths | 8 |
| Broken benchmarks | 2 |
| Missing features | 2 |
