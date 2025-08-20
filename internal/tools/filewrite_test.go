package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func TestFileWrite_SimpleWrite(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	content := "hello world"
	result, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "test.txt",
			"content": content,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "Wrote") {
		t.Errorf("expected success message, got: %s", result.Output)
	}

	// Verify written content
	read, err := os.ReadFile(filepath.Join(dir, "test.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if string(read) != content {
		t.Errorf("expected %q, got %q", content, string(read))
	}
}

func TestFileWrite_OverwriteWithBackup(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	// First write
	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "file.txt",
			"content": "version 1",
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	// Second write (overwrite)
	_, err = fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "file.txt",
			"content": "version 2",
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	// Verify final content
	read, err := os.ReadFile(filepath.Join(dir, "file.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if string(read) != "version 2" {
		t.Errorf("expected 'version 2', got %q", string(read))
	}

	// Verify backup exists
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) == 0 {
		t.Fatal("expected at least one backup file")
	}
	backupContent, err := os.ReadFile(filepath.Join(backupDir, entries[0].Name()))
	if err != nil {
		t.Fatal(err)
	}
	if string(backupContent) != "version 1" {
		t.Errorf("expected backup to contain 'version 1', got %q", string(backupContent))
	}
}

func TestFileWrite_CreateDirs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "nested/deep/dir/file.txt",
			"content": "deep content",
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	// Verify file exists in nested directory
	read, err := os.ReadFile(filepath.Join(dir, "nested", "deep", "dir", "file.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if string(read) != "deep content" {
		t.Errorf("expected 'deep content', got %q", string(read))
	}
}

func TestFileWrite_PathOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "/etc/evil.txt",
			"content": "malicious",
		},
	})
	if err == nil {
		t.Fatal("expected error for path outside workDir")
	}
	if !strings.Contains(err.Error(), "outside working directory") {
		t.Errorf("expected 'outside working directory', got: %v", err)
	}
}

func TestFileWrite_Atomicty(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "atomic.txt",
			"content": "atomic content",
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	// Verify no temp files remain
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".m31a_tmp_") {
			t.Errorf("temp file left behind: %s", e.Name())
		}
	}
}

func TestFileWrite_BinaryContent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "binary.bin",
			"content": "hello\x00world",
		},
	})
	if err != m31errors.ErrNoBinaryContent {
		t.Errorf("expected ErrNoBinaryContent, got: %v", err)
	}
}

func TestFileWrite_MissingPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"content": "test",
		},
	})
	if err == nil {
		t.Fatal("expected error for missing path param")
	}
	if !strings.Contains(err.Error(), "missing parameter: path") {
		t.Errorf("expected 'missing parameter: path', got: %v", err)
	}
}

func TestFileWrite_MissingContentParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path": "test.txt",
		},
	})
	if err == nil {
		t.Fatal("expected error for missing content param")
	}
	if !strings.Contains(err.Error(), "missing parameter: content") {
		t.Errorf("expected 'missing parameter: content', got: %v", err)
	}
}

func TestFileWrite_Name(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fw := NewFileWrite(dir, dir)
	if fw.Name() != "FileWrite" {
		t.Errorf("expected name 'FileWrite', got %s", fw.Name())
	}
}

func TestFileWrite_Description(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fw := NewFileWrite(dir, dir)
	if fw.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestFileWrite_RiskLevel(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fw := NewFileWrite(dir, dir)
	if fw.RiskLevel() != types.RiskDestructive {
		t.Errorf("expected RiskDestructive, got %s", fw.RiskLevel())
	}
}

func TestFileWrite_CreateDirsDisabled(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":        "nested/deep/dir/file.txt",
			"content":     "deep content",
			"create_dirs": false,
		},
	})
	if err == nil {
		t.Fatal("expected error when create_dirs is false and dirs don't exist")
	}
}

func TestFileWrite_AbsolutePath(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	content := "absolute path content"
	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    filepath.Join(dir, "abs_test.txt"),
			"content": content,
		},
	})
	if err != nil {
		t.Fatal(err)
	}

	read, err := os.ReadFile(filepath.Join(dir, "abs_test.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if string(read) != content {
		t.Errorf("expected %q, got %q", content, string(read))
	}
}

func TestFileWrite_PathNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    123, // not a string
			"content": "test",
		},
	})
	if err == nil {
		t.Fatal("expected error for non-string path")
	}
	if !strings.Contains(err.Error(), "parameter path must be a string") {
		t.Errorf("expected type error, got: %v", err)
	}
}

func TestFileWrite_ContentNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "test.txt",
			"content": 123, // not a string
		},
	})
	if err == nil {
		t.Fatal("expected error for non-string content")
	}
	if !strings.Contains(err.Error(), "parameter content must be a string") {
		t.Errorf("expected type error, got: %v", err)
	}
}

func TestFileWrite_BackupPruning(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	// Write the file 12 times to trigger pruning (max is 10)
	for i := 0; i < 12; i++ {
		_, err := fw.Execute(context.Background(), types.ToolInput{
			Name: "FileWrite",
			Params: map[string]any{
				"path":    "prune_test.txt",
				"content": fmt.Sprintf("version %d", i),
			},
		})
		if err != nil {
			t.Fatalf("write %d failed: %v", i, err)
		}
	}

	// Verify at most 10 backups exist
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) > MaxBackupsPerFile {
		t.Errorf("expected at most %d backups, got %d", MaxBackupsPerFile, len(entries))
	}
}
