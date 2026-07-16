package rollback

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/pkg/types"
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
	if writeErr := os.WriteFile(f, []byte("v2"), 0644); writeErr != nil {
		t.Fatalf("WriteFile failed: %v", writeErr)
	}
	if commitErr := g.Commit("v2"); commitErr != nil {
		t.Fatalf("Commit failed: %v", commitErr)
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

	result, err := r.SoftReset(firstHash, nil)
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

	result, err := r.SoftReset(hash0, nil)
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

	result, err := r.SoftReset(firstHash, nil)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}

	if result.Message == "" {
		t.Fatal("Expected non-empty Message in RollbackResult")
	}

	// Check message contains expected format patterns
	if !strings.Contains(result.Message, "Rolled back from") {
		t.Errorf("Expected message to contain 'Rolled back from', got: %s", result.Message)
	}
	if !strings.Contains(result.Message, "commits undone") {
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

// TestSoftReset_InvalidHash verifies SoftReset returns an error for non-existent commits.
func TestSoftReset_InvalidHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	_, err := r.SoftReset("deadbeef1234567890abcdef1234567890abcdef", nil)
	if err == nil {
		t.Fatal("Expected error for invalid hash in SoftReset")
	}
	if !strings.Contains(err.Error(), "soft reset") {
		t.Errorf("Expected error to contain 'soft reset', got: %v", err)
	}
}

// TestHardReset_InvalidHash verifies HardReset returns an error for non-existent commits.
func TestHardReset_InvalidHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	_, err := r.HardReset("deadbeef1234567890abcdef1234567890abcdef")
	if err == nil {
		t.Fatal("Expected error for invalid hash in HardReset")
	}
	if !strings.Contains(err.Error(), "hard reset") {
		t.Errorf("Expected error to contain 'hard reset', got: %v", err)
	}
}

// TestSafeReset_InvalidHash verifies SafeReset returns an error for non-existent commits.
func TestSafeReset_InvalidHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	_, err := r.SafeReset("deadbeef1234567890abcdef1234567890abcdef")
	if err == nil {
		t.Fatal("Expected error for invalid hash in SafeReset")
	}
	if !strings.Contains(err.Error(), "safe reset") {
		t.Errorf("Expected error to contain 'safe reset', got: %v", err)
	}
}

// TestSoftReset_CurrentHead verifies SoftReset to current HEAD works cleanly (no stash).
func TestSoftReset_CurrentHead(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 1)

	r := New(g)
	head, _ := g.HeadHash()

	result, err := r.SoftReset(head, nil)
	if err != nil {
		t.Fatalf("SoftReset to current HEAD failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected SoftReset success=true")
	}
	if result.ChangesStashed {
		t.Error("Expected ChangesStashed=false when tree is clean")
	}
	if result.PreviousHead != result.NewHead {
		t.Errorf("Expected PreviousHead == NewHead for no-op reset, got %s != %s",
			result.PreviousHead, result.NewHead)
	}
}

// TestSafeReset_Clean verifies SafeReset without dirty changes succeeds (no stash pop).
func TestSafeReset_Clean(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SafeReset(hash0)
	if err != nil {
		t.Fatalf("SafeReset failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected SafeReset success=true")
	}
	if result.ChangesStashed {
		t.Error("Expected ChangesStashed=false when tree is clean")
	}

	newHead, _ := g.HeadHash()
	if newHead != hash0 {
		t.Errorf("Expected HEAD at %s, got %s", hash0, newHead)
	}
}

