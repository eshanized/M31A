package investigate_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/intelligence/investigate"
	"github.com/stretchr/testify/require"
)

// mockGitRunner implements InvestigateGitRunner for testing without real git
type mockGitRunner struct {
	calls [][]string
	err   error
}

func (m *mockGitRunner) Run(args ...string) (string, error) {
	m.calls = append(m.calls, args)
	if m.err != nil {
		return "", m.err
	}
	return "", nil
}

func TestWorktreeManager_ValidationRejectsHostileRefs(t *testing.T) {
	mock := &mockGitRunner{}
	mgr := investigate.NewWorktreeManager(mock, t.TempDir())

	// These should all be rejected before any git invocation
	hostileRefs := []string{
		"--bad",
		"-bad",
		"bad; ls",
		"bad|ls",
		"bad&ls",
		"bad`ls`",
		"bad$(ls)",
		"bad..ref",
		".hidden",
		"path/traversal",
		"back\\slash",
		"spaces in ref",
		"tabs\tin",
	}

	for _, ref := range hostileRefs {
		t.Run(ref, func(t *testing.T) {
			mock.calls = nil
			_, err := mgr.Create(ref)
			require.Error(t, err)
			require.Len(t, mock.calls, 0, "git should not be called for hostile ref %q", ref)
		})
	}
}

func TestWorktreeManager_CreateCheckoutRemoveCycle(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)
	mgr := investigate.NewWorktreeManager(git, repo)

	// Create worktree at bad ref (HEAD)
	wtDir, err := mgr.Create("HEAD")
	require.NoError(t, err)
	require.NotEmpty(t, wtDir)
	require.True(t, filepath.IsAbs(wtDir))
	require.Contains(t, wtDir, "m31a-investigate-")

	// Verify we can checkout a specific commit
	commits, err := git.Run("log", "--format=%H", "-n", "3")
	require.NoError(t, err)
	shas := splitLines(commits)
	require.Len(t, shas, 3)

	// Checkout to the middle commit (good2)
	err = mgr.CheckoutTo(shas[1])
	require.NoError(t, err)

	// Verify HEAD is detached at that commit (in the worktree)
	wtGit := investigate.NewGitRunner(wtDir)
	head, err := wtGit.Run("rev-parse", "HEAD")
	require.NoError(t, err)
	require.Equal(t, shas[1], strings.TrimSpace(head))

	// Remove worktree
	err = mgr.Remove()
	require.NoError(t, err)

	// Remove should be idempotent
	err = mgr.Remove()
	require.NoError(t, err)
}

func TestWorktreeManager_OrphanPruneOnStartup(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	// Create first manager and worktree
	mgr1 := investigate.NewWorktreeManager(git, repo)
	wtDir, err := mgr1.Create("HEAD")
	require.NoError(t, err)

	// Simulate kill: delete worktree dir without calling Remove
	err = os.RemoveAll(wtDir)
	require.NoError(t, err)

	// Create second manager - should prune orphan and succeed
	mgr2 := investigate.NewWorktreeManager(git, repo)
	wtDir2, err := mgr2.Create("HEAD")
	require.NoError(t, err)
	require.NotEmpty(t, wtDir2)

	// Clean up
	err = mgr2.Remove()
	require.NoError(t, err)
}

func TestWorktreeManager_ParentRepoHEADUnchanged(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	// Capture parent repo HEAD before
	parentHeadBefore, err := git.Run("rev-parse", "HEAD")
	require.NoError(t, err)

	mgr := investigate.NewWorktreeManager(git, repo)
	_, err = mgr.Create("HEAD")
	require.NoError(t, err)

	// Checkout to an older commit in worktree
	commits, _ := git.Run("log", "--format=%H", "-n", "3")
	shas := splitLines(commits)
	err = mgr.CheckoutTo(shas[2])
	require.NoError(t, err)

	// Parent repo HEAD should be unchanged
	parentHeadAfter, err := git.Run("rev-parse", "HEAD")
	require.NoError(t, err)
	require.Equal(t, parentHeadBefore, parentHeadAfter, "parent repo HEAD must not move")

	err = mgr.Remove()
	require.NoError(t, err)
}

// Test helper functions

// seedRepoWithHistory creates a test repo with a known commit history:
// good1 -> good2 -> culprit -> good3 (HEAD)
func seedRepoWithHistory(t *testing.T) string {
	t.Helper()
	repo := t.TempDir()

	git := investigate.NewGitRunner(repo)
	_, err := git.Run("init")
	require.NoError(t, err)
	_, err = git.Run("config", "user.name", "Test User")
	require.NoError(t, err)
	_, err = git.Run("config", "user.email", "test@example.com")
	require.NoError(t, err)

	// good1
	require.NoError(t, os.WriteFile(filepath.Join(repo, "marker.txt"), []byte("good1"), 0o644))
	_, err = git.Run("add", "marker.txt")
	require.NoError(t, err)
	_, err = git.Run("commit", "-m", "good1")
	require.NoError(t, err)

	// good2
	require.NoError(t, os.WriteFile(filepath.Join(repo, "marker.txt"), []byte("good2"), 0o644))
	_, err = git.Run("add", "marker.txt")
	require.NoError(t, err)
	_, err = git.Run("commit", "-m", "good2")
	require.NoError(t, err)

	// culprit (breaks the test)
	require.NoError(t, os.WriteFile(filepath.Join(repo, "marker.txt"), []byte("culprit"), 0o644))
	_, err = git.Run("add", "marker.txt")
	require.NoError(t, err)
	_, err = git.Run("commit", "-m", "culprit")
	require.NoError(t, err)

	// good3
	require.NoError(t, os.WriteFile(filepath.Join(repo, "marker.txt"), []byte("good3"), 0o644))
	_, err = git.Run("add", "marker.txt")
	require.NoError(t, err)
	_, err = git.Run("commit", "-m", "good3")
	require.NoError(t, err)

	return repo
}

func splitLines(s string) []string {
	var lines []string
	for _, line := range splitLinesNoEmpty(s) {
		if line != "" {
			lines = append(lines, line)
		}
	}
	return lines
}

func splitLinesNoEmpty(s string) []string {
	var result []string
	for _, line := range split(s, "\n") {
		result = append(result, line)
	}
	return result
}

func split(s, sep string) []string {
	if s == "" {
		return []string{""}
	}
	var result []string
	start := 0
	for i := 0; i <= len(s)-len(sep); i++ {
		if s[i:i+len(sep)] == sep {
			result = append(result, s[start:i])
			start = i + len(sep)
			i += len(sep) - 1
		}
	}
	result = append(result, s[start:])
	return result
}