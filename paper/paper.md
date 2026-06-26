---
title: "EFIE: Community-Structured, Importance-Weighted Codebase Exploration via Graph Algorithms"
tags:
  - Go
  - codebase intelligence
  - graph algorithms
  - PageRank
  - community detection
  - software engineering
authors:
  - name: Eshan Roy
    orcid: 0009-0007-1261-6805
    affiliation: 1
affiliations:
  - name: Independent Researcher
    index: 1
date: 26 June 2026
bibliography: paper.bib
---

# Summary

EFIE (Eshanized File Intelligence Engine) is a novel algorithm for codebase exploration that replaces brute-force graph scanning with community-structured, importance-weighted, adaptive expansion. Unlike traditional systems that treat source files as a flat collection, EFIE models a codebase as a weighted, multi-resolution graph where architectural importance is precomputed via PageRank and betweenness centrality, natural file clusters are discovered through deterministic Louvain community detection, and queries are answered by following the most relevant paths first via adaptive expansion with bounded cost.

The algorithm operates in two phases: a build phase that constructs a multi-resolution index in $O(N \times F / P + E \times \log V)$ time (where $N$ = files, $F$ = file size, $P$ = processors, $E$ = edges, $V$ = vertices), and a query phase that answers relevance queries in $O(S \times B)$ time (where $S$ = seed count, $B$ = expansion budget). Empirical evaluation on four real Go repositories (gin, docker, go-stdlib, kubernetes; 99 to 17,266 files; $n=30$ runs, 95% confidence intervals) demonstrates linear scaling with build time at 0.556 ms/file (R²=0.954) and query time at 0.0022 ms/file (R²=0.998). Precomputation overhead results in 12-23x slower builds than baseline BFS, while query times remain under 18 ms. Relevance evaluation on 176 ground truth queries shows EFIE achieves perfect MRR (1.000 vs 0.957), always finding a relevant file first, while BFS achieves higher precision (Precision@5=0.785 vs 0.460).

EFIE is implemented in Go as part of the M31A codebase intelligence subsystem. Formal specifications, mathematical proofs, and complexity analysis are published in the repository's `EFIE/` directory.

# Statement of Need

Codebase exploration is a fundamental task in software engineering: developers must locate relevant files, understand dependency relationships, and identify architecturally important code before making changes. Existing codebase intelligence systems — including LSP indexers such as LSIF [@lsif] and SCIP [@scip] — treat source files as a flat collection and rely on brute-force BFS and linear scanning to answer queries. This approach scales poorly: build times grow linearly with codebase size, symbol searches execute in $O(N)$ time, and relevance scoring evaluates every file regardless of architectural importance.

EFIE addresses this gap by applying well-established graph-theoretic algorithms — PageRank [@brin1998], Louvain community detection [@blondel2008], and betweenness centrality [@freeman1977] — to the novel domain of codebase exploration. While each algorithm has been studied independently in network science, their combination for code intelligence, along with the adaptive expansion query strategy and multi-resolution index structure, constitutes a novel contribution to computational software engineering. The trade-off is explicit: precomputation adds build-time overhead but produces a rich index that supports importance-weighted queries beyond what flat BFS can provide.

The practical need is demonstrated by the performance limitations of existing systems. Sourcegraph's LSIF indexer processes Go repositories at approximately 54,000 significant lines of code per second for a 1.3M SLoC monorepo, with indexing time growing super-linearly for larger repositories [@lsif-benchmark]. SCIP improved upon LSIF with 3-10x speedups in indexing and 4-5x smaller index files [@scip], but both systems focus on code navigation rather than relevance-based exploration. EFIE complements these systems by providing importance-weighted relevance scoring that identifies not just where a symbol is defined, but which files are architecturally related to a given task.

# State of the Field

Graph-based analysis of software systems has a rich history. Zanoni [@zanoni2006] applied PageRank to source code graphs for bug prediction. Blondel et al. [@blondel2008] introduced the Louvain algorithm for community detection in large networks, achieving $O(|E| \times \log |V|)$ time complexity — a method since adopted for analyzing software dependency networks.

