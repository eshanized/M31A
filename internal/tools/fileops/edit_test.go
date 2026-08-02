package fileops

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestEdit_ExecuteExactMatch(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, ".backups")

	// Create test file
	testFile := filepath.Join(workDir, "test.txt")
	originalContent := "hello world\nhello universe\n"
	err := os.WriteFile(testFile, []byte(originalContent), 0644)
	if err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	e := NewEdit(workDir, backupDir)

	result, err := e.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "world",
			"new_string": "go",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Verify file was modified
	newContent, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	expected := "hello go\nhello universe\n"
	if string(newContent) != expected {
		t.Errorf("expected %q, got %q", expected, string(newContent))
	}

	if result.Output == "" {
		t.Errorf("expected non-empty output")
	}
}

func TestEdit_ExecuteReplaceAll(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, ".backups")

	testFile := filepath.Join(workDir, "test.txt")
	originalContent := "a b c a b c\n"
	err := os.WriteFile(testFile, []byte(originalContent), 0644)
	if err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	e := NewEdit(workDir, backupDir)

	result, err := e.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":        "test.txt",
			"old_string":  "a",
			"new_string":  "x",
			"replace_all": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	newContent, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	expected := "x b c x b c\n"
	if string(newContent) != expected {
		t.Errorf("expected %q, got %q", expected, string(newContent))
	}

	if result.Output == "" {
		t.Errorf("expected non-empty output")
	}
}

func TestEdit_ExecuteLineRange(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, ".backups")

	testFile := filepath.Join(workDir, "test.txt")
	originalContent := "line 1\nline 2\nline 3\nline 4\n"
	err := os.WriteFile(testFile, []byte(originalContent), 0644)
	if err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	e := NewEdit(workDir, backupDir)

	result, err := e.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "test.txt",
			"start_line": float64(2),
			"end_line":   float64(3),
			"new_string": "replaced\nlines",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	newContent, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatalf("failed to read file: %v", err)
	}
	expected := "line 1\nreplaced\nlines\nline 4\n"
	if string(newContent) != expected {
		t.Errorf("expected %q, got %q", expected, string(newContent))
	}

	if result.Output == "" {
		t.Errorf("expected non-empty output")
	}
}

func TestEdit_ExecuteNoMatch(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := filepath.Join(workDir, ".backups")

	testFile := filepath.Join(workDir, "test.txt")
	originalContent := "hello world\n"
	err := os.WriteFile(testFile, []byte(originalContent), 0644)
	if err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	e := NewEdit(workDir, backupDir)

	result, err := e.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "nonexistent",
			"new_string": "replacement",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Errorf("expected error output for no match, got Output=%q", result.Output)
	}
}

func TestEdit_RiskLevel(t *testing.T) {
	t.Parallel()
	e := NewEdit("", "")
	if e.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %v", e.RiskLevel())
	}
}

func TestEdit_Name(t *testing.T) {
	t.Parallel()
	e := NewEdit("", "")
	if e.Name() != "Edit" {
		t.Errorf("expected 'Edit', got %q", e.Name())
	}
}