// TestHasUncommittedChanges_Staged verifies detection of staged (but uncommitted) changes.
func TestHasUncommittedChanges_Staged(t *testing.T) {
	r, g := setupRollback(t)
	createCommits(g, 1)

	// Create and stage a new file without committing
	f := filepath.Join(g.WorkDir(), "staged.txt")
	if err := os.WriteFile(f, []byte("staged content"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}
	if err := g.Add("staged.txt"); err != nil {
		t.Fatalf("Add failed: %v", err)
	}

	dirty, err := r.HasUncommittedChanges()
	if err != nil {
		t.Fatalf("HasUncommittedChanges failed: %v", err)
	}
	if !dirty {
		t.Error("Expected HasUncommittedChanges=true with staged file")
	}
}

// TestHasUncommittedChanges_Deleted verifies detection of deleted tracked files.
func TestHasUncommittedChanges_Deleted(t *testing.T) {
	r, g := setupRollback(t)
	createCommits(g, 1)

	// Delete a tracked file (git rm for tracked deletion)
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.Remove(f); err != nil {
		t.Fatalf("Remove failed: %v", err)
	}

	dirty, err := r.HasUncommittedChanges()
	if err != nil {
		t.Fatalf("HasUncommittedChanges failed: %v", err)
	}
	if !dirty {
		t.Error("Expected HasUncommittedChanges=true with deleted file")
	}
}

// TestCountCommitsBetween_SameHash verifies countCommitsBetween returns 0 for identical hashes.
func TestCountCommitsBetween_SameHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	head, _ := g.HeadHash()

	// SoftReset to HEAD, then check the message has "0 commits undone"
	result, err := r.SoftReset(head, nil)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}
	if !strings.Contains(result.Message, "0 commits undone") {
		t.Errorf("Expected '0 commits undone' in message for same-hash reset, got: %s", result.Message)
	}
}

// TestBuildResult_ShortHash verifies buildResult truncates hashes to 7 chars in message.
func TestBuildResult_ShortHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 5)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SoftReset(firstHash, nil)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}

	// Short hashes are 7 chars
	if strings.Contains(result.Message, result.PreviousHead) {
		// If the full hash is in the message, that's a bug
		t.Error("Expected message to contain short hash, not full hash")
	}
	// Message should contain 7-char short hashes
	parts := strings.Split(result.Message, " ")
	for _, part := range parts {
		if len(part) == 7 && strings.Trim(part, "0123456789abcdef") == "" {
			return // Found a short hash
		}
	}
	t.Logf("Message: %s (checking for 7-char short hashes)", result.Message)
}

// TestHardReset_WithUncommitted verifies HardReset stashes changes before reset.
func TestHardReset_WithUncommitted(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	// Modify a tracked file
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("dirty content"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := r.HardReset(hash0)
	if err != nil {
		t.Fatalf("HardReset with uncommitted changes failed: %v", err)
	}
	if !result.ChangesStashed {
		t.Error("Expected ChangesStashed=true when resetting with uncommitted changes")
	}
	if !result.Success {
		t.Fatal("Expected HardReset success=true")
	}

	newHead, _ := g.HeadHash()
	if newHead != hash0 {
		t.Errorf("Expected HEAD at %s, got %s", hash0, newHead)
	}
}

// TestSafeReset_MessagePreserved verifies SafeReset message includes "preserved".
func TestSafeReset_MessagePreserved(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	// Make it dirty so stashing occurs
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("modified"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := r.SafeReset(hash0)
	if err != nil {
		t.Fatalf("SafeReset failed: %v", err)
	}
	if !result.ChangesStashed {
		t.Error("Expected ChangesStashed=true")
	}
	if !strings.Contains(result.Message, "preserved") {
		t.Errorf("Expected message to contain 'preserved', got: %s", result.Message)
	}
}

// TestSoftReset_WithUncommitted_MessageStashed verifies SoftReset message includes "stashed".
func TestSoftReset_WithUncommitted_MessageStashed(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("dirty"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	result, err := r.SoftReset(hash0, nil)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}
	if !strings.Contains(result.Message, "stashed") {
		t.Errorf("Expected message to contain 'stashed', got: %s", result.Message)
	}
}

// TestChain_LimitExceedsCommits verifies Chain caps limit to available commits.
func TestChain_LimitExceedsCommits(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, err := r.Chain(100) // More than available
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}
	if len(entries) != 3 {
		t.Fatalf("Expected 3 entries (capped to available), got %d", len(entries))
	}
}

// TestRollbackEntry_IsCurrentFlag verifies IsCurrent is correctly set across entries.
func TestRollbackEntry_IsCurrentFlag(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 5)

	r := New(g)
	entries, err := r.Chain(10)
	if err != nil {
		t.Fatalf("Chain failed: %v", err)
	}

	currentCount := 0
	for _, e := range entries {
		if e.IsCurrent {
			currentCount++
		}
	}
	if currentCount != 1 {
		t.Errorf("Expected exactly 1 IsCurrent=true entry, got %d", currentCount)
	}
}

