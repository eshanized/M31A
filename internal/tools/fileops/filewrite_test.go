package fileops

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileWrite_Name(t *testing.T) {
	t.Parallel()
	d := NewFileWrite("/tmp", "/tmp/backup")
	if d.Name() != "FileWrite" {
		t.Errorf("expected 'FileWrite', got %q", d.Name())
	}
}

func TestFileWrite_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewFileWrite("/tmp", "/tmp/backup")
	if d.RiskLevel() != types.RiskDestructive {
		t.Errorf("expected RiskDestructive, got %v", d.RiskLevel())
	}
}

func TestFileWrite_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewFileWrite("/tmp", "/tmp/backup")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "path") || !Contains(schema, "content") || !Contains(schema, "create_dirs") || !Contains(schema, "append") {
		t.Error("schema missing required parameters")
	}
}

func TestFileWrite_ExecuteMissingParams(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

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

	result, err = d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{"path": "test.txt"},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for missing content")
	}
	errorMsg = ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "missing parameter: content") {
		t.Errorf("expected 'missing parameter: content' error, got %q", errorMsg)
	}
}

func TestFileWrite_ExecuteWriteNewFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "newfile.txt",
			"content": "hello world",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "Wrote 11 bytes to newfile.txt") {
		t.Errorf("expected write message, got %q", result.Output)
	}

	// Verify file content
	data, err := os.ReadFile(filepath.Join(workDir, "newfile.txt"))
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	if string(data) != "hello world" {
		t.Errorf("expected 'hello world', got %q", string(data))
	}
}

func TestFileWrite_ExecuteOverwriteFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	// Create existing file
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte("old content"), 0644)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "test.txt",
			"content": "new content",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}

	// Verify file content is new
	data, err := os.ReadFile(filepath.Join(workDir, "test.txt"))
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	if string(data) != "new content" {
		t.Errorf("expected 'new content', got %q", string(data))
	}

	// Verify backup exists
	entries, _ := os.ReadDir(backupDir)
	if len(entries) != 1 {
		t.Errorf("expected 1 backup file, got %d", len(entries))
	}
}

func TestFileWrite_ExecuteAppendMode(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	// Create existing file
	_ = os.WriteFile(filepath.Join(workDir, "test.txt"), []byte("initial "), 0644)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "test.txt",
			"content": "appended",
			"append":  true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "Appended to") {
		t.Errorf("expected append message, got %q", result.Output)
	}

	// Verify file content
	data, err := os.ReadFile(filepath.Join(workDir, "test.txt"))
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	if string(data) != "initial appended" {
		t.Errorf("expected 'initial appended', got %q", string(data))
	}
}

func TestFileWrite_ExecuteCreateDirs(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":        "nested/subdir/file.txt",
			"content":     "content",
			"create_dirs": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}

	filePath := filepath.Join(workDir, "nested", "subdir", "file.txt")
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		t.Error("expected file to be created with parent directories")
	}
}

func TestFileWrite_ExecuteCreateDirsDisabled(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":        "nested/subdir/file.txt",
			"content":     "content",
			"create_dirs": false,
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for missing parent directories")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "cannot create directories") && !Contains(errorMsg, "permission denied") {
		t.Errorf("expected directory creation error, got %q", errorMsg)
	}
}

func TestFileWrite_BinaryContentRejected(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	// Try to write binary content (with null byte)
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "test.txt",
			"content": "hello\x00world",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for binary content")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "binary content") && !Contains(errorMsg, "binary content not displayable") {
		t.Errorf("expected 'binary content' error, got %q", errorMsg)
	}
}

func TestFileWrite_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "../etc/passwd",
			"content": "malicious",
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

func TestFileWrite_AtomicWrite(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileWrite(workDir, backupDir)

	// Verify temp file is cleaned up on failure - we can't easily test the failure
	// but we can verify the atomic rename works by checking the file doesn't exist
	// until the write is complete
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":    "atomic.txt",
			"content": "atomic write test",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}

	// Verify file exists and has correct content
	data, err := os.ReadFile(filepath.Join(workDir, "atomic.txt"))
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	if string(data) != "atomic write test" {
		t.Errorf("expected 'atomic write test', got %q", string(data))
	}

	// Verify no temp files left behind
	entries, _ := os.ReadDir(workDir)
	for _, entry := range entries {
		if len(entry.Name()) > 10 && entry.Name()[:10] == ".m31a_tmp_" {
			t.Errorf("temp file %s left behind", entry.Name())
		}
	}
}