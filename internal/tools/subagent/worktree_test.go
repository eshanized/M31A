package subagent

import (
	"testing"
	"path/filepath"
)

func TestSanitizePath_SpecialChars(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input, want string
	}{
		{"hello world", "hello_world"},
		{"a/b/c", "a_b_c"},
		{"..", "__"},
		{".", "_"},
		{"...", "___"},
		{"@#$%", "____"},
		{"agent-abc-123", "agent-abc-123"},
	}
	for _, tt := range tests {
		got := sanitizePath(tt.input)
		if got != tt.want {
			t.Errorf("sanitizePath(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestWorktreeBranchName_WithSuffix(t *testing.T) {
	t.Parallel()
	got := worktreeBranchName("abc", "my branch!")
	want := "m31a/agent-abc-my_branch_"
	if got != want {
		t.Errorf("worktreeBranchName(%q, %q) = %q, want %q", "abc", "my branch!", got, want)
	}
}

func TestParseWorktreeList_AllBranchesAndPaths(t *testing.T) {
	t.Parallel()
	porcelain := `worktree /path/to/wt1
HEAD abc123
branch refs/heads/m31a/agent-abc
worktree /path/to/wt2
HEAD def456
branch refs/heads/main
worktree /path/to/wt3
HEAD ghi789
branch refs/heads/m31a/agent-def-fix
`
	branches, paths := parseWorktreeList(porcelain)
	// The function returns ALL branches from porcelain output
	if len(branches) != 3 {
		t.Fatalf("expected 3 branches, got %d: %v", len(branches), branches)
	}
	if !branches["m31a/agent-abc"] {
		t.Error("missing m31a/agent-abc")
	}
	if !branches["main"] {
		t.Error("missing main")
	}
	if !branches["m31a/agent-def-fix"] {
		t.Error("missing m31a/agent-def-fix")
	}
	// Verify paths are also parsed
	if len(paths) != 3 {
		t.Fatalf("expected 3 paths, got %d: %v", len(paths), paths)
	}
	if !paths["/path/to/wt1"] {
		t.Error("missing /path/to/wt1")
	}
	if !paths["/path/to/wt2"] {
		t.Error("missing /path/to/wt2")
	}
	if !paths["/path/to/wt3"] {
		t.Error("missing /path/to/wt3")
	}
}

func TestGitWorktrees_RootFor_EmptyCustom(t *testing.T) {
	t.Parallel()
	g := &GitWorktrees{Root: ""}
	got := g.rootFor("/home/user/project")
	want := filepath.Join("/home/user/project", ".m31a-worktrees")
	if got != want {
		t.Errorf("rootFor() = %q, want %q (empty Root should use default)", got, want)
	}
}