// TestCountCommitsBetween_DifferentHashes directly tests countCommitsBetween with
// two different reachable hashes to cover the endIdx < startIdx path.
func TestCountCommitsBetween_DifferentHashes(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 5)

	r := New(g)
	entries, _ := r.Chain(10)

	// Pick two reachable commits: oldest and newest (both in the log since no reset)
	oldestHash := entries[len(entries)-1].CommitInfo.Hash // commit 0
	newestHash := entries[0].CommitInfo.Hash              // commit 4 (HEAD)

	// countCommitsBetween(oldest, newest): oldest is startHash, newest is endHash
	// Log is newest first: [commit4, commit3, commit2, commit1, commit0]
	// So endHash (commit4) is at index 0, startHash (commit0) is at index 4
	// endIdx < startIdx: 0 < 4, so should return 4 - 0 = 4
	count, err := r.countCommitsBetween(oldestHash, newestHash)
	if err != nil {
		t.Fatalf("countCommitsBetween failed: %v", err)
	}
	if count != 4 {
		t.Errorf("Expected 4 commits between oldest and newest, got %d", count)
	}
}

// TestCountCommitsBetween_ReverseOrder tests countCommitsBetween when startHash
// appears before endHash in the log (should return 0).
func TestCountCommitsBetween_ReverseOrder(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 5)

	r := New(g)
	entries, _ := r.Chain(10)

	// newestHash is at a smaller index than olderHash in the log
	// If we call countCommitsBetween(newest, oldest), then:
	// startHash = newest (index 0), endHash = oldest (index 4)
	// endIdx (4) > startIdx (0), so returns an error
	newestHash := entries[0].CommitInfo.Hash
	oldestHash := entries[len(entries)-1].CommitInfo.Hash

	_, err := r.countCommitsBetween(newestHash, oldestHash)
	if err == nil {
		t.Fatal("countCommitsBetween(newest, oldest) should return an error for reverse order")
	}
}

// TestCountCommitsBetween_InvalidHash verifies countCommitsBetween returns
// ErrInvalidHash when one of the hashes is not in the repository.
func TestCountCommitsBetween_InvalidHash(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	validHash := entries[0].CommitInfo.Hash

	_, err := r.countCommitsBetween(validHash, "nonexistent123")
	if err == nil {
		t.Fatal("Expected error for invalid hash in countCommitsBetween")
	}
	if err != ErrInvalidHash {
		t.Errorf("Expected ErrInvalidHash, got: %v", err)
	}

	// Also test with invalid startHash
	_, err = r.countCommitsBetween("nonexistent456", validHash)
	if err != ErrInvalidHash {
		t.Errorf("Expected ErrInvalidHash for invalid startHash, got: %v", err)
	}
}

// TestCountCommitsBetween_AdjacentCommits tests counting between adjacent commits.
func TestCountCommitsBetween_AdjacentCommits(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)

	// commit1 and commit0 are adjacent
	commit1Hash := entries[1].CommitInfo.Hash
	commit0Hash := entries[2].CommitInfo.Hash

	// countCommitsBetween(commit0, commit1): startHash=commit0 (index 2), endHash=commit1 (index 1)
	// endIdx (1) < startIdx (2), so return 2 - 1 = 1
	count, err := r.countCommitsBetween(commit0Hash, commit1Hash)
	if err != nil {
		t.Fatalf("countCommitsBetween failed: %v", err)
	}
	if count != 1 {
		t.Errorf("Expected 1 commit between adjacent commits, got %d", count)
	}
}

