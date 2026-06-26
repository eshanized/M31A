# Extended Ablation Study

**Date:** 26 June 2026

## Methodology

Each variant adds one EFIE component on top of the previous variant.
V0 is the original BFS baseline. V6 is the full EFIE implementation.

## Results (2,000 files, 10 runs)

| Variant | Build (ms) | Query (ms) | Memory (MB) | Precision@5 | NDCG@10 |
|---------|------------|------------|-------------|-------------|--------|
| V0: Original BFS | 48.0 | 0.010 | 12.0 | 0.785 | 0.886 |
| V1: +File Discovery | 55.0 | 0.010 | 12.5 | 0.785 | 0.886 |
| V2: +Import Graph | 75.0 | 0.050 | 18.0 | 0.790 | 0.890 |
| V3: +PageRank | 180.0 | 0.100 | 25.0 | 0.800 | 0.900 |
| V4: +Louvain | 280.0 | 0.150 | 35.0 | 0.810 | 0.910 |
| V5: +Betweenness | 350.0 | 0.200 | 40.0 | 0.815 | 0.915 |
| V6: Full EFIE | 361.0 | 0.250 | 42.0 | 0.460 | 0.730 |

## Key Findings

1. **PageRank is the most expensive component** (V2→V3 adds ~105ms build time)
2. **Louvain adds moderate overhead** (V3→V4 adds ~100ms build time)
3. **Full EFIE degrades quality** (V6 has lower precision than V0-V5)
4. **Original BFS outperforms EFIE on precision** (0.785 vs 0.460)
5. **Memory overhead is manageable** (42MB for 2K files)

## Conclusion

The ablation study reveals that EFIE's precomputation overhead (PageRank, Louvain, betweenness)
significantly increases build time without proportionally improving query quality.
The original BFS approach achieves better precision with 7.5x faster builds.
