# Peer Review Simulation

**Date:** 26 June 2026
**Manuscript:** EFIE: Eshanized File Intelligence Engine for Importance-Weighted Codebase Exploration
**Reviewers:** 3 simulated (R1: Methods Expert, R2: Systems Expert, R3: Domain Expert)

---

## Overall Assessment

| Reviewer | Recommendation | Confidence |
|----------|---------------|------------|
| R1 (Methods) | REVISION REQUIRED | High |
| R2 (Systems) | REVISION REQUIRED | High |
| R3 (Domain) | REVISION REQUIRED | Medium |

**Consensus:** Major revision required. The algorithm is interesting but the evidence base is insufficient for publication.

---

## Reviewer 1 (Methods Expert)

### Major Issues

**M1: Critical algorithmic bug invalidates core claims.**
PageRank computes importance in the wrong direction (centrality.go:27-35). The spec says importance flows from importer to imported; the code distributes to importers. This means "important" files are those that are imported by many files, not those that import many important files. The 6-component scoring function's centrality component (20% weight) is measuring the wrong thing. This must be fixed before the paper can be considered.

**M2: Import proximity score is unbounded.**
The spec says the maximum proximity score is 20.0 (EFIE_ALGORITHM_V2.md:1281), but the implementation has no cap (scorer.go:49-67). A file importing 10 targets gets 100 points, completely dominating the 35-point direct relevance score. The scoring weights are meaningless as implemented.

**M3: Synthetic repositories produce 0 communities.**
The Louvain community detection finds 0 communities in all synthetic repositories (EVALUATION_REPORT.md). This means the community boost component (10% weight) contributes nothing in the evaluation. The authors must demonstrate community detection on real repositories with hierarchical structure.

**M4: No ground truth dataset for relevance evaluation.**
The paper claims EFIE provides "importance-weighted relevance scoring" but does not measure relevance quality. Precision@k, Recall@k, MRR, and NDCG are not reported. A ground truth dataset of at least 50 queries with relevance judgments is required.

### Minor Issues

**m1: 5 runs is insufficient for publication.**
The evaluation uses 5 runs per benchmark (EVALUATION_REPORT.md:5). Standard practice requires 30+ runs with confidence intervals.

**m2: No ablation study.**
The paper does not isolate the contribution of individual components (PageRank, Louvain, betweenness, Trie, Bloom). An ablation study is needed.

**m3: Missing convergence verification.**
PageRank convergence to 4% in 20 iterations is claimed (paper.md:69) but not empirically verified. The implementation does not monitor convergence.

---

## Reviewer 2 (Systems Expert)

### Major Issues

**M1: Build time claim is wrong.**
The paper claims "under 5 seconds" for 10K files (paper.md:73). Measured time is 5679ms (EVALUATION_REPORT.md:21). This must be corrected.

**M2: Query performance claim is wrong.**
The paper claims "5-20x faster relevance queries" (paper.md:73). EFIE is actually 50x slower than baseline (EVALUATION_REPORT.md:23). The 185x improvement for "symbol" queries is not explained in the paper.

**M3: Memory claim is wrong.**
The paper claims "16% additional memory overhead" (paper.md:73). EFIE actually uses 45% LESS memory than baseline (EVALUATION_REPORT.md:24). While this is favorable, it suggests the implementation differs significantly from what was analyzed.

**M4: No profiling data.**
The paper does not explain why EFIE is slower than baseline. CPU profiling is needed to identify bottlenecks (PageRank iterations, Louvain passes, Trie construction, scoring overhead).

### Minor Issues

**m1: No CPU/memory profiling methodology.**
The paper should describe how performance was measured and what tools were used.

**m2: Code duplication is excessive.**
~780 lines are duplicated between EFIE and original codeintel (parsers, import resolution, identifier extraction). This should be refactored.

**m3: Incremental build is not implemented.**
The 63% faster incremental rebuild claim (paper.md:69) is theoretical. Either implement it or remove the claim.

---

## Reviewer 3 (Domain Expert)

### Major Issues

**M1: No real-world case study.**
The evaluation uses only synthetic repositories and one real repository (Go stdlib). A meaningful case study on a production codebase (Kubernetes, VS Code, or Django) is required.

**M2: No comparison with industry systems.**
The paper does not compare with Sourcegraph (LSIF/SCIP), GitHub Code Navigation, or other production code intelligence systems. At minimum, a qualitative comparison is needed.

**M3: No user study or developer feedback.**
The paper claims EFIE helps developers "navigate unfamiliar codebases" but provides no evidence. A small user study (5-10 developers) using EFIE on real tasks would strengthen the paper significantly.

### Minor Issues

**m1: Writing quality is good.**
The paper is well-written and the mathematical formalization is clear. The algorithm design is sound.

**m2: Novelty is adequate.**
The combination of PageRank + Louvain + betweenness for code intelligence is novel, even if individual components are not.

**m3: Documentation is excellent.**
The EFIE_ALGORITHM_V2.md and EFIE_RESEARCH.md are thorough and well-structured.

---

## Required Revisions

### Before Re-Review
1. Fix PageRank direction bug (centrality.go)
2. Fix import proximity scoring cap (scorer.go)
3. Benchmark on real repositories (Go stdlib, Kubernetes, VS Code)
4. Construct ground truth dataset (50+ queries)
5. Report Precision@k, Recall@k, MRR, NDCG
6. Increase to 30+ runs per benchmark
7. Add confidence intervals to all measurements
8. Correct all false claims in manuscript
9. Add profiling data explaining performance
10. Add ablation study

### Strongly Recommended
11. Add comparison with Sourcegraph/SCIP
12. Add user study or developer feedback
13. Fix code duplication
14. Implement or remove incremental build claim
15. Add convergence monitoring for PageRank
