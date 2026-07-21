package subagent

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/eshanized/M31A/internal/integrations/git"
)

// GitWorktrees is a WorktreeOps implementation backed by the local git
// binary via internal/git. It creates worktrees under a sibling directory
// of the main repo so they share the same .git object store.
type GitWorktrees struct {
	// Root is the directory under which worktree subdirectories are created.
	// Defaults to filepath.Join(parentWorkDir, ".m31a-worktrees") when empty.
	Root string
}

// Compile-time interface check
var _ WorktreeOps = (*GitWorktrees)(nil)

// IsRepo reports whether parentWorkDir lives inside a git repository.
func (g *GitWorktrees) IsRepo(parentWorkDir string) bool {
	return git.New(parentWorkDir).IsRepo()
}

// Create provisions a git worktree at <root>/<agentID> on branch
// m31a/agent-<agentID>[-<suffix>], starting from the current HEAD.
func (g *GitWorktrees) Create(ctx context.Context, parentWorkDir, agentID, branchSuffix string) (string, error) {
	root := g.rootFor(parentWorkDir)
	if err := os.MkdirAll(root, 0o755); err != nil {
		return "", fmt.Errorf("worktree root: %w", err)
	}
	path := filepath.Join(root, sanitizePath(agentID))
	branch := worktreeBranchName(agentID, branchSuffix)

	// If the branch already exists (stale agent), delete it so `worktree add -b`
	// succeeds. Best-effort — if it doesn't exist, ignore the error.
	_, _ = git.New(parentWorkDir).Run("branch", "-D", branch)

	// `git worktree add -b <branch> <path>` creates a new working tree at
	// <path> checked out on a new branch <branch> based at HEAD.
	cmd := exec.CommandContext(ctx, "git", "worktree", "add", "-b", branch, path)
	cmd.Dir = parentWorkDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("git worktree add: %w: %s", err, string(out))
	}
	return path, nil
}

// Remove deletes the worktree at path and its branch. Idempotent.
func (g *GitWorktrees) Remove(ctx context.Context, path string) error {
	if path == "" {
		return nil
	}
	// Find the parent repo by walking up from the worktree.
	parentRepo, err := parentRepoOf(path)
	if err != nil {
		// If we can't find a parent repo, just remove the directory.
		_ = os.RemoveAll(path)
		return nil
	}
	// Read the branch name from the worktree before removing it.
	branch, _ := branchOf(ctx, path)

	// `git worktree remove --force <path>` removes the worktree entry and
	// its directory. Tolerate "not a worktree" errors.
	cmd := exec.CommandContext(ctx, "git", "worktree", "remove", "--force", path)
	cmd.Dir = parentRepo
	_, _ = cmd.CombinedOutput()

	// Also try removing the directory directly in case git refused (e.g. the
	// worktree was already pruned from git's metadata).
	_ = os.RemoveAll(path)

	// Best-effort branch cleanup.
	if branch != "" && strings.HasPrefix(branch, "m31a/agent-") {
		cmd := exec.CommandContext(ctx, "git", "branch", "-D", branch)
		cmd.Dir = parentRepo
		_, _ = cmd.CombinedOutput()
	}
	return nil
}

// Sweep removes every stale m31a/agent-* branch that no longer has a worktree
// AND removes orphaned worktree directories that are no longer tracked by git.
// Call this at startup to recover from crashes.
func Sweep(ctx context.Context, parentWorkDir string) error {
	g := git.New(parentWorkDir)
	if !g.IsRepo() {
		return nil
	}
	_, err := g.Run("worktree", "list", "--porcelain")
	if err != nil {
		slog.Warn("failed to list git worktrees for sweep", "error", err)
		return nil
	}
	// Prune metadata first (clears references to deleted worktree dirs).
	_, _ = g.Run("worktree", "prune")

	// Re-read after prune to get the accurate live set.
	out, err := g.Run("worktree", "list", "--porcelain")
	if err != nil {
		out = "" // best-effort; proceed with empty set
	}

	liveBranches, livePaths := parseWorktreeList(out)

	// Phase 1: delete orphaned m31a/agent-* branches that have no worktree dir.
	branchesOut, err := g.Run("branch", "--list", "m31a/agent-*", "--format=%(refname:short)")
	if err == nil {
		for _, b := range strings.Split(strings.TrimSpace(branchesOut), "\n") {
			b = strings.TrimSpace(b)
			if b == "" {
				continue
			}
			if liveBranches[b] {
				continue
			}
			_, _ = g.Run("branch", "-D", b)
		}
	}

	// Phase 2: remove orphaned worktree directories under .m31a-worktrees/
	// that git no longer tracks.
	root := (&GitWorktrees{}).rootFor(parentWorkDir)
	entries, err := os.ReadDir(root)
	if err != nil {
		// .m31a-worktrees/ doesn't exist — nothing to clean.
		return nil
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		dirPath := filepath.Join(root, entry.Name())
		if livePaths[dirPath] {
			continue
		}
		// Directory is not tracked by any live worktree — remove it.
		slog.Info("sweep: removing orphaned worktree directory", "path", dirPath)
		_ = os.RemoveAll(dirPath)
	}
	return nil
}

// parseWorktreeList parses `git worktree list --porcelain` output and returns
// the set of live branches and the set of live worktree directory paths.
func parseWorktreeList(porcelain string) (branches map[string]bool, paths map[string]bool) {
	branches = make(map[string]bool)
	paths = make(map[string]bool)
	for _, line := range strings.Split(porcelain, "\n") {
		if strings.HasPrefix(line, "worktree ") {
			p := strings.TrimPrefix(line, "worktree ")
			paths[p] = true
		} else if strings.HasPrefix(line, "branch ") {
			ref := strings.TrimPrefix(line, "branch ")
			ref = strings.TrimPrefix(ref, "refs/heads/")
			branches[ref] = true
		}
	}
	return branches, paths
}

// parentRepoOf returns the root of the git repo that contains path (or the
// main repo for a worktree).
func parentRepoOf(path string) (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	cmd.Dir = path
	out, err := cmd.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("rev-parse toplevel: %w: %s", err, string(out))
	}
	return strings.TrimSpace(string(out)), nil
}

// branchOf returns the checked-out branch name of a worktree.
func branchOf(ctx context.Context, path string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "rev-parse", "--abbrev-ref", "HEAD")
	cmd.Dir = path
	out, err := cmd.CombinedOutput()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}

// rootFor returns the worktree root directory, creating it lazily isn't
// performed here (Create does it).
func (g *GitWorktrees) rootFor(parentWorkDir string) string {
	if g.Root != "" {
		return g.Root
	}
	return filepath.Join(parentWorkDir, ".m31a-worktrees")
}

// sanitizePath removes characters that would be unsafe in a filesystem path.
func sanitizePath(s string) string {
	out := make([]rune, 0, len(s))
	for _, r := range s {
		switch {
		case r >= 'a' && r <= 'z', r >= 'A' && r <= 'Z', r >= '0' && r <= '9',
			r == '-', r == '_':
			out = append(out, r)
		default:
			out = append(out, '_')
		}
	}
	result := string(out)
	// Reject dot-only IDs (., .., etc.) which would collapse to the root.
	if result == "" || strings.Trim(result, ".") == "" {
		return "_invalid"
	}
	return result
}

// worktreeBranchName builds the git branch name for a subagent worktree.
func worktreeBranchName(agentID, suffix string) string {
	if suffix == "" {
		return "m31a/agent-" + agentID
	}
	return "m31a/agent-" + agentID + "-" + sanitizePath(suffix)
}
