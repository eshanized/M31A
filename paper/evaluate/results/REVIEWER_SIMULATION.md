# Simulated Peer Review

**Date:** 26 June 2026
**Manuscript:** "EFIE: Community-Structured, Importance-Weighted Codebase Exploration via Graph Algorithms"

---

## Reviewer A: Software Engineering Methodology

### Strengths
1. Novel combination of graph algorithms for codebase exploration
2. Formal mathematical foundations with proofs
3. Comprehensive algorithm specification with pseudocode
4. Implementation in Go with compatibility wrapper

### Weaknesses
1. **Critical: Unverified performance claims** — The manuscript claimed "approximately 2.5 seconds" for 10K files build, but measured time is 5679ms (2.3x slower). The "5-20x faster relevance queries" claim is refuted: EFIE is 50x slower than baseline.
2. **No real-world evaluation** — All experiments use synthetic codebases with weak community structure (0 communities detected). The algorithm's advantages may only manifest on real codebases with natural package structure.
3. **Missing implementation details** — The incremental build feature (claimed "approximately 0.9 seconds") is not implemented.
4. **Incomplete baseline comparison** — The comparison only measures latency, not relevance quality. EFIE may provide better results even if slower.

### Recommendations
1. Remove all unverified performance claims
2. Add evaluation on real codebases (e.g., Go standard library, Kubernetes)
3. Measure relevance quality, not just speed
4. Implement and benchmark incremental build
5. Add ablation study for each algorithm component

---

## Reviewer B: Algorithms and Graph Theory

### Strengths
1. Well-formulated mathematical foundations
2. Proper complexity analysis with proofs
3. Deterministic Louvain with fixed seed for reproducibility
4. Stratified sampling for betweenness centrality

### Weaknesses
1. **Community detection failure** — The algorithm detected 0 communities in all synthetic codebases. This suggests either: (a) the synthetic graphs lack community structure, (b) the implementation has a bug, or (c) the algorithm requires minimum edge density.
2. **Theoretical vs. empirical gap** — The $O(N \times F / P + E \times \log V)$ complexity is verified (R²=0.97), but the constant factor is large (0.51 ms/file), making EFIE slower than the simpler baseline.
3. **Missing convergence verification** — The claim that "PageRank converges to within 4% in 20 iterations" is not empirically verified.
4. **No optimality analysis** — No analysis of whether the 6-component scoring function weights are optimal.

### Recommendations
1. Investigate why community detection finds 0 communities
2. Measure PageRank convergence empirically
3. Analyze sensitivity of scoring function weights
4. Add theoretical analysis of constant factors

---

## Reviewer C: Reproducibility and Empirical Evaluation

### Strengths
1. Automated benchmarking framework provided
2. Statistical analysis with 5 runs per benchmark
3. Linear regression for complexity analysis
4. Determinism test passes (score distributions match)

### Weaknesses
1. **Insufficient runs** — Only 5 runs per benchmark (quick mode). JOSS requires at least 30 runs for statistical significance.
2. **Synthetic data only** — No real-world codebases tested. Synthetic codebases have uniform random imports, lacking the hierarchical structure of real software.
3. **No raw data provided** — The benchmark framework generates JSON results but they are not included in the submission.
4. **Missing metrics** — No measurement of: allocation counts, GC cycles, CPU profiles, memory profiles, or flame graphs.
5. **No confidence intervals** — The manuscript does not report confidence intervals for measured values.

### Recommendations
1. Run full evaluation with 30+ runs
2. Test on real codebases: Go stdlib, Kubernetes, Docker, etc.
3. Include all raw benchmark data in submission
4. Add CPU/memory profiling results
5. Report 95% confidence intervals for all metrics
6. Provide Dockerfile for reproducible environment

---

## Consolidated Revision Checklist

### Must Fix (Required for Acceptance)
- [ ] Remove all unverified performance claims (C1, C2, C3, C4)
- [ ] Mark theoretical claims as "Expected" or provide empirical evidence
- [ ] Add evaluation on at least 2 real-world codebases
- [ ] Run full evaluation with 30+ runs per benchmark
- [ ] Include all raw benchmark data
- [ ] Add 95% confidence intervals to all reported values

### Should Fix (Strongly Recommended)
- [ ] Investigate community detection failure (0 communities)
- [ ] Measure PageRank convergence empirically
- [ ] Add ablation study for each component
- [ ] Measure relevance quality (precision/recall)
- [ ] Add CPU/memory profiling results
- [ ] Provide Dockerfile for reproducibility

### Nice to Have (Optional)
- [ ] Implement and benchmark incremental build
- [ ] Analyze scoring function weight sensitivity
- [ ] Compare against other code intelligence tools (LSIF, SCIP)
- [ ] Add flame graphs to supplementary material

---

## Verdict

**REVISION REQUIRED**

The manuscript presents a novel algorithm with strong mathematical foundations, but the empirical evaluation is insufficient for publication. The performance claims are refuted by the provided evidence, and the evaluation uses only synthetic data. Major revision is required to:

1. Correct the manuscript to reflect actual measured performance
2. Add real-world evaluation on established codebases
3. Provide comprehensive statistical analysis with sufficient runs
4. Include all raw data for reproducibility

The algorithm's theoretical contributions are sound, but the practical evaluation must demonstrate measurable benefits on real-world software.
