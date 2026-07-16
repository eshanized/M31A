package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestFileMove_Execute_RelativePaths(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir, t.TempDir())

	src := filepath.Join(dir, "old.txt")
	dst := filepath.Join(dir, "new.txt")
	os.WriteFile(src, []byte("content"), 0o644)

	result, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "old.txt",
			"destination": "new.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Moved") {
		t.Errorf("expected 'Moved' in output, got: %s", result.Output)
	}

	// Verify source gone, destination exists
	if _, err := os.Stat(src); !os.IsNotExist(err) {
		t.Error("source should be gone")
	}
	if _, err := os.Stat(dst); err != nil {
		t.Errorf("destination should exist: %v", err)
	}
}

func TestFileMove_Execute_CreatesDirs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir, t.TempDir())

	src := filepath.Join(dir, "a.txt")
	dst := filepath.Join(dir, "sub", "b.txt")
	os.WriteFile(src, []byte("data"), 0o644)

	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "a.txt",
			"destination": "sub/b.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	data, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("destination should exist: %v", err)
	}
	if string(data) != "data" {
		t.Errorf("expected 'data', got %q", string(data))
	}
}
