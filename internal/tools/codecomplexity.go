package tools

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*CodeComplexity)(nil)

// ComplexityReport holds the results of a codebase complexity analysis.
type ComplexityReport struct {
	TotalFiles int
	TotalLines int
	Packages   []PackageStat
	Files      []FileStat
	Score      string
}

// PackageStat holds aggregate stats for a single package (directory).
type PackageStat struct {
	Path  string
	Files int
	Lines int
}

// FileStat holds stats for a single source file.
type FileStat struct {
	Path  string
	Lines int
}

// CodeComplexity is a tool that analyzes the M31A Go codebase and returns
// a complexity report with file counts, line counts, top packages, top files,
// and a complexity score.
type CodeComplexity struct {
	workDir  string
	skipDirs []string
}

// NewCodeComplexity creates a new CodeComplexity tool.
// skipDirs is an optional override; if nil, types.SkipDirs is used.
func NewCodeComplexity(workDir string, skipDirs []string) *CodeComplexity {
	return &CodeComplexity{
		workDir:  workDir,
		skipDirs: skipDirs,
	}
}

func (t *CodeComplexity) Name() string {
	return "CodeComplexity"
}

func (t *CodeComplexity) Description() string {
	return "Analyze the codebase and display a complexity report showing total source files and lines, " +
		"top packages and files by size, and a complexity score."
}

func (t *CodeComplexity) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

