package main

import (
	"os"
	"path/filepath"
	"sort"
	"time"
)

// generateSmallRepo creates a 200-file test repository.
func generateSmallRepo() (*GeneratedRepo, error) {
	dir, err := os.MkdirTemp("", "efie_bench_small_*")
	if err != nil {
		return nil, err
	}
	return GenerateRepo(GenerateConfig{
		Dir:          dir,
		FileCount:    200,
		AvgImports:   5,
		MaxImports:   10,
		MinSymbols:   5,
		MaxSymbols:   15,
		PackageCount: 10,
		Seed:         42,
	})
}

// generateMediumRepo creates a 1000-file test repository.
func generateMediumRepo() (*GeneratedRepo, error) {
	dir, err := os.MkdirTemp("", "efie_bench_medium_*")
	if err != nil {
		return nil, err
	}
	return GenerateRepo(GenerateConfig{
		Dir:          dir,
		FileCount:    1000,
		AvgImports:   5,
		MaxImports:   15,
		MinSymbols:   5,
		MaxSymbols:   20,
		PackageCount: 20,
		Seed:         42,
	})
}

// getTestTargets returns the first count Go files from the repository.
func getTestTargets(repoDir string, count int) []string {
	files := listGoFiles(repoDir)
	if len(files) < count {
		count = len(files)
	}
	return files[:count]
}

// listGoFiles returns all .go files in a directory tree.
func listGoFiles(dir string) []string {
	var files []string
	filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !info.IsDir() && filepath.Ext(path) == ".go" {
			files = append(files, path)
		}
		return nil
	})
	sort.Strings(files)
	return files
}

// benchQueryMS benchmarks a function and returns average time in milliseconds.
func benchQueryMS(fn func()) float64 {
	for i := 0; i < 5; i++ {
		fn()
	}
	start := time.Now()
	n := 30
	for i := 0; i < n; i++ {
		fn()
	}
	return float64(time.Since(start).Microseconds()) / float64(n) / 1000.0
}
