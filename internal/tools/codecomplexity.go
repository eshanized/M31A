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
	"github.com/eshanized/M31A/pkg/types"
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
	Languages  map[string]int // extension → file count
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

// DefaultExtensions are the common source file extensions to analyze
// when no extensions are specified. Covers Go, Python, JS/TS, Rust, Java, C/C++.
var DefaultExtensions = []string{
	".go", ".py", ".js", ".ts", ".jsx", ".tsx",
	".rs", ".java", ".c", ".cpp", ".h", ".hpp",
	".rb", ".php", ".cs", ".swift", ".kt",
}

// CodeComplexity is a tool that analyzes the codebase and returns
// a complexity report with file counts, line counts, top packages, top files,
// and a complexity score. Default extensions cover all common languages.
type CodeComplexity struct {
	workDir  string
	skipDirs []string
}

// NewCodeComplexity creates a new CodeComplexity tool instance.
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
		"top packages and files by size, language breakdown, and a complexity score. " +
		"Default: all common languages (Go, Python, JS/TS, Rust, Java, C/C++, etc.)"
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
			},
			"extensions": {
				"type": "array",
				"items": {"type": "string"},
				"description": "File extensions to analyze (default: all common languages). E.g. ['*.go', '*.py', '*.js']"
			}
		},
		"required": []
	}`
}

func (t *CodeComplexity) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	// Merge default skip dirs with any provided via params.
	skip := t.effectiveSkipDirs(input)

	// Parse extensions parameter
	var extensions []string
	if extRaw, ok := input.Params["extensions"]; ok {
		if extArr, ok := extRaw.([]any); ok {
			for _, v := range extArr {
				if s, ok := v.(string); ok && s != "" {
					// Normalize: ensure extension starts with dot
					if s[0] != '.' {
						s = "." + s
					}
					extensions = append(extensions, s)
				}
			}
		}
	}

	// Default to all common languages if no extensions specified
	if len(extensions) == 0 {
		extensions = DefaultExtensions
	}

	report, err := t.analyze(ctx, skip, extensions)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %w", m31errors.ErrToolExecution, err)
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
func (t *CodeComplexity) analyze(ctx context.Context, skipDirs map[string]bool, extensions []string) (*ComplexityReport, error) {
	// Always skip .m31a-worktrees
	skipDirs[".m31a-worktrees"] = true

	// Build extension lookup set
	extSet := make(map[string]bool, len(extensions))
	for _, ext := range extensions {
		extSet[ext] = true
	}

	report := &ComplexityReport{
		Languages: make(map[string]int),
	}
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

		// Check file extension
		matched := false
		for ext := range extSet {
			if strings.HasSuffix(d.Name(), ext) {
				matched = true
				break
			}
		}
		if !matched {
			return nil
		}

		// Skip test files (common convention across languages)
		name := d.Name()
		if strings.HasSuffix(name, "_test.go") || strings.HasSuffix(name, ".test.js") ||
			strings.HasSuffix(name, ".test.ts") || strings.HasSuffix(name, ".test.jsx") ||
			strings.HasSuffix(name, ".test.tsx") || strings.HasSuffix(name, "_test.py") ||
			strings.HasSuffix(name, "_test.rs") || strings.HasSuffix(name, "_test.java") ||
			strings.HasSuffix(name, "Test.java") || strings.HasSuffix(name, "_test.rb") ||
			strings.Contains(name, ".spec.") || strings.HasSuffix(name, "_test.go") {
			return nil
		}

		lines, err := countLines(path)
		if err != nil {
			return nil // skip unreadable files
		}

		report.TotalFiles++
		report.TotalLines += lines

		// Track language
		ext := filepath.Ext(d.Name())
		report.Languages[ext]++

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
	case totalLines < 200_000:
		return "large (50K–200K lines)"
	default:
		return "complex (200K+ lines)"
	}
}

// formatComplexityReport renders the report as a readable markdown-style table.
func formatComplexityReport(r *ComplexityReport) string {
	var sb strings.Builder

	fmt.Fprintf(&sb, "## Codebase Complexity Report\n\n")
	fmt.Fprintf(&sb, "**Source files:** %d  \n", r.TotalFiles)
	fmt.Fprintf(&sb, "**Total lines:** %s  \n", formatNumber(r.TotalLines))
	fmt.Fprintf(&sb, "**Complexity score:** %s\n\n", r.Score)

	// Language breakdown
	if len(r.Languages) > 0 {
		type langStat struct {
			Ext   string
			Count int
		}
		var langs []langStat
		for ext, count := range r.Languages {
			langs = append(langs, langStat{ext, count})
		}
		sort.Slice(langs, func(i, j int) bool { return langs[i].Count > langs[j].Count })

		fmt.Fprintf(&sb, "### Language Breakdown\n\n")
		fmt.Fprintf(&sb, "| Language | Files |\n")
		fmt.Fprintf(&sb, "|----------|-------|\n")
		for _, l := range langs {
			fmt.Fprintf(&sb, "| %s | %d |\n", l.Ext, l.Count)
		}
		sb.WriteString("\n")
	}

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
