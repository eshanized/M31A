package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

// BenchmarkCodeComplexity benchmarks the CodeComplexity tool's analysis.
func BenchmarkCodeComplexity(b *testing.B) {
	// Create a temporary directory structure for benchmarking
	tmpDir := b.TempDir()

	sizes := []struct {
		name      string
		numFiles  int
		linesPer  int
		packages  int
	}{
		{"small_10files", 10, 100, 2},
		{"medium_50files", 50, 200, 5},
		{"large_200files", 200, 300, 10},
		{"xlarge_500files", 500, 400, 20},
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			// Setup test directory
			setupTestDir(b, tmpDir, size.numFiles, size.linesPer, size.packages)

			tool := NewCodeComplexity(tmpDir, nil)
			ctx := context.Background()
			input := types.ToolInput{
				Params: map[string]any{},
			}

			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				tool.Execute(ctx, input)
			}
		})
	}
}

// BenchmarkCountLines benchmarks the line counting function.
func BenchmarkCountLines(b *testing.B) {
	sizes := []struct {
		name  string
		lines int
	}{
		{"small_100", 100},
		{"medium_1000", 1000},
		{"large_10000", 10000},
		{"xlarge_50000", 50000},
	}

	for _, size := range sizes {
		b.Run(size.name, func(b *testing.B) {
			// Create temp file with specified lines
			tmpFile := filepath.Join(b.TempDir(), "test.go")
			content := generateGoContent(size.lines)
			if err := os.WriteFile(tmpFile, []byte(content), 0644); err != nil {
				b.Fatal(err)
			}

			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				countLines(tmpFile)
			}
		})
	}
}

// BenchmarkComplexityScore benchmarks the complexity scoring function.
func BenchmarkComplexityScore(b *testing.B) {
	scores := []int{1000, 5000, 10000, 25000, 50000, 100000}
	for _, score := range scores {
		b.Run(fmt.Sprintf("lines_%d", score), func(b *testing.B) {
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				complexityScore(score)
			}
		})
	}
}

// BenchmarkFormatComplexityReport benchmarks the report formatting.
func BenchmarkFormatComplexityReport(b *testing.B) {
	report := &ComplexityReport{
		TotalFiles: 150,
		TotalLines: 45000,
		Packages: []PackageStat{
			{Path: "internal/workflow", Files: 25, Lines: 8500},
			{Path: "internal/tui", Files: 40, Lines: 12000},
			{Path: "pkg/session", Files: 10, Lines: 3500},
		},
		Files: []FileStat{
			{Path: "internal/tui/app.go", Lines: 850},
			{Path: "internal/workflow/engine.go", Lines: 720},
		},
		Score: "moderate (10K–50K lines)",
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		formatComplexityReport(report)
	}
}

// Helper functions

func setupTestDir(b *testing.B, baseDir string, numFiles, linesPerFile, numPackages int) {
	b.Helper()

	filesPerPkg := numFiles / numPackages
	if filesPerPkg < 1 {
		filesPerPkg = 1
	}

	for pkg := 0; pkg < numPackages; pkg++ {
		pkgDir := filepath.Join(baseDir, fmt.Sprintf("package%d", pkg))
		os.MkdirAll(pkgDir, 0755)

		for f := 0; f < filesPerPkg && pkg*filesPerPkg+f < numFiles; f++ {
			filePath := filepath.Join(pkgDir, fmt.Sprintf("file%d.go", f))
			content := generateGoContent(linesPerFile)
			os.WriteFile(filePath, []byte(content), 0644)
		}
	}
}

func generateGoContent(lines int) string {
	var sb strings.Builder
	boilerplate := []string{
		"package main",
		"",
		"import (",
		"\t\"fmt\"",
		"\t\"os\"",
		")",
		"",
		"func main() {",
		"\tfmt.Println(\"hello\")",
		"}",
		"",
	}

	for i := 0; i < lines; i++ {
		sb.WriteString(boilerplate[i%len(boilerplate)])
		sb.WriteByte('\n')
	}
	return sb.String()
}
