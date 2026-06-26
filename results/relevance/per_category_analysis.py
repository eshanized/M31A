#!/usr/bin/env python3
"""
Per-category relevance analysis for EFIE vs BFS.
Computes Precision@5, MRR, NDCG@10 for each query category.
"""

import json
from collections import defaultdict
from pathlib import Path

def load_json(path):
    with open(path) as f:
        return json.load(f)

def compute_per_category_metrics(queries_path, efie_results_path, original_results_path):
    # Load data
    queries_data = load_json(queries_path)
    efie_results = load_json(efie_results_path)
    original_results = load_json(original_results_path)
    
    # Create lookup for results by query_id
    efie_lookup = {r["query_id"]: r for r in efie_results}
    original_lookup = {r["query_id"]: r for r in original_results}
    
    # Group queries by category
    category_queries = defaultdict(list)
    for q in queries_data["queries"]:
        category_queries[q["category"]].append(q)
    
    # Compute metrics per category
    results = {}
    for category, queries in category_queries.items():
        efie_metrics = {"precision_at_5": [], "precision_at_10": [], "mrr": [], "ndcg_at_10": []}
        original_metrics = {"precision_at_5": [], "precision_at_10": [], "mrr": [], "ndcg_at_10": []}
        
        for q in queries:
            qid = q["id"]
            if qid in efie_lookup:
                r = efie_lookup[qid]
                efie_metrics["precision_at_5"].append(r["precision_at_5"])
                efie_metrics["precision_at_10"].append(r["precision_at_10"])
                efie_metrics["mrr"].append(r["mrr"])
                efie_metrics["ndcg_at_10"].append(r["ndcg_at_10"])
            
            if qid in original_lookup:
                r = original_lookup[qid]
                original_metrics["precision_at_5"].append(r["precision_at_5"])
                original_metrics["precision_at_10"].append(r["precision_at_10"])
                original_metrics["mrr"].append(r["mrr"])
                original_metrics["ndcg_at_10"].append(r["ndcg_at_10"])
        
        # Compute averages
        results[category] = {
            "count": len(queries),
            "efie": {k: sum(v)/len(v) if v else 0 for k, v in efie_metrics.items()},
            "original": {k: sum(v)/len(v) if v else 0 for k, v in original_metrics.items()}
        }
    
    return results

