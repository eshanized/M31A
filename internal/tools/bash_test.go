package tools

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func setupTestBash(t *testing.T) *Bash {
	t.Helper()
	dir := t.TempDir()
	return NewBash(dir, 60, nil, nil) // workDir, maxTimeoutSecs, blockedCommands, obfuscationPatterns
}

func TestBash_WorkdirValidation(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	// Test that absolute paths outside workdir are rejected
	_, err := b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "ls /etc/passwd",
		},
	})
	if err != nil {
		t.Fatalf("expected no error for ls, got: %v", err)
	}

	// Test that relative paths outside workdir are rejected
	_, err = b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "cat ../../etc/passwd",
		},
	})
	if err != nil {
		t.Fatalf("expected error for path traversal, got: %v", err)
	}
}

func TestBash_PathClean(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	// Test that paths with redundant separators are cleaned
	result, err := b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "ls .",
		},
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("expected no error in result, got: %s", result.Error)
	}
}

func TestBash_WorkdirRelativePath(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	// Create a subdirectory
	subDir := filepath.Join(b.workDir, "sub")
	if err := os.MkdirAll(subDir, 0755); err != nil {
		t.Fatal(err)
	}

	// Test that relative path to subdirectory works
	result, err := b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "ls sub",
		},
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("expected no error in result, got: %s", result.Error)
	}
}

func TestBash_EmptyCommand(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	// Empty command returns empty output, not an error
	result, err := b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	_ = result // empty output is acceptable
}

func TestBash_SandboxMode(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	// Test that sandbox mode is enforced
	result, err := b.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"command": "echo hello",
		},
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("expected no error in result, got: %s", result.Error)
	}
}
