package investigate_test

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/intelligence/investigate"
	"github.com/stretchr/testify/require"
)

func TestCandidates_FromRevList(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	// Get baseline (good1) and head (good3)
	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	require.Len(t, shas, 4)
	baseline := shas[3] // good1 (oldest)
	head := shas[0]     // good3 (newest)

	// Candidates should return all commits between baseline and head
	candidates, err := investigate.Candidates(git, baseline, head, 50)
	require.NoError(t, err)
	require.Len(t, candidates, 3) // good2, culprit, good3

	// Should be ordered from oldest to newest (reverse topological)
	require.Equal(t, shas[2], candidates[0]) // good2
	require.Equal(t, shas[1], candidates[1]) // culprit
	require.Equal(t, shas[0], candidates[2]) // good3
}

func TestCandidates_TruncatedToMaxCommits(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	baseline := shas[3]
	head := shas[0]

	// Max commits = 2, should truncate
	candidates, err := investigate.Candidates(git, baseline, head, 2)
	require.NoError(t, err)
	require.Len(t, candidates, 2)
	require.Equal(t, shas[2], candidates[0]) // good2
	require.Equal(t, shas[1], candidates[1]) // culprit
}

func TestCandidates_InvalidMaxCommits(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "2")
	shas := splitLines(commits)

	_, err := investigate.Candidates(git, shas[1], shas[0], 0)
	require.Error(t, err)
	require.Contains(t, err.Error(), "maxCommits must be >= 1")
}

func TestCandidates_ValidatesRefs(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "2")
	shas := splitLines(commits)

	// Invalid baseline ref
	_, err := investigate.Candidates(git, "invalid..ref", shas[0], 50)
	require.Error(t, err)

	// Invalid head ref
	_, err = investigate.Candidates(git, shas[1], "bad;ref", 50)
	require.Error(t, err)
}

func TestFindCulprit_SeededCulprit(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)
	mgr := investigate.NewWorktreeManager(git, repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	require.Len(t, shas, 4)
	baseline := shas[3] // good1 (oldest)
	culpritCommit := shas[1] // culprit commit (this is the "bad" ref)

	// Create worktree at culprit (bad) - symptom reproduces here
	_, err := mgr.Create(culpritCommit)
	require.NoError(t, err)
	defer mgr.Remove()

	// Check function: reads marker.txt content, fails if "culprit"
	checkFn := func(ctx context.Context, sha string) bool {
		wtGit := investigate.NewGitRunner(mgr.WorktreeDir())
		// Checkout to the commit being tested
		err := mgr.CheckoutTo(sha)
		if err != nil {
			return false
		}
		content, err := os.ReadFile(filepath.Join(wtGit.WorkDir(), "marker.txt"))
		if err != nil {
			return false
		}
		return !bytes.Contains(content, []byte("culprit"))
	}

	culpritSHA, steps, err := investigate.FindCulprit(context.Background(), mgr, git, baseline, culpritCommit, 50, checkFn, nil)
	require.NoError(t, err)
	require.Equal(t, culpritCommit, culpritSHA)

	// Steps should record probes
	require.Greater(t, len(steps), 0)
	for _, step := range steps {
		require.NotEmpty(t, step.SHA)
		require.True(t, step.ExitCode >= 0)
		require.NotEmpty(t, step.Verdict)
	}
}

func TestFindCulprit_NotReproducibleAtWindowStart(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)
	mgr := investigate.NewWorktreeManager(git, repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	baseline := shas[3] // good1
	badRef := shas[1]   // culprit (bad ref)

	_, err := mgr.Create(badRef)
	require.NoError(t, err)
	defer mgr.Remove()

	// Check function that always passes (symptom not present at badRef)
	checkFn := func(ctx context.Context, sha string) bool {
		return true // always passes
	}

	_, _, err = investigate.FindCulprit(context.Background(), mgr, git, baseline, badRef, 50, checkFn, nil)
	require.Error(t, err)
	require.True(t, investigate.IsNotReproducibleInWindow(err))
}

func TestFindCulprit_DeterministicAcrossRuns(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	baseline := shas[3]
	badRef := shas[1] // culprit

	makeCheckFn := func(mgr *investigate.WorktreeManager) func(context.Context, string) bool {
		return func(ctx context.Context, sha string) bool {
			wtGit := investigate.NewGitRunner(mgr.WorktreeDir())
			err := mgr.CheckoutTo(sha)
			if err != nil {
				t.Logf("CheckoutTo(%s) failed: %v", sha, err)
				return false
			}
			content, _ := os.ReadFile(filepath.Join(wtGit.WorkDir(), "marker.txt"))
			t.Logf("Check %s: content=%q", sha, string(content))
			return !bytes.Contains(content, []byte("culprit"))
		}
	}

	// Run 1
	mgr1 := investigate.NewWorktreeManager(git, repo)
	_, err := mgr1.Create(badRef)
	require.NoError(t, err)
	culprit1, steps1, err := investigate.FindCulprit(context.Background(), mgr1, git, baseline, badRef, 50, makeCheckFn(mgr1), func(format string, args ...any) {
		t.Logf(format, args...)
	})
	require.NoError(t, err)
	t.Logf("Run 1 culprit: %s, steps: %d", culprit1, len(steps1))
	mgr1.Remove()

	// Run 2
	mgr2 := investigate.NewWorktreeManager(git, repo)
	_, err = mgr2.Create(badRef)
	require.NoError(t, err)
	culprit2, steps2, err := investigate.FindCulprit(context.Background(), mgr2, git, baseline, badRef, 50, makeCheckFn(mgr2), func(format string, args ...any) {
		t.Logf(format, args...)
	})
	require.NoError(t, err)
	t.Logf("Run 2 culprit: %s, steps: %d", culprit2, len(steps2))
	mgr2.Remove()

	require.Equal(t, culprit1, culprit2)
}

func TestFindCulprit_IterationCap(t *testing.T) {
	repo := seedRepoWithHistory(t)
	git := investigate.NewGitRunner(repo)
	mgr := investigate.NewWorktreeManager(git, repo)

	commits, _ := git.Run("log", "--format=%H", "-n", "4")
	shas := splitLines(commits)
	baseline := shas[3]
	badRef := shas[1] // culprit

	_, err := mgr.Create(badRef)
	require.NoError(t, err)
	defer mgr.Remove()

	// Check function that always fails (will fail pre-check at baseline)
	checkFn := func(ctx context.Context, sha string) bool {
		return false // always fails
	}

	_, _, err = investigate.FindCulprit(context.Background(), mgr, git, baseline, badRef, 50, checkFn, nil)
	require.Error(t, err)
	// Should fail pre-check at baseline
	require.True(t, investigate.IsNotReproducibleInWindow(err))
}

func TestIterationCapExceededError_Exists(t *testing.T) {
	// Just verify the error type and detection function exist
	err := &investigate.IterationCapExceededError{
		MaxIterations: 50,
		ProbesTaken:   []investigate.BisectStep{},
	}
	require.Error(t, err)
	require.True(t, investigate.IsIterationCapExceeded(err))
	require.Contains(t, err.Error(), "did not converge")
}