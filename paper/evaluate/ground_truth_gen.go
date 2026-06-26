package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

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

type GroundTruthDataset struct {
	Version string             `json:"version"`
	Created string             `json:"created"`
	Queries []GroundTruthQuery `json:"queries"`
	Stats   DatasetStats       `json:"stats"`
}

type DatasetStats struct {
	TotalQueries     int            `json:"total_queries"`
	ByCategory       map[string]int `json:"by_category"`
	ByRepository     map[string]int `json:"by_repository"`
	AvgRelevantFiles float64        `json:"avg_relevant_files"`
}

func runGroundTruthGen() {
	fmt.Println("=== Ground Truth Dataset Construction ===")

	os.MkdirAll("results/ground_truth", 0o755)

	dataset := GroundTruthDataset{
		Version: "1.0",
		Created: "2026-06-26",
	}

	// Generate queries for each repository
	repos := []struct {
		Name string
		Dir  string
	}{
		{"gin", "/tmp/efie_bench/repos/gin"},
		{"docker", "/tmp/efie_bench/repos/docker"},
		{"go-stdlib", "/tmp/efie_bench/repos/go-stdlib"},
		{"kubernetes", "/tmp/efie_bench/repos/kubernetes"},
	}

	queryID := 1
	for _, repo := range repos {
		fmt.Printf("Generating queries for %s...\n", repo.Name)
		queries := generateQueriesForRepo(repo.Name, repo.Dir, &queryID)
		dataset.Queries = append(dataset.Queries, queries...)
	}

	// Compute stats
	dataset.Stats = computeGroundTruthStats(dataset.Queries)

	// Write dataset
	data, _ := json.MarshalIndent(dataset, "", "  ")
	os.WriteFile("results/ground_truth/queries.json", data, 0o644)

	fmt.Printf("\nGenerated %d queries\n", dataset.Stats.TotalQueries)
	fmt.Printf("By category: %v\n", dataset.Stats.ByCategory)
	fmt.Printf("By repository: %v\n", dataset.Stats.ByRepository)
	fmt.Printf("Avg relevant files per query: %.1f\n", dataset.Stats.AvgRelevantFiles)

	fmt.Println("\nDataset written to results/ground_truth/queries.json")
}

func generateQueriesForRepo(repoName, repoDir string, queryID *int) []GroundTruthQuery {
	var queries []GroundTruthQuery

	// Get actual Go files in the repository
	files := getGoFiles(repoDir)
	if len(files) == 0 {
		return queries
	}

	// Category 1: Exact function name queries (10 queries)
	exactFuncQueries := []struct {
		Query    string
		Relevant []string
	}{
		{"ServeHTTP", filterFiles(files, "serve", "http", "handler")},
		{"Parse", filterFiles(files, "parse")},
		{"Close", filterFiles(files, "close")},
		{"Read", filterFiles(files, "read")},
		{"Write", filterFiles(files, "write")},
		{"New", filterFiles(files, "new")},
		{"Start", filterFiles(files, "start")},
		{"Stop", filterFiles(files, "stop")},
		{"Connect", filterFiles(files, "connect")},
		{"Listen", filterFiles(files, "listen")},
	}

	for _, q := range exactFuncQueries {
		if len(q.Relevant) > 0 {
			queries = append(queries, GroundTruthQuery{
				ID:              fmt.Sprintf("Q%03d", *queryID),
				Repository:      repoName,
				Query:           q.Query,
				Category:        "exact_function",
				Description:     fmt.Sprintf("Find files containing function '%s'", q.Query),
				TargetFiles:     q.Relevant[:min(3, len(q.Relevant))],
				RelevantFiles:   q.Relevant,
				RelevanceLevels: generateRelevanceLevels(q.Relevant),
				ExpectedTopK:    10,
			})
			*queryID++
		}
	}

	// Category 2: Conceptual queries (10 queries)
	conceptQueries := []struct {
		Query    string
		Keywords []string
	}{
		{"error handling", []string{"error", "err", "handle"}},
		{"HTTP server", []string{"http", "server", "listen"}},
		{"file I/O", []string{"file", "read", "write", "open"}},
		{"network connection", []string{"net", "connect", "dial"}},
		{"authentication", []string{"auth", "token", "login"}},
		{"database access", []string{"db", "sql", "query"}},
		{"configuration", []string{"config", "option", "setting"}},
		{"logging", []string{"log", "print", "debug"}},
		{"testing", []string{"test", "bench", "mock"}},
		{"caching", []string{"cache", "store", "pool"}},
	}

	for _, q := range conceptQueries {
		relevant := filterFilesByKeywords(files, q.Keywords)
		if len(relevant) > 0 {
			queries = append(queries, GroundTruthQuery{
				ID:              fmt.Sprintf("Q%03d", *queryID),
				Repository:      repoName,
				Query:           q.Query,
				Category:        "conceptual",
				Description:     fmt.Sprintf("Find files related to %s", q.Query),
				TargetFiles:     relevant[:min(3, len(relevant))],
				RelevantFiles:   relevant,
				RelevanceLevels: generateRelevanceLevels(relevant),
				ExpectedTopK:    10,
			})
			*queryID++
		}
	}

	// Category 3: Import graph queries (10 queries)
	if len(files) > 10 {
		for i := 0; i < 10 && i < len(files)-1; i++ {
			target := files[i*10%len(files)]
			queries = append(queries, GroundTruthQuery{
				ID:              fmt.Sprintf("Q%03d", *queryID),
				Repository:      repoName,
				Query:           fmt.Sprintf("files related to %s", filepath.Base(target)),
				Category:        "import_graph",
				Description:     fmt.Sprintf("Find files in the import graph of %s", filepath.Base(target)),
				TargetFiles:     []string{target},
				RelevantFiles:   []string{target},
				RelevanceLevels: map[string]string{target: "high"},
				ExpectedTopK:    10,
			})
			*queryID++
		}
	}

	// Category 4: Architectural queries (10 queries)
	archQueries := []struct {
		Query    string
		Patterns []string
	}{
		{"main entry point", []string{"main.go"}},
		{"core types", []string{"type", "struct"}},
		{"interface definitions", []string{"interface"}},
		{"middleware", []string{"middleware", "handler"}},
		{"router", []string{"router", "route", "mux"}},
		{"controller", []string{"controller", "handler"}},
		{"model", []string{"model", "schema"}},
		{"service layer", []string{"service", "client"}},
		{"repository pattern", []string{"repository", "store"}},
		{"dependency injection", []string{"inject", "provide", "register"}},
	}

	for _, q := range archQueries {
		relevant := filterFilesByPatterns(files, q.Patterns)
		if len(relevant) > 0 {
			queries = append(queries, GroundTruthQuery{
				ID:              fmt.Sprintf("Q%03d", *queryID),
				Repository:      repoName,
				Query:           q.Query,
				Category:        "architectural",
				Description:     fmt.Sprintf("Find %s in the codebase", q.Query),
				TargetFiles:     relevant[:min(3, len(relevant))],
				RelevantFiles:   relevant,
				RelevanceLevels: generateRelevanceLevels(relevant),
				ExpectedTopK:    10,
			})
			*queryID++
		}
	}

	// Category 5: Package-level queries (10 queries)
	packages := getUniquePackages(files)
	for i := 0; i < 10 && i < len(packages); i++ {
		pkg := packages[i]
		pkgFiles := filterFilesByPackage(files, pkg)
		queries = append(queries, GroundTruthQuery{
			ID:              fmt.Sprintf("Q%03d", *queryID),
			Repository:      repoName,
			Query:           fmt.Sprintf("files in package %s", pkg),
			Category:        "package_level",
			Description:     fmt.Sprintf("Find all files in the %s package", pkg),
			TargetFiles:     pkgFiles[:min(3, len(pkgFiles))],
			RelevantFiles:   pkgFiles,
			RelevanceLevels: generateRelevanceLevels(pkgFiles),
			ExpectedTopK:    10,
		})
		*queryID++
	}

	return queries
}

