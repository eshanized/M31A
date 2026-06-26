# Experimental Plan

**Date:** 26 June 2026
**Scope:** Complete experimental methodology for EFIE validation

---

## 1. Experimental Objectives

1. Determine whether EFIE provides measurable benefits over the existing codeintel backend
2. Validate all 25 quantitative claims from the ClaimInventory
3. Identify the conditions under which EFIE is beneficial vs harmful
4. Generate publication-ready figures, tables, and statistical analyses

---

## 2. Benchmark Infrastructure

### 2.1 Repositories

| ID | Repository | Size | Language | Purpose |
|----|-----------|------|----------|---------|
| R1 | Go stdlib (1.22) | ~2,000 files | Go | Standard library baseline |
| R2 | Kubernetes | ~15,000 files | Go | Large Go monorepo |
| R3 | VS Code | ~12,000 files | TypeScript | Large TypeScript codebase |
| R4 | Django | ~4,000 files | Python | Medium Python codebase |
| R5 | Redis | ~1,000 files | C | Small C codebase |
| R6 | Synthetic-100 | 100 files | Go | Controlled experiments |
| R7 | Synthetic-1K | 1,000 files | Go | Controlled experiments |
| R8 | Synthetic-10K | 10,000 files | Go | Controlled experiments |

### 2.2 Metrics

| Metric | Unit | Collection Method |
|--------|------|------------------|
| Build time | ms | `time.Now()` before/after `Build()` |
| Build RSS peak | MB | `runtime.MemStats.Sys` delta |
| Build allocations | count | `runtime.MemStats.TotalAlloc` delta |
| Query time | ms | `time.Now()` before/after `Query()` |
| Query RSS peak | MB | `runtime.MemStats.Sys` delta |
| Query allocations | count | `runtime.MemStats.TotalAlloc` delta |
| Communities detected | count | `len(index.Communities)` |
| Modularity Q | [0,1] | Return from `detectCommunities()` |
| PageRank convergence | L1 norm | $\|PR^{t+1} - PR^t\|_1$ |
| Precision@k | [0,1] | Ground truth evaluation |
| Recall@k | [0,1] | Ground truth evaluation |
| MRR | [0,1] | Ground truth evaluation |
| NDCG@k | [0,1] | Ground truth evaluation |

### 2.3 Statistical Rigor

- **Minimum 30 independent runs** per (repo, backend) pair
- **95% confidence intervals** via t-distribution
- **Effect size** via Cohen's d
- **Paired t-tests** for EFIE vs Original comparisons
- **Bonferroni correction** for multiple comparisons

---

## 3. Experiment Protocols

### 3.1 Build Performance (E1)

**Objective:** Measure build time and memory for EFIE vs Original across all repositories.

**Procedure:**
1. For each repository R in {R1..R8}:
   a. Warm up filesystem cache by reading all files once
   b. For each backend B in {Original, EFIE}:
      i. Create fresh indexer instance
      ii. Record T_start
      iii. Call `indexer.Build(repoPath)`
      iv. Record T_end, MemStats
      v. Repeat 30 times
2. Compute mean, std, 95% CI for each (R, B) pair
3. Compute EFIE/Original ratio and Cohen's d

**Deliverables:** `RawResults/build_perf.json`, `Statistics/build_perf_stats.json`, `Plots/build_perf.png`

### 3.2 Query Performance (E2)

**Objective:** Measure query time for EFIE vs Original across all repositories and query types.

**Query Types:**
- Q1: Exact function name (e.g., "parseFile")
- Q2: Partial symbol (e.g., "parse")
- Q3: Related files (e.g., files related to "HTTP handler")
- Q4: Import graph (e.g., files importing "fmt")
- Q5: Broad search (e.g., "error handling")

**Procedure:**
1. Build index for each (R, B) pair
2. For each query type Q in {Q1..Q5}:
   a. Execute 30 independent query runs
   b. Record query time, result count, result list
3. Compute statistics and comparisons

**Deliverables:** `RawResults/query_perf.json`, `Statistics/query_perf_stats.json`, `Plots/query_perf.png`

### 3.3 Relevance Quality (E3)

