# EGINE: Eshanized Graph Intelligence Network Engine

## Detailed Algorithm Specifications

**A Technical Implementation Reference**

**Author:** Eshan Roy | Eshanized  
**Status:** Design Specification v1.0  
**Version:** 1.0  
**Date:** June 26, 2026

---

## Table of Contents

1. [Data Structures](#1-data-structures)
2. [Sub-Algorithm: PageRank](#2-sub-algorithm-pagerank)
3. [Sub-Algorithm: Deterministic Louvain](#3-sub-algorithm-deterministic-louvain)
4. [Sub-Algorithm: Approximate Betweenness Centrality](#4-sub-algorithm-approximate-betweenness-centrality)
5. [Sub-Algorithm: Trie Symbol Index](#5-sub-algorithm-trie-symbol-index)
6. [Sub-Algorithm: Bloom Filter](#6-sub-algorithm-bloom-filter)
7. [Sub-Algorithm: Composite Scoring](#7-sub-algorithm-composite-scoring)
8. [Innovation 1: Importance-Weighted Task Scheduling](#8-innovation-1-importance-weighted-task-scheduling)
9. [Innovation 2: Graph-Based Code Complexity](#9-innovation-2-graph-based-code-complexity)
10. [Innovation 3: Provider Reliability Ranking](#10-innovation-3-provider-reliability-ranking)
11. [Innovation 4: Trie-Based Symbol Search](#11-innovation-4-trie-based-symbol-search)
12. [Innovation 5: Bloom Filter Pre-Filters](#12-innovation-5-bloom-filter-pre-filters)
13. [Innovation 6: Composite Scoring Functions](#13-innovation-6-composite-scoring-functions)
14. [Innovation 7: Adaptive Expansion for Code Navigation](#14-innovation-7-adaptive-expansion-for-code-navigation)
15. [Innovation 8: Multi-Resolution Indexing](#15-innovation-8-multi-resolution-indexing)
16. [Innovation 9: Importance-Weighted Context Allocation](#16-innovation-9-importance-weighted-context-allocation)
17. [Innovation 10: Usage-Graph Importance Weighting](#17-innovation-10-usage-graph-importance-weighting)
18. [Innovation 11: Task Graph Visualization](#18-innovation-11-task-graph-visualization)
19. [Innovation 12: Context Registry Optimization](#19-innovation-12-context-registry-optimization)
20. [Incremental Update Algorithms](#20-incremental-update-algorithms)
21. [Concurrency and Thread-Safety Model](#21-concurrency-and-thread-safety-model)
22. [Error Handling and Edge Cases](#22-error-handling-and-edge-cases)
23. [Changelog](#23-changelog)

---

## 1. Data Structures

### 1.1 WeightedNode (Shared Across All Innovations)

Extends basic node structures with precomputed importance metrics.

```go
type WeightedNode struct {
    ID               string
    Path             string
    Imports          []string    // outgoing edges (dependencies)
    ImportedBy       []string    // incoming edges (dependents)
    Language         string
    Mtime            time.Time

    // Precomputed importance metrics
    PageRank         float64
    Betweenness      float64
    Community        int
    DegreeCentrality float64

    // Semantic fingerprint
    SymbolBloom      *BloomFilter
    ImportSet        map[string]bool
}

type WeightedGraph struct {
    nodes        map[string]*WeightedNode
    nCommunities int
    maxCentrality float64
}
```

### 1.2 MultiResIndex

Three-level index enabling queries at different granularities.

```go
type MultiResIndex struct {
    // Level 0: File → FileInfo
    files map[string]*FileInfo

    // Level 1: Package → []file paths
    packages map[string][]string

    // Level 2: Community → []file paths
    communities map[int][]string

    // Symbol index
    symbolTrie  *Trie
    byName      map[string][]SymbolLocation

    // Reverse lookups
    fileToCommunity map[string]int
    communityAdj    map[int]map[int]bool

    // Precomputed rankings
    importanceRank []string
    centralityP50  float64
    centralityP95  float64
}
```

### 1.3 Trie

Efficient prefix-based symbol search with Unicode support.

```go
type TrieNode struct {
    children map[rune]*TrieNode  // Unicode support (UTF-8)
    symbols  []string
    isEnd    bool
}

type Trie struct {
    root *TrieNode
}

// Operations:
// Insert:   O(K) where K = symbol name length
// Delete:   O(K) where K = symbol name length
// Search:   O(K) exact match
// Prefix:   O(K + M) where M = number of matches
// Fuzzy:    O(K x E) where E = edit distance budget
```

### 1.4 BloomFilter

Probabilistic set membership with unbiased hash indexing.

```go
type BloomFilter struct {
    bits    []uint64
    numHash int
    size    uint
    seeds   []uint64  // Per-hash-function seeds
}

// Insert:    O(k) where k = number of hash functions (typically 7)
// Contains:  O(k) with zero allocations
// False positive rate: ~1% with optimal sizing
// Memory: ~1.2 bytes per element at 1% FP rate
// Uses Lemire's fast range reduction for unbiased bit indexing
```

### 1.5 CompositeScorer

Weighted multi-component scoring.

```go
type ScoringComponent struct {
    Name       string
    Weight     float64
    Compute    func(candidate, context) float64
    Normalize  func(rawScore, stats) float64
}

type CompositeScorer struct {
    components []ScoringComponent
    stats      *ScoreStatistics  // percentiles for normalization
}

type ScoreStatistics struct {
    p50 float64
    p95 float64
    max float64
}
```

---

## 2. Sub-Algorithm: PageRank

Computes stationary distribution probability for each node. For code dependency graphs, uses **reverse-edge distribution** — importance flows from dependents to their dependencies (a file imported by many important files receives high rank).

```
ALGORITHM ComputePageRank(graph, iterations=20, damping=0.85):

  N ← graph.NodeCount()
  PR ← map[node → 1.0/N]  // uniform initialization

  FOR i IN range(iterations):
    newPR ← map[node → (1 - damping) / N]

    // Distribute PR along REVERSE edges (ImportedBy)
    // File A imports File B => edge A -> B
    // B's importance flows to A (the importer)
    FOR EACH node IN graph:
      importers ← node.ImportedBy
      IF len(importers) > 0:
        share ← PR[node] / len(importers)
        FOR EACH importer IN importers:
          newPR[importer] += damping × share

    // Dangling node redistribution (nodes with no incoming edges)
    danglingSum ← 0.0
    FOR EACH node IN graph:
      IF len(node.ImportedBy) == 0:
        danglingSum += PR[node]

    FOR EACH node IN graph:
      newPR[node] += damping × danglingSum / N

    // Convergence check
    diff ← sum(|newPR[n] - PR[n]| for all n)
    IF diff < 1e-6:
      BREAK

    PR ← newPR

  RETURN PR
```

### Properties

- **Time complexity:** O(|E| x iterations)
- **Space complexity:** O(|V|)
- **Convergence:** Usually within 10-15 iterations for codebase graphs
- **Damping factor:** 0.85 (standard for web graphs, works for codebases)

### Interpretation

| PageRank Range | Interpretation | Example |
|---|---|---|
| > 0.01 | Architectural hub | `types.go`, `config.go` |
| 0.001-0.01 | Core module | `handler.go`, `store.go` |
| 0.0001-0.001 | Regular file | Most source files |
| < 0.0001 | Leaf file | `main.go`, test files |

---

## 3. Sub-Algorithm: Deterministic Louvain

Discovers natural clusters from graph structure.

```
ALGORITHM LouvainDetect_Deterministic(graph, seed=42):

  rng ← NewRandom(seed)

  communityOf ← map[node → community_id]
  FOR EACH node IN graph:
    communityOf[node] = node.id

  FOR pass IN range(maxPasses=10):
    improved ← false
    nodes ← shuffle(graph.AllPaths(), rng)

    FOR EACH node IN nodes:
      IF node == ExternalNode:
        CONTINUE

      bestCommunity ← communityOf[node]
      bestGain ← 0.0

      FOR EACH neighbor IN graph.Neighbors(node):
        IF neighbor == ExternalNode:
          CONTINUE
        gain ← modularityGain(node, communityOf[neighbor], graph, communityOf)
        IF gain > bestGain:
          bestGain ← gain
          bestCommunity ← communityOf[neighbor]

      IF bestCommunity != communityOf[node]:
        communityOf[node] ← bestCommunity
        improved ← true

    IF !improved:
      BREAK

  // Canonical renumbering
  canonicalMap ← map[int → int]
  nextID ← 0
  FOR EACH node IN graph (sorted by path):
    IF node == ExternalNode:
      CONTINUE
    c := communityOf[node]
    IF _, ok := canonicalMap[c]; !ok:
      canonicalMap[c] = nextID
      nextID++
    communityOf[node] = canonicalMap[c]

  RETURN communityOf

FUNCTION modularityGain(node, targetCommunity, graph, communityOf):
  m ← graph.EdgeCount()
  kin ← edgesBetween(node, targetCommunity, graph)
  Σtot ← communityDegree(targetCommunity, graph)
  ktotal ← degree(node, graph)

  gain ← (2×kin - Σtot×ktotal/m) / (2×m)
  RETURN gain
```

### Properties

- **Time complexity:** O(|E| x log|V|)
- **Space complexity:** O(|V|)
- **Quality:** Modularity Q in [-0.5, 1.0]; typical values 0.3-0.7
- **Determinism:** Fixed seed + canonical renumbering = same output every run

---

## 4. Sub-Algorithm: Approximate Betweenness Centrality

Measures bridge potential via stratified random sampling.

```
ALGORITHM ComputeApproxBetweenness(graph, sampleSize):

  N ← graph.NodeCount()
  betweenness ← map[node → 0.0]

  sources ← stratifiedSample(graph, sampleSize)

  FOR EACH source IN sources:
    distances ← map[node → -1]
    predecessors ← map[node → []string]
    sigma ← map[node → 0.0]

    distances[source] ← 0
    sigma[source] ← 1.0
    queue ← [source]

    WHILE !queue.empty():
      v ← queue.dequeue()
      FOR EACH w IN v.Imports:
        IF distances[w] == -1:
          distances[w] ← distances[v] + 1
          queue.enqueue(w)
        IF distances[w] == distances[v] + 1:
          sigma[w] += sigma[v]
          predecessors[w].append(v)

    delta ← map[node → 0.0]
    FOR EACH v IN REVERSE(BFS order):
      FOR EACH u IN predecessors[v]:
        delta[u] += (sigma[u] / sigma[v]) × (1 + delta[v])
      IF v != source:
        betweenness[v] += delta[v]

  normalizeFactor := 1.0 / float64(sampleSize × (N-1))
  FOR EACH node IN graph:
    betweenness[node] *= normalizeFactor

  RETURN betweenness

FUNCTION stratifiedSample(graph, sampleSize):
  communityNodes ← groupBy(graph.AllPaths(), node → node.Community)

  samples ← []
  FOR EACH community, nodes IN communityNodes:
    proportion ← len(nodes) / graph.NodeCount()
    communitySamples ← max(1, int(sampleSize × proportion))
    samples.extend(randomSample(nodes, communitySamples, rng))

  RETURN samples[:sampleSize]
```

### Properties

- **Time complexity:** O(|V| x sampleSize)
- **Approximation error:** Within 10% of exact for sampleSize >= |V|/5
- **Stratified sampling:** Ensures proportional community representation

---

## 5. Sub-Algorithm: Trie Symbol Index

```
ALGORITHM BuildTrie(symbolNames):

  root ← new TrieNode()

  FOR EACH name IN symbolNames:
    current ← root
    FOR EACH char IN name:
      idx ← charToIndex(char)
      IF current.children[idx] == nil:
        current.children[idx] ← new TrieNode()
      current ← current.children[idx]
    current.isEnd ← true
    current.symbols.append(name)

  RETURN Trie{root}

ALGORITHM TriePrefixSearch(trie, query):

  current ← trie.root
  FOR EACH char IN query:
    idx ← charToIndex(char)
    IF current.children[idx] == nil:
      RETURN []
    current ← current.children[idx]

  result ← []
  collectSymbols(current, result)
  RETURN result

ALGORITHM TrieFuzzySearch(trie, query, maxEditDistance):
  result ← []
  FOR EACH prefix IN allPrefixes(query):
    FOR EACH edit IN range(maxEditDistance + 1):
      matches ← trie.PrefixSearch(applyEdit(query, edit))
      result.extend(matches)
  RETURN deduplicate(result)

FUNCTION collectSymbols(node, result):
  IF node.isEnd:
    result.extend(node.symbols)
  FOR EACH child IN node.children:
    IF child != nil:
      collectSymbols(child, result)
```

### Operations

| Operation | Time | Notes |
|-----------|------|-------|
| Insert | O(K) | K = symbol name length |
| Exact match | O(K) | Single path traversal |
| Prefix search | O(K + M) | M = number of matches |
| Fuzzy search | O(K x E) | E = edit distance budget |
| Memory | O(N x K avg) | N = total symbols |

---

## 6. Sub-Algorithm: Bloom Filter

```
ALGORITHM BloomFilter_Create(expectedItems, falsePositiveRate):

  n ← expectedItems
  p ← falsePositiveRate
  m ← ceil(-n × ln(p) / (ln2 × ln2))
  k ← ceil(m/n × ln2)
  seeds ← generateRandomSeeds(k)

  RETURN BloomFilter{bits: make([]uint64, (m+63)/64), numHash: k, size: m, seeds: seeds}

ALGORITHM BloomFilter_Add(filter, item):

  FOR i IN range(filter.numHash):
    h ← hash(item, filter.seeds[i])
    idx ← fastRange(h, filter.size)  // Lemire's method
    filter.bits[idx/64] |= 1 << (idx % 64)

ALGORITHM BloomFilter_Contains(filter, item):

  FOR i IN range(filter.numHash):
    h ← hash(item, filter.seeds[i])
    idx ← fastRange(h, filter.size)
    IF filter.bits[idx/64] & (1 << (idx % 64)) == 0:
      RETURN false
  RETURN true

FUNCTION fastRange(hash, size):
  // Unbiased range reduction (Lemire's method)
  IF size is power of 2:
    RETURN hash & (size-1)
  ELSE:
    RETURN (hash × size) >> 64

FUNCTION hash(item, seed):
  // FNV-1a inspired with xxhash mixing
  h ← seed
  FOR EACH byte IN item:
    h ← h XOR byte
    h ← h × 0x100000001b3  // FNV prime
  h ← h XOR (h >> 33)
  h ← h × 0xff51afd7ed558ccd
  h ← h XOR (h >> 33)
  h ← h × 0xc4ceb9fe1a85ec53
  h ← h XOR (h >> 33)
  RETURN h
```

### False Positive Mitigation

1. **Critical path verification:** Top-3 candidates verified with exact map lookup
2. **Score penalty:** Bloom-only matches receive 0.95× multiplier
3. **Adaptive sizing:** 0.1% FP rate for high-traffic filters

---

## 7. Sub-Algorithm: Composite Scoring

```
ALGORITHM CompositeScore(candidate, context, scorer):

  totalScore ← 0.0
  reasons ← []

  FOR EACH component IN scorer.components:
    rawScore ← component.Compute(candidate, context)
    normalizedScore ← component.Normalize(rawScore, scorer.stats)
    weightedScore ← normalizedScore × component.Weight × 10.0
    totalScore += weightedScore
    reasons.append(component.Name + ": " + str(rawScore))

  RETURN ScoredItem{score: totalScore, reasons: reasons}

FUNCTION PercentileNormalize(rawScore, stats):
  IF stats.p95 > 0:
    normalized ← min(rawScore / stats.p95, 1.0)
  ELSE:
    normalized ← 0.0
  RETURN normalized
```

### Score Component Summary

| Component | Weight | Range | Data Source |
|-----------|--------|-------|-------------|
| Graph Centrality | 20% | [0, 10] | PageRank + Betweenness |
| Direct Relevance | 35% | 0 or 35 | Target files |
| Import Proximity | 20% | [0, 20] | Import graph |
| Symbol Match | 15% | [0, 15] | Trie prefix search |
| Community Coherence | 10% | {0, 5, 10} | Louvain communities |
| Bloom Cross-Check | adjustment | {0.95, 1.0} | Bloom filter + verification |

---

## 8. Innovation 1: Importance-Weighted Task Scheduling

**Location:** `pkg/taskrunner/runner.go:73-146`

### Current System

Kahn's topological sort. All tasks in a wave treated equally.

### Proposed System

PageRank on task dependency graph for importance weighting within waves.

```
ALGORITHM ImportanceWeightedSchedule(tasks, dependencies):

  // Build dependency graph
  graph ← DirectedGraph()
  FOR EACH task IN tasks:
    graph.AddNode(task.id)
    FOR EACH dep IN task.dependencies:
      graph.AddEdge(dep, task.id)

  // Compute PageRank
  pageRank ← ComputePageRank(graph, iterations=20, damping=0.85)

  // Topological sort with PageRank ordering
  inDegree ← map[id → count]
  FOR EACH task IN tasks:
    inDegree[task.id] ← len(task.dependencies)

  waves ← []
  ready ← maxHeap(orderBy=pageRank)
  FOR EACH task IN tasks:
    IF inDegree[task.id] == 0:
      ready.push(task.id, pageRank[task.id])

  WHILE !ready.empty():
    wave ← []
    nextReady ← []
    WHILE !ready.empty():
      current ← ready.popMax()
      wave.append(current)
      FOR EACH successor IN graph.Successors(current):
        inDegree[successor]--
        IF inDegree[successor] == 0:
          nextReady.push(successor, pageRank[successor])
    waves.append(wave)
    ready ← nextReady

  RETURN waves
```

### Critical-Path Analysis

```
ALGORITHM CriticalPathAnalysis(tasks, dependencies):
  graph ← buildDependencyGraph(tasks, dependencies)
  betweenness ← ComputeApproxBetweenness(graph, sampleSize=|V|/5)

  FOR EACH task IN tasks:
    cb ← betweenness[task.id]
    IF cb > 0.1:
      task.riskLevel ← "critical"
    ELSE IF cb > 0.01:
      task.riskLevel ← "high"
    ELSE IF cb > 0.001:
      task.riskLevel ← "medium"
    ELSE:
      task.riskLevel ← "low"

  RETURN tasks
```

### Time Budget

| Operation | Time (1K tasks) | Notes |
|-----------|-----------------|-------|
| Graph build | ~10ms | O(V + E) |
| PageRank | ~20ms | O(E × 20) |
| Betweenness | ~50ms | O(V × S) |
| Topo sort | ~5ms | O(V + E) |
| **Total** | **~85ms** | Acceptable |

---

## 9. Innovation 2: Graph-Based Code Complexity

**Location:** `internal/tools/codecomplexity.go:148-290`

### Current System

Line count per file/package. Simple heuristic scoring.

### Proposed System

Import graph coupling metrics + PageRank hotspot detection.

```
ALGORITHM GraphBasedComplexity(workDir):
  graph ← BuildImportGraph(workDir)

  pageRank ← ComputePageRank(graph, iterations=20, damping=0.85)
  communities ← LouvainDetect_Deterministic(graph, seed=42)

  packages ← groupBy(graph.AllPaths(), path → dirOf(path))
  results ← []

  FOR EACH pkg, files IN packages:
    Ca ← 0  // afferent coupling
    Ce ← 0  // efferent coupling
    FOR EACH file IN files:
      FOR EACH importer IN graph[file].ImportedBy:
        IF dirOf(importer) != pkg:
          Ca++
      FOR EACH import IN graph[file].Imports:
        IF dirOf(import) != pkg:
          Ce++

    I ← Ce / (Ca + Ce)  // instability

    pkgPageRank ← sum(pageRank[file] for file in files)
    maxFilePR ← max(pageRank[file] for file in files)

    results.append({
      package: pkg,
      afferentCoupling: Ca,
      efferentCoupling: Ce,
      instability: I,
      totalPageRank: pkgPageRank,
      maxFilePageRank: maxFilePR,
      community: communities[files[0]],
      fileCount: len(files),
      totalLines: sum(lines(f) for f in files)
    })

  RETURN results
```

### God File Detection

```
ALGORITHM DetectGodFiles(graph, pageRank, betweenness):
  FOR EACH file IN graph.AllPaths():
    hotspot_score ← 0.5 × pageRank[file]
                  + 0.3 × (lines(file) / P95(lines))
                  + 0.2 × betweenness[file]

  RETURN sortBy(hotspot_score, descending)
```

---

## 10. Innovation 3: Provider Reliability Ranking

**Location:** `internal/provider/fallback.go:22-111`

### Current System

Priority-ordered sequential scan.

### Proposed System

PageRank on reliability history graph + Bloom filter for rate-limit tracking.

```
ALGORITHM ProviderReliabilityRanking(history):
  graph ← DirectedGraph()
  FOR EACH provider IN allProviders:
    graph.AddNode(provider.id)

  FOR EACH event IN history:
    IF event.type == "fallback":
      graph.AddEdge(event.failedProvider, event.successProvider,
                    weight=event.successCount)

  reliabilityPR ← ComputePageRank(graph, iterations=20, damping=0.85)

  ranked ← sortBy(reliabilityPR, descending)
  RETURN ranked
```

### Rate-Limit Bloom Filter

```go
type RateLimitTracker struct {
    bloom  *BloomFilter
    exact  map[string]bool
    mu     sync.RWMutex
}

func (t *RateLimitTracker) IsRateLimited(providerID string) bool {
    if !t.bloom.Contains(providerID) {
        return false
    }
    t.mu.RLock()
    defer t.mu.RUnlock()
    return t.exact[providerID]
}

func (t *RateLimitTracker) MarkRateLimited(providerID string) {
    t.mu.Lock()
    t.exact[providerID] = true
    t.mu.Unlock()
    t.bloom.Add(providerID)
}
```

### Provider Clustering

```
ALGORITHM ClusterProviders(providers, metrics):
  graph ← UndirectedGraph()
  FOR EACH p1, p2 IN providers:
    similarity ← computeSimilarity(p1, p2, metrics)
    IF similarity > threshold:
      graph.AddEdge(p1, p2, weight=similarity)

  communities ← LouvainDetect_Deterministic(graph, seed=42)

  clusterMap ← map[providerID → communityID]
  FOR EACH provider IN providers:
    clusterMap[provider.id] = communities[provider.id]

  RETURN clusterMap
```

---

## 11. Innovation 4: Trie-Based Symbol Search

**Location:** `internal/codeintel/index.go:140-151`

### Current System

Linear scan over all symbols: O(N × K) per query.

### Proposed System

Trie index for O(K) prefix search.

```
ALGORITHM BuildSymbolTrie(allSymbols):
  trie ← new Trie()
  FOR EACH symbol IN allSymbols:
    trie.Insert(symbol.name, metadata={file: symbol.file, line: symbol.line})
  RETURN trie

ALGORITHM SearchSymbols(trie, query, mode="prefix"):
  SWITCH mode:
    CASE "prefix":
      RETURN trie.PrefixSearch(query)  // O(K + M)
    CASE "exact":
      RETURN trie.ExactSearch(query)   // O(K)
    CASE "fuzzy":
      RETURN trie.FuzzySearch(query, maxEditDistance=1)  // O(K × E)
```

### Application Locations

| File | Line | Current | With Trie |
|------|------|---------|-----------|
| `internal/codeintel/index.go` | 140 | O(N × K) scan | O(K + M) Trie |
| `internal/tools/codemap.go` | 114 | O(L) line scan | O(K + M) Trie |
| `pkg/history/history.go` | 90 | O(N × L) match | O(K + M) Trie |
| `internal/workflow/engine_verify.go` | 94 | O(L) line scan | O(K + M) Trie |
| `internal/context/registry.go` | 116 | O(N) linear | O(K) Trie |

---

## 12. Innovation 5: Bloom Filter Pre-Filters

### Application Pattern

```
IF bloomFilter.Contains(item):
    IF exactCheck(item):
        // Definitely in set
    ELSE:
        // False positive — apply 0.95× penalty
ELSE:
    // Definitely not in set — skip
```

### Application Locations

| Location | Current Check | With Bloom |
|----------|--------------|------------|
| Provider rate-limit | O(M) mutex map | O(1) Bloom |
| Task dependency | O(D) per task | O(1) Bloom |
| Session dedup | O(S) linear | O(1) Bloom |
| Context source | O(N) linear | O(1) Bloom |
| File extension | Set lookup | O(1) Bloom |

### Sizing Guide

| Use Case | Expected Items | FP Rate | Memory |
|----------|---------------|---------|--------|
| Rate-limit tracking | 100 providers | 1% | ~120 bytes |
| Task dependencies | 1000 tasks | 1% | ~1.2 KB |
| Session dedup | 100 sessions | 1% | ~120 bytes |
| Context sources | 50 sources | 1% | ~60 bytes |

---

## 13. Innovation 6: Composite Scoring Functions

### Framework

```
ALGORITHM CompositeScore(candidate, context, components):
  totalScore ← 0.0
  reasons ← []

  FOR EACH component IN components:
    rawScore ← component.Compute(candidate, context)
    normalizedScore ← min(rawScore / stats.p95, 1.0)
    weightedScore ← normalizedScore × component.Weight × 10.0
    totalScore += weightedScore
    reasons.append(component.Name + ": " + str(rawScore))

  RETURN {score: totalScore, reasons: reasons}
```

### Task Complexity Scorer

| Component | Weight | Measures | Data Source |
|-----------|--------|----------|-------------|
| Graph Centrality | 25% | Architectural importance | PageRank + Betweenness |
| Transitive Dependencies | 20% | Reachable dependency count | BFS from task |
| Community Cohesion | 15% | Same-community dependency ratio | Louvain |
| File Count | 15% | Number of files affected | Direct count |
| Keyword Match | 15% | Action/description keywords | Text analysis |
| Historical Complexity | 10% | Similar past task outcomes | History graph |

### History Frecency Scorer

| Component | Weight | Measures | Data Source |
|-----------|--------|----------|-------------|
| Recency | 25% | Time since last use | Timestamp |
| Frequency | 20% | Total use count | Counter |
| Context Similarity | 20% | Similarity to current context | Trie prefix match |
| Co-occurrence | 15% | Used with recently used entries | Usage graph PageRank |
| Category Coherence | 10% | Same category as recent entries | Community detection |
| Freshness | 10% | Days since first use | Timestamp |

---

## 14. Innovation 7: Adaptive Expansion for Code Navigation

**Location:** `internal/tools/codemap.go:114-128`

### Current System

Depth-1 downstream traversal only. Misses indirect references.

### Proposed System

Importance-weighted max-heap BFS with bounded expansion.

```
ALGORITHM AdaptiveReferencesQuery(graph, symbol, topN):
  locs ← DefineQuery(symbol)
  IF len(locs) == 0:
    RETURN []

  seeds ← set()
  FOR EACH loc IN locs:
    seeds.add(loc.File)
    FOR EACH neighbor IN graph.Neighbors(loc.File):
      seeds.add(neighbor)

  candidates ← maxHeap(maxSize=topN)
  visited ← set(seeds)

  FOR EACH seed IN seeds:
    score ← ReferenceScore(seed, locs, graph)
    candidates.push(seed, score)

  expansionBudget ← topN × 3
  explored ← 0
  medianPageRank ← median(graph.PageRankValues())
  expansionThreshold ← medianPageRank × 0.5

  WHILE explored < expansionBudget:
    current ← candidates.popMax()
    IF current == nil: BREAK

    FOR EACH neighbor IN graph.Neighbors(current.path):
      IF neighbor IN visited: CONTINUE
      visited.add(neighbor)

      importance ← graph.nodes[neighbor].PageRank
      IF importance > expansionThreshold:
        score ← ReferenceScore(neighbor, locs, graph)
        candidates.push(neighbor, score)
        explored++

  RETURN candidates.extractTopN(topN)

FUNCTION ReferenceScore(file, definitions, graph):
  score ← 0.0

  // Direct reference (imports the definition file)
  FOR EACH def IN definitions:
    IF file IN graph.Imports(def.File):
      score += 20.0

  // Transitive reference (imports something that imports the definition)
  FOR EACH def IN definitions:
    FOR EACH importer IN graph.Imports(def.File):
      IF file IN graph.Imports(importer):
        score += 10.0

  // Centrality bonus
  score += graph.nodes[file].PageRank × 100.0

  // Community bonus
  defCommunity ← graph.nodes[definitions[0].File].Community
  IF graph.nodes[file].Community == defCommunity:
    score += 5.0

  RETURN score
```

---

## 15. Innovation 8: Multi-Resolution Indexing

### Architecture

| Level | Key | Value | Purpose |
|-------|-----|-------|---------|
| 0 | File path | `FileInfo` | Direct file lookup |
| 1 | Package path | `[]file paths` | Package-level queries |
| 2 | Community ID | `[]file paths` | Community-level queries |

### Application Across System

```
ALGORITHM MultiResQuery(index, query, level):
  SWITCH level:
    CASE "file":
      RETURN index.files[query]
    CASE "package":
      RETURN index.packages[query]
    CASE "community":
      RETURN index.communities[query]
    CASE "auto":
      // Determine best level based on query specificity
      IF query in index.files:
        RETURN {level: "file", result: index.files[query]}
      ELSE IF query in index.packages:
        RETURN {level: "package", result: index.packages[query]}
      ELSE:
        RETURN {level: "community", result: index.communities[query]}
```

---

## 16. Innovation 9: Importance-Weighted Context Allocation

**Location:** `internal/workflow/engine_verify.go:22-92`

### Current System

Equal budget allocation for all task files.

### Proposed System

Budget proportional to PageRank importance.

```
ALGORITHM ImportanceWeightedContext(graph, files, totalBudget):
  pageRank ← graph.PageRankValues()
  totalPR ← sum(pageRank[f] for f in files)

  allocations ← map[file → int]
  FOR EACH file IN files:
    weight ← pageRank[file] / totalPR
    allocations[file] ← int(weight × totalBudget)

  // Enforce min/max bounds
  FOR EACH file IN files:
    allocations[file] ← max(allocations[file], 4096)   // min 4KB
    allocations[file] ← min(allocations[file], 65536)  // max 64KB

  contexts ← []
  FOR EACH file IN files:
    content ← readFile(file, maxBytes=allocations[file])
    contexts.append({file, content, importance: pageRank[file]})

  sortBy(contexts, importance, descending)
  RETURN contexts
```

### Community-Aware Grouping

```
ALGORITHM GroupByCommunity(contexts, communities):
  groups ← map[communityID → []context]
  FOR EACH ctx IN contexts:
    cid ← communities[ctx.file]
    groups[cid].append(ctx)

  // Sort groups by total importance
  sortedGroups ← sortBy(groups, totalImportance, descending)
  RETURN sortedGroups
```

---

## 17. Innovation 10: Usage-Graph Importance Weighting

**Location:** `pkg/history/history.go:90-140`

### Current System

2-component scoring (recency + frequency). Entries scored independently.

### Proposed System

Co-occurrence graph + PageRank for structural importance.

```
ALGORITHM UsageGraphImportance(history, sessions):
  graph ← UndirectedGraph()
  FOR EACH session IN sessions:
    entries ← history.EntriesInSession(session)
    FOR EACH e1, e2 IN pairs(entries):
      graph.AddEdge(e1.id, e2.id)

  importance ← ComputePageRank(graph, iterations=20, damping=0.85)

  FOR EACH entry IN history:
    entry.importance ← importance[entry.id]

  RETURN history
```

### Enhanced Scoring

```
ALGORITHM EnhancedFrecencyScore(entry, currentTime, context, importance):
  // Recency (exponential decay)
  ageHours ← (currentTime - entry.lastUsed) / 3600
  recency ← 1.0 / (1.0 + ageHours/24.0)

  // Frequency
  frequency ← entry.useCount / maxUseCount

  // Context similarity (Trie prefix match)
  contextSimilarity ← trie.PrefixMatchScore(entry.text, context)

  // Co-occurrence importance
  cooccurrence ← importance[entry.id]

  // Category coherence
  categoryCoherence ← 1.0 if sameCategory(entry, recentEntries) else 0.0

  // Freshness
  freshDays ← (currentTime - entry.firstUsed) / 86400
  freshness ← 1.0 / (1.0 + freshDays/30.0)

  // Composite score
  score ← 0.25 × recency
         + 0.20 × frequency
         + 0.20 × contextSimilarity
         + 0.15 × cooccurrence
         + 0.10 × categoryCoherence
         + 0.10 × freshness

  RETURN score
```

---

## 18. Innovation 11: Task Graph Visualization

**Location:** `internal/tui/components/taskgraph.go:38-47`

### Current System

Wave-based layout, insertion order within waves, status-based coloring.

### Proposed System

PageRank ordering within waves + community-based coloring.

```
ALGORITHM EnhancedTaskGraphLayout(tasks, dependencies):
  graph ← buildDependencyGraph(tasks, dependencies)
  pageRank ← ComputePageRank(graph, iterations=20, damping=0.85)
  communities ← LouvainDetect_Deterministic(graph, seed=42)

  waves ← topologicalSort(graph)

  // Sort within waves by PageRank
  FOR EACH wave IN waves:
    sortBy(wave, pageRank, descending)

  // Assign colors by community
  communityColors ← generateColorPalette(len(set(communities.values())))
  FOR EACH task IN tasks:
    task.color ← communityColors[communities[task.id]]

  RETURN {waves, colors: communityColors}
```

---

## 19. Innovation 12: Context Registry Optimization

**Location:** `internal/context/registry.go:116`

### Current System

Linear scan through sources for each key: O(N) per lookup.

### Proposed System

Trie index + Bloom filter for fast lookups.

```
ALGORITHM OptimizedSourceLookup(registry, key):
  // Fast probabilistic check
  IF registry.sourceBloom.Contains(key):
    // Probably exists — do Trie lookup
    RETURN registry.sourceTrie.Search(key)
  ELSE:
    // Definitely doesn't exist
    RETURN nil
```

### Build Phase

```
ALGORITHM BuildSourceIndex(sources):
  trie ← new Trie()
  bloom ← BloomFilter_Create(len(sources), 0.01)

  FOR EACH source IN sources:
    trie.Insert(source.key, metadata=source)
    bloom.Add(source.key)

  RETURN {trie, bloom}
```

---

## 20. Incremental Update Algorithms

Recomputing all metrics from scratch on every file change is prohibitively expensive for large codebases. The following algorithms enable incremental updates.

### 20.1 Incremental PageRank

When a single file changes (add/remove import), only nodes within distance 2 of the change are affected.

```
ALGORITHM IncrementalPageRank(graph, changedNode, oldPR, iterations=5):
  affected ← set()
  affected.add(changedNode)
  FOR EACH neighbor IN graph.Neighbors(changedNode):
    affected.add(neighbor)
    FOR EACH neighbor2 IN graph.Neighbors(neighbor):
      affected.add(neighbor2)

  // Initialize affected nodes with current PR values
  newPR ← copy(oldPR)
  FOR EACH node IN affected:
    newPR[node] ← (1 - damping) / graph.NodeCount()

  // Run reduced iterations on affected subgraph
  FOR i IN range(iterations):
    FOR EACH node IN affected:
      importers ← node.ImportedBy INTERSECT affected
      IF len(importers) > 0:
        share ← oldPR[node] / len(importers)
        FOR EACH importer IN importers:
          newPR[importer] += damping × share

  RETURN newPR
```

**Complexity:** O(|affected| × E_affected × 5) where |affected| ≤ 2-hop radius, typically O(100 × 5) = O(500) vs O(V × E × 20) for full recomputation.

### 20.2 Incremental Louvain

When edges change, only the communities of affected nodes and their neighbors need recomputation.

```
ALGORITHM IncrementalLouvain(graph, changedEdges, oldCommunities):
  affectedNodes ← set()
  FOR EACH edge IN changedEdges:
    affectedNodes.add(edge.source)
    affectedNodes.add(edge.target)
    FOR EACH neighbor IN graph.Neighbors(edge.source):
      affectedNodes.add(neighbor)
    FOR EACH neighbor IN graph.Neighbors(edge.target):
      affectedNodes.add(neighbor)

  // Extract affected subgraph
  subgraph ← graph.ExtractSubgraph(affectedNodes)

  // Run Louvain on subgraph only
  subCommunities ← LouvainDetect_Deterministic(subgraph, seed=42)

  // Merge back: use subgraph communities for affected nodes,
  // keep original communities for unaffected nodes
  newCommunities ← copy(oldCommunities)
  FOR EACH node IN affectedNodes:
    newCommunities[node] ← subCommunities[node]

  RETURN newCommunities
```

### 20.3 Incremental Trie Update

Trie supports O(K) insert and delete without rebuilding.

```
ALGORITHM TrieDelete(trie, name):
  current ← trie.root
  path ← []
  FOR EACH char IN name:
    IF current.children[char] == nil:
      RETURN false  // not found
    path.append({node: current, char: char})
    current ← current.children[char]

  IF !current.isEnd:
    RETURN false  // not found

  current.isEnd ← false
  current.symbols ← remove(current.symbols, name)

  // Prune empty paths (optional, saves memory)
  FOR i IN REVERSE(path):
    node ← path[i].node
    child ← node.children[path[i].char]
    IF !child.isEnd AND len(child.children) == 0:
      delete node.children[path[i].char]

  RETURN true
```

### 20.4 Incremental Bloom Filter

Bloom filters do not support deletion. For dynamic sets, use a counting Bloom filter or rotating filter pair.

```
ALGORITHM RotatingBloomFilter:
  primary ← BloomFilter(expectedItems, fpRate)
  secondary ← BloomFilter(expectedItems, fpRate)

  // On insert: add to primary
  func Add(item):
    primary.Add(item)

  // On contains: check both
  func Contains(item):
    RETURN primary.Contains(item) || secondary.Contains(item)

  // On rotation (periodic): swap primary/secondary, rebuild primary
  func Rotate(allItems):
    secondary ← primary
    primary ← NewBloomFilter(len(allItems), fpRate)
    FOR EACH item IN allItems:
      primary.Add(item)
```

---

## 21. Concurrency and Thread-Safety Model

### 21.1 Read-Write Separation

Most EGINE operations are read-heavy (many queries, few updates). The concurrency model uses RWMutex for shared structures:

```go
type SafeIndex struct {
    mu          sync.RWMutex
    trie        *Trie
    bloom       *BloomFilter
    pagerank    map[string]float64
    communities map[string]int
}

// Read path: concurrent, no lock contention
func (idx *SafeIndex) Search(query string) []string {
    idx.mu.RLock()
    defer idx.mu.RUnlock()
    return idx.trie.PrefixSearch(query)
}

// Write path: exclusive lock, batch updates
func (idx *SafeIndex) Update(changedFiles []string) {
    idx.mu.Lock()
    defer idx.mu.Unlock()
    for _, f := range changedFiles {
        idx.trie.Delete(f)
        idx.trie.Insert(f, nil)
        idx.bloom.Add(f)
    }
}
```

### 21.2 Parallel PageRank

PageRank iterations can be parallelized across nodes:

```go
func ParallelPageRank(graph *WeightedGraph, iterations int, damping float64, numWorkers int) map[string]float64 {
    n := graph.NodeCount()
    pr := make(map[string]float64)
    for _, node := range graph.Nodes() {
        pr[node.ID] = 1.0 / float64(n)
    }

    for i := 0; i < iterations; i++ {
        newPR := make(map[string]float64)
        for _, node := range graph.Nodes() {
            newPR[node.ID] = (1 - damping) / float64(n)
        }

        // Parallel distribution
        var wg sync.WaitGroup
        ch := make(chan *WeightedNode, numWorkers)
        
        // Workers compute partial contributions
        for w := 0; w < numWorkers; w++ {
            wg.Add(1)
            go func() {
                defer wg.Done()
                for node := range ch {
                    importers := node.ImportedBy
                    if len(importers) > 0 {
                        share := pr[node.ID] / float64(len(importers))
                        for _, importer := range importers {
                            atomicAdd(&newPR[importer], damping*share)
                        }
                    }
                }
            }()
        }

        for _, node := range graph.Nodes() {
            ch <- node
        }
        close(ch)
        wg.Wait()

        // Dangling redistribution (sequential, small)
        // ... same as sequential version ...

        pr = newPR
    }
    return pr
}
```

### 21.3 Lock-Free Bloom Filter Reads

Bloom filter `Contains` is naturally thread-safe for read-only access (no writes during query). This enables lock-free pre-filtering:

```go
func (idx *SafeIndex) FastMembershipCheck(item string) bool {
    // Lock-free Bloom check (snapshot of bits is consistent)
    if !idx.bloom.Contains(item) {
        return false  // definitely not in set
    }
    
    // Expensive exact check requires lock
    idx.mu.RLock()
    defer idx.mu.RUnlock()
    return idx.exactMap[item]
}
```

### 21.4 Batch Update Protocol

For file-watcher triggered updates, use a batched update protocol:

```
ALGORITHM BatchedUpdate(watcher, index):
  pendingChanges ← new Queue()
  
  FOR EACH event IN watcher:
    pendingChanges.enqueue(event)
    
    // Debounce: wait 100ms for more changes
    IF pendingChanges.size() > 0:
      sleep(100ms)
      CONTINUE
    
    // Process batch
    batch ← pendingChanges.drain()
    affectedFiles ← extractFiles(batch)
    
    // Phase 1: Update Trie and Bloom (fast, per-file)
    FOR EACH file IN affectedFiles:
      index.trie.Delete(oldName)
      index.trie.Insert(newName)
      index.bloom.Add(newName)
    
    // Phase 2: Rebuild graph edges (incremental)
    graph ← index.graph
    FOR EACH file IN affectedFiles:
      graph.UpdateImports(file, newImports)
    
    // Phase 3: Incremental PageRank (bounded iterations)
    FOR EACH file IN affectedFiles:
      IncrementalPageRank(graph, file, index.pagerank, iterations=5)
    
    // Phase 4: Periodic full rebuild (every 1000 changes)
    IF changeCount % 1000 == 0:
      FullRebuild(graph, index)
```

---

## 22. Error Handling and Edge Cases

### 22.1 Disconnected Graphs

PageRank on disconnected graphs still converges. The damping factor redistributes rank from dangling nodes uniformly. No special handling needed.

### 22.2 Cyclic Dependencies

Task scheduling assumes DAG (directed acyclic graph). Cycle detection is mandatory:

```
ALGORITHM DetectCycles(graph):
  WHITE ← 0; GRAY ← 1; BLACK ← 2
  color ← map[node → WHITE]
  parent ← map[node → nil]
  
  FOR EACH node IN graph:
    IF color[node] == WHITE:
      IF DFS_CycleCheck(node, color, parent):
        RETURN cyclePath
  
  RETURN nil  // no cycles

FUNCTION DFS_CycleCheck(node, color, parent):
  color[node] ← GRAY
  FOR EACH neighbor IN graph.Successors(node):
    IF color[neighbor] == GRAY:
      // Found cycle: reconstruct path
      RETURN reconstructCycle(parent, node, neighbor)
    IF color[neighbor] == WHITE:
      parent[neighbor] ← node
      IF DFS_CycleCheck(neighbor, color, parent):
        RETURN true
  color[node] ← BLACK
  RETURN false
```

### 22.3 Empty Graphs

All algorithms gracefully handle empty graphs (0 nodes, 0 edges). PageRank returns empty map, Louvain returns empty communities, Trie returns empty results.

### 22.4 Single-Node Graphs

PageRank on a single node returns 1.0. Louvain returns single community. Betweenness returns 0.0 (no paths through a single node).

### 22.5 Numerical Stability

PageRank uses float64 (64-bit IEEE 754) with ~15 decimal digits of precision. For graphs with 10K nodes, the smallest meaningful PageRank is ~10⁻⁵ (1/N), well above float64 epsilon (~2.2×10⁻¹⁶). No numerical stability issues for practical graph sizes.

---

## 23. Changelog

### v1.0 (Initial Release)

| Innovation | Status | Location |
|-----------|--------|----------|
| Importance-Weighted Task Scheduling | Design complete | `pkg/taskrunner/runner.go` |
| Graph-Based Code Complexity | Design complete | `internal/tools/codecomplexity.go` |
| Provider Reliability Ranking | Design complete | `internal/provider/fallback.go` |
| Trie-Based Symbol Search | Design complete | `internal/codeintel/index.go` |
| Bloom Filter Pre-Filters | Design complete | Multiple locations |
| Composite Scoring Functions | Design complete | Multiple locations |
| Adaptive Expansion for Navigation | Design complete | `internal/tools/codemap.go` |
| Multi-Resolution Indexing | Design complete | `internal/codeintel/` |
| Importance-Weighted Context | Design complete | `internal/workflow/engine_verify.go` |
| Usage-Graph Importance | Design complete | `pkg/history/history.go` |
| Task Graph Visualization | Design complete | `internal/tui/components/taskgraph.go` |
| Context Registry Optimization | Design complete | `internal/context/registry.go` |

---

*This document provides detailed algorithm specifications for all EGINE innovations. It is ready for implementation.*
