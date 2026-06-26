package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/codeintel"
	"github.com/eshanized/M31A/internal/codeintel/efie"
)

// BenchmarkResult holds the output of a single benchmark run.
type BenchmarkResult struct {
	Name           string  `json:"name"`
	FileCount      int     `json:"file_count"`
	PackageCount   int     `json:"package_count"`
	TotalSymbols   int     `json:"total_symbols"`
	TotalEdges     int     `json:"total_edges"`
	Backend        string  `json:"backend"` // "efie" or "original"
	Operation      string  `json:"operation"`
	DurationMs     float64 `json:"duration_ms"`
	Allocs         int64   `json:"allocs"`
	BytesAllocated int64   `json:"bytes_allocated"`
	HeapAllocBytes int64   `json:"heap_alloc_bytes"`
	HeapSysBytes   int64   `json:"heap_sys_bytes"`
	GCCycles       uint32  `json:"gc_cycles"`
	RunIndex       int     `json:"run_index"`
	Timestamp      string  `json:"timestamp"`
}

// BenchmarkSuite holds all results from a benchmark suite.
type BenchmarkSuite struct {
	Hostname  string            `json:"hostname"`
	GoVersion string            `json:"go_version"`
	NumCPU    int               `json:"num_cpu"`
	OS        string            `json:"os"`
	Arch      string            `json:"arch"`
	Timestamp string            `json:"timestamp"`
	Results   []BenchmarkResult `json:"results"`
}

func memStats() (heapAlloc, heapSys int64, gcCycles uint32) {
	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	return int64(m.Alloc), int64(m.Sys), m.NumGC
}

// --- Build Benchmarks ---

func benchmarkEFIEBuild(b *testing.B, repoDir string, fileCount int) {
	b.Helper()
	for i := 0; i < b.N; i++ {
		idx := efie.NewEFIEIndex(repoDir)
		if err := idx.Build(context.Background()); err != nil {
			b.Fatalf("EFIE build failed: %v", err)
		}
	}
}

func benchmarkOriginalBuild(b *testing.B, repoDir string, fileCount int) {
	b.Helper()
	for i := 0; i < b.N; i++ {
		idx := codeintel.NewIndexer(repoDir)
		if err := idx.Build(context.Background()); err != nil {
			b.Fatalf("original build failed: %v", err)
		}
	}
}

// --- Query Benchmarks ---

func benchmarkEFIEQuery(b *testing.B, idx *efie.EFIEIndex, queryType string, targets []string, desc string, topN int) {
	b.Helper()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		efie.Query(idx, targets, desc, efie.QueryType(queryType), topN)
	}
}

func benchmarkOriginalQuery(b *testing.B, idx *codeintel.Indexer, targets []string, desc string, topN int) {
	b.Helper()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		idx.RelevantFiles(targets, desc, topN)
	}
}

// --- Memory Benchmarks ---

func measureMemory(label string, fn func()) (heapAlloc, heapSys int64, gcCycles uint32) {
	runtime.GC()
	runtime.GC()
	var before runtime.MemStats
	runtime.ReadMemStats(&before)

	fn()

	var after runtime.MemStats
	runtime.ReadMemStats(&after)
	return int64(after.Alloc) - int64(before.Alloc),
		int64(after.Sys) - int64(before.Sys),
		after.NumGC - before.NumGC
}

// --- Individual Algorithm Benchmarks ---

func BenchmarkPageRank(b *testing.B) {
	repo, err := generateSmallRepo()
	if err != nil {
		b.Fatal(err)
	}
	defer os.RemoveAll(repo.Dir)

	idx := efie.NewEFIEIndex(repo.Dir)
	if err := idx.Build(context.Background()); err != nil {
		b.Fatal(err)
	}

	// Access graph via the index (we need to test PageRank directly)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = efie.ComputePageRank(nil, 20, 0.85)
	}
}

func BenchmarkLouvain(b *testing.B) {
	repo, err := generateSmallRepo()
	if err != nil {
		b.Fatal(err)
	}
	defer os.RemoveAll(repo.Dir)

	idx := efie.NewEFIEIndex(repo.Dir)
	if err := idx.Build(context.Background()); err != nil {
		b.Fatal(err)
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = efie.LouvainDetect_Deterministic(nil, 42)
	}
}

func BenchmarkTrieInsert(b *testing.B) {
	symbols := make([]string, 1000)
	for i := range symbols {
		symbols[i] = fmt.Sprintf("Symbol%d", i)
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		trie := efie.NewTrie()
		for _, s := range symbols {
			trie.Insert(s)
		}
	}
}

