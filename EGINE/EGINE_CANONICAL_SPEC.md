# EGINE Canonical Algorithm Specification

**Single Source of Truth for All Algorithm Definitions**

**Version:** 1.0  
**Date:** June 26, 2026  
**Status:** Canonical Reference

---

## Document Purpose

This document is the **canonical reference** for all EGINE algorithms. Other documents (WHITE_PAPER, ALGORITHM, RESEARCH, BENCHMARK, JOSS) should reference this document for algorithm definitions rather than duplicating pseudocode.

**Convention:** If any other EGINE document conflicts with this specification, this document takes precedence.

---

## 1. Reverse PageRank

### Definition

For code dependency graphs, EGINE computes **reverse PageRank** — importance flows from dependents to their dependencies. A file imported by many important files receives high rank.

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Damping factor | 0.85 | [0.7, 0.95] | Higher = more iteration-dependent |
| Max iterations | 20 | [10, 50] | Converges within 15 typically |
| Convergence threshold | 1e-6 | [1e-4, 1e-8] | L1 norm of PR delta |

### Algorithm

```
ComputePageRank(graph, iterations=20, damping=0.85):
  N ← graph.NodeCount()
  PR ← uniform(1/N)

  FOR i IN range(iterations):
    newPR ← uniform((1-damping)/N)
    
    FOR EACH node IN graph:
      importers ← node.ImportedBy  // REVERSE edges
      IF len(importers) > 0:
        share ← PR[node] / len(importers)
        FOR EACH importer IN importers:
          newPR[importer] += damping × share
    
    // Dangling redistribution
    danglingSum ← sum(PR[node] for node with len(ImportedBy)==0)
    FOR EACH node IN graph:
      newPR[node] += damping × danglingSum / N
    
    IF L1(newPR - PR) < 1e-6: BREAK
    PR ← newPR

  RETURN PR
```

### Complexity

- Time: O(|E| × iterations)
- Space: O(|V|)
- Parallelizable: Yes (per-node updates)

### Implementation Location

`pkg/taskrunner/runner.go`, `internal/tools/codecomplexity.go`, `internal/provider/fallback.go`

---

## 2. Deterministic Louvain Community Detection

### Definition

Discovers natural clusters from graph structure using modularity optimization with fixed seeds for reproducibility.

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Random seed | 42 | any int | Fixed for determinism |
| Max passes | 10 | [5, 20] | Early exit if no improvement |
| Resolution | 1.0 | [0.5, 2.0] | Higher = more communities |

### Algorithm

```
LouvainDetect_Deterministic(graph, seed=42):
  rng ← NewRandom(seed)
  communityOf ← map[node → node.id]  // each node own community

  FOR pass IN range(maxPasses=10):
    improved ← false
    nodes ← shuffle(graph.AllPaths(), rng)

    FOR EACH node IN nodes:
      IF node == ExternalNode: CONTINUE
      bestCommunity ← communityOf[node]
      bestGain ← 0.0

      FOR EACH neighbor IN graph.Neighbors(node):
        IF neighbor == ExternalNode: CONTINUE
        gain ← modularityGain(node, communityOf[neighbor], graph, communityOf)
        IF gain > bestGain:
          bestGain ← gain
          bestCommunity ← communityOf[neighbor]

      IF bestCommunity != communityOf[node]:
        communityOf[node] ← bestCommunity
        improved ← true

    IF !improved: BREAK

  // Canonical renumbering (sorted by path)
  canonicalMap ← {}
  nextID ← 0
  FOR EACH node IN graph (sorted by path):
    IF node == ExternalNode: CONTINUE
    IF communityOf[node] NOT IN canonicalMap:
      canonicalMap[communityOf[node]] ← nextID++
    communityOf[node] ← canonicalMap[communityOf[node]]

  RETURN communityOf

modularityGain(node, targetCommunity, graph, communityOf):
  m ← graph.EdgeCount()
  kin ← edgesBetween(node, targetCommunity, graph)
  Σtot ← communityDegree(targetCommunity, graph)
  ktotal ← degree(node, graph)
  RETURN (2×kin - Σtot×ktotal/m) / (2×m)
```

### Complexity

- Time: O(|E| × log|V|) typical
- Space: O(|V|)
- Deterministic: Yes (fixed seed + canonical renumbering)

### Implementation Location

`internal/tools/codecomplexity.go`, `internal/provider/fallback.go`

---

## 3. Approximate Betweenness Centrality

### Definition

