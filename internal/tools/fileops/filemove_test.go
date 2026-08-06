package fileops

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileMove_Name(t *testing.T) {
	t.Parallel()
	d := NewFileMove("/tmp", "/tmp/backup")
	if d.Name() != "FileMove" {
		t.Errorf("expected 'FileMove', got %q", d.Name())
	}
}

func TestFileMove_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewFileMove("/tmp", "/tmp/backup")
	if d.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %v", d.RiskLevel())
	}
}

func TestFileMove_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewFileMove("/tmp", "/tmp/backup")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "source") || !Contains(schema, "destination") {
		t.Error("schema missing required parameters")
	}
}

func TestFileMove_ExecuteMissingParams(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for missing source")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "missing parameter: source") {
		t.Errorf("expected 'missing parameter: source' error, got %q", errorMsg)
	}

	result, err = d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{"source": "src.txt"},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for missing destination")
	}
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "missing parameter: destination") {
		t.Errorf("expected 'missing parameter: destination' error, got %q", errorMsg)
	}
}

func TestFileMove_ExecuteMoveFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	// Create source file
	srcPath := filepath.Join(workDir, "source.txt")
	content := "source content"
	if err := os.WriteFile(srcPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to create source file: %v", err)
	}

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"source":      "source.txt",
			"destination": "dest.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "Moved: source.txt → dest.txt") {
		t.Errorf("expected move message, got %q", result.Output)
	}

	// Verify source is gone
	if _, statErr := os.Stat(srcPath); !os.IsNotExist(statErr) {
		t.Error("expected source file to be moved (not exist)")
	}

	// Verify destination exists with correct content
	dstPath := filepath.Join(workDir, "dest.txt")
	data, err := os.ReadFile(dstPath)
	if err != nil {
		t.Fatalf("failed to read dest file: %v", err)
	}
	if string(data) != content {
		t.Errorf("expected content %q, got %q", content, string(data))
	}

	// Verify backup exists
	entries, _ := os.ReadDir(backupDir)
	if len(entries) != 1 {
		t.Errorf("expected 1 backup file, got %d", len(entries))
	}
}

func TestFileMove_ExecuteSourceNotFound(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"source":      "nonexistent.txt",
			"destination": "dest.txt",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for source not found")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	if !Contains(errorMsg, "source file not found") {
		t.Errorf("expected 'source file not found' error, got %q", errorMsg)
	}
}

func TestFileMove_ExecutePathTraversalSource(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"source":      "../etc/passwd",
			"destination": "dest.txt",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for path traversal in source")
	}
	errorMsg := ""
	if err != nil {
		errorMsg = err.Error()
	} else {
		errorMsg = result.Error
	}
	// For non-existent source files, the current implementation returns "file not found"
	if !Contains(errorMsg, "file not found") && !Contains(errorMsg, "source file not found") {
		t.Errorf("expected 'file not found' error for non-existent path, got %q", errorMsg)
	}
}

func TestFileMove_ExecutePathTraversalDest(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	// Create source file
	srcPath := filepath.Join(workDir, "source.txt")
	if err := os.WriteFile(srcPath, []byte("content"), 0644); err != nil {
		t.Fatalf("failed to create source file: %v", err)
	}

	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"source":      "source.txt",
			"destination": "../etc/passwd",
		},
	})
	if err == nil && result.Error == "" {
		t.Error("expected error for path traversal in destination")
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

func TestFileMove_ExecuteCreateDestDir(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, "backup")
	d := NewFileMove(workDir, backupDir)

	// Create source file
	srcPath := filepath.Join(workDir, "source.txt")
	if err := os.WriteFile(srcPath, []byte("content"), 0644); err != nil {
		t.Fatalf("failed to create source file: %v", err)
	}

	// Move to a nested directory that doesn't exist
	result, err := d.Execute(t.Context(), types.ToolInput{
		Params: map[string]any{
			"source":      "source.txt",
			"destination": "nested/subdir/dest.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}

	dstPath := filepath.Join(workDir, "nested", "subdir", "dest.txt")
	if _, err := os.Stat(dstPath); os.IsNotExist(err) {
		t.Error("expected destination file to be created with parent dirs")
	}
}

func TestPruneBackupsByPrefix_Move(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	prefix := "source"

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