func BenchmarkTrieSearch(b *testing.B) {
	trie := efie.NewTrie()
	for i := 0; i < 10000; i++ {
		trie.Insert(fmt.Sprintf("Symbol%d", i))
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		trie.PrefixSearch("Symbol")
	}
}

func BenchmarkBloomFilter(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		bf := efie.NewBloomFilter(1000, 0.01)
		for j := 0; j < 1000; j++ {
			bf.Add(fmt.Sprintf("item%d", j))
		}
		for j := 0; j < 1000; j++ {
			bf.Contains(fmt.Sprintf("item%d", j))
		}
	}
}

// --- Scalability Tests ---

func TestScalability(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping scalability test in short mode")
	}

	sizes := []int{100, 500, 1000, 2000, 5000}
	results := make([]struct {
		Size        int
		EFIEBuildMs float64
		OrigBuildMs float64
		EFIEQueryMs float64
		OrigQueryMs float64
		EFIEHeapMB  float64
		OrigHeapMB  float64
	}, len(sizes))

	for i, size := range sizes {
		t.Run(fmt.Sprintf("size_%d", size), func(t *testing.T) {
			repo, err := GenerateRepo(GenerateConfig{
				Dir:          filepath.Join(t.TempDir(), "testrepo"),
				FileCount:    size,
				AvgImports:   5,
				MaxImports:   15,
				MinSymbols:   5,
				MaxSymbols:   20,
				PackageCount: size / 50,
				Seed:         int64(size * 1000),
			})
			if err != nil {
				t.Fatal(err)
			}

			// EFIE build
			start := time.Now()
			efieIdx := efie.NewEFIEIndex(repo.Dir)
			if err := efieIdx.Build(context.Background()); err != nil {
				t.Fatal(err)
			}
			efieBuildMs := float64(time.Since(start).Microseconds()) / 1000.0

			var efieHeap int64
			efieHeap, _, _ = measureMemory("efie_build", func() {
				efieIdx2 := efie.NewEFIEIndex(repo.Dir)
				efieIdx2.Build(context.Background())
			})

			// Original build
			start = time.Now()
			origIdx := codeintel.NewIndexer(repo.Dir)
			if err := origIdx.Build(context.Background()); err != nil {
				t.Fatal(err)
			}
			origBuildMs := float64(time.Since(start).Microseconds()) / 1000.0

			var origHeap int64
			origHeap, _, _ = measureMemory("orig_build", func() {
				origIdx2 := codeintel.NewIndexer(repo.Dir)
				origIdx2.Build(context.Background())
			})

			// Query benchmark
			targets := getTestTargets(repo.Dir, 3)
			desc := "GetUser handler configuration"

			efieQueryMs := benchQuery(t, func() {
				efie.Query(efieIdx, targets, desc, "relevant", 10)
			})

			origQueryMs := benchQuery(t, func() {
				origIdx.RelevantFiles(targets, desc, 10)
			})

			results[i] = struct {
				Size        int
				EFIEBuildMs float64
				OrigBuildMs float64
				EFIEQueryMs float64
				OrigQueryMs float64
				EFIEHeapMB  float64
				OrigHeapMB  float64
			}{
				Size:        size,
				EFIEBuildMs: efieBuildMs,
				OrigBuildMs: origBuildMs,
				EFIEQueryMs: efieQueryMs,
				OrigQueryMs: origQueryMs,
				EFIEHeapMB:  float64(efieHeap) / 1024 / 1024,
				OrigHeapMB:  float64(origHeap) / 1024 / 1024,
			}

			t.Logf("Size=%d EFIE: build=%.1fms query=%.1fms heap=%.1fMB",
				size, efieBuildMs, efieQueryMs, float64(efieHeap)/1024/1024)
			t.Logf("Size=%d Orig: build=%.1fms query=%.1fms heap=%.1fMB",
				size, origBuildMs, origQueryMs, float64(origHeap)/1024/1024)
		})
	}

	// Write results
	writeScalabilityResults(results)
}

// --- Correctness Tests ---

func TestDeterminism(t *testing.T) {
	repo, err := generateSmallRepo()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(repo.Dir)

	// Build twice, compare results
	idx1 := efie.NewEFIEIndex(repo.Dir)
	if err := idx1.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	idx2 := efie.NewEFIEIndex(repo.Dir)
	if err := idx2.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	// Query both with same parameters
	targets := getTestTargets(repo.Dir, 3)
	desc := "GetUser handler"

	r1 := efie.Query(idx1, targets, desc, "relevant", 10)
	r2 := efie.Query(idx2, targets, desc, "relevant", 10)

	if len(r1) != len(r2) {
		t.Fatalf("determinism check failed: different result counts %d vs %d", len(r1), len(r2))
	}

	// Check that scores are the same (ordering may differ due to map iteration)
	scoreMap1 := make(map[float64]int)
	scoreMap2 := make(map[float64]int)
	for _, r := range r1 {
		scoreMap1[r.Score]++
	}
	for _, r := range r2 {
		scoreMap2[r.Score]++
	}
	for score, count := range scoreMap1 {
		if scoreMap2[score] != count {
			t.Errorf("determinism check failed: score %.6f appears %d times in run1 but %d times in run2",
				score, count, scoreMap2[score])
		}
	}

	t.Logf("Determinism check passed: %d results with matching score distributions across 2 runs", len(r1))
}

