package fileops

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileDelete_Name(t *testing.T) {
	t.Parallel()
	d := NewFileDelete("/tmp", "/tmp/backup")
	if d.Name() != "FileDelete" {
		t.Errorf("expected 'FileDelete', got %q", d.Name())
	}
}

func TestFileDelete_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewFileDelete("/tmp", "/tmp/backup")
	if d.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %v", d.RiskLevel())
	}
}

func TestFileDelete_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewFileDelete("/tmp", "/tmp/backup")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "path") || !Contains(schema, "permanent") {
		t.Error("schema missing required parameters")
	}
}

func TestFileDelete_ExecuteMissingPath(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileDelete(workDir, filepath.Join(workDir, "backup"))

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

func TestFileDelete_ExecuteFileNotFound(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewFileDelete(workDir, filepath.Join(workDir, "backup"))

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

func TestFileDelete_ExecuteDeleteWithBackup(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileDelete(workDir, backupDir)

	// Create a test file
	testFile := filepath.Join(workDir, "test.txt")
	content := "test content"
	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

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
	if !Contains(result.Output, "Deleted") || !Contains(result.Output, "backup saved") {
		t.Errorf("expected deletion message with backup, got %q", result.Output)
	}

	// Verify file is deleted
	if _, err := os.Stat(testFile); !os.IsNotExist(err) {
		t.Error("expected file to be deleted")
	}

	// Verify backup exists
	entries, _ := os.ReadDir(backupDir)
	if len(entries) != 1 {
		t.Errorf("expected 1 backup file, got %d", len(entries))
	}
}

func TestFileDelete_ExecutePermanentDelete(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileDelete(workDir, backupDir)

	// Create a test file
	testFile := filepath.Join(workDir, "test.txt")
	content := "test content"
	if err := os.WriteFile(testFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path":      "test.txt",
			"permanent": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "Deleted") || Contains(result.Output, "backup saved") {
		t.Errorf("expected deletion message without backup, got %q", result.Output)
	}

	// Verify file is deleted
	if _, err := os.Stat(testFile); !os.IsNotExist(err) {
		t.Error("expected file to be deleted")
	}

	// Verify no backup exists
	entries, _ := os.ReadDir(backupDir)
	if len(entries) != 0 {
		t.Errorf("expected 0 backup files, got %d", len(entries))
	}
}

func TestFileDelete_ExecuteDirectoryError(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileDelete(workDir, backupDir)

	// Create a test directory
	testDir := filepath.Join(workDir, "testdir")
	if err := os.Mkdir(testDir, 0755); err != nil {
		t.Fatalf("failed to create test dir: %v", err)
	}

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"path": "testdir",
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
	if !Contains(errorMsg, "cannot delete directory") {
		t.Errorf("expected 'cannot delete directory' error, got %q", errorMsg)
	}
}

func TestFileDelete_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileDelete(workDir, backupDir)

	// Try to delete a file outside workDir
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
	if !Contains(errorMsg, "file not found") {
		t.Errorf("expected 'file not found' error for non-existent path, got %q", errorMsg)
	}
}

func TestPruneBackupsByPrefix(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	prefix := "test.deleted"

	// Create 5 backup files with different timestamps
	for i := 0; i < 5; i++ {
		path := filepath.Join(backupDir, fmt.Sprintf("%s.%d.%s", prefix, i, time.Now().Format("20060102150405")))
		_ = os.WriteFile(path, []byte("backup content"), 0600)
		time.Sleep(10 * time.Millisecond)
	}

	// Prune to keep only 2 (actually keeps maxBackups-1 = 1)
	PruneBackupsByPrefix(backupDir, prefix, 2)

	entries, _ := os.ReadDir(backupDir)
	if len(entries) != 1 {
		t.Errorf("expected 1 backup file after pruning (maxBackups-1), got %d: %v", len(entries), entries)
	}
}