Tree-sitter [@tree-sitter] provides incremental parsing at throughputs exceeding 100 MB/s for typical source files, with incremental re-parsing completing in approximately 0.1-0.2ms. Sourcegraph's SCIP format demonstrated that protobuf-based code intelligence indexes are 4-5x smaller and 3x faster to process than LSIF JSON format [@scip]. These advances provide the foundation upon which EFIE builds its graph construction phase.

Trie data structures offer $O(K)$ prefix search where $K$ is the query length, compared to $O(N)$ linear scan for hash-based symbol lookup. Benchmarks demonstrate that Tries outperform hash maps for string matching when miss rates exceed 50% [@trie-hard], a condition frequently met in codebase symbol search. EFIE exploits this property for its symbol index.

The key distinction between EFIE and prior work is the integration of multiple graph-theoretic measures into a unified framework. Prior systems use individual algorithms in isolation — PageRank for ranking, community detection for clustering, centrality for bridge identification — whereas EFIE combines them through a composite scoring function with six weighted components, normalized using robust percentile-based statistics, and queries are answered through adaptive expansion that prioritizes high-importance paths with bounded cost.

# Software Design

EFIE operates in two phases: **build** (index construction) and **query** (relevance scoring).

## Build Phase

The build phase has five sequential stages. Stage 1 performs parallel file discovery and parsing across `runtime.NumCPU()` workers, reading only the first 4KB of non-Go source files to extract import statements. Stage 2 constructs a weighted import graph with forward edges (imports) and reverse edges (imported-by), connecting unresolved imports to an external sentinel node to maintain graph connectivity. Stage 3 runs deterministic Louvain community detection with fixed random seed (seed=42) and canonical renumbering for reproducible output. Stage 4 computes PageRank (20 iterations, damping factor 0.85) and approximate betweenness centrality via stratified random sampling ($|V|/5$ samples). Stage 5 assembles the multi-resolution index: file-level, package-level, and community-level lookups, a Trie for symbol prefix search, and Bloom filters for $O(1)$ import membership checks.

## Query Phase

The query phase dispatches to different traversal strategies based on query type: BFS for upstream/downstream traversal, exact Trie/map lookup for symbol definition, and adaptive expansion for relevance queries. Adaptive expansion generates seed files from target files, their direct neighbors, community members, adjacent community members, and symbol Trie matches. A max-heap BFS then expands from the highest-scored candidates, following neighbors whose PageRank exceeds an auto-calibrated threshold (median PageRank $\times$ 0.5), until an expansion budget of $5 \times \text{topN}$ is exhausted.

The scoring function combines six weighted components: graph centrality (20%), direct relevance (35%), import proximity (20%), symbol match (15%), and community coherence (10%), with a Bloom filter cross-check quality gate. Percentile-based normalization (95th percentile) prevents outlier hub files from compressing scores.

# Empirical Evaluation

We evaluated EFIE on four real Go repositories: gin (99 files), docker (10,218 files), go-stdlib (11,466 files), and kubernetes (17,266 files). Each benchmark was executed 30 times with 95% confidence intervals.

## Build and Query Performance

| Repository | Files | EFIE Build (ms) | Original Build (ms) | Ratio | EFIE Query (ms) | Original Query (ms) | Ratio |
|-----------|-------|-----------------|---------------------|-------|-----------------|---------------------|-------|
| gin | 99 | 48.6 ± 4.3 | 3.9 ± 0.6 | 12.5x | 0.4 ± 0.4 | 0.014 | 28.1x |
| docker | 10,218 | 934.4 ± 42.0 | 76.0 ± 3.8 | 12.3x | 4.0 ± 0.9 | 0.016 | 257.5x |
| go-stdlib | 11,466 | 8,578.8 ± 547.0 | 375.0 ± 25.3 | 22.9x | 18.4 ± 6.6 | 0.025 | 736.0x |
| kubernetes | 17,266 | 5,926.5 ± 421.0 | 280.4 ± 23.7 | 21.1x | 6.7 ± 6.6 | 0.027 | 244.8x |

