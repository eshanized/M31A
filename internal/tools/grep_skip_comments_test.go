package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestIsCommentLine(t *testing.T) {
	t.Parallel()
	cases := []struct {
		line string
		want bool
	}{
		{"// this is a comment", true},
		{"  // indented comment", true},
		{"# hash comment", true},
		{"  # indented hash", true},
		{"-- sql comment", true},
		{"; lisp comment", true},
		{"/* block comment start", true},
		{" * block comment line", true},
		{"func main() {}", false},
		{"x := 42", false},
		{"", false},
		{"  ", false},
	}
	for _, tc := range cases {
		t.Run(tc.line, func(t *testing.T) {
			got := isCommentLine(tc.line)
			if got != tc.want {
				t.Errorf("isCommentLine(%q) = %v, want %v", tc.line, got, tc.want)
			}
		})
	}
}

func TestGrep_SkipComments_PureGo(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := `package main

// This is a comment with TODO
func main() {
	x := 42 // inline comment
	# hash comment
	y := x + 1
}
`
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(content), 0644)

	g := &Grep{workDir: dir, hasRg: false}

	// Without skip_comments: should find "TODO" in comment
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "TODO",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "TODO") {
		t.Errorf("expected TODO in results without skip_comments, got: %s", result.Output)
	}

	// With skip_comments: should NOT find "TODO" in comment
	result, err = g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":       "TODO",
			"skip_comments": true,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, "TODO") {
		t.Errorf("expected no TODO in results with skip_comments, got: %s", result.Output)
	}
}

func TestGrep_SkipComments_CodeInComments(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := `package main

// x := 42
func main() {
	x := 42
}
`
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(content), 0644)

	g := &Grep{workDir: dir, hasRg: false}

	// Without skip_comments: find both
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "x := 42",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(result.Output), "\n")
	if len(lines) != 2 {
		t.Errorf("expected 2 matches without skip_comments, got %d: %s", len(lines), result.Output)
	}

	// With skip_comments: find only the code line
	result, err = g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":       "x := 42",
			"skip_comments": true,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, "//") {
		t.Errorf("expected no comment matches with skip_comments, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "x := 42") {
		t.Errorf("expected code match, got: %s", result.Output)
	}
}

func TestGrep_SkipComments_Disabled(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := `// comment
func main() {}
`
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(content), 0644)

	g := &Grep{workDir: dir, hasRg: false}

	// skip_comments=false (default) should find comment
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":       "comment",
			"skip_comments": false,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "comment") {
		t.Errorf("expected comment in results, got: %s", result.Output)
	}
}

func TestGrep_SkipComments_EmptyResult(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := `// only comments here
// nothing else
`
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(content), 0644)

	g := &Grep{workDir: dir, hasRg: false}

	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":       "comment",
			"skip_comments": true,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "No results found") {
		t.Errorf("expected no results when all matches are comments, got: %s", result.Output)
	}
}

func TestGrep_SkipComments_ParameterSchema(t *testing.T) {
	t.Parallel()
	g := NewGrep(t.TempDir())
	schema := g.ParameterSchema()
	if !strings.Contains(schema, "skip_comments") {
		t.Error("ParameterSchema should contain skip_comments")
	}
}

func TestGrep_SkipComments_WithGlob(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("// TODO fix this\nfunc main() {}\n"), 0644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# TODO: update\n"), 0644)

	g := &Grep{workDir: dir, hasRg: false}

	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":       "TODO",
			"include":       "*.go",
			"skip_comments": true,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(result.Output, "TODO") {
		t.Errorf("expected no TODO with skip_comments + glob, got: %s", result.Output)
	}
}