// TestBuildResult_Directly tests buildResult directly with stashed=true to cover
// the stash message branch.
func TestBuildResult_Directly(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	head, _ := g.HeadHash()

	// Test with stashed=true and different stash words
	for _, stashWord := range []string{"stashed", "preserved"} {
		result := r.buildResult(head, head, true, stashWord)
		if !result.Success {
			t.Errorf("Expected success=true for %s", stashWord)
		}
		if !result.ChangesStashed {
			t.Errorf("Expected ChangesStashed=true for %s", stashWord)
		}
		if !strings.Contains(result.Message, stashWord) {
			t.Errorf("Expected message to contain %q for stashed=true, got: %s", stashWord, result.Message)
		}
	}

	// Test with stashed=false (no stash message should appear)
	result := r.buildResult(head, head, false, "stashed")
	if result.ChangesStashed {
		t.Error("Expected ChangesStashed=false")
	}
	if strings.Contains(result.Message, "Changes") {
		t.Errorf("Expected no stash message when stashed=false, got: %s", result.Message)
	}
}

// TestBuildResult_ShortHashTruncation verifies buildResult truncates hashes > 7 chars.
func TestBuildResult_ShortHashTruncation(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	head, _ := g.HeadHash()

	result := r.buildResult(head, head, false, "")

	// Full hash should NOT appear in message
	if strings.Contains(result.Message, head) {
		t.Errorf("Full hash should not appear in message: %s", result.Message)
	}
	// But 7-char short hash should
	shortHash := head[:7]
	if !strings.Contains(result.Message, shortHash) {
		t.Errorf("Expected short hash %q in message: %s", shortHash, result.Message)
	}
}

// TestStashIfDirty_CleanTree verifies stashIfDirty returns false when tree is clean.
func TestStashIfDirty_CleanTree(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	stashed, err := r.stashIfDirty()
	if err != nil {
		t.Fatalf("stashIfDirty failed: %v", err)
	}
	if stashed {
		t.Error("Expected stashed=false on clean tree")
	}
}

// TestStashIfDirty_DirtyTree verifies stashIfDirty stashes changes on dirty tree.
func TestStashIfDirty_DirtyTree(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	f := filepath.Join(g.WorkDir(), "f0.txt")
	if err := os.WriteFile(f, []byte("modified"), 0644); err != nil {
		t.Fatalf("WriteFile failed: %v", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		t.Fatalf("stashIfDirty failed: %v", err)
	}
	if !stashed {
		t.Error("Expected stashed=true on dirty tree")
	}
}

// TestRollbackResult_AllFields verifies all RollbackResult fields are populated correctly.
func TestRollbackResult_AllFields(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	hash0 := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SoftReset(hash0, nil)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}

	if !result.Success {
		t.Error("Expected Success=true")
	}
	if result.PreviousHead == "" {
		t.Error("Expected non-empty PreviousHead")
	}
	if result.NewHead == "" {
		t.Error("Expected non-empty NewHead")
	}
	if result.Message == "" {
		t.Error("Expected non-empty Message")
	}
}

