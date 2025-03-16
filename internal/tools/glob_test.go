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

func TestGlob_SimplePattern(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(dir, "utils.go"), []byte("package utils"), 0644)
	os.WriteFile(filepath.Join(dir, "README.md"), []byte("# Readme"), 0644)

	g := NewGlob(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "*.go"))
	if err != nil {
		t.Fatal(err)
	}
	// Output should contain relative paths, not absolute
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in output, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "utils.go") {
		t.Errorf("expected utils.go in output, got: %s", result.Output)
	}
	if strings.Contains(result.Output, "README.md") {
		t.Errorf("did not expect README.md in *.go output, got: %s", result.Output)
	}
	// Should not contain the full absolute path
	if strings.Contains(result.Output, dir) {
		t.Errorf("output should not contain absolute path %q, got: %s", dir, result.Output)
	}
}

func TestGlob_RecursivePattern(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main"), 0644)
	os.MkdirAll(filepath.Join(dir, "utils", "inner"), 0755)
	os.WriteFile(filepath.Join(dir, "utils", "helper.go"), []byte("package utils"), 0644)
	os.WriteFile(filepath.Join(dir, "utils", "inner", "deep.go"), []byte("package inner"), 0644)

	g := NewGlob(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "**/*.go"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "main.go") {
		t.Errorf("expected main.go in output, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, filepath.Join("utils", "helper.go")) {
		t.Errorf("expected utils/helper.go in output, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, filepath.Join("utils", "inner", "deep.go")) {
		t.Errorf("expected utils/inner/deep.go in output, got: %s", result.Output)
	}
	// Should not contain absolute paths
	if strings.Contains(result.Output, dir) {
		t.Errorf("output should not contain absolute path %q, got: %s", dir, result.Output)
	}
}

func TestGlob_NoMatches(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGlob(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "*.py"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "No files matched pattern") {
		t.Errorf("expected no-match message, got: %s", result.Output)
	}
}

func TestGlob_MaxResults(t *testing.T) {
	dir := t.TempDir()
	for i := 0; i < 1005; i++ {
		os.WriteFile(filepath.Join(dir, fmt.Sprintf("file_%d.txt", i)), []byte("x"), 0644)
	}

	g := NewGlob(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "*.txt"))
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(result.Output), "\n")
	// First line is header, then data lines
	if len(lines) < 2 {
		t.Errorf("expected multiple result lines, got %d", len(lines))
	}
}

func TestGlob_InvalidPattern(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGlob(dir)
	_, err := g.Execute(context.Background(), toolInput("pattern", "[invalid"))
	if err == nil {
		t.Error("expected error for invalid pattern")
	}
}

func TestGlob_MissingPatternParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGlob(dir)
	_, err := g.Execute(context.Background(), types.ToolInput{Name: "Glob", Params: map[string]any{}})
	if err == nil {
		t.Error("expected error for missing pattern param")
	}
}

func toolInput(key, value string) types.ToolInput {
	return types.ToolInput{
		Name:   "test",
		Params: map[string]any{key: value},
	}
}