**Objective:** Measure whether EFIE produces more relevant results than Original.

**Procedure:**
1. Construct ground truth for 50 representative queries
2. For each (R, B) pair:
   a. Execute all queries
   b. Compute Precision@5, Precision@10, Recall@5, Recall@10, MRR, NDCG@10
3. Compare EFIE vs Original using paired t-tests

**Deliverables:** `RawResults/relevance.json`, `Statistics/relevance_stats.json`, `Plots/relevance.png`

### 3.4 Scalability Analysis (E4)

**Objective:** Verify linear scaling claims for both build and query phases.

**Procedure:**
1. Use synthetic repositories of sizes {100, 500, 1K, 2K, 5K, 10K}
2. Measure build and query times (30 runs each)
3. Fit linear model: time = α + β × N
4. Report R², slope (ms/file), intercept

**Deliverables:** `RawResults/scalability.json`, `Statistics/scalability_stats.json`, `Plots/scalability_build.png`, `Plots/scalability_query.png`

### 3.5 Ablation Study (E5)

**Objective:** Determine contribution of each EFIE subsystem.

**Variants:**
- V0: Original baseline (no EFIE)
- V1: EFIE without PageRank (centrality=0)
- V2: EFIE without Louvain (no community boost)
- V3: EFIE without Betweenness (centrality=PageRank only)
- V4: EFIE without Trie (linear symbol search)
- V5: EFIE without Bloom filter (no cross-check)
- V6: Full EFIE

**Procedure:**
1. For each variant V in {V0..V6}:
   a. Build and query on R1 (Go stdlib) and R3 (VS Code)
   b. 30 runs each
2. Compare full EFIE vs each ablated variant

**Deliverables:** `RawResults/ablation.json`, `Statistics/ablation_stats.json`, `Plots/ablation.png`

### 3.6 Algorithm Correctness (E6)

**Objective:** Verify PageRank convergence, betweenness accuracy, Louvain quality.

**Procedure:**
1. **PageRank convergence:** Run 50 iterations, record L1 norm after each. Verify <4% at iteration 20.
2. **Betweenness accuracy:** Compute exact betweenness for small graphs (≤500 nodes). Compare with approximate (|V|/5 samples).
3. **Louvain quality:** Compare with reference Louvain implementation on small graphs.

**Deliverables:** `RawResults/correctness.json`, `Statistics/correctness_stats.json`, `Plots/pagerank_convergence.png`, `Plots/betweenness_accuracy.png`

### 3.7 Profiling (E7)

**Objective:** Identify why EFIE is slower than baseline.

**Procedure:**
1. CPU profile: `pprof CPUProfile` for build and query
2. Memory profile: `pprof AllocsProfile`
3. Flame graph analysis
4. Identify top-5 hotspots

**Deliverables:** `RawResults/profile_cpu.pb.gz`, `RawResults/profile_mem.pb.gz`, `Statistics/hotspots.json`

---

## 4. Analysis Pipeline

```
Raw data collection (Go benchmarks)
    ↓
RawResults/*.json (individual measurements)
    ↓
Statistics/*.json (aggregated statistics, CIs, p-values)
    ↓
Plots/*.png (publication-ready figures)
    ↓
Evaluation/ (final tables, narrative analysis)
    ↓
Manuscript update (paper/paper.md)
```

---

## 5. Timeline

| Phase | Task | Duration | Dependencies |
|-------|------|----------|-------------|
| 5 | Ground truth construction | 2 days | None |
| 6 | Real repo benchmark setup | 1 day | None |
| 7-8 | Run experiments E1-E3 | 3 days | Phase 5, 6 |
| 9 | Run experiments E4-E7 | 2 days | Phase 5, 6 |
| 10 | Statistical analysis | 1 day | Phase 7-9 |
| 11 | Profiling and root cause | 1 day | Phase 7-9 |
| 12 | Ablation study | 1 day | Phase 7-9 |
| 13 | Reproducibility pipeline | 1 day | Phase 10-12 |
| 14 | Peer review simulation | 1 day | Phase 10-13 |
| 15 | Manuscript update | 1 day | Phase 14 |
| 16 | Publication readiness | 1 day | Phase 15 |
