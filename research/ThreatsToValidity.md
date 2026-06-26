# Threats to Validity

**Date:** 26 June 2026

---

## 1. Internal Validity

### 1.1 Benchmark Contamination
**Threat:** Filesystem cache effects may bias build time measurements.
**Mitigation:** Warm-up run before measurement; 30 independent runs with randomized order.

### 1.2 Measurement Overhead
**Threat:** `runtime.MemStats` sampling may miss allocation spikes.
**Mitigation:** Use `runtime.MemStats.Alloc` before/after for exact delta; also profile with `pprof`.

### 1.3 Concurrent Modifications
**Threat:** Background processes may interfere with timing.
**Mitigation:** Pin CPU cores; disable CPU frequency scaling; run in isolated container.

### 1.4 Code Path Differences
**Threat:** EFIE and Original use different code paths, making direct comparison unfair.
**Mitigation:** Both backends share the same parser (`parser.go`) and graph representation. Differences are intentional (the algorithm's contribution).

---

## 2. External Validity

### 2.1 Synthetic Repository Representativeness
**Threat:** Randomly generated repositories do not exhibit real-world community structure, import patterns, or file organization.
**Status:** **CONFIRMED** — Synthetic repos produce 0 communities with Louvain, making community-boost claims untestable.
**Mitigation:** Use real repositories (Go stdlib, Kubernetes, VS Code, Django, Redis) as primary benchmarks.

### 2.2 Language Coverage
**Threat:** Benchmarks use only Go, TypeScript, Python, C. Results may not generalize to Java, Rust, JavaScript.
**Mitigation:** Acknowledge as limitation; focus on languages with mature parser support.

### 2.3 Codebase Size Range
**Threat:** 10K files is small compared to production monorepos (100K+ files).
**Mitigation:** Report scaling laws (ms/file) to enable extrapolation; acknowledge as limitation.

---

## 3. Construct Validity

### 3.1 Relevance Quality Metric
**Threat:** Precision@k, Recall@k, MRR, NDCG assume binary relevance (relevant/not-relevant). Real code exploration has graded relevance.
**Mitigation:** Use 3-level relevance scale (high/medium/low); report inter-annotator agreement.

### 3.2 Ground Truth Completeness
**Threat:** Ground truth for 50 queries may not cover all query types or difficulty levels.
**Mitigation:** Stratified sampling across query types; report coverage statistics.

### 3.3 "Relevance" Definition
**Threat:** What constitutes a "relevant" file is subjective and varies by developer task.
**Mitigation:** Use concrete task scenarios (bug fix, feature addition, refactoring) with pre-defined relevance judgments.

---

## 4. Reliability

### 4.1 Statistical Power
**Threat:** 30 runs may be insufficient for small effect sizes.
**Mitigation:** Compute minimum detectable effect (MDE) given α=0.05, power=0.8, n=30. For typical variances, MDE ≈ 0.73σ.

### 4.2 Multiple Comparisons
**Threat:** Testing 25 claims increases false positive risk (family-wise error rate).
**Mitigation:** Apply Bonferroni correction for family-level tests; report adjusted p-values.

### 4.3 Reproducibility
**Threat:** Results depend on specific commit, hardware, and Go version.
**Mitigation:** Record exact commit hash, hardware specs, Go version; provide Docker container for reproduction.

---

## 5. Known Limitations (Post-Audit)

| # | Limitation | Impact | Mitigation |
|---|-----------|--------|-----------|
| L1 | PageRank computes wrong direction | Algorithm is fundamentally different from spec | Document as "design decision" or fix implementation |
| L2 | Synthetic repos have 0 communities | Community boost claims untestable on synthetic data | Use real repos only for community claims |
| L3 | Import proximity is unbounded | Scoring weights are meaningless | Cap at 20.0 or reweight |
| L4 | Incremental build not implemented | 63% faster rebuild claim is theoretical | Remove claim or implement feature |
| L5 | Convergence not monitored | 4% convergence guarantee is unverified | Add convergence check |
| L6 | No ground truth dataset | Relevance quality claims untestable | Construct ground truth (Phase 5) |
| L7 | No profiling data | Root cause of slowness unknown | Run profiling (Phase 7) |
| L8 | Code duplication (~780 lines) | Maintenance burden | Refactor to share code |