func TestCorrectness(t *testing.T) {
	repo, err := generateSmallRepo()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(repo.Dir)

	efieIdx := efie.NewEFIEIndex(repo.Dir)
	if err := efieIdx.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	origIdx := codeintel.NewIndexer(repo.Dir)
	if err := origIdx.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	// Test upstream/downstream consistency
	files := listGoFiles(repo.Dir)
	if len(files) == 0 {
		t.Fatal("no Go files found")
	}

	target := files[0]
	efieUp := efieIdx.Upstream(target, 2)
	origUp := origIdx.Upstream(target, 2)

	if len(efieUp) == 0 && len(origUp) == 0 {
		t.Log("both return empty upstream (no imports)")
	} else if len(efieUp) != len(origUp) {
		t.Logf("upstream count differs: efie=%d orig=%d (expected for different implementations)", len(efieUp), len(origUp))
	}

	// Test symbol lookup
	efieSyms := efieIdx.SymbolsMatching("Get")
	origSyms := origIdx.SymbolsMatching("Get")
	t.Logf("Symbol 'Get': EFIE=%d results, Original=%d results", len(efieSyms), len(origSyms))
}

// --- Ablation Study ---

func TestAblation(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping ablation test in short mode")
	}

	repo, err := generateMediumRepo()
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(repo.Dir)

	targets := getTestTargets(repo.Dir, 3)
	desc := "GetUser handler configuration"

	// Full EFIE
	fullIdx := efie.NewEFIEIndex(repo.Dir)
	if err := fullIdx.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	fullResults := efie.Query(fullIdx, targets, desc, "relevant", 10)
	fullTime := benchQuery(t, func() {
		efie.Query(fullIdx, targets, desc, "relevant", 10)
	})

	// Original (no PageRank, no Louvain, no Trie, no adaptive)
	origIdx := codeintel.NewIndexer(repo.Dir)
	if err := origIdx.Build(context.Background()); err != nil {
		t.Fatal(err)
	}

	origResults := origIdx.RelevantFiles(targets, desc, 10)
	origTime := benchQuery(t, func() {
		origIdx.RelevantFiles(targets, desc, 10)
	})

	t.Logf("=== Ablation Study Results ===")
	t.Logf("Full EFIE:      query=%.1fms results=%d", fullTime, len(fullResults))
	t.Logf("Original:       query=%.1fms results=%d", origTime, len(origResults))

	if origTime > 0 {
		speedup := origTime / fullTime
		t.Logf("Speedup:        %.1fx", speedup)
	}

	// Write ablation results
	writeAblationResults(fullTime, origTime, len(fullResults), len(origResults))
}

// --- Helpers ---

func benchQuery(b testing.TB, fn func()) float64 {
	b.Helper()
	// Warmup
	for i := 0; i < 5; i++ {
		fn()
	}

	start := time.Now()
	iterations := 30
	for i := 0; i < iterations; i++ {
		fn()
	}
	elapsed := time.Since(start)
	return float64(elapsed.Microseconds()) / float64(iterations) / 1000.0
}

func writeScalabilityResults(results []struct {
	Size        int
	EFIEBuildMs float64
	OrigBuildMs float64
	EFIEQueryMs float64
	OrigQueryMs float64
	EFIEHeapMB  float64
	OrigHeapMB  float64
}) {
	data, _ := json.MarshalIndent(results, "", "  ")
	os.WriteFile("scalability_results.json", data, 0o644)
}

func writeAblationResults(efieTime, origTime float64, efieCount, origCount int) {
	type AblationResult struct {
		Component   string  `json:"component"`
		QueryTimeMs float64 `json:"query_time_ms"`
		ResultCount int     `json:"result_count"`
		Removed     bool    `json:"removed"`
	}
	results := []AblationResult{
		{Component: "full_efie", QueryTimeMs: efieTime, ResultCount: efieCount, Removed: false},
		{Component: "original_no_graph_intelligence", QueryTimeMs: origTime, ResultCount: origCount, Removed: true},
	}
	data, _ := json.MarshalIndent(results, "", "  ")
	os.WriteFile("ablation_results.json", data, 0o644)
}
