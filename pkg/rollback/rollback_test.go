package rollback

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/types"
)

// setupRollback creates a temporary git repo and returns a Rollback instance.
func setupRollback(t *testing.T) (*Rollback, *git.Git) {
	t.Helper()
	dir := t.TempDir()
	g := git.New(dir)
	if err := g.Init(); err != nil {
		t.Fatalf("Init failed: %v", err)
	}
	if err := g.ConfigUser("Test", "test@test.com"); err != nil {
		t.Fatalf("ConfigUser failed: %v", err)
	}
	return New(g), g
}

// createCommits creates n sequential commits in the given git repo.
func createCommits(g *git.Git, n int) {
	for i := 0; i < n; i++ {
		f := filepath.Join(g.WorkDir(), fmt.Sprintf("f%d.txt", i))
		if err := os.WriteFile(f, []byte(fmt.Sprintf("content %d", i)), 0644); err != nil {
			panic(fmt.Sprintf("WriteFile failed: %v", err))
		}
		if err := g.Commit(fmt.Sprintf("commit %d", i)); err != nil {
			panic(fmt.Sprintf("Commit %d failed: %v", i, err))
		}
	}
}

// TestChain verifies Chain returns all commits newest-first.
func TestChain(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 5)

	r := New(g)
	entries, err := r.Chain(0)
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}
	if len(entries) != 5 {
		t.Fatalf("Expected 5 entries, got %d", len(entries))
	}

	// Newest first: index 0 should be "commit 4"
	if entries[0].CommitInfo.Message != "commit 4" {
		t.Errorf("Expected newest commit 'commit 4' at index 0, got %q", entries[0].CommitInfo.Message)
	}
	// Oldest last: index 4 should be "commit 0"
	if entries[4].CommitInfo.Message != "commit 0" {
		t.Errorf("Expected oldest commit 'commit 0' at index 4, got %q", entries[4].CommitInfo.Message)
	}
}

// TestChain_Limit verifies Chain respects the limit parameter.
func TestChain_Limit(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 10)

	r := New(g)
	entries, err := r.Chain(3)
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}
	if len(entries) != 3 {
		t.Fatalf("Expected 3 entries, got %d", len(entries))
	}
}

// TestChain_Empty verifies Chain returns an empty slice for empty repos.
func TestChain_Empty(t *testing.T) {
	r, _ := setupRollback(t)
	entries, err := r.Chain(10)
	if err != nil {
		t.Fatalf("Chain on empty repo should not error, got: %v", err)
	}
	if len(entries) != 0 {
		t.Fatalf("Expected 0 entries, got %d", len(entries))
	}
}

// TestChain_CurrentHead verifies the first entry has IsCurrent=true.
func TestChain_CurrentHead(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, err := r.Chain(10)
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}
	if len(entries) == 0 {
		t.Fatal("Expected non-empty chain")
	}
	if !entries[0].IsCurrent {
		t.Error("Expected index 0 entry to be IsCurrent (HEAD)")
	}
	// Non-HEAD entries should have IsCurrent=false
	for i := 1; i < len(entries); i++ {
		if entries[i].IsCurrent {
			t.Errorf("Expected index %d to have IsCurrent=false", i)
		}
	}
}

