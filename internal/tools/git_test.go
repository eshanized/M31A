package tools

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/types"
)

func TestGit_ArgValidation(t *testing.T) {
	tests := []struct {
		name      string
		operation string
		args      []string
		wantErr   bool
	}{
		// Add operation
		{"add valid flag", "add", []string{"-f"}, false},
		{"add valid --dry-run", "add", []string{"--dry-run"}, false},
		{"add invalid --force", "add", []string{"--force"}, true},
		{"add invalid --no-index", "add", []string{"--no-index"}, true},

		// Commit operation
		{"commit valid -m", "commit", []string{"-m"}, false},
		{"commit valid --amend", "commit", []string{"--amend"}, false},
		{"commit invalid --force", "commit", []string{"--force"}, true},

		// Diff operation
		{"diff valid --stat", "diff", []string{"--stat"}, false},
		{"diff valid --cached", "diff", []string{"--cached"}, false},
		{"diff invalid --no-index", "diff", []string{"--no-index"}, true},

		// Log operation
		{"log valid --oneline", "log", []string{"--oneline"}, false},
		{"log valid -n", "log", []string{"-n"}, false},
		{"log invalid --force", "log", []string{"--force"}, true},

		// Branch operation
		{"branch valid -d", "branch", []string{"-d"}, false},
		{"branch valid --list", "branch", []string{"--list"}, false},
		{"branch invalid --force", "branch", []string{"--force"}, true},

		// Checkout operation
		{"checkout valid -b", "checkout", []string{"-b"}, false},
		{"checkout invalid --force", "checkout", []string{"--force"}, true},

		// Stash operation
		{"stash valid push", "stash", []string{"push"}, false},
		{"stash valid pop", "stash", []string{"pop"}, false},
		{"stash invalid --force", "stash", []string{"--force"}, true},

		// Status operation
		{"status valid --short", "status", []string{"--short"}, false},
		{"status valid --porcelain", "status", []string{"--porcelain"}, false},
		{"status invalid --force", "status", []string{"--force"}, true},

		// Empty args (no validation needed)
		{"empty args", "add", []string{}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateGitArgs(tt.operation, tt.args)
			if (err != nil) != tt.wantErr {
				t.Errorf("validateGitArgs(%q, %v) error = %v, wantErr %v", tt.operation, tt.args, err, tt.wantErr)
			}
		})
	}
}

func TestGit_ExtractCommitMessage(t *testing.T) {
	tests := []struct {
		name string
		args string
		want string
	}{
		{"simple message", "-m fix bug", "fix bug"},
		{"quoted message", "-m 'fix bug'", "fix bug"},
		{"double quoted", "-m \"fix bug\"", "fix bug"},
		{"multi word", "-m fix a serious bug", "fix a serious bug"},
		{"no -m flag", "fix bug", "fix bug"},
		{"empty args", "", ""},
		{"just -m", "-m", "-m"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractCommitMessage(tt.args)
			if got != tt.want {
				t.Errorf("extractCommitMessage(%q) = %q, want %q", tt.args, got, tt.want)
			}
		})
	}
}

func setupTestGit(t *testing.T) (*Git, string) {
	t.Helper()
	dir := t.TempDir()
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test User", "test@example.com")
	return NewGit(dir), dir
}

func TestGit_Operations(t *testing.T) {
	g, dir := setupTestGit(t)
	ctx := context.Background()

	// Create a test file
	testFile := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(testFile, []byte("hello"), 0644); err != nil {
		t.Fatal(err)
	}

	// Test add
	result, err := g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "add",
			"args":      "",
			"paths":     []any{"."},
		},
	})
	if err != nil {
		t.Fatalf("git add failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("git add error: %s", result.Error)
	}

	// Test status
	result, err = g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "status",
		},
	})
	if err != nil {
		t.Fatalf("git status failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("git status error: %s", result.Error)
	}

	// Test commit
	result, err = g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "commit",
			"args":      "-m test commit",
		},
	})
	if err != nil {
		t.Fatalf("git commit failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("git commit error: %s", result.Error)
	}

	// Test log
	result, err = g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "log",
		},
	})
	if err != nil {
		t.Fatalf("git log failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("git log error: %s", result.Error)
	}

	// Test diff (no changes)
	result, err = g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "diff",
		},
	})
	if err != nil {
		t.Fatalf("git diff failed: %v", err)
	}

	// Test branch
	result, err = g.Execute(ctx, types.ToolInput{
		Params: map[string]any{
			"operation": "branch",
		},
	})
	if err != nil {
		t.Fatalf("git branch failed: %v", err)
	}
	if result.Error != "" {
		t.Fatalf("git branch error: %s", result.Error)
	}
}
