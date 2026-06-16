package types

import (
	"testing"
	"time"
)

func TestCommitInfo_Fields(t *testing.T) {
	t.Parallel()
	ci := CommitInfo{
		Hash:      "abc123def456",
		ShortHash: "abc123d",
		Author:    "test@example.com",
		Message:   "feat: add feature",
		Timestamp: time.Now(),
	}
	if ci.Hash != "abc123def456" {
		t.Errorf("expected hash, got %q", ci.Hash)
	}
	if ci.ShortHash != "abc123d" {
		t.Errorf("expected short hash, got %q", ci.ShortHash)
	}
	if ci.Author != "test@example.com" {
		t.Errorf("expected author, got %q", ci.Author)
	}
}

func TestGitClient_Interface(t *testing.T) {
	t.Parallel()
	// Verify interface exists at compile time
	var _ GitClient = (GitClient)(nil)
}
