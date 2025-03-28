package rollback

import (
	"errors"
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/types"
)

// ErrInvalidHash is returned when a commit hash is not found in the repository.
var ErrInvalidHash = errors.New("invalid commit hash")

// RollbackEntry represents a single commit in the rollback chain.
type RollbackEntry struct {
	CommitInfo    git.CommitInfo `json:"commit_info"`
	Diff          string         `json:"diff"`
	IsCurrent     bool           `json:"is_current"`
	HasCheckpoint bool           `json:"has_checkpoint"`
}

// RollbackResult represents the outcome of a reset operation.
type RollbackResult struct {
	Success        bool   `json:"success"`
	PreviousHead   string `json:"previous_head"`
	NewHead        string `json:"new_head"`
	ChangesStashed bool   `json:"changes_stashed"`
	Message        string `json:"message"`
}

// Rollback provides safe commit rollback operations.
type Rollback struct {
	git *git.Git
}

// New creates a new Rollback instance backed by the given git wrapper.
func New(g *git.Git) *Rollback {
	return &Rollback{git: g}
}

// Chain returns up to `limit` commits newest-first with their diffs to HEAD.
// If limit <= 0, defaults to 20. The current HEAD is at index 0.
// Returns an empty slice for empty repos without error.
func (r *Rollback) Chain(limit int) ([]RollbackEntry, error) {
	if limit <= 0 {
		limit = 20
	}

	commits, err := r.git.Log(false, "")
	if err != nil {
		return nil, fmt.Errorf("chain: %w", err)
	}

	if len(commits) == 0 {
		return []RollbackEntry{}, nil
	}

	head, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("chain: %w", err)
	}

	if limit > len(commits) {
		limit = len(commits)
	}
	commits = commits[:limit]

	entries := make([]RollbackEntry, limit)
	for i, c := range commits {
		isCurrent := c.Hash == head
		var diff string
		if !isCurrent {
			diff, _ = r.git.Diff(c.Hash, "HEAD")
		}
		entries[i] = RollbackEntry{
			CommitInfo:    c,
			Diff:          diff,
			IsCurrent:     isCurrent,
			HasCheckpoint: false,
		}
	}

	return entries, nil
}

// CurrentHead returns the full SHA of the current HEAD commit.
func (r *Rollback) CurrentHead() (string, error) {
	return r.git.HeadHash()
}

// Preview returns the diff between the given commit hash and HEAD.
// Output is capped at types.BashOutputLimit (50,000 characters).
func (r *Rollback) Preview(hash string) (string, error) {
	diff, err := r.git.Diff(hash, "HEAD")
	if err != nil {
		return "", fmt.Errorf("preview: %w", err)
	}

	if len(diff) > types.BashOutputLimit {
		diff = diff[:types.BashOutputLimit] + "\n... [output truncated]"
	}

	return diff, nil
}

// SoftReset performs a git reset --soft to the given commit.
// If uncommitted changes exist, they are stashed first.
func (r *Rollback) SoftReset(hash string) (*RollbackResult, error) {
	prevHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	if err := r.git.ResetSoft(hash); err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	return r.buildResult(prevHead, newHead, stashed, "stashed"), nil
}

// HardReset performs a git reset --hard to the given commit.
// If uncommitted changes exist, they are stashed first.
func (r *Rollback) HardReset(hash string) (*RollbackResult, error) {
	prevHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	if err := r.git.ResetHard(hash); err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	return r.buildResult(prevHead, newHead, stashed, "stashed"), nil
}

// SafeReset performs a git reset --hard to the given commit and then
// pops the stash to preserve any uncommitted changes.
func (r *Rollback) SafeReset(hash string) (*RollbackResult, error) {
	prevHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("safe reset: %w", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		return nil, fmt.Errorf("safe reset: %w", err)
	}

	if err := r.git.ResetHard(hash); err != nil {
		return nil, fmt.Errorf("safe reset: %w", err)
	}

	if stashed {
		if err := r.git.StashPop(); err != nil {
			return nil, fmt.Errorf("safe reset: stash pop: %w", err)
		}
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("safe reset: %w", err)
	}

	return r.buildResult(prevHead, newHead, stashed, "preserved"), nil
}

// HasUncommittedChanges returns true if the working tree has uncommitted changes
// (modified, staged, untracked, or deleted files).
func (r *Rollback) HasUncommittedChanges() (bool, error) {
	status, err := r.git.Status()
	if err != nil {
		return false, fmt.Errorf("has uncommitted changes: %w", err)
	}

	dirtyIndicators := []string{
		"Changes not staged",
		"Untracked files",
		"Changes to be committed",
		"modified:",
		"new file:",
		"deleted:",
	}

	for _, indicator := range dirtyIndicators {
		if strings.Contains(status, indicator) {
			return true, nil
		}
	}

	return false, nil
}

// stashIfDirty checks for uncommitted changes and stashes them if found.
// Returns true if changes were stashed, false if the tree was clean.
func (r *Rollback) stashIfDirty() (bool, error) {
	dirty, err := r.HasUncommittedChanges()
	if err != nil {
		return false, err
	}

	if !dirty {
		return false, nil
	}

	if err := r.git.StashPush("rollback-auto-stash"); err != nil {
		return false, fmt.Errorf("stash: %w", err)
	}

	return true, nil
}

// countCommitsBetween counts the number of commits between two hashes.
// startHash is the older commit, endHash the newer one.
func (r *Rollback) countCommitsBetween(startHash, endHash string) (int, error) {
	if startHash == endHash {
		return 0, nil
	}

	commits, err := r.git.Log(false, "")
	if err != nil {
		return 0, err
	}

	startIdx := -1
	endIdx := -1
	for i, c := range commits {
		if c.Hash == startHash {
			startIdx = i
		}
		if c.Hash == endHash {
			endIdx = i
		}
	}

	if startIdx == -1 || endIdx == -1 {
		return 0, ErrInvalidHash
	}

	// Log returns newest first. endHash (HEAD before reset) should appear
	// earlier (smaller index) than startHash (older commit being reset to).
	if endIdx < startIdx {
		return startIdx - endIdx, nil
	}

	// If startHash is not actually older than endHash, or same commit
	// (already handled above), return 0.
	return 0, nil
}

// buildResult creates a RollbackResult with a user-friendly message.
func (r *Rollback) buildResult(prevHead, newHead string, stashed bool, stashWord string) *RollbackResult {
	shortPrev := prevHead
	shortNew := newHead
	if len(shortPrev) > 7 {
		shortPrev = shortPrev[:7]
	}
	if len(shortNew) > 7 {
		shortNew = shortNew[:7]
	}

	commitsUndone, _ := r.countCommitsBetween(newHead, prevHead)

	msg := fmt.Sprintf("Rolled back from %s to %s. %d commits undone.",
		shortPrev, shortNew, commitsUndone)

	if stashed {
		msg += fmt.Sprintf(" Changes %s.", stashWord)
	}

	return &RollbackResult{
		Success:        true,
		PreviousHead:   prevHead,
		NewHead:        newHead,
		ChangesStashed: stashed,
		Message:        msg,
	}
}
