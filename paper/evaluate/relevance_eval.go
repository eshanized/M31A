package main

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"sort"
	"strings"

	"github.com/eshanized/M31A/internal/codeintel"
	"github.com/eshanized/M31A/internal/codeintel/efie"
)

type QueryResult struct {
	QueryID        string             `json:"query_id"`
	Repository     string             `json:"repository"`
	Backend        string             `json:"backend"`
	PrecisionAt5   float64            `json:"precision_at_5"`
	PrecisionAt10  float64            `json:"precision_at_10"`
	RecallAt5      float64            `json:"recall_at_5"`
	RecallAt10     float64            `json:"recall_at_10"`
	MRR            float64            `json:"mrr"`
	NDCGAt10       float64            `json:"ndcg_at_10"`
	ReturnedFiles  int                `json:"returned_files"`
	RelevantFiles  int                `json:"relevant_files"`
}

type RelevanceStats struct {
	Backend           string  `json:"backend"`
	MeanPrecisionAt5  float64 `json:"mean_precision_at_5"`
	MeanPrecisionAt10 float64 `json:"mean_precision_at_10"`
	MeanRecallAt5     float64 `json:"mean_recall_at_5"`
	MeanRecallAt10    float64 `json:"mean_recall_at_10"`
	MeanMRR           float64 `json:"mean_mrr"`
	MeanNDCGAt10      float64 `json:"mean_ndcg_at_10"`
	QueryCount        int     `json:"query_count"`
}

type GroundTruthQuery struct {
	ID              string            `json:"id"`
	Repository      string            `json:"repository"`
	Query           string            `json:"query"`
	Category        string            `json:"category"`
	Description     string            `json:"description"`
	TargetFiles     []string          `json:"target_files"`
	RelevantFiles   []string          `json:"relevant_files"`
	RelevanceLevels map[string]string `json:"relevance_levels"`
	ExpectedTopK    int               `json:"expected_top_k"`
}

func main() {
	fmt.Println("=== EFIE Relevance Quality Evaluation ===")

	os.MkdirAll("results/relevance", 0o755)

	// Load ground truth
	var dataset struct {
		Queries []GroundTruthQuery `json:"queries"`
	}
	data, err := os.ReadFile("results/ground_truth/queries.json")
	if err != nil {
		fmt.Printf("Error loading ground truth: %v\n", err)
		return
	}
	json.Unmarshal(data, &dataset)

	fmt.Printf("Loaded %d queries\n", len(dataset.Queries))

	// Run evaluation for each backend
	efieResults := evaluateBackend("efie", dataset.Queries)
	origResults := evaluateBackend("original", dataset.Queries)

	// Compute aggregate stats
	efieStats := computeAggregateStats("efie", efieResults)
	origStats := computeAggregateStats("original", origResults)

	// Print results
	fmt.Println("\n=== Results ===")
	printStats("EFIE", efieStats)
	printStats("Original", origStats)

	// Write results
	resultsJSON, _ := json.MarshalIndent(efieResults, "", "  ")
	os.WriteFile("results/relevance/efie_results.json", resultsJSON, 0o644)

	resultsJSON, _ = json.MarshalIndent(origResults, "", "  ")
	os.WriteFile("results/relevance/original_results.json", resultsJSON, 0o644)

	statsJSON, _ := json.MarshalIndent([]RelevanceStats{efieStats, origStats}, "", "  ")
	os.WriteFile("results/relevance/aggregate_stats.json", statsJSON, 0o644)

	// Write summary table
 writeSummaryTable(efieResults, origResults, efieStats, origStats)

	fmt.Println("\nResults written to results/relevance/")
}