Measures bridge potential via stratified random sampling proportional to community size.

### Parameters

| Parameter | Default | Range | Notes |
|-----------|---------|-------|-------|
| Sample size | |V|/5 | [|V|/10, |V|/3] | Higher = more accurate |

### Algorithm

```
ComputeApproxBetweenness(graph, sampleSize):
  N ← graph.NodeCount()
  betweenness ← map[node → 0.0]
  sources ← stratifiedSample(graph, sampleSize)

  FOR EACH source IN sources:
    // BFS from source
    distances ← map[node → -1]
    predecessors ← map[node → []]
    sigma ← map[node → 0.0]
    distances[source] ← 0
    sigma[source] ← 1.0
    queue ← [source]
    bfsOrder ← [source]

    WHILE !queue.empty():
      v ← queue.dequeue()
      FOR EACH w IN v.Imports:
        IF distances[w] == -1:
          distances[w] ← distances[v] + 1
          queue.enqueue(w)
          bfsOrder.append(w)
        IF distances[w] == distances[v] + 1:
          sigma[w] += sigma[v]
          predecessors[w].append(v)

    // Back-propagation
    delta ← map[node → 0.0]
    FOR i IN REVERSE(range(len(bfsOrder))):
      v ← bfsOrder[i]
      FOR EACH u IN predecessors[v]:
        delta[u] += (sigma[u]/sigma[v]) × (1 + delta[v])
      IF v != source:
        betweenness[v] += delta[v]

  // Normalize
  normalizeFactor ← 1.0 / (sampleSize × (N-1))
  FOR EACH node IN graph:
    betweenness[node] *= normalizeFactor

  RETURN betweenness

stratifiedSample(graph, sampleSize):
  communityNodes ← groupBy(graph.AllPaths(), node → node.Community)
  samples ← []
  FOR EACH community, nodes IN communityNodes:
    proportion ← len(nodes) / graph.NodeCount()
    communitySamples ← max(1, int(sampleSize × proportion))
    samples.extend(randomSample(nodes, communitySamples))
  RETURN samples[:sampleSize]
```

### Complexity

- Time: O(|V| × sampleSize)
- Space: O(|V|)
- Error: Within 10% of exact for sampleSize ≥ |V|/5

### Implementation Location

`pkg/taskrunner/runner.go`, `internal/tools/codecomplexity.go`

---

## 4. Trie (Prefix Tree)

### Definition

Efficient prefix-based symbol search with Unicode support.

### Data Structure

```go
type TrieNode struct {
    children map[rune]*TrieNode  // Unicode support
    symbols  []string
    isEnd    bool
}

type Trie struct {
    root *TrieNode
}
```

### Operations

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Insert | O(K) | O(1) amortized | K = key length |
| Delete | O(K) | O(1) amortized | With pruning |
| Exact match | O(K) | O(1) | Single path |
| Prefix search | O(K + M) | O(M) | M = matches |
| Fuzzy search | O(K × E) | O(M) | E = edit budget |

### Complexity

- Insert: O(K) where K = symbol name length
- Search: O(K + M) where M = number of matching symbols
- Memory: O(N × K_avg) where N = total symbols

### Implementation Location

`internal/codeintel/index.go`, `internal/context/registry.go`

---

## 5. Bloom Filter

### Definition

Probabilistic set membership with optimal sizing and unbiased hash indexing.

### Parameters

| Parameter | Formula | Notes |
|-----------|---------|-------|
| Bit array size m | ceil(-n × ln(p) / (ln 2)²) | n = items, p = FP rate |
| Hash functions k | ceil(m/n × ln 2) | Optimal for given m, n |
| FP rate (1% tier) | 0.01 | Low-traffic filters |
| FP rate (0.1% tier) | 0.001 | High-traffic filters |

### Algorithm

```
BloomFilter_Create(n, p):
  m ← ceil(-n × ln(p) / (ln2 × ln2))
  k ← ceil(m/n × ln2)
  seeds ← generateRandomSeeds(k)
  RETURN BloomFilter{bits: make([]uint64, (m+63)/64), k, m, seeds}

BloomFilter_Add(filter, item):
  FOR i IN range(filter.k):
    h ← hash(item, filter.seeds[i])
    idx ← fastRange(h, filter.m)
    filter.bits[idx/64] |= 1 << (idx%64)

BloomFilter_Contains(filter, item):
  FOR i IN range(filter.k):
    h ← hash(item, filter.seeds[i])
    idx ← fastRange(h, filter.m)
    IF filter.bits[idx/64] & (1<<(idx%64)) == 0:
      RETURN false
  RETURN true

fastRange(hash, size):
  IF size is power of 2:
    RETURN hash & (size-1)
  ELSE:
    RETURN (hash × size) >> 64  // Lemire's method
```

