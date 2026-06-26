package main

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/codeintel"
	"github.com/eshanized/M31A/internal/codeintel/efie"
)

type RealRepoResult struct {
	Repository   string  `json:"repository"`
	FileCount    int     `json:"file_count"`
	Backend      string  `json:"backend"`
	BuildMs      float64 `json:"build_ms"`
	BuildCI95Lo  float64 `json:"build_ci95_lo"`
	BuildCI95Hi  float64 `json:"build_ci95_hi"`
	BuildStdDev  float64 `json:"build_std_dev"`
	QueryMs      float64 `json:"query_ms"`
	QueryCI95Lo  float64 `json:"query_ci95_lo"`
	QueryCI95Hi  float64 `json:"query_ci95_hi"`
	QueryStdDev  float64 `json:"query_std_dev"`
	HeapMB       float64 `json:"heap_mb"`
	Communities  int     `json:"communities"`
	SampleSize   int     `json:"sample_size"`
}

type Stats struct {
	Mean   float64
	StdDev float64
	CI95Lo float64
	CI95Hi float64
}

func computeStats(values []float64) Stats {
	n := len(values)
	if n == 0 {
		return Stats{}
	}

	sum := 0.0
	for _, v := range values {
		sum += v
	}
	mean := sum / float64(n)

	sumSq := 0.0
	for _, v := range values {
		sumSq += (v - mean) * (v - mean)
	}
	stddev := math.Sqrt(sumSq / float64(n-1)) // sample stddev

	// 95% CI using t-distribution approximation
	se := stddev / math.Sqrt(float64(n))
	ci95 := 1.96 * se // approximation for large n

	return Stats{
		Mean:   mean,
		StdDev: stddev,
		CI95Lo: mean - ci95,
		CI95Hi: mean + ci95,
	}
}

func main() {
	fmt.Println("=== EFIE Real Repository Benchmarks ===")
	fmt.Printf("Go: %s, CPUs: %d\n\n", runtime.Version(), runtime.NumCPU())

	os.MkdirAll("results/real_repos", 0o755)

	repos := []struct {
		Name string
		Dir  string
	}{
		{"gin", "/tmp/efie_bench/repos/gin"},
		{"docker", "/tmp/efie_bench/repos/docker"},
		{"go-stdlib", "/tmp/efie_bench/repos/go-stdlib"},
		{"kubernetes", "/tmp/efie_bench/repos/kubernetes"},
	}

	var allResults []RealRepoResult
	runCount := 30

	for _, repo := range repos {
		fmt.Printf("=== Benchmarking: %s ===\n", repo.Name)

		// Count Go files
		fileCount := countGoFiles(repo.Dir)
		fmt.Printf("  Files: %d\n", fileCount)

		// EFIE benchmarks
		fmt.Printf("  Running EFIE (%d runs)...\n", runCount)
		efieBuildTimes := make([]float64, runCount)
		efieQueryTimes := make([]float64, runCount)
		efieHeap := 0.0
		communities := 0

		for i := 0; i < runCount; i++ {
			runtime.GC()
			var m1 runtime.MemStats
			runtime.ReadMemStats(&m1)

			start := time.Now()
			idx := efie.NewEFIEIndex(repo.Dir)
			if err := idx.Build(context.Background()); err != nil {
				fmt.Printf("  EFIE build error: %v\n", err)
				continue
			}
			efieBuildTimes[i] = float64(time.Since(start).Microseconds()) / 1000.0

			var m2 runtime.MemStats
			runtime.ReadMemStats(&m2)
			efieHeap = float64(m2.Alloc) / 1024 / 1024

			// Query benchmark
			targets := getTestTargets(repo.Dir, 3)
			if len(targets) > 0 {
				start = time.Now()
				efie.Query(idx, targets, "HTTP handler implementation", "relevant", 10)
				efieQueryTimes[i] = float64(time.Since(start).Microseconds()) / 1000.0
			}

			if i == 0 {
				// Get community count from first run
				communities = len(idx.Communities())
			}
		}

		efieBuildStats := computeStats(efieBuildTimes)
		efieQueryStats := computeStats(efieQueryTimes)

		// Original benchmarks
		fmt.Printf("  Running Original (%d runs)...\n", runCount)
		origBuildTimes := make([]float64, runCount)
		origQueryTimes := make([]float64, runCount)

		for i := 0; i < runCount; i++ {
			runtime.GC()
			start := time.Now()
			idx := codeintel.NewIndexer(repo.Dir)
			if err := idx.Build(context.Background()); err != nil {
				fmt.Printf("  Original build error: %v\n", err)
				continue
			}
			origBuildTimes[i] = float64(time.Since(start).Microseconds()) / 1000.0

			// Query benchmark
			targets := getTestTargets(repo.Dir, 3)
			if len(targets) > 0 {
				start = time.Now()
				idx.RelevantFiles(targets, "HTTP handler implementation", 10)
				origQueryTimes[i] = float64(time.Since(start).Microseconds()) / 1000.0
			}
		}

		origBuildStats := computeStats(origBuildTimes)
		origQueryStats := computeStats(origQueryTimes)

		// Print results
		fmt.Printf("  EFIE  build: %8.1f ± %5.1f ms (95%% CI: [%.1f, %.1f])\n",
			efieBuildStats.Mean, efieBuildStats.StdDev, efieBuildStats.CI95Lo, efieBuildStats.CI95Hi)
		fmt.Printf("  Orig  build: %8.1f ± %5.1f ms (95%% CI: [%.1f, %.1f])\n",
			origBuildStats.Mean, origBuildStats.StdDev, origBuildStats.CI95Lo, origBuildStats.CI95Hi)
		fmt.Printf("  EFIE  query: %8.1f ± %5.1f ms (95%% CI: [%.1f, %.1f])\n",
			efieQueryStats.Mean, efieQueryStats.StdDev, efieQueryStats.CI95Lo, efieQueryStats.CI95Hi)
		fmt.Printf("  Orig  query: %8.1f ± %5.1f ms (95%% CI: [%.1f, %.1f])\n",
			origQueryStats.Mean, origQueryStats.StdDev, origQueryStats.CI95Lo, origQueryStats.CI95Hi)
		fmt.Printf("  Build ratio: %.1fx, Query ratio: %.1fx\n",
			efieBuildStats.Mean/origBuildStats.Mean, efieQueryStats.Mean/origQueryStats.Mean)
		fmt.Printf("  Communities: %d, Heap: %.1f MB\n\n", communities, efieHeap)

		// Store results
		allResults = append(allResults, RealRepoResult{
			Repository:  repo.Name,
			FileCount:   fileCount,
			Backend:     "efie",
			BuildMs:     efieBuildStats.Mean,
			BuildCI95Lo: efieBuildStats.CI95Lo,
			BuildCI95Hi: efieBuildStats.CI95Hi,
			BuildStdDev: efieBuildStats.StdDev,
			QueryMs:     efieQueryStats.Mean,
			QueryCI95Lo: efieQueryStats.CI95Lo,
			QueryCI95Hi: efieQueryStats.CI95Hi,
			QueryStdDev: efieQueryStats.StdDev,
			HeapMB:      efieHeap,
			Communities: communities,
			SampleSize:  runCount,
		})
		allResults = append(allResults, RealRepoResult{
			Repository:  repo.Name,
			FileCount:   fileCount,
			Backend:     "original",
			BuildMs:     origBuildStats.Mean,
			BuildCI95Lo: origBuildStats.CI95Lo,
			BuildCI95Hi: origBuildStats.CI95Hi,
			BuildStdDev: origBuildStats.StdDev,
			QueryMs:     origQueryStats.Mean,
			QueryCI95Lo: origQueryStats.CI95Lo,
			QueryCI95Hi: origQueryStats.CI95Hi,
			QueryStdDev: origQueryStats.StdDev,
			SampleSize:  runCount,
		})
	}

	// Write results
	resultsJSON, _ := json.MarshalIndent(allResults, "", "  ")
	os.WriteFile("results/real_repos/benchmark_results.json", resultsJSON, 0o644)

	// Write summary table
	writeRealRepoSummary(allResults)

	fmt.Println("Results written to results/real_repos/")
	fmt.Println("Done.")
}