func evaluateBackend(backend string, queries []GroundTruthQuery) []QueryResult {
	var results []QueryResult

	repoDirs := map[string]string{
		"gin":        "/tmp/efie_bench/repos/gin",
		"docker":     "/tmp/efie_bench/repos/docker",
		"go-stdlib":  "/tmp/efie_bench/repos/go-stdlib",
		"kubernetes": "/tmp/efie_bench/repos/kubernetes",
	}

	// Build indexes
	indexes := make(map[string]interface{})
	for repo, dir := range repoDirs {
		fmt.Printf("Building %s index for %s...\n", backend, repo)
		if backend == "efie" {
			idx := efie.NewEFIEIndex(dir)
			idx.Build(context.Background())
			indexes[repo] = idx
		} else {
			idx := codeintel.NewIndexer(dir)
			idx.Build(context.Background())
			indexes[repo] = idx
		}
	}

	// Evaluate each query
	for i, q := range queries {
		if (i+1)%10 == 0 {
			fmt.Printf("  Evaluating query %d/%d...\n", i+1, len(queries))
		}

		idx := indexes[q.Repository]
		if idx == nil {
			continue
		}

		var returnedFiles []string
		if backend == "efie" {
			efieIdx := idx.(*efie.EFIEIndex)
			scored := efie.Query(efieIdx, q.TargetFiles, q.Description, "relevant", 10)
			for _, s := range scored {
				returnedFiles = append(returnedFiles, s.Path)
			}
		} else {
			origIdx := idx.(*codeintel.Indexer)
			scored := origIdx.RelevantFiles(q.TargetFiles, q.Description, 10)
			for _, s := range scored {
				returnedFiles = append(returnedFiles, s.Path)
			}
		}

		// Compute metrics
		relevantSet := make(map[string]bool)
		for _, f := range q.RelevantFiles {
			relevantSet[f] = true
		}

		precisionAt5 := computePrecisionAtK(returnedFiles, relevantSet, 5)
		precisionAt10 := computePrecisionAtK(returnedFiles, relevantSet, 10)
		recallAt5 := computeRecallAtK(returnedFiles, relevantSet, 5)
		recallAt10 := computeRecallAtK(returnedFiles, relevantSet, 10)
		mrr := computeMRR(returnedFiles, relevantSet)
		ndcgAt10 := computeNDCGAtK(returnedFiles, q.RelevanceLevels, 10)

		results = append(results, QueryResult{
			QueryID:       q.ID,
			Repository:    q.Repository,
			Backend:       backend,
			PrecisionAt5:  precisionAt5,
			PrecisionAt10: precisionAt10,
			RecallAt5:     recallAt5,
			RecallAt10:    recallAt10,
			MRR:           mrr,
			NDCGAt10:      ndcgAt10,
			ReturnedFiles: len(returnedFiles),
			RelevantFiles: len(q.RelevantFiles),
		})
	}

	return results
}

func computePrecisionAtK(returned []string, relevant map[string]bool, k int) float64 {
	if k > len(returned) {
		k = len(returned)
	}
	if k == 0 {
		return 0
	}

	relevantCount := 0
	for i := 0; i < k; i++ {
		if relevant[returned[i]] {
			relevantCount++
		}
	}
	return float64(relevantCount) / float64(k)
}

func computeRecallAtK(returned []string, relevant map[string]bool, k int) float64 {
	if len(relevant) == 0 {
		return 0
	}
	if k > len(returned) {
		k = len(returned)
	}

	relevantCount := 0
	for i := 0; i < k; i++ {
		if relevant[returned[i]] {
			relevantCount++
		}
	}
	return float64(relevantCount) / float64(len(relevant))
}

func computeMRR(returned []string, relevant map[string]bool) float64 {
	for i, f := range returned {
		if relevant[f] {
			return 1.0 / float64(i+1)
		}
	}
	return 0
}

func computeNDCGAtK(returned []string, relevanceLevels map[string]string, k int) float64 {
	if k > len(returned) {
		k = len(returned)
	}
	if k == 0 {
		return 0
	}

	// Compute DCG
	dcg := 0.0
	for i := 0; i < k; i++ {
		rel := 0.0
		if level, ok := relevanceLevels[returned[i]]; ok {
			switch level {
			case "high":
				rel = 1.0
			case "medium":
				rel = 0.5
			case "low":
				rel = 0.25
			}
		}
		dcg += rel / math.Log2(float64(i+2))
	}

	// Compute ideal DCG
	idealRels := make([]float64, 0)
	for _, level := range relevanceLevels {
		switch level {
		case "high":
			idealRels = append(idealRels, 1.0)
		case "medium":
			idealRels = append(idealRels, 0.5)
		case "low":
			idealRels = append(idealRels, 0.25)
		}
	}
	sort.Sort(sort.Reverse(sort.Float64Slice(idealRels)))

	idcg := 0.0
	for i := 0; i < k && i < len(idealRels); i++ {
		idcg += idealRels[i] / math.Log2(float64(i+2))
	}

	if idcg == 0 {
		return 0
	}
	return dcg / idcg
}

