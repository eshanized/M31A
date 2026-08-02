package exec

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestOutputStore_Bound_NoTruncation(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 100, 10000)

	output := "short output"
	bounded, savedPath, truncated := store.Bound(output)

	if truncated {
		t.Error("expected no truncation for short output")
	}
	if savedPath != "" {
		t.Errorf("expected empty saved path, got %s", savedPath)
	}
	if bounded != output {
		t.Errorf("expected unchanged output, got %q", bounded)
	}
}

func TestOutputStore_Bound_TruncationByLines(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 10, 10000)

	// Create output with 20 lines
	lines := make([]string, 20)
	for i := range lines {
		lines[i] = "line content"
	}
	output := ""
	for i, line := range lines {
		if i > 0 {
			output += "\n"
		}
		output += line
	}

	bounded, savedPath, truncated := store.Bound(output)

	if !truncated {
		t.Error("expected truncation for long output")
	}
	if savedPath == "" {
		t.Error("expected saved path for truncated output")
	}
	// Check that file was created
	if _, err := os.Stat(savedPath); err != nil {
		t.Errorf("expected saved file to exist: %v", err)
	}
	// Check that output contains truncation marker
	if !Contains(bounded, "... output truncated") {
		t.Errorf("expected truncation marker in output, got %q", bounded)
	}
	// Check preview has head and tail
	if !Contains(bounded, "line content") {
		t.Errorf("expected preview to contain content")
	}
}

func TestOutputStore_Bound_TruncationByBytes(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 1000, 50)

	output := "this is a very long output that exceeds the byte limit"
	_, savedPath, truncated := store.Bound(output)

	if !truncated {
		t.Error("expected truncation by byte limit")
	}
	if savedPath == "" {
		t.Error("expected saved path")
	}
}

func TestOutputStore_Bound_EmptyOutput(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 100, 10000)

	bounded, savedPath, truncated := store.Bound("")

	if truncated {
		t.Error("expected no truncation for empty output")
	}
	if savedPath != "" {
		t.Error("expected empty saved path")
	}
	if bounded != "" {
		t.Error("expected empty output")
	}
}

func TestOutputStore_SaveFull(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 100, 10000)

	output := "test content to save"
	savedPath, err := store.saveFull(output)
	if err != nil {
		t.Fatalf("saveFull failed: %v", err)
	}

	if savedPath == "" {
		t.Error("expected non-empty path")
	}

	// Verify file content
	content, err := os.ReadFile(savedPath)
	if err != nil {
		t.Fatalf("failed to read saved file: %v", err)
	}
	if string(content) != output {
		t.Errorf("saved content mismatch: expected %q, got %q", output, string(content))
	}

	// Verify permissions are restrictive (0600)
	info, err := os.Stat(savedPath)
	if err != nil {
		t.Fatalf("failed to stat file: %v", err)
	}
	if info.Mode().Perm() != 0600 {
		t.Errorf("expected file permissions 0600, got %o", info.Mode().Perm())
	}
}

func TestOutputStore_HeadTailPreview(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 10, 10000)

	// 20 lines
	lines := make([]string, 20)
	for i := range lines {
		lines[i] = "content"
	}
	output := ""
	for i, line := range lines {
		if i > 0 {
			output += "\n"
		}
		output += line
	}

	preview := store.headTailPreview(output)

	// Should have head (60% = 6 lines) + separator (3 lines) + tail (4 lines) = 13 lines total
	previewLines := SplitLines(preview)
	if len(previewLines) > 13 {
		t.Errorf("preview should not exceed 13 lines (head + separator + tail), got %d lines", len(previewLines))
	}
}

func TestOutputStore_Cleanup(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 100, 10000)

	// Create some test files
	for i := 0; i < 5; i++ {
		path := filepath.Join(tmpDir, fmt.Sprintf("test%d.txt", i))
		_ = os.WriteFile(path, []byte("content"), 0600)
		time.Sleep(time.Millisecond) // ensure different timestamps
	}

	// Use 0 retention to delete all files immediately
	removed, err := store.Cleanup(0)
	if err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}
	if removed != 5 {
		t.Errorf("expected 5 files removed, got %d", removed)
	}

	// Verify files are gone
	entries, _ := os.ReadDir(tmpDir)
	if len(entries) != 0 {
		t.Errorf("expected empty directory, got %d entries", len(entries))
	}
}

func TestOutputStore_Cleanup_NotExist(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	nonExistent := filepath.Join(tmpDir, "nonexistent")
	store := NewOutputStore(nonExistent, 100, 10000)

	removed, err := store.Cleanup(24 * time.Hour)
	if err != nil {
		t.Fatalf("Cleanup failed: %v", err)
	}
	if removed != 0 {
		t.Errorf("expected 0 files removed, got %d", removed)
	}
}

func TestOutputStore_TruncateInPlace(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 5, 10000)

	output := "line 1\nline 2\nline 3\nline 4\nline 5\nline 6"
	result := store.truncateInPlace(output)

	// Should be truncated with head+tail and marker
	if !Contains(result, "truncated") {
		t.Errorf("expected truncation marker in fallback output, got %q", result)
	}
}

func TestOutputStore_NewOutputStore_Defaults(t *testing.T) {
	t.Parallel()
	tmpDir := t.TempDir()
	store := NewOutputStore(tmpDir, 0, 0)

	// Should use defaults from types
	if store.maxLines != types.DefaultOutputMaxLines {
		t.Errorf("expected maxLines=%d, got %d", types.DefaultOutputMaxLines, store.maxLines)
	}
	if store.maxBytes != types.DefaultOutputMaxBytes {
		t.Errorf("expected maxBytes=%d, got %d", types.DefaultOutputMaxBytes, store.maxBytes)
	}
}
