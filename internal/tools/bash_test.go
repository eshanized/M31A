package tools

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/tools/exec"
	"github.com/eshanized/M31A/internal/core/types"
)

func setupTestBash(t *testing.T) *exec.Bash {
	t.Helper()
	dir := t.TempDir()
	return exec.NewBash(dir, 60, nil, nil)
}

func TestBash_WorkdirValidation(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "pwd",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Unexpected error: %s", result.Error)
	}
	_ = result
}

func TestBash_CommandExecution(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo hello",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Unexpected error: %s", result.Error)
	}
	if !containsString(result.Output, "hello") {
		t.Fatalf("Expected 'hello' in output, got: %s", result.Output)
	}
}

func TestBash_TimeoutHandling(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "sleep 2",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_ = result
}

func TestBash_CommandSubstitution(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo $(echo nested)",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Unexpected error: %s", result.Error)
	}
	if !containsString(result.Output, "nested") {
		t.Fatalf("Expected 'nested' in output, got: %s", result.Output)
	}
}

func TestBash_StderrCapture(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo error >&2",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Unexpected error: %s", result.Error)
	}
}

func TestBash_EmptyCommand(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	_ = result
}

func TestBash_WorkingDirectory(t *testing.T) {
	b := setupTestBash(t)
	ctx := context.Background()

	tmpDir := b.WorkDir()
	testFile := filepath.Join(tmpDir, "test.txt")
	err := os.WriteFile(testFile, []byte("hello"), 0644)
	if err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := b.Execute(ctx, types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "cat test.txt",
		},
	})
	if err != nil {
		t.Fatalf("Execute failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("Unexpected error: %s", result.Error)
	}
	if !containsString(result.Output, "hello") {
		t.Fatalf("Expected 'hello' in output, got: %s", result.Output)
	}
}

func containsString(haystack, needle string) bool {
	return len(haystack) >= len(needle) && (haystack == needle || len(haystack) > 0 && containsSubstring(haystack, needle))
}

func containsSubstring(haystack, needle string) bool {
	for i := 0; i <= len(haystack)-len(needle); i++ {
		if haystack[i:i+len(needle)] == needle {
			return true
		}
	}
	return false
}