### Complexity

- Insert: O(k) per item
- Contains: O(k) per item, zero allocations
- Memory: ~1.2 bytes/element at 1% FP rate

### Implementation Location

`internal/provider/fallback.go`, `internal/context/registry.go`

---

## 6. Composite Scoring

### Definition

Weighted combination of K scoring components with percentile-based normalization.

### Framework

```
CompositeScore(candidate, context, components):
  totalScore ← 0.0
  reasons ← []

  FOR EACH component IN components:
    rawScore ← component.Compute(candidate, context)
    normalizedScore ← min(rawScore / stats.p95, 1.0)
    weightedScore ← normalizedScore × component.Weight × 10.0
    totalScore += weightedScore
    reasons.append(component.Name + ": " + str(rawScore))

  RETURN {score: totalScore, reasons: reasons}

PercentileNormalize(rawScore, stats):
  IF stats.p95 > 0:
    RETURN min(rawScore / stats.p95, 1.0)
  ELSE:
    RETURN 0.0
```

### Component Configurations

#### Task Complexity Scoring

| Component | Weight | Data Source |
|-----------|--------|-------------|
| Graph Centrality | 25% | PageRank + Betweenness |
| Transitive Dependencies | 20% | BFS from task |
| Community Cohesion | 15% | Louvain communities |
| File Count | 15% | Direct count |
| Keyword Match | 15% | Text analysis |
| Historical Complexity | 10% | History graph |

#### History Frecency Scoring

| Component | Weight | Data Source |
|-----------|--------|-------------|
| Recency | 25% | Timestamp |
| Frequency | 20% | Counter |
| Context Similarity | 20% | Trie prefix match |
| Co-occurrence | 15% | Usage graph PageRank |
| Category Coherence | 10% | Community detection |
| Freshness | 10% | Timestamp |

#### Relevance Scoring

| Component | Weight | Data Source |
|-----------|--------|-------------|
| Direct Relevance | 35% | Target files |
| Graph Centrality | 20% | PageRank + Betweenness |
| Import Proximity | 20% | Import graph |
| Symbol Match | 15% | Trie prefix search |
| Community Coherence | 10% | Louvain communities |
| Bloom Cross-Check | adjustment | {0.95, 1.0} multiplier |

### Complexity

- Time: O(N × K) where N = candidates, K = components
- Space: O(N) for scores

### Implementation Location

Multiple locations across codebase

---

## 7. Performance Characteristics

| Algorithm | 10K nodes | 50K nodes | Notes |
|-----------|-----------|-----------|-------|
| PageRank (20 iter) | 150 ms | 800 ms | Linear in edges |
| Louvain | 300 ms | 1.8 s | Linear in edges |
| Betweenness (|V|/5) | 1.2 s | 8 s | Linear in V × S |
| Trie prefix search | 2.1 µs | 2.5 µs | O(K + M) |
| Bloom Contains | 60 ns | 65 ns | Zero allocations |
| Composite scoring | 180 µs (1K) | — | Linear in N × K |

---

## 8. Memory Overhead

| Innovation | Per-Node | Total (10K nodes) |
|-----------|----------|-------------------|
| PageRank | 8 bytes | 80 KB |
| Betweenness | 8 bytes | 80 KB |
| Community | 4 bytes | 40 KB |
| Degree Centrality | 8 bytes | 80 KB |
| Trie | ~15 bytes/symbol | 150 KB |
| Bloom Filter | ~1.2 bytes/item | 12 KB |
| **Total** | **~43 bytes/node** | **~430 KB** |

---

## 9. Cross-References

- **White Paper** (EGINE_WHITE_PAPER.md): Patent filing with claims referencing these algorithms
- **Algorithm Spec** (EGINE_ALGORITHM.md): Detailed pseudocode and implementation locations
- **Research Paper** (EGINE_RESEARCH.md): Mathematical proofs and complexity analysis
- **Benchmark Report** (EGINE_BENCHMARK.md): Empirical validation of these specifications
- **JOSS Paper** (EGINE_JOSS.md): Publication-ready summary for Journal of Open Source Software

---

*This document is the canonical reference for EGINE algorithms. All other documents should cite this specification rather than duplicating algorithm definitions.*
