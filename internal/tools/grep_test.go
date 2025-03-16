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
