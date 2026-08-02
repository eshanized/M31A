package fileops

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileList_Name(t *testing.T) {
	t.Parallel()
	d := NewFileList("/tmp")
	if d.Name() != "FileList" {
		t.Errorf("expected 'FileList', got %q", d.Name())
	}
}

func TestFileList_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewFileList("/tmp")
	if d.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %v", d.RiskLevel())
	}
}

func TestFileList_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewFileList("/tmp")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "path") || !Contains(schema, "depth") || !Contains(schema, "sort") {
		t.Error("schema missing required parameters")
	}
}

func TestFileList_ExecuteDefault(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileList(workDir)

	// Create some test files
	_ = os.Mkdir(filepath.Join(workDir, "subdir"), 0755)
	_ = os.WriteFile(filepath.Join(workDir, "file1.txt"), []byte("content"), 0644)
	_ = os.WriteFile(filepath.Join(workDir, "file2.txt"), []byte("content"), 0644)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "file1.txt") || !Contains(result.Output, "file2.txt") || !Contains(result.Output, "subdir") {
		t.Errorf("expected files and directories in output, got %q", result.Output)
	}
}

func TestFileList_ExecuteWithPath(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	subDir := filepath.Join(workDir, "subdir")
	_ = os.Mkdir(subDir, 0755)
	_ = os.WriteFile(filepath.Join(subDir, "nested.txt"), []byte("content"), 0644)

	d := NewFileList(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "subdir",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "nested.txt") {
		t.Errorf("expected nested file in output, got %q", result.Output)
	}
}

func TestFileList_ExecuteWithDepth(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	// Create nested directories: workDir/a/b/c/
	_ = os.MkdirAll(filepath.Join(workDir, "a", "b", "c"), 0755)
	_ = os.WriteFile(filepath.Join(workDir, "a", "b", "c", "deep.txt"), []byte("content"), 0644)

	d := NewFileList(workDir)

	// Depth 2 should not reach c (a is depth 2)
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"depth": float64(2),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if Contains(result.Output, "deep.txt") {
		t.Errorf("depth 2 should not reach deep.txt, got %q", result.Output)
	}

	// Depth 5 should reach c (a=2, b=3, c=4, deep.txt=5)
	result, err = d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"depth": float64(5),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !Contains(result.Output, "deep.txt") {
		t.Errorf("depth 5 should reach deep.txt, got %q", result.Output)
	}
}

func TestFileList_ExecuteWithSort(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	_ = os.WriteFile(filepath.Join(workDir, "aaa.txt"), []byte("small"), 0644)
	_ = os.WriteFile(filepath.Join(workDir, "zzz.txt"), []byte("larger content here"), 0644)

	d := NewFileList(workDir)

	// Sort by name (default)
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"sort": "name",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// aaa should come before zzz
	idxAAA := findIndex(result.Output, "aaa.txt")
	idxZZZ := findIndex(result.Output, "zzz.txt")
	if idxAAA > idxZZZ {
		t.Errorf("expected aaa.txt before zzz.txt with name sort")
	}

	// Sort by size (largest first)
	result, err = d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"sort": "size",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// zzz.txt is larger, should come first
	idxAAA = findIndex(result.Output, "aaa.txt")
	idxZZZ = findIndex(result.Output, "zzz.txt")
	if idxZZZ > idxAAA {
		t.Errorf("expected zzz.txt before aaa.txt with size sort")
	}
}

func TestFileList_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileList(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "../",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for path traversal")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "resolves outside working directory") {
		t.Errorf("expected path traversal error, got %q", errorMsg)
	}
}

func TestFileList_SkipDirs(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	_ = os.Mkdir(filepath.Join(workDir, "node_modules"), 0755)
	_ = os.WriteFile(filepath.Join(workDir, "node_modules", "config"), []byte("config"), 0644)

	d := NewFileList(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if Contains(result.Output, "node_modules") {
		t.Errorf("expected node_modules to be skipped, got %q", result.Output)
	}
}

func TestHumanSize(t *testing.T) {
	tests := []struct {
		input    int64
		expected string
	}{
		{0, "0 B"},
		{500, "500 B"},
		{1024, "1.0 KB"},
		{1536, "1.5 KB"},
		{1024 * 1024, "1.0 MB"},
		{1024 * 1024 * 1024, "1.0 GB"},
	}
	for _, tc := range tests {
		t.Run(tc.expected, func(t *testing.T) {
			result := HumanSize(tc.input)
			if result != tc.expected {
				t.Errorf("HumanSize(%d) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}
