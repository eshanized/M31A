// Package evaluate provides automated benchmarking for EFIE.
//
// Usage:
//
//	cd paper/evaluate
//	go run ./...                  # full evaluation
//	go test -bench=. -benchmem   # quick benchmarks
package main

import (
	"fmt"
	"math/rand"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// GenerateConfig controls the synthetic codebase generator.
type GenerateConfig struct {
	Dir         string
	FileCount   int
	AvgImports  int   // average imports per file
	MaxImports  int   // max imports per file
	MinSymbols  int   // min symbols per file
	MaxSymbols  int   // max symbols per file
	PackageCount int  // number of packages (directories)
	Seed        int64
}

// GeneratedRepo holds metadata about a generated repository.
type GeneratedRepo struct {
	Dir         string
	FileCount   int
	TotalSymbols int
	TotalEdges  int
	PackageCount int
	Languages   map[string]int
}

// GenerateRepo creates a synthetic codebase of the given size.
func GenerateRepo(cfg GenerateConfig) (*GeneratedRepo, error) {
	rng := rand.New(rand.NewSource(cfg.Seed))

	if err := os.MkdirAll(cfg.Dir, 0o755); err != nil {
		return nil, fmt.Errorf("mkdir: %w", err)
	}

	repo := &GeneratedRepo{
		Dir:       cfg.Dir,
		Languages: make(map[string]int),
	}

	// Create packages
	packages := make([]string, cfg.PackageCount)
	for i := 0; i < cfg.PackageCount; i++ {
		pkg := filepath.Join(cfg.Dir, fmt.Sprintf("pkg%d", i))
		if err := os.MkdirAll(pkg, 0o755); err != nil {
			return nil, err
		}
		packages[i] = pkg
	}

	// Generate files
	allPaths := make([]string, 0, cfg.FileCount)
	for i := 0; i < cfg.FileCount; i++ {
		pkg := packages[i%len(packages)]
		ext := []string{".go", ".go", ".go", ".ts", ".py", ".rs"}[rng.Intn(6)]
		name := fmt.Sprintf("file%d%s", i, ext)
		path := filepath.Join(pkg, name)

		symbols := generateSymbols(rng, cfg.MinSymbols, cfg.MaxSymbols)
		imports := generateImports(rng, packages, cfg.AvgImports, cfg.MaxImports, path)

		if err := writeSourceFile(path, symbols, imports, ext); err != nil {
			return nil, err
		}

		allPaths = append(allPaths, path)
		repo.TotalSymbols += len(symbols)
		repo.Languages[ext]++
	}

	repo.FileCount = len(allPaths)
	repo.PackageCount = cfg.PackageCount
	repo.TotalEdges = cfg.FileCount * cfg.AvgImports

	return repo, nil
}

func generateSymbols(rng *rand.Rand, min, max int) []string {
	count := min + rng.Intn(max-min+1)
	symbols := make([]string, count)
	prefixes := []string{"Get", "Set", "New", "Is", "Has", "Can", "Make", "Run", "Process", "Handle"}
	suffixes := []string{"User", "Config", "Handler", "Store", "Cache", "Manager", "Service", "Error", "Result", "Request", "Response", "Context", "State", "Options", "Event"}
	for i := 0; i < count; i++ {
		symbols[i] = prefixes[rng.Intn(len(prefixes))] + suffixes[rng.Intn(len(suffixes))]
	}
	return symbols
}

func generateImports(rng *rand.Rand, packages []string, avg, max int, self string) []string {
	count := rng.Intn(max) + 1
	if count > avg*2 {
		count = avg * 2
	}
	imports := make([]string, 0, count)
	used := make(map[string]bool)
	for i := 0; i < count; i++ {
		pkg := packages[rng.Intn(len(packages))]
		if pkg == filepath.Dir(self) {
			continue
		}
		if used[pkg] {
			continue
		}
		used[pkg] = true
		imports = append(imports, pkg)
	}
	return imports
}

func writeSourceFile(path string, symbols, imports []string, ext string) error {
	var sb strings.Builder

	// Write imports
	if ext == ".go" {
		if len(imports) > 0 {
			sb.WriteString("package main\n\nimport (\n")
			for _, imp := range imports {
				sb.WriteString(fmt.Sprintf("\t\"%s\"\n", imp))
			}
			sb.WriteString(")\n\n")
		} else {
			sb.WriteString("package main\n\n")
		}
		for _, sym := range symbols {
			sb.WriteString(fmt.Sprintf("func %s() {}\n\n", sym))
		}
	} else if ext == ".ts" {
		for _, imp := range imports {
			sb.WriteString(fmt.Sprintf("import { X } from '%s';\n", imp))
		}
		sb.WriteString("\n")
		for _, sym := range symbols {
			sb.WriteString(fmt.Sprintf("export function %s(): void {}\n\n", sym))
		}
	} else if ext == ".py" {
		for _, imp := range imports {
			sb.WriteString(fmt.Sprintf("import %s\n", filepath.Base(imp)))
		}
		sb.WriteString("\n")
		for _, sym := range symbols {
			sb.WriteString(fmt.Sprintf("def %s(): pass\n\n", strings.ToLower(sym[:1])+sym[1:]))
		}
	} else if ext == ".rs" {
		for _, imp := range imports {
			sb.WriteString(fmt.Sprintf("use %s::*;\n", filepath.Base(imp)))
		}
		sb.WriteString("\n")
		for _, sym := range symbols {
			sb.WriteString(fmt.Sprintf("pub fn %s() {{}}\n\n", strings.ToLower(sym[:1])+sym[1:]))
		}
	}

	return os.WriteFile(path, []byte(sb.String()), 0o644)
}

// GenerateScalabilityRepos creates repos of increasing sizes for scalability testing.
func GenerateScalabilityRepos(baseDir string) ([]*GeneratedRepo, error) {
	sizes := []int{100, 500, 1000, 2000, 5000, 10000}
	repos := make([]*GeneratedRepo, 0, len(sizes))

	for _, size := range sizes {
		dir := filepath.Join(baseDir, fmt.Sprintf("repo_%d", size))
		pkgCount := size / 50
		if pkgCount < 5 {
			pkgCount = 5
		}
		repo, err := GenerateRepo(GenerateConfig{
			Dir:          dir,
			FileCount:    size,
			AvgImports:   5,
			MaxImports:   15,
			MinSymbols:   5,
			MaxSymbols:   20,
			PackageCount: pkgCount,
			Seed:         int64(size * 1000),
		})
		if err != nil {
			return nil, fmt.Errorf("generate repo size %d: %w", size, err)
		}
		repos = append(repos, repo)
	}

	return repos, nil
}

// CleanupRepos removes generated repositories.
func CleanupRepos(repos []*GeneratedRepo) {
	for _, repo := range repos {
		os.RemoveAll(repo.Dir)
	}
}

// PrintRepoStats prints metadata about a generated repository.
func PrintRepoStats(repo *GeneratedRepo) {
	fmt.Printf("  Dir: %s\n", repo.Dir)
	fmt.Printf("  Files: %d\n", repo.FileCount)
	fmt.Printf("  Packages: %d\n", repo.PackageCount)
	fmt.Printf("  Symbols: %d\n", repo.TotalSymbols)
	fmt.Printf("  Languages: %v\n", repo.Languages)
}

// SortedKeys returns sorted keys from a map.
func SortedKeys(m map[string]int) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