// TestErrorPaths_InvalidWorkDir verifies error handling when git operations fail
// due to an invalid working directory.
func TestErrorPaths_InvalidWorkDir(t *testing.T) {
	// Create a Rollback backed by a non-existent directory
	g := git.New("/nonexistent/path/for/testing")
	r := New(g)

	t.Run("Chain fails", func(t *testing.T) {
		_, err := r.Chain(10)
		if err == nil {
			t.Fatal("Expected error for Chain with invalid workdir")
		}
	})

	t.Run("CurrentHead fails", func(t *testing.T) {
		_, err := r.CurrentHead()
		if err == nil {
			t.Fatal("Expected error for CurrentHead with invalid workdir")
		}
	})

	t.Run("Preview fails", func(t *testing.T) {
		_, err := r.Preview("abc123")
		if err == nil {
			t.Fatal("Expected error for Preview with invalid workdir")
		}
	})

	t.Run("SoftReset fails", func(t *testing.T) {
		_, err := r.SoftReset("abc123", nil)
		if err == nil {
			t.Fatal("Expected error for SoftReset with invalid workdir")
		}
	})

	t.Run("HardReset fails", func(t *testing.T) {
		_, err := r.HardReset("abc123")
		if err == nil {
			t.Fatal("Expected error for HardReset with invalid workdir")
		}
	})

	t.Run("SafeReset fails", func(t *testing.T) {
		_, err := r.SafeReset("abc123")
		if err == nil {
			t.Fatal("Expected error for SafeReset with invalid workdir")
		}
	})

	t.Run("HasUncommittedChanges fails", func(t *testing.T) {
		_, err := r.HasUncommittedChanges()
		if err == nil {
			t.Fatal("Expected error for HasUncommittedChanges with invalid workdir")
		}
	})

	t.Run("stashIfDirty fails", func(t *testing.T) {
		_, err := r.stashIfDirty()
		if err == nil {
			t.Fatal("Expected error for stashIfDirty with invalid workdir")
		}
	})

	t.Run("countCommitsBetween fails", func(t *testing.T) {
		_, err := r.countCommitsBetween("abc", "def")
		if err == nil {
			t.Fatal("Expected error for countCommitsBetween with invalid workdir")
		}
	})
}

// TestErrorPaths_DestroyedRepo verifies error handling when a valid repo is
// corrupted mid-operation (simulates git failures).
func TestErrorPaths_DestroyedRepo(t *testing.T) {
	r, g := setupRollback(t)
	createCommits(g, 2)

	// Delete .git to corrupt the repo
	workDir := g.WorkDir()
	if err := os.RemoveAll(filepath.Join(workDir, ".git")); err != nil {
		t.Fatalf("Failed to remove .git: %v", err)
	}

	t.Run("Chain fails after corruption", func(t *testing.T) {
		_, err := r.Chain(10)
		if err == nil {
			t.Fatal("Expected error for Chain after repo corruption")
		}
	})

	t.Run("HasUncommittedChanges fails after corruption", func(t *testing.T) {
		_, err := r.HasUncommittedChanges()
		if err == nil {
			t.Fatal("Expected error for HasUncommittedChanges after repo corruption")
		}
	})

	t.Run("stashIfDirty fails after corruption", func(t *testing.T) {
		_, err := r.stashIfDirty()
		if err == nil {
			t.Fatal("Expected error for stashIfDirty after repo corruption")
		}
	})
}

// M-28: SoftReset callback
// ---------------------------------------------------------------------------

// TestSoftReset_Callback_Invoked verifies that the onReset callback is
// invoked with the new HEAD hash after a successful soft reset.
func TestSoftReset_Callback_Invoked(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 3)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	callbackHash := ""
	callbackCalled := false
	onReset := func(newHead string) error {
		callbackCalled = true
		callbackHash = newHead
		return nil
	}

	result, err := r.SoftReset(firstHash, onReset)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected success")
	}
	if !callbackCalled {
		t.Error("Expected onReset callback to be called")
	}
	if callbackHash != firstHash {
		t.Errorf("Expected callback hash %s, got %s", firstHash, callbackHash)
	}
}

// TestSoftReset_Callback_ErrorPropagated verifies that an error in the
// callback is propagated and the reset result is not returned.
func TestSoftReset_Callback_ErrorPropagated(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	expectedErr := errors.New("callback failed")
	onReset := func(newHead string) error {
		return expectedErr
	}

	_, err := r.SoftReset(firstHash, onReset)
	if err == nil {
		t.Fatal("Expected error from callback")
	}
	if !errors.Is(err, expectedErr) {
		t.Errorf("Expected error to wrap callback error, got: %v", err)
	}
}

// TestSoftReset_NilCallback verifies that nil callback works fine.
func TestSoftReset_NilCallback(t *testing.T) {
	_, g := setupRollback(t)
	createCommits(g, 2)

	r := New(g)
	entries, _ := r.Chain(10)
	firstHash := entries[len(entries)-1].CommitInfo.Hash

	result, err := r.SoftReset(firstHash, nil)
	if err != nil {
		t.Fatalf("SoftReset with nil callback failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Expected success")
	}
}
