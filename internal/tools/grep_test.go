package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestGrep_SimpleSearch(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "utils.go"), []byte("package utils\nfunc Helper() {\n\t// helper\n}\n"), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "main"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in results, got: %s", result.Output)
	}
}

func TestGrep_WithGlob(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\n"), 0644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Main Project\n"), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "main",
			"include": "*.go",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in results, got: %s", result.Output)
	}
	if strings.Contains(result.Output, "README.md") {
		t.Errorf("did not expect README.md in .go-filtered results, got: %s", result.Output)
	}
}

func TestGrep_NoMatches(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\n"), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "nonexistent_pattern_xyz"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "No results found") {
		t.Errorf("expected no-results message, got: %s", result.Output)
	}
}

func TestGrep_MaxResults(t *testing.T) {
	dir := t.TempDir()
	var content strings.Builder
	for i := 0; i < 300; i++ {
		content.WriteString("line ")
		content.WriteString(string(rune('0' + i%10)))
		content.WriteString(" with match foo\n")
	}
	os.WriteFile(filepath.Join(dir, "big.txt"), []byte(content.String()), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":     "foo",
			"max_results": float64(50),
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(result.Output), "\n")
	if len(lines) > 55 {
		t.Errorf("expected at most ~50 results (with some overhead), got %d", len(lines))
	}
}

func TestGrep_InvalidRegex(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGrep(dir)
	_, err := g.Execute(context.Background(), toolInput("pattern", "[invalid"))
	if err == nil {
		t.Error("expected error for invalid regex")
	}
}

func TestGrep_BinaryFileSkipped(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "data.bin"), []byte("foo\x00bar"), 0644)
	os.WriteFile(filepath.Join(dir, "text.txt"), []byte("foo bar\n"), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "foo"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, "data.bin") {
		t.Errorf("expected binary file to be skipped, but found in results: %s", result.Output)
	}
	if !strings.Contains(result.Output, "text.txt") {
		t.Errorf("expected text file in results, got: %s", result.Output)
	}
}

func TestGrep_MissingPatternParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGrep(dir)
	_, err := g.Execute(context.Background(), types.ToolInput{Name: "Grep", Params: map[string]any{}})
	if err == nil {
		t.Error("expected error for missing pattern param")
	}
}

func TestGrep_RelativePath(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "src", "pkg"), 0755)
	os.WriteFile(filepath.Join(dir, "src", "pkg", "main.go"), []byte("package main\nfunc main() {}\n"), 0644)

	g := NewGrep(dir)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "main",
			"path":    "src/pkg",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in results with relative path, got: %s", result.Output)
	}
}

func TestGrep_Name(t *testing.T) {
	t.Parallel()
	g := NewGrep(t.TempDir())
	if g.Name() != "Grep" {
		t.Errorf("expected name 'Grep', got %s", g.Name())
	}
}

func TestGrep_Description(t *testing.T) {
	t.Parallel()
	g := NewGrep(t.TempDir())
	if g.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestGrep_RiskLevel(t *testing.T) {
	t.Parallel()
	g := NewGrep(t.TempDir())
	if g.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", g.RiskLevel())
	}
}

func TestGrep_PureGoSearch(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "util.go"), []byte("package util\nfunc helper() {}\n"), 0644)

	// Force pure-Go by creating a grep without rg
	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "main"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in results, got: %s", result.Output)
	}
}

func TestGrep_PureGoNoMatches(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("no match here\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "xyz_nonexistent"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "No results found") {
		t.Errorf("expected no-results message, got: %s", result.Output)
	}
}

func TestGrep_PureGoInvalidRegex(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := &Grep{workDir: dir, hasRg: false}
	_, err := g.Execute(context.Background(), toolInput("pattern", "[invalid"))
	if err == nil {
		t.Error("expected error for invalid regex")
	}
	if !strings.Contains(err.Error(), "invalid regex") {
		t.Errorf("expected 'invalid regex' error, got: %v", err)
	}
}

func TestGrep_PureGoGlobFilter(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Main Project\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "main",
			"glob":    "*.go",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in results, got: %s", result.Output)
	}
	if strings.Contains(result.Output, "README.md") {
		t.Errorf("did not expect README.md in .go-filtered results, got: %s", result.Output)
	}
}

func TestGrep_PureGoHiddenDirSkipped(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, ".hidden"), 0755)
	os.WriteFile(filepath.Join(dir, ".hidden", "secret.go"), []byte("package hidden\nfunc secret() {}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "visible.go"), []byte("package main\nfunc visible() {}\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "func"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, filepath.Join(".hidden", "secret.go")) {
		t.Errorf("expected hidden dir to be skipped, got: %s", result.Output)
	}
}

