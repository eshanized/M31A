package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestFileDelete_Execute_DeleteWithBackup(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)

	testFile := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(testFile, []byte("hello"), 0o644); err != nil {
		t.Fatal(err)
	}

	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path": "test.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Deleted") {
		t.Errorf("expected 'Deleted' in output, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "backup saved") {
		t.Errorf("expected 'backup saved' in output, got: %s", result.Output)
	}
}

func TestFileDelete_Execute_Permanent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)

	testFile := filepath.Join(dir, "perm.txt")
	if err := os.WriteFile(testFile, []byte("data"), 0o644); err != nil {
		t.Fatal(err)
	}

	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path":      "perm.txt",
			"permanent": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Contains(result.Output, "backup") {
		t.Errorf("should not mention backup for permanent delete, got: %s", result.Output)
	}
}

func TestFileDelete_Execute_DirectoryBlocked(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fd := NewFileDelete(dir, t.TempDir())

	subdir := filepath.Join(dir, "subdir")
	os.MkdirAll(subdir, 0o755)

	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path": "subdir",
		},
	})
	if err == nil {
		t.Fatal("expected error for directory deletion")
	}
	if !strings.Contains(err.Error(), "cannot delete directory") {
		t.Errorf("expected directory error, got: %v", err)
	}
}

func TestFileDelete_PruneBackups(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)

	// Create multiple backups to test pruning
	for i := 0; i < MaxBackupsPerFile+2; i++ {
		os.WriteFile(filepath.Join(backupDir, "test.txt.deleted.2024010100000"+string(rune('0'+i))), []byte("x"), 0o644)
	}

	// pruneBackups should not panic
	PruneBackupsByPrefix(fd.BackupDir, "test.txt.deleted", MaxBackupsPerFile)
}
