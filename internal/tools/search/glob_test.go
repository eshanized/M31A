package search

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestGlob_Name(t *testing.T) {
	g := NewGlob("/tmp")
	if g.Name() != "Glob" {
		t.Errorf("expected 'Glob', got %q", g.Name())
	}
}

func TestGlob_RiskLevel(t *testing.T) {
	g := NewGlob("/tmp")
	if g.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %v", g.RiskLevel())
	}
}

func TestGlob_ParameterSchema(t *testing.T) {
	g := NewGlob("/tmp")
	schema := g.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !contains(schema, "pattern") {
		t.Error("schema missing pattern field")
	}
	if !contains(schema, "path") {
		t.Error("schema missing path field")
	}
	if !contains(schema, "type") {
		t.Error("schema missing type field")
	}
}

func TestGlob_ExecuteMissingPattern(t *testing.T) {
	workDir := t.TempDir()
	g := NewGlob(workDir)
	
	_, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing pattern")
	}
	if !contains(err.Error(), "missing parameter: pattern") {
		t.Errorf("expected 'missing parameter: pattern' error, got %q", err.Error())
	}
}

func TestGlob_ExecuteSimplePattern(t *testing.T) {
	workDir := t.TempDir()
	// Create test files
	_ = os.WriteFile(filepath.Join(workDir, "file1.txt"), []byte("content"), 0644)
	_ = os.WriteFile(filepath.Join(workDir, "file2.go"), []byte("content"), 0644)
	_ = os.Mkdir(filepath.Join(workDir, "subdir"), 0755)
	_ = os.WriteFile(filepath.Join(workDir, "subdir", "file3.txt"), []byte("content"), 0644)
	
	g := NewGlob(workDir)
	
	result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "*.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "file1.txt") {
		t.Errorf("expected file1.txt in output, got %q", result.Output)
	}
	// "*.txt" does not match subdirectories, so subdir/file3.txt should NOT be in output
	if contains(result.Output, "subdir/file3.txt") {
		t.Errorf("expected no subdir/file3.txt with *.txt pattern, got %q", result.Output)
	}
	if contains(result.Output, "file2.go") {
		t.Errorf("expected no .go files in output, got %q", result.Output)
	}
}

func TestGlob_ExecuteRecursivePattern(t *testing.T) {
	workDir := t.TempDir()
	// Create nested files
	_ = os.MkdirAll(filepath.Join(workDir, "a", "b", "c"), 0755)
	_ = os.WriteFile(filepath.Join(workDir, "a", "b", "c", "deep.txt"), []byte("content"), 0644)
	_ = os.WriteFile(filepath.Join(workDir, "a", "shallow.txt"), []byte("content"), 0644)
	
	g := NewGlob(workDir)
	
result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "**/*.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "a/b/c/deep.txt") {
		t.Errorf("expected deep file in output, got %q", result.Output)
	}
	if !contains(result.Output, "a/shallow.txt") {
		t.Errorf("expected shallow file in output, got %q", result.Output)
	}
}

func TestGlob_ExecuteWithPath(t *testing.T) {
	workDir := t.TempDir()
	subDir := filepath.Join(workDir, "subdir")
	_ = os.Mkdir(subDir, 0755)
	_ = os.WriteFile(filepath.Join(subDir, "test.txt"), []byte("content"), 0644)
	
	g := NewGlob(workDir)
	
	// Use recursive pattern to match files in subdirectories
result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "**/*.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "subdir/test.txt") {
		t.Errorf("expected subdir/test.txt in output, got %q", result.Output)
	}
}

func TestGlob_ExecuteTypeFilterFile(t *testing.T) {
	workDir := t.TempDir()
	_ = os.WriteFile(filepath.Join(workDir, "file.txt"), []byte("content"), 0644)
	_ = os.Mkdir(filepath.Join(workDir, "dir"), 0755)
	
	g := NewGlob(workDir)
	
	result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "*",
			"type":    "file",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if contains(result.Output, "dir") {
		t.Errorf("expected no directories with type=file, got %q", result.Output)
	}
	if !contains(result.Output, "file.txt") {
		t.Errorf("expected file.txt in output, got %q", result.Output)
	}
}

func TestGlob_ExecuteTypeFilterDir(t *testing.T) {
	workDir := t.TempDir()
	_ = os.WriteFile(filepath.Join(workDir, "file.txt"), []byte("content"), 0644)
	_ = os.Mkdir(filepath.Join(workDir, "dir"), 0755)
	
	g := NewGlob(workDir)
	
	result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "*",
			"type":    "dir",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if contains(result.Output, "file.txt") {
		t.Errorf("expected no files with type=dir, got %q", result.Output)
	}
	if !contains(result.Output, "dir") {
		t.Errorf("expected dir in output, got %q", result.Output)
	}
}

func TestGlob_ExecuteNoMatches(t *testing.T) {
	workDir := t.TempDir()
	g := NewGlob(workDir)
	
	result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "*.nonexistent",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "No files matched pattern") {
		t.Errorf("expected 'No files matched pattern' message, got %q", result.Output)
	}
}

func TestGlob_ExecuteTruncation(t *testing.T) {
	workDir := t.TempDir()
	// Create more than MaxGlobResults files
	for i := 0; i < 1100; i++ {
		_ = os.WriteFile(filepath.Join(workDir, fmt.Sprintf("file%d.txt", i)), []byte("content"), 0644)
	}
	
	g := NewGlob(workDir)
	
	result, err := g.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"pattern": "*.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !contains(result.Output, "more files") {
		t.Errorf("expected truncation message, got %q", result.Output)
	}
}

