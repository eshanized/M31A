package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

func TestFileRead_SimpleRead(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := "hello world\nthis is a test\n"
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte(content), 0644)

	fr := NewFileRead(dir)
	result, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "test.txt",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if result.Output != content {
		t.Errorf("expected %q, got %q", content, result.Output)
	}
}

func TestFileRead_FileNotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fr := NewFileRead(dir)
	_, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "nonexistent.txt",
		},
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "file not found") {
		t.Errorf("expected file not found error, got: %v", err)
	}
}

func TestFileRead_TooLarge(t *testing.T) {
	dir := t.TempDir()
	filePath := filepath.Join(dir, "large.bin")
	f, err := os.Create(filePath)
	if err != nil {
		t.Fatal(err)
	}
	// Create a 6MB sparse file
	if err := f.Truncate(6 * 1024 * 1024); err != nil {
		t.Fatal(err)
	}
	f.Close()

	fr := NewFileRead(dir)
	_, err = fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "large.bin",
		},
	})
	if err != m31errors.ErrFileTooLarge {
		t.Errorf("expected ErrFileTooLarge, got: %v", err)
	}
}

func TestFileRead_BinaryFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "data.bin"), []byte("ELF\x00\x01\x02\x03hello"), 0644)

	fr := NewFileRead(dir)
	result, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "data.bin",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(result.Output, "[binary file") {
		t.Errorf("expected binary file placeholder, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "bytes]") {
		t.Errorf("expected byte count in binary placeholder, got: %s", result.Output)
	}
}

func TestFileRead_PathOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fr := NewFileRead(dir)
	_, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "/etc/hostname",
		},
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "outside working directory") {
		t.Errorf("expected path outside working directory error, got: %v", err)
	}
}

func TestFileRead_SymlinkOutsideWorkDir(t *testing.T) {
	dir := t.TempDir()
	outsideFile := filepath.Join(dir, "..", "target.txt")
	os.WriteFile(outsideFile, []byte("outside"), 0644)

	symlinkPath := filepath.Join(dir, "link")
	if err := os.Symlink(outsideFile, symlinkPath); err != nil {
		t.Skip("symlinks not supported on this system")
	}

	fr := NewFileRead(dir)
	_, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "link",
		},
	})
	if err == nil {
		t.Fatal("expected error for symlink outside workDir")
	}
	if !strings.Contains(err.Error(), "outside working directory") {
		t.Errorf("expected outside working directory error, got: %v", err)
	}
}

func TestFileRead_Directory(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fr := NewFileRead(dir)
	_, err := fr.Execute(context.Background(), types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": ".",
		},
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "is a directory") {
		t.Errorf("expected directory error, got: %v", err)
	}
}

func TestFileRead_MissingPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fr := NewFileRead(dir)
	_, err := fr.Execute(context.Background(), types.ToolInput{
		Name:   "FileRead",
		Params: map[string]any{},
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "missing parameter: path") {
		t.Errorf("expected missing parameter error, got: %v", err)
	}
}
