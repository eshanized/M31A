package workflow

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/engine/rollback"
	"github.com/eshanized/M31A/internal/integrations/git"
)

// TestBisectRollback_SoftResetIntegration verifies that rollback.SoftReset
// correctly reverts a commit in a real git repository. This validates the
// wiring between the bisect flow and the rollback package.
func TestBisectRollback_SoftResetIntegration(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()

	// Initialize a real git repo
	g := git.New(dir)
	if err := g.Init(); err != nil {
		t.Fatalf("git init: %v", err)
	}
	if err := g.ConfigUser("Test", "test@test.com"); err != nil {
		t.Fatalf("git config: %v", err)
	}

	// Create commit 1 (good baseline)
	if err := writeAndCommit(g, dir, "base.txt", "base content\n", "commit 1: base"); err != nil {
		t.Fatalf("commit 1: %v", err)
	}

	// Create commit 2 (good)
	if err := writeAndCommit(g, dir, "good.txt", "good content\n", "commit 2: good"); err != nil {
		t.Fatalf("commit 2: %v", err)
	}
	hash2, err := g.HeadHash()
	if err != nil {
		t.Fatalf("head hash after commit 2: %v", err)
	}

	// Create commit 3 (bad — the offender)
	err = writeAndCommit(g, dir, "bad.txt", "bad content\n", "commit 3: bad")
	if err != nil {
		t.Fatalf("commit 3: %v", err)
	}
	var hash3 string
	hash3, err = g.HeadHash()
	if err != nil {
		t.Fatalf("head hash after commit 3: %v", err)
	}

	// Verify all 3 commits exist
	log, err := g.Run("log", "--oneline")
	if err != nil {
		t.Fatalf("git log: %v", err)
	}
	if !strings.Contains(log, "commit 3: bad") {
		t.Errorf("expected commit 3 in log, got:\n%s", log)
	}

	// Call rollback.SoftReset with the bad commit (hash3)
	// This does: git revert --no-commit <hash3> to undo commit 3's changes
	err = rollback.SoftReset(hash3, dir)
	if err != nil {
		t.Fatalf("SoftReset failed: %v", err)
	}

	// Verify HEAD is still at hash3 (SoftReset doesn't change HEAD)
	currentHead, err := g.HeadHash()
	if err != nil {
		t.Fatalf("head hash after reset: %v", err)
	}
	if currentHead != hash3 {
		t.Errorf("expected HEAD still at %s (commit 3), got %s", hash3, currentHead)
	}

	// Verify the bad file's changes have been reverted
	// (bad.txt should be staged for deletion via revert --no-commit)
	badContent, readErr := os.ReadFile(filepath.Join(dir, "bad.txt"))
	if readErr == nil && strings.Contains(string(badContent), "bad content") {
		// The file might still exist but with reverted content,
		// or it might be staged for deletion. Check git status.
		status, _ := g.Run("status", "--porcelain", "bad.txt")
		if !strings.Contains(status, "D") && !strings.Contains(status, "M") {
			t.Error("expected bad.txt changes to be reverted")
		}
	}

	// Verify the good file still exists and is intact
	goodContent, err := os.ReadFile(filepath.Join(dir, "good.txt"))
	if err != nil {
		t.Errorf("expected good.txt to still exist: %v", err)
	}
	if string(goodContent) != "good content\n" {
		t.Errorf("expected good.txt content preserved, got %q", string(goodContent))
	}

	// Verify base file still exists
	if _, err := os.Stat(filepath.Join(dir, "base.txt")); err != nil {
		t.Errorf("expected base.txt to still exist: %v", err)
	}

	_ = hash2 // used for reference in comments
}

// writeAndCommit writes a file and creates a commit.
func writeAndCommit(g *git.Git, dir, filename, content, message string) error {
	path := filepath.Join(dir, filename)
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		return err
	}
	if err := g.Add(filename); err != nil {
		return err
	}
	return g.Commit(message)
}