func computeAggregateStats(backend string, results []QueryResult) RelevanceStats {
	stats := RelevanceStats{
		Backend:     backend,
		QueryCount: len(results),
	}

	if len(results) == 0 {
		return stats
	}

	sumP5, sumP10, sumR5, sumR10, sumMRR, sumNDCG := 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
	for _, r := range results {
		sumP5 += r.PrecisionAt5
		sumP10 += r.PrecisionAt10
		sumR5 += r.RecallAt5
		sumR10 += r.RecallAt10
		sumMRR += r.MRR
		sumNDCG += r.NDCGAt10
	}

	n := float64(len(results))
	stats.MeanPrecisionAt5 = sumP5 / n
	stats.MeanPrecisionAt10 = sumP10 / n
	stats.MeanRecallAt5 = sumR5 / n
	stats.MeanRecallAt10 = sumR10 / n
	stats.MeanMRR = sumMRR / n
	stats.MeanNDCGAt10 = sumNDCG / n

	return stats
}

func printStats(backend string, stats RelevanceStats) {
	fmt.Printf("\n%s Relevance Stats (%d queries):\n", backend, stats.QueryCount)
	fmt.Printf("  Precision@5:  %.3f\n", stats.MeanPrecisionAt5)
	fmt.Printf("  Precision@10: %.3f\n", stats.MeanPrecisionAt10)
	fmt.Printf("  Recall@5:     %.3f\n", stats.MeanRecallAt5)
	fmt.Printf("  Recall@10:    %.3f\n", stats.MeanRecallAt10)
	fmt.Printf("  MRR:          %.3f\n", stats.MeanMRR)
	fmt.Printf("  NDCG@10:      %.3f\n", stats.MeanNDCGAt10)
}

func writeSummaryTable(efieResults, origResults []QueryResult, efieStats, origStats RelevanceStats) {
	var sb strings.Builder
	sb.WriteString("# Relevance Quality Evaluation Results\n\n")
	sb.WriteString("**Methodology:** 176 queries across 4 repositories, 5 categories\n\n")
	sb.WriteString("## Aggregate Metrics\n\n")
	sb.WriteString("| Metric | EFIE | Original | Delta |\n")
	sb.WriteString("|--------|------|----------|-------|\n")
	sb.WriteString(fmt.Sprintf("| Precision@5 | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanPrecisionAt5, origStats.MeanPrecisionAt5,
		efieStats.MeanPrecisionAt5-origStats.MeanPrecisionAt5))
	sb.WriteString(fmt.Sprintf("| Precision@10 | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanPrecisionAt10, origStats.MeanPrecisionAt10,
		efieStats.MeanPrecisionAt10-origStats.MeanPrecisionAt10))
	sb.WriteString(fmt.Sprintf("| Recall@5 | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanRecallAt5, origStats.MeanRecallAt5,
		efieStats.MeanRecallAt5-origStats.MeanRecallAt5))
	sb.WriteString(fmt.Sprintf("| Recall@10 | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanRecallAt10, origStats.MeanRecallAt10,
		efieStats.MeanRecallAt10-origStats.MeanRecallAt10))
	sb.WriteString(fmt.Sprintf("| MRR | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanMRR, origStats.MeanMRR,
		efieStats.MeanMRR-origStats.MeanMRR))
	sb.WriteString(fmt.Sprintf("| NDCG@10 | %.3f | %.3f | %+.3f |\n",
		efieStats.MeanNDCGAt10, origStats.MeanNDCGAt10,
		efieStats.MeanNDCGAt10-origStats.MeanNDCGAt10))

	os.WriteFile("results/relevance/summary.md", []byte(sb.String()), 0o644)
}
