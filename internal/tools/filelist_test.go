package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestFileList_Execute_EmptyDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)

	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name:   "FileList",
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
}

func TestFileList_Execute_SubDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	subdir := filepath.Join(dir, "target")
	os.MkdirAll(subdir, 0o755)
	os.WriteFile(filepath.Join(subdir, "file.txt"), []byte("content"), 0o644)

	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": "target",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "file.txt") {
		t.Errorf("expected 'file.txt' in output, got: %s", result.Output)
	}
}

func TestFileList_Execute_MaxDepth(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	deep := filepath.Join(dir, "a", "b", "c", "d")
	os.MkdirAll(deep, 0o755)
	os.WriteFile(filepath.Join(deep, "deep.txt"), []byte("x"), 0o644)

	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"depth": float64(2),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// With depth 2, deeply nested files should not appear
	if strings.Contains(result.Output, "deep.txt") {
		t.Errorf("deeply nested file should not appear with depth 2")
	}
}

func TestFileList_Execute_NotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)

	_, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": "nonexistent",
		},
	})
	if err == nil {
		t.Fatal("expected error for nonexistent directory")
	}
}

func TestFileList_Execute_NonStringPath(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)

	// Non-string path should use default directory
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": 123,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
}

func TestFileList_Execute_WithFiles(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "a.go"), []byte("package main"), 0o644)
	os.WriteFile(filepath.Join(dir, "b.txt"), []byte("hello"), 0o644)

	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name:   "FileList",
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "a.go") {
		t.Errorf("expected 'a.go' in output")
	}
	if !strings.Contains(result.Output, "b.txt") {
		t.Errorf("expected 'b.txt' in output")
	}
}
