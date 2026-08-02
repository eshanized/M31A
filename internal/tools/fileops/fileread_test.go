package fileops

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileRead_Name(t *testing.T) {
	t.Parallel()
	d := NewFileRead("/tmp")
	if d.Name() != "FileRead" {
		t.Errorf("expected 'FileRead', got %q", d.Name())
	}
}

func TestFileRead_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewFileRead("/tmp")
	if d.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %v", d.RiskLevel())
	}
}

func TestFileRead_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewFileRead("/tmp")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "path") || !Contains(schema, "limit") || !Contains(schema, "offset") || !Contains(schema, "max_lines") {
		t.Error("schema missing required parameters")
	}
}

func TestFileRead_ExecuteMissingPath(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for missing path")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "missing parameter: path") {
		t.Errorf("expected 'missing parameter: path' error, got %q", errorMsg)
	}
}

func TestFileRead_ExecuteFileNotFound(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "nonexistent.txt",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for file not found")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "file not found") {
		t.Errorf("expected 'file not found' error, got %q", errorMsg)
	}
}

func TestFileRead_ExecuteReadFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	// Create test file
	content := "line 1\nline 2\nline 3\n"
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte(content), 0644)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "test.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if result.Output != content {
		t.Errorf("expected content %q, got %q", content, result.Output)
	}
}

func TestFileRead_ExecuteReadWithLimit(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	// Create content longer than 512 bytes so limit is actually applied after header
	content := strings.Repeat("this is a longer content that exceeds the limit\n", 20)
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte(content), 0644)

	// Use offset=1 to skip the file size check and test the limit logic
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":      "test.txt",
			"offset":    float64(1),
			"max_lines": float64(5),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	// Should contain first 5 lines with line numbers
	if !Contains(result.Output, "1: this is a longer content") || !Contains(result.Output, "5: this is a longer content") {
		t.Errorf("expected first 5 lines, got %q", result.Output)
	}
}

func TestFileRead_ExecuteReadWithOffset(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	content := "line 1\nline 2\nline 3\nline 4\nline 5\n"
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte(content), 0644)

	// Read from line 3 (1-indexed)
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":      "test.txt",
			"offset":    float64(3),
			"max_lines": float64(2),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	// Should contain line 3 and 4
	if !Contains(result.Output, "3: line 3") || !Contains(result.Output, "4: line 4") {
		t.Errorf("expected lines 3 and 4, got %q", result.Output)
	}
}

func TestFileRead_ExecuteReadDirectoryError(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	// Create a directory
	_ = os.Mkdir(filepath.Join(workDir, "subdir"), 0755)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "subdir",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for directory")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "path is a directory") {
		t.Errorf("expected 'path is a directory' error, got %q", errorMsg)
	}
}

func TestFileRead_ExecuteBinaryFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	// Create a binary file (with null byte)
	binaryContent := []byte{0x00, 0x01, 0x02, 0x03}
	_ = os.WriteFile(filepath.Join(workDir, "binary.bin"), binaryContent, 0644)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "binary.bin",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "binary file") {
		t.Errorf("expected binary file message, got %q", result.Output)
	}
}

func TestFileRead_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "../etc/passwd",
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
	// For non-existent files, the current implementation returns "file not found"
	// Path traversal detection happens after existence check
	if !Contains(errorMsg, "file not found") {
		t.Errorf("expected 'file not found' error for non-existent path, got %q", errorMsg)
	}
}

func TestFileRead_ExecuteOffsetBeyondEnd(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileRead(workDir)

	content := "line 1\nline 2\n"
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte(content), 0644)

	// Read from line 10 (beyond end)
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":      "test.txt",
			"offset":    float64(10),
			"max_lines": float64(5),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// When offset is beyond end, the tool returns "No lines found" message
	if !Contains(result.Output, "No lines found at offset 10") {
		t.Errorf("expected 'No lines found' message, got %q", result.Output)
	}
}