func (t *CodeComplexity) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"skip_dirs": {
				"type": "array",
				"items": {"type": "string"},
				"description": "Additional directories to skip during analysis. Merged with default skip list."
			}
		},
		"required": []
	}`
}

func (t *CodeComplexity) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	// Merge default skip dirs with any provided via params.
	skip := t.effectiveSkipDirs(input)

	report, err := t.analyze(ctx, skip)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %v", m31errors.ErrToolExecution, err)
	}

	output := formatComplexityReport(report)
	elapsed := time.Since(start).Milliseconds()

	return types.ToolResult{
		Output:     output,
		DurationMs: elapsed,
	}, nil
}

// effectiveSkipDirs merges the built-in skip dirs with user-supplied ones.
func (t *CodeComplexity) effectiveSkipDirs(input types.ToolInput) map[string]bool {
	// Start with the tool-level skip dirs (from constructor / config).
	base := types.SkipDirsMap()
	for _, d := range t.skipDirs {
		base[d] = true
	}

	// Merge any additional dirs from the tool input params.
	if extra, ok := input.Params["skip_dirs"].([]any); ok {
		for _, v := range extra {
			if s, ok := v.(string); ok && s != "" {
				base[s] = true
			}
		}
	}

	return base
}

// analyze walks the workDir and computes the complexity report.
func (t *CodeComplexity) analyze(ctx context.Context, skipDirs map[string]bool) (*ComplexityReport, error) {
	// Always skip .m31a-worktrees
	skipDirs[".m31a-worktrees"] = true

	report := &ComplexityReport{}
	pkgMap := make(map[string]*PackageStat) // directory path → stats
	fileStats := make([]FileStat, 0)

	err := filepath.WalkDir(t.workDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil // skip unreadable entries
		}

		// Check context cancellation
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		relPath, relErr := filepath.Rel(t.workDir, path)
		if relErr != nil {
			return nil
		}

		if d.IsDir() {
			dirName := d.Name()
			if dirName == ".git" || dirName == ".m31a-worktrees" {
				return filepath.SkipDir
			}
			if skipDirs[dirName] {
				return filepath.SkipDir
			}
			return nil
		}

		// Only analyze .go files
		if !strings.HasSuffix(d.Name(), ".go") {
			return nil
		}

		// Skip test files
		if strings.HasSuffix(d.Name(), "_test.go") {
			return nil
		}

		lines, err := countLines(path)
		if err != nil {
			return nil // skip unreadable files
		}

		report.TotalFiles++
		report.TotalLines += lines

		fileStats = append(fileStats, FileStat{
			Path:  relPath,
			Lines: lines,
		})

		// Aggregate by package (directory)
		pkgDir := filepath.Dir(relPath)
		if pkgDir == "" || pkgDir == "." {
			pkgDir = "(root)"
		}
		if _, ok := pkgMap[pkgDir]; !ok {
			pkgMap[pkgDir] = &PackageStat{Path: pkgDir}
		}
		pkgMap[pkgDir].Files++
		pkgMap[pkgDir].Lines += lines

		return nil
	})
	if err != nil {
		return nil, err
	}

	// Sort packages by lines descending
	pkgs := make([]PackageStat, 0, len(pkgMap))
	for _, p := range pkgMap {
		pkgs = append(pkgs, *p)
	}
	sort.Slice(pkgs, func(i, j int) bool {
		return pkgs[i].Lines > pkgs[j].Lines
	})
	if len(pkgs) > 10 {
		pkgs = pkgs[:10]
	}
	report.Packages = pkgs

	// Sort files by lines descending
	sort.Slice(fileStats, func(i, j int) bool {
		return fileStats[i].Lines > fileStats[j].Lines
	})
	if len(fileStats) > 10 {
		fileStats = fileStats[:10]
	}
	report.Files = fileStats

	// Compute complexity score
	report.Score = complexityScore(report.TotalLines)

	return report, nil
}

// countLines counts non-empty lines in a file.
func countLines(path string) (int, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer func() { _ = f.Close() }()

	count := 0
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		count++
	}
	return count, scanner.Err()
}

// complexityScore returns a human-readable complexity label based on total lines.
func complexityScore(totalLines int) string {
	switch {
	case totalLines < 10_000:
		return "simple (< 10K lines)"
	case totalLines < 50_000:
		return "moderate (10K–50K lines)"
	default:
		return "complex (50K+ lines)"
	}
}

// formatComplexityReport renders the report as a readable markdown-style table.
func formatComplexityReport(r *ComplexityReport) string {
	var sb strings.Builder

	fmt.Fprintf(&sb, "## Codebase Complexity Report\n\n")
	fmt.Fprintf(&sb, "**Source files:** %d  \n", r.TotalFiles)
	fmt.Fprintf(&sb, "**Total lines:** %s  \n", formatNumber(r.TotalLines))
	fmt.Fprintf(&sb, "**Complexity score:** %s\n\n", r.Score)

	// Top packages table
	if len(r.Packages) > 0 {
		fmt.Fprintf(&sb, "### Top %d Packages by Line Count\n\n", len(r.Packages))
		fmt.Fprintf(&sb, "| # | Package | Files | Lines |\n")
		fmt.Fprintf(&sb, "|---|---------|-------|-------|\n")
		for i, p := range r.Packages {
			fmt.Fprintf(&sb, "| %d | `%s` | %d | %s |\n", i+1, p.Path, p.Files, formatNumber(p.Lines))
		}
		sb.WriteString("\n")
	}

	// Top files table
	if len(r.Files) > 0 {
		fmt.Fprintf(&sb, "### Top %d Files by Line Count\n\n", len(r.Files))
		fmt.Fprintf(&sb, "| # | File | Lines |\n")
		fmt.Fprintf(&sb, "|---|------|-------|\n")
		for i, f := range r.Files {
			fmt.Fprintf(&sb, "| %d | `%s` | %s |\n", i+1, f.Path, formatNumber(f.Lines))
		}
		sb.WriteString("\n")
	}

	return sb.String()
}

// formatNumber adds comma separators to an integer for readability.
func formatNumber(n int) string {
	if n < 1000 {
		return fmt.Sprintf("%d", n)
	}
	// Build from right to left
	s := fmt.Sprintf("%d", n)
	var result []byte
	for i, c := range s {
		if i > 0 && (len(s)-i)%3 == 0 {
			result = append(result, ',')
		}
		result = append(result, byte(c))
	}
	return string(result)
}