func countGoFiles(dir string) int {
	count := 0
	filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !info.IsDir() && filepath.Ext(path) == ".go" {
			count++
		}
		return nil
	})
	return count
}

func getTestTargets(repoDir string, count int) []string {
	var files []string
	filepath.Walk(repoDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !info.IsDir() && filepath.Ext(path) == ".go" {
			files = append(files, path)
		}
		return nil
	})
	sort.Strings(files)
	if len(files) > count {
		files = files[:count]
	}
	return files
}

func writeRealRepoSummary(results []RealRepoResult) {
	var sb strings.Builder
	sb.WriteString("# Real Repository Benchmark Results\n\n")
	sb.WriteString("**Methodology:** 30 runs per (repository, backend) pair, 95% confidence intervals\n\n")
	sb.WriteString("| Repository | Files | Backend | Build (ms) | 95% CI | Query (ms) | 95% CI | Ratio |\n")
	sb.WriteString("|-----------|-------|---------|------------|--------|------------|--------|-------|\n")

	for i := 0; i < len(results); i += 2 {
		efie := results[i]
		orig := results[i+1]
		buildRatio := efie.BuildMs / orig.BuildMs
		queryRatio := efie.QueryMs / orig.QueryMs
		sb.WriteString(fmt.Sprintf("| %s | %d | EFIE | %.1f ± %.1f | [%.1f, %.1f] | %.1f ± %.1f | [%.1f, %.1f] | %.1fx build, %.1fx query |\n",
			efie.Repository, efie.FileCount,
			efie.BuildMs, efie.BuildStdDev, efie.BuildCI95Lo, efie.BuildCI95Hi,
			efie.QueryMs, efie.QueryStdDev, efie.QueryCI95Lo, efie.QueryCI95Hi,
			buildRatio, queryRatio))
		sb.WriteString(fmt.Sprintf("| | | Original | %.1f ± %.1f | [%.1f, %.1f] | %.1f ± %.1f | [%.1f, %.1f] | | |\n",
			orig.BuildMs, orig.BuildStdDev, orig.BuildCI95Lo, orig.BuildCI95Hi,
			orig.QueryMs, orig.QueryStdDev, orig.QueryCI95Lo, orig.QueryCI95Hi))
	}

	os.WriteFile("results/real_repos/summary.md", []byte(sb.String()), 0o644)
}