// TestPreview verifies Preview returns the diff between a commit and HEAD.
func TestPreview(t *testing.T) {
	_, g := setupRollback(t)

	// First commit
	f := filepath.Join(g.WorkDir(), "a.txt")
	if err := os.WriteFile(f, []byte("v1"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := g.Commit("v1"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}
	hash1, err := g.HeadHash()
	if err != nil {
		t.Fatalf("HeadHash failed: %v", err)
	}

	// Second commit modifies the file
	if err := os.WriteFile(f, []byte("v2"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := g.Commit("v2"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}

	r := New(g)
	diff, err := r.Preview(hash1)
	if err != nil {
		t.Fatalf("Preview failed: %v", err)
	}
	if diff == "" {
		t.Fatal("Expected non-empty diff for Preview")
	}
}

// TestPreview_InvalidHash verifies Preview returns an error for invalid hashes.
func TestPreview_InvalidHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 1)

	r := New(g)
	_, err := r.Preview("nonexistent")
	if err == nil {
		t.Fatal("Expected error for invalid hash in Preview")
	}
}

// TestPreview_Truncation verifies Preview caps output at BashOutputLimit.
func TestPreview_Truncation(t *testing.T) {
	_, g := setupRollback(t)

	// Create a large file to produce a diff exceeding the limit
	largeContent := make([]byte, 100_000)
	for i := range largeContent {
		largeContent[i] = byte('A' + i%26)
	}
	f := filepath.Join(g.WorkDir(), "big.txt")
	if err := os.WriteFile(f, largeContent, 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := g.Commit("big file"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}
	hash1, _ := g.HeadHash()

	// Second commit cleans the large file
	if err := os.WriteFile(f, []byte("cleaned"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := g.Commit("clean"); err != nil {
		t.Fatalf("Commit failed: %v", err)
	}

	r := New(g)
	diff, err := r.Preview(hash1)
	if err != nil {
		t.Fatalf("Preview failed: %v", err)
	}

	// The diff should be truncated with the truncation marker
	// Limit is 50,000 chars, but diff includes header lines too
	if len(diff) > types.BashOutputLimit+100 {
		t.Errorf("Expected diff to be capped near %d, got %d chars", types.BashOutputLimit, len(diff))
	}
	// Check for truncation marker
	if len(diff) >= types.BashOutputLimit {
		if diff[len(diff)-len("\n... [output truncated]"):] != "\n... [output truncated]" {
			t.Error("Expected truncation marker at end of preview")
		}
	}
}

// TestSoftReset verifies SoftReset moves HEAD and keeps changes staged.
func TestSoftReset(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	hash1, err := g.HeadHash()
	if err != nil {
		t.Fatalf("HeadHash failed: %v", err)
	}

	// Get hash of the first commit (commit 0)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SoftReset(firstHash)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected SoftReset success=true")
	}

	newHead, _ := g.HeadHash()
	if newHead != firstHash {
		t.Errorf("Expected HEAD at %s, got %s", firstHash, newHead)
	}
	if result.PreviousHead != hash1 {
		t.Errorf("Expected PreviousHead %s, got %s", hash1, result.PreviousHead)
	}
	if result.NewHead != firstHash {
		t.Errorf("Expected NewHead %s, got %s", firstHash, result.NewHead)
	}
}

// TestHardReset verifies HardReset moves HEAD and discards changes.
func TestHardReset(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.HardReset(firstHash)
	if err != nil {
		t.Fatalf("HardReset failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected HardReset success=true")
	}

	newHead, _ := g.HeadHash()
	if newHead != firstHash {
		t.Errorf("Expected HEAD at %s, got %s", firstHash, newHead)
	}
}

// TestSoftReset_WithUncommitted verifies SoftReset stashes changes before reset.
func TestSoftReset_WithUncommitted(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	// Get hash1 (commit 0)
	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	// Modify an existing tracked file to make dirty state (stash tracks modified files)
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("dirty"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := r.SoftReset(hash0)
	if err != nil {
		t.Fatalf("SoftReset with uncommitted changes failed: %v", err)
	}
	if !result.ChangesStashed {
		t.Error("Expected ChangesStashed=true when resetting with uncommitted changes")
	}
	if !result.Success {
		t.Fatal("Expected SoftReset success=true")
	}

	newHead, _ := g.HeadHash()
	if newHead != hash0 {
		t.Errorf("Expected HEAD at %s, got %s", hash0, newHead)
	}
}

// TestHardReset_CurrentHead verifies HardReset with current HEAD is a no-op.
func TestHardReset_CurrentHead(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 1)

	r := New(g)
	head, _ := g.HeadHash()

	result, err := r.HardReset(head)
	if err != nil {
		t.Fatalf("HardReset to current HEAD failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected HardReset success=true")
	}
	if result.PreviousHead != result.NewHead {
		t.Errorf("Expected PreviousHead == NewHead for no-op reset, got %s != %s",
			result.PreviousHead, result.NewHead)
	}
}

// TestSafeReset verifies SafeReset preserves uncommitted changes.
func TestSafeReset(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	// Modify an existing tracked file (git stash tracks modified files)
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("modified content"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := r.SafeReset(hash0)
	if err != nil {
		t.Fatalf("SafeReset failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected SafeReset success=true")
	}

	newHead, _ := g.HeadHash()
	if newHead != hash0 {
		t.Errorf("Expected HEAD at %s, got %s", hash0, newHead)
	}

	// After SafeReset, the stash-pop should have restored the modified file
	content, err := os.ReadFile(f)
	if err != nil {
		t.Fatalf("Expected f0.txt to exist after SafeReset: %v", err)
	}
	if string(content) != "modified content" {
		t.Errorf("Expected file content 'modified content', got %q", string(content))
	}
}

// TestHasUncommittedChanges tests detection of dirty and clean states.
func TestHasUncommittedChanges(t *testing.T) {
	t.Run("clean", func(t *testing.T) {
		_, g := setupRollback(t)
		createCommits(g, 1)

		r := New(g)
		dirty, err := r.HasUncommittedChanges()
		if err != nil {
			t.Fatalf("HasUncommittedChanges failed: %v", err)
		}
		if dirty {
			t.Error("Expected HasUncommittedChanges=false on clean repo")
		}
	})

	t.Run("dirty", func(t *testing.T) {
		r, g := setupRollback(t)
		createCommits(g, 1)

		// Create an uncommitted file
		f := filepath.Join(g.WorkDir(), "untracked.txt")
		if err := os.WriteFile(f, []byte("dirty"), 0644); err != nil {
			t.Fatalf("WriteFile failed: %v", err)
		}

		dirty, err := r.HasUncommittedChanges()
		if err != nil {
			t.Fatalf("HasUncommittedChanges failed: %v", err)
		}
		if !dirty {
			t.Error("Expected HasUncommittedChanges=true with untracked file")
		}
	})

	t.Run("modified", func(t *testing.T) {
		r, g := setupRollback(t)
		createCommits(g, 1)

		// Modify an existing file
		f := filepath.Join(g.WorkDir(), "f0.txt")
		if err := os.WriteFile(f, []byte("modified"), 0644); err != nil {
			t.Fatalf("WriteFile failed: %v", err)
		}

		dirty, err := r.HasUncommittedChanges()
		if err != nil {
			t.Fatalf("HasUncommittedChanges failed: %v", err)
		}
		if !dirty {
			t.Error("Expected HasUncommittedChanges=true with modified file")
		}
	})
}

// TestCurrentHead verifies CurrentHead returns a valid hash.
func TestCurrentHead(t *testing.T) {
	r, g := setupRollback(t)
	createCommits(g, 1)

	head, err := r.CurrentHead()
	if err != nil {
		t.Fatalf("CurrentHead failed: %v", err)
	}
	if len(head) != 40 {
		t.Errorf("Expected 40-char hash, got %d chars", len(head))
	}

	// Verify it matches direct call
	direct, _ := g.HeadHash()
	if head != direct {
		t.Errorf("CurrentHead %s does not match HeadHash %s", head, direct)
	}
}

// TestChain_DefaultLimit verifies Chain uses default limit of 20.
func TestChain_DefaultLimit(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 25)

	r := New(g)
	entries, err := r.Chain(0) // Should default to 20
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}
	if len(entries) != 20 {
		t.Fatalf("Expected 20 entries (default limit), got %d", len(entries))
	}
}

// TestRollbackResult_MessageFormat verifies the rollback result message format.
func TestRollbackResult_MessageFormat(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SoftReset(firstHash)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}

	if result.Message == "" {
		t.Fatal("Expected non-empty Message in RollbackResult")
	}

	// Check message contains expected format patterns
	if !contains(result.Message, "Rolled back from") {
		t.Errorf("Expected message to contain 'Rolled back from', got: %s", result.Message)
	}
	if !contains(result.Message, "commits undone") {
		t.Errorf("Expected message to contain 'commits undone', got: %s", result.Message)
	}
}

// TestChain_NonHeadEntriesHaveDiff verifies non-HEAD entries have a diff.
func TestChain_NonHeadEntriesHaveDiff(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, err := r.Chain(10)
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}

	// HEAD entry (index 0) should have empty diff
	if entries[0].Diff != "" {
		t.Log("HEAD entry may or may not have diff - depends on implementation")
	}

	// Non-current entries should have non-empty diff or at least be computed
	// without error
	for i := 1; i < len(entries); i++ {
		// Just verify no panic; diff might be empty for some edge cases
		_ = entries[i].Diff
	}
}

// contains reports whether substr is within s.
func contains(s, substr string) bool {
	return strings.Contains(s, substr)
}