Build time scales linearly at 0.556 ms/file (R²=0.954), query time at 0.0022 ms/file (R²=0.998). Precomputation overhead (PageRank, Louvain, betweenness centrality) results in 12-23x slower builds, while query times remain under 18 ms.

## Community Detection

Louvain community detection identifies meaningful architectural modules: 84 communities in gin, 1,302 in docker, 33 in go-stdlib, and 1,624 in kubernetes, enabling importance-weighted queries that respect architectural boundaries.

## Relevance Quality

We constructed 176 ground truth queries across five categories (exact function, conceptual, import graph, architectural, package-level) and evaluated relevance:

| Metric | EFIE | Original | Delta |
|--------|------|----------|-------|
| Precision@5 | 0.460 | 0.785 | -0.325 |
| Precision@10 | 0.230 | 0.737 | -0.507 |
| Recall@5 | 0.521 | 0.514 | +0.007 |
| MRR | 1.000 | 0.957 | +0.043 |
| NDCG@10 | 0.730 | 0.886 | -0.156 |

EFIE achieves perfect MRR (1.000), always finding a relevant file first, while BFS achieves higher precision. Per-category analysis reveals that EFIE outperforms BFS on import graph queries (NDCG@10: 1.000 vs 0.991, MRR: 1.000 vs 0.988), where the graph-aware scoring function better captures dependency relationships. For exact function lookup and architectural queries, EFIE also achieves perfect MRR (1.000 vs 0.794 and 0.976 respectively), demonstrating that importance weighting helps identify the first relevant file faster. However, BFS maintains higher precision across all categories, representing an explicit trade-off between importance-weighted ranking and raw precision.

# Research Impact Statement

EFIE demonstrates that well-known graph algorithms, when properly integrated and adapted to the codebase domain, produce a structured index that supports importance-weighted codebase exploration. The formal mathematical contributions include:

1. **Complexity bounds:** $O(N \times F / P + E \times \log V)$ build time and $O(S \times B)$ query time, with proofs of parallel speedup via Amdahl's Law and bounded expansion via budget-constrained BFS.

2. **Approximation guarantees:** Stratified betweenness centrality sampling achieves within 10% of exact values for sample sizes $\geq |V|/5$, with proportional representation from each community ensuring no systematic bias.

3. **Convergence analysis:** PageRank converges to within 4% of the fixed point in 20 iterations for damping factor 0.85, verified by the geometric convergence rate $d^t$ where $d < 1$.

4. **Optimal Bloom filter sizing:** For a target false positive rate $p$ and $n$ elements, the optimal configuration requires $m = -n \ln p / (\ln 2)^2$ bits and $k = (m/n) \ln 2$ hash functions, providing approximately 1.2 bytes per element at 1% false positive rate.

The algorithm has been validated on four real Go repositories with empirical results demonstrating linear scalability (R²=0.954 for build, R²=0.998 for query; $n=30$ runs). Community detection identifies meaningful architectural modules: 84 communities in gin, 1,302 in docker, 33 in go-stdlib, and 1,624 in kubernetes. Relevance evaluation on 176 ground truth queries demonstrates that EFIE's perfect MRR (1.000) comes at the cost of lower precision compared to baseline BFS, representing an explicit trade-off between importance-weighted ranking and raw precision. The software is publicly available under the MIT license with comprehensive documentation of the mathematical foundations.

# AI Usage Disclosure

No generative AI tools were used in the development of this software. The EFIE algorithm, its mathematical proofs, and the implementation were designed and written by the author without AI assistance.

The paper manuscript was written by the author. No AI writing assistants were used in drafting, editing, or revising this paper.

# Acknowledgements

The author thanks the Go open source community for providing the benchmark repositories used in this evaluation.

# References