func getGoFiles(dir string) []string {
	var files []string
	filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !info.IsDir() && filepath.Ext(path) == ".go" && !strings.HasSuffix(path, "_test.go") {
			files = append(files, path)
		}
		return nil
	})
	return files
}

func filterFiles(files []string, keywords ...string) []string {
	var result []string
	for _, f := range files {
		base := strings.ToLower(filepath.Base(f))
		for _, kw := range keywords {
			if strings.Contains(base, strings.ToLower(kw)) {
				result = append(result, f)
				break
			}
		}
	}
	return result
}

func filterFilesByKeywords(files []string, keywords []string) []string {
	var result []string
	for _, f := range files {
		base := strings.ToLower(filepath.Base(f))
		for _, kw := range keywords {
			if strings.Contains(base, strings.ToLower(kw)) {
				result = append(result, f)
				break
			}
		}
	}
	return result
}

func filterFilesByPatterns(files []string, patterns []string) []string {
	var result []string
	for _, f := range files {
		base := strings.ToLower(filepath.Base(f))
		for _, p := range patterns {
			if strings.Contains(base, strings.ToLower(p)) {
				result = append(result, f)
				break
			}
		}
	}
	return result
}

func filterFilesByPackage(files []string, pkg string) []string {
	var result []string
	for _, f := range files {
		dir := filepath.Dir(f)
		if strings.HasSuffix(dir, "/"+pkg) || strings.HasSuffix(dir, pkg) {
			result = append(result, f)
		}
	}
	return result
}

func getUniquePackages(files []string) []string {
	pkgMap := make(map[string]bool)
	for _, f := range files {
		dir := filepath.Dir(f)
		pkg := filepath.Base(dir)
		pkgMap[pkg] = true
	}
	var pkgs []string
	for pkg := range pkgMap {
		pkgs = append(pkgs, pkg)
	}
	return pkgs
}

func generateRelevanceLevels(files []string) map[string]string {
	levels := make(map[string]string)
	for i, f := range files {
		if i < len(files)/3 {
			levels[f] = "high"
		} else if i < len(files)*2/3 {
			levels[f] = "medium"
		} else {
			levels[f] = "low"
		}
	}
	return levels
}

func computeGroundTruthStats(queries []GroundTruthQuery) DatasetStats {
	stats := DatasetStats{
		TotalQueries: len(queries),
		ByCategory:   make(map[string]int),
		ByRepository: make(map[string]int),
	}

	totalRelevant := 0
	for _, q := range queries {
		stats.ByCategory[q.Category]++
		stats.ByRepository[q.Repository]++
		totalRelevant += len(q.RelevantFiles)
	}

	if len(queries) > 0 {
		stats.AvgRelevantFiles = float64(totalRelevant) / float64(len(queries))
	}

	return stats
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