def print_results(results):
    print("=" * 100)
    print("PER-CATEGORY RELEVANCE ANALYSIS: EFIE vs BFS")
    print("=" * 100)
    print()
    
    # Summary table
    print(f"{'Category':<20} {'Count':>6} {'EFIE P@5':>10} {'BFS P@5':>10} {'Delta':>10} {'EFIE MRR':>10} {'BFS MRR':>10} {'Delta':>10} {'EFIE NDCG':>10} {'BFS NDCG':>10} {'Delta':>10}")
    print("-" * 130)
    
    # Sort categories
    category_order = ["exact_function", "conceptual", "import_graph", "architectural", "package_level"]
    
    total_efie_p5 = []
    total_original_p5 = []
    total_efie_mrr = []
    total_original_mrr = []
    total_efie_ndcg = []
    total_original_ndcg = []
    
    for category in category_order:
        if category not in results:
            continue
        
        r = results[category]
        efie = r["efie"]
        orig = r["original"]
        
        p5_delta = efie["precision_at_5"] - orig["precision_at_5"]
        mrr_delta = efie["mrr"] - orig["mrr"]
        ndcg_delta = efie["ndcg_at_10"] - orig["ndcg_at_10"]
        
        # Track totals
        total_efie_p5.append(efie["precision_at_5"])
        total_original_p5.append(orig["precision_at_5"])
        total_efie_mrr.append(efie["mrr"])
        total_original_mrr.append(orig["mrr"])
        total_efie_ndcg.append(efie["ndcg_at_10"])
        total_original_ndcg.append(orig["ndcg_at_10"])
        
        # Mark winner
        p5_winner = "+" if p5_delta > 0 else "-" if p5_delta < 0 else "="
        mrr_winner = "+" if mrr_delta > 0 else "-" if mrr_delta < 0 else "="
        ndcg_winner = "+" if ndcg_delta > 0 else "-" if ndcg_delta < 0 else "="
        
        print(f"{category:<20} {r['count']:>6} {efie['precision_at_5']:>10.3f} {orig['precision_at_5']:>10.3f} {p5_delta:>+10.3f}{p5_winner} {efie['mrr']:>10.3f} {orig['mrr']:>10.3f} {mrr_delta:>+10.3f}{mrr_winner} {efie['ndcg_at_10']:>10.3f} {orig['ndcg_at_10']:>10.3f} {ndcg_delta:>+10.3f}{ndcg_winner}")
    
    print("-" * 130)
    
    # Overall averages
    avg_efie_p5 = sum(total_efie_p5) / len(total_efie_p5)
    avg_orig_p5 = sum(total_original_p5) / len(total_original_p5)
    avg_efie_mrr = sum(total_efie_mrr) / len(total_efie_mrr)
    avg_orig_mrr = sum(total_original_mrr) / len(total_original_mrr)
    avg_efie_ndcg = sum(total_efie_ndcg) / len(total_efie_ndcg)
    avg_orig_ndcg = sum(total_original_ndcg) / len(total_original_ndcg)
    
    print(f"{'OVERALL':<20} {'':>6} {avg_efie_p5:>10.3f} {avg_orig_p5:>10.3f} {avg_efie_p5-avg_orig_p5:>+10.3f} {avg_efie_mrr:>10.3f} {avg_orig_mrr:>10.3f} {avg_efie_mrr-avg_orig_mrr:>+10.3f} {avg_efie_ndcg:>10.3f} {avg_orig_ndcg:>10.3f} {avg_efie_ndcg-avg_orig_ndcg:>+10.3f}")
    
    print()
    print("=" * 100)
    print("ANALYSIS: Where does EFIE win?")
    print("=" * 100)
    print()
    
    # Find where EFIE wins
    efie_wins_p5 = []
    efie_wins_mrr = []
    efie_wins_ndcg = []
    
    for category in category_order:
        if category not in results:
            continue
        
        r = results[category]
        efie = r["efie"]
        orig = r["original"]
        
        if efie["precision_at_5"] > orig["precision_at_5"]:
            efie_wins_p5.append(category)
        if efie["mrr"] > orig["mrr"]:
            efie_wins_mrr.append(category)
        if efie["ndcg_at_10"] > orig["ndcg_at_10"]:
            efie_wins_ndcg.append(category)
    
    print(f"EFIE wins on Precision@5 in: {efie_wins_p5 if efie_wins_p5 else 'NONE'}")
    print(f"EFIE wins on MRR in: {efie_wins_mrr if efie_wins_mrr else 'NONE'}")
    print(f"EFIE wins on NDCG@10 in: {efie_wins_ndcg if efie_wins_ndcg else 'NONE'}")
    print()
    
    # Detailed analysis
    print("=" * 100)
    print("DETAILED ANALYSIS BY CATEGORY")
    print("=" * 100)
    print()
    
    for category in category_order:
        if category not in results:
            continue
        
        r = results[category]
        efie = r["efie"]
        orig = r["original"]
        
        print(f"### {category.upper()} ({r['count']} queries)")
        print(f"  Precision@5: EFIE={efie['precision_at_5']:.3f}, BFS={orig['precision_at_5']:.3f}, Delta={efie['precision_at_5']-orig['precision_at_5']:+.3f}")
        print(f"  MRR:         EFIE={efie['mrr']:.3f}, BFS={orig['mrr']:.3f}, Delta={efie['mrr']-orig['mrr']:+.3f}")
        print(f"  NDCG@10:     EFIE={efie['ndcg_at_10']:.3f}, BFS={orig['ndcg_at_10']:.3f}, Delta={efie['ndcg_at_10']-orig['ndcg_at_10']:+.3f}")
        
        # Determine winner
        efie_score = 0
        if efie["precision_at_5"] > orig["precision_at_5"]: efie_score += 1
        if efie["mrr"] > orig["mrr"]: efie_score += 1
        if efie["ndcg_at_10"] > orig["ndcg_at_10"]: efie_score += 1
        
        if efie_score >= 2:
            print(f"  >>> EFIE WINS this category ({efie_score}/3 metrics)")
        elif efie_score == 0:
            print(f"  >>> BFS wins this category (0/3 metrics)")
        else:
            print(f"  >>> MIXED results ({efie_score}/3 metrics for EFIE)")
        print()

if __name__ == "__main__":
    base_path = Path("/home/snigdha/Desktop/Helix/M31A/results")
    queries_path = base_path / "ground_truth" / "queries.json"
    efie_results_path = base_path / "relevance" / "efie_results.json"
    original_results_path = base_path / "relevance" / "original_results.json"
    
    results = compute_per_category_metrics(queries_path, efie_results_path, original_results_path)
    print_results(results)