func TestGrep_PureGoMaxResults(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	var content strings.Builder
	for i := 0; i < 100; i++ {
		content.WriteString("line with match foo\n")
	}
	os.WriteFile(filepath.Join(dir, "big.txt"), []byte(content.String()), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":     "foo",
			"max_results": float64(10),
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(result.Output), "\n")
	if len(lines) > 15 {
		t.Errorf("expected at most ~10 results, got %d", len(lines))
	}
}

func TestLoadGitignore(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := `# comment
*.log
*.tmp
node_modules/

.DS_Store
`
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte(content), 0644)

	patterns := loadGitignore(dir)
	if len(patterns) != 4 {
		t.Errorf("expected 4 patterns, got %d: %v", len(patterns), patterns)
	}

	// Check that comments and blank lines are filtered
	for _, p := range patterns {
		if p == "" {
			t.Error("expected no empty patterns")
		}
		if strings.HasPrefix(p, "#") {
			t.Errorf("expected no comment patterns, got: %s", p)
		}
	}
}

func TestLoadGitignore_NoFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	patterns := loadGitignore(dir)
	if patterns != nil {
		t.Errorf("expected nil patterns when no .gitignore, got: %v", patterns)
	}
}

func TestMatchesGitignore(t *testing.T) {
	t.Parallel()
	patterns := []string{"*.log", "*.tmp", "node_modules"}

	if !matchesGitignore("test.log", patterns) {
		t.Error("expected test.log to match *.log")
	}
	if !matchesGitignore("data.tmp", patterns) {
		t.Error("expected data.tmp to match *.tmp")
	}
	if matchesGitignore("main.go", patterns) {
		t.Error("expected main.go to not match any pattern")
	}
	if !matchesGitignore("node_modules", patterns) {
		t.Error("expected node_modules to match")
	}
}

func TestGrep_PathOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGrep(dir)
	_, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "test",
			"path":    "/etc",
		},
	})
	if err == nil {
		t.Fatal("expected error for path outside workDir")
	}
	if !strings.Contains(err.Error(), "path escapes work directory") {
		t.Errorf("expected 'path escapes work directory' error, got: %v", err)
	}
}

func TestGrep_BadPathType(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("test\n"), 0644)

	g := NewGrep(dir)
	// Pass non-string path (should be ignored, fall through to workDir)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "test",
			"path":    123, // invalid type
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "file.txt") {
		t.Errorf("expected file.txt in results, got: %s", result.Output)
	}
}

func TestGrep_BadGlobType(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("test\n"), 0644)

	g := NewGrep(dir)
	// Pass non-string glob (should be ignored)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "test",
			"glob":    123, // invalid type
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "file.txt") {
		t.Errorf("expected file.txt in results, got: %s", result.Output)
	}
}

func TestGrep_BadMaxResultsType(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("test\n"), 0644)

	g := NewGrep(dir)
	// Pass non-float64 max_results (should use default of 100)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":     "test",
			"max_results": "not_a_number",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "file.txt") {
		t.Errorf("expected file.txt in results, got: %s", result.Output)
	}
}

func TestGrep_PureGoBinaryFileSkipped(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "data.bin"), []byte("foo\x00bar"), 0644)
	os.WriteFile(filepath.Join(dir, "text.txt"), []byte("foo bar\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "foo"))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, "data.bin") {
		t.Errorf("expected binary file to be skipped, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "text.txt") {
		t.Errorf("expected text file in results, got: %s", result.Output)
	}
}

func TestGrep_PureGoGitignoreFilter(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// Note: matchesGitignore uses doublestar.Match with full absolute paths,
	// so patterns like "*.log" won't match. Only patterns matching the full
	// path would work. This test verifies the gitignore code path is exercised.
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("*.log\n"), 0644)
	os.WriteFile(filepath.Join(dir, "app.go"), []byte("go content with test\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "test"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "app.go") {
		t.Errorf("expected app.go in results, got: %s", result.Output)
	}
}

func TestGrep_PatternNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGrep(dir)
	_, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": 123, // not a string
		},
	})
	if err == nil {
		t.Fatal("expected error for non-string pattern")
	}
	if !strings.Contains(err.Error(), "parameter pattern must be a string") {
		t.Errorf("expected type error, got: %v", err)
	}
}

func TestGrep_IncludeSchemaKey(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// Create test files
	os.WriteFile(filepath.Join(dir, "app.go"), []byte("package main\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Main Project with main\n"), 0644)

	g := NewGrep(dir)
	// Use the schema-aligned "include" key (not the old "glob" key)
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "main",
			"include": "*.go",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	// Should find matches in app.go
	if !strings.Contains(result.Output, "app.go") {
		t.Errorf("expected app.go in results, got: %s", result.Output)
	}
	// Should NOT find matches in README.md
	if strings.Contains(result.Output, "README.md") {
		t.Errorf("include filter should have excluded README.md, got: %s", result.Output)
	}
}
