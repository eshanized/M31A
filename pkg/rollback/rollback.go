package rollback

import (
	"errors"
	"fmt"
	"time"

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

	commits, err := r.git.LogAll()
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
			d, err := r.git.DiffRefs(c.Hash, "HEAD")
			if err != nil {
				diff = fmt.Sprintf("[diff unavailable: %v]", err)
			} else {
				diff = d
			}
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
	diff, err := r.git.DiffRefs(hash, "HEAD")
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
// The onReset callback, if non-nil, is called after the reset succeeds
// with the new HEAD hash. Use this to sync TASKS.md status updates (M-28).
func (r *Rollback) SoftReset(hash string, onReset func(newHead string) error) (*RollbackResult, error) {
	prevHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	if resetErr := r.git.ResetSoft(hash); resetErr != nil {
		return nil, fmt.Errorf("soft reset: %w", resetErr)
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("soft reset: %w", err)
	}

	// Invoke callback to sync TASKS.md or other state
	if onReset != nil {
		if cbErr := onReset(newHead); cbErr != nil {
			return nil, fmt.Errorf("soft reset callback: %w", cbErr)
		}
	}

	return r.buildResult(prevHead, newHead, stashed, "stashed"), nil
}

// HardReset performs a git reset --hard to the given commit.
// If uncommitted changes exist, they are stashed first.
// C-11: Creates a timestamped backup branch before the destructive reset.
func (r *Rollback) HardReset(hash string) (*RollbackResult, error) {
	prevHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	stashed, err := r.stashIfDirty()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	// C-11: Create timestamped backup branch before destructive reset
	backupBranch := fmt.Sprintf("m31a/rollback-backup-%d", time.Now().Unix())
	if _, branchErr := r.git.Run("branch", "--force", backupBranch); branchErr != nil {
		return nil, fmt.Errorf("create backup branch: %w", branchErr)
	}

	if resetErr := r.git.ResetHard(hash); resetErr != nil {
		return nil, fmt.Errorf("hard reset: %w", resetErr)
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("hard reset: %w", err)
	}

	result := r.buildResult(prevHead, newHead, stashed, "stashed")
	result.Message += fmt.Sprintf(" Backup branch: %s", backupBranch)
	return result, nil
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

	if resetErr := r.git.ResetHard(hash); resetErr != nil {
		return nil, fmt.Errorf("safe reset: %w", resetErr)
	}

	if stashed {
		if popErr := r.git.StashPop(); popErr != nil {
			return nil, fmt.Errorf("safe reset: stash pop: %w", popErr)
		}
	}

	newHead, err := r.git.HeadHash()
	if err != nil {
		return nil, fmt.Errorf("safe reset: %w", err)
	}

	return r.buildResult(prevHead, newHead, stashed, "preserved"), nil
}

// HasUncommittedChanges returns true if the working tree has uncommitted changes
// (modified, staged, untracked, or deleted files). Uses porcelain format for
// reliable, locale-independent parsing.
func (r *Rollback) HasUncommittedChanges() (bool, error) {
	return r.git.HasUncommittedChanges()
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
// Uses `git rev-list --count` for O(1) counting instead of loading the full log (PK-19 fix).
// startHash is the older commit, endHash the newer one. Returns an error if
// the hashes are in the wrong order (startHash newer than endHash).
func (r *Rollback) countCommitsBetween(startHash, endHash string) (int, error) {
	if startHash == endHash {
		return 0, nil
	}

	count, err := r.git.CountCommits(startHash, endHash)
	if err != nil {
		return 0, ErrInvalidHash
	}

	if count == 0 {
		// Either the hashes are in the wrong order or one doesn't exist.
		// Verify both hashes exist by trying the reverse direction.
		reverseCount, rerr := r.git.CountCommits(endHash, startHash)
		if rerr != nil {
			return 0, ErrInvalidHash
		}
		if reverseCount > 0 {
			// Hashes are valid but in reverse order — that's an error per contract.
			return 0, fmt.Errorf("commits in unexpected order: %s should be older than %s", startHash, endHash)
		}
		return 0, ErrInvalidHash
	}

	return count, nil
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

	commitsUndone, countErr := r.countCommitsBetween(newHead, prevHead)
	if countErr != nil {
		commitsUndone = 0
	}

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
