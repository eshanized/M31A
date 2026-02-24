package git

import (
	"fmt"
	"log/slog"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// CommitInfo is a type alias for the canonical definition in internal/types.
type CommitInfo = types.CommitInfo

// Compile-time check: *Git must satisfy types.GitClient.
var _ types.GitClient = (*Git)(nil)

// Git wraps git operations for a working directory.
type Git struct {
	workDir string
}

// New creates a Git instance for the given working directory.
func New(workDir string) *Git {
	return &Git{workDir: workDir}
}

// run executes a git command with the working directory set.
func (g *Git) run(args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = g.workDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
	}
	return string(out), nil
}

// Run executes a git command and returns the output. This is the exported
// version of run for commands not covered by specific wrapper methods.
func (g *Git) Run(args ...string) (string, error) {
	return g.run(args...)
}

// Init initializes a new git repository in the working directory.
func (g *Git) Init() error {
	_, err := g.run("init")
	if err != nil {
		return fmt.Errorf("git init: %w", err)
	}
	return nil
}

// IsRepo returns true if the working directory is a git repository.
func (g *Git) IsRepo() bool {
	_, err := g.run("rev-parse", "--git-dir")
	return err == nil
}

// Add stages the given paths.
func (g *Git) Add(paths ...string) error {
	args := append([]string{"add", "--"}, paths...)
	_, err := g.run(args...)
	if err != nil {
		return fmt.Errorf("git add: %w", err)
	}
	return nil
}

// AddAll stages all changes including deletions.
func (g *Git) AddAll() error {
	_, err := g.run("add", "-A")
	if err != nil {
		return fmt.Errorf("git add -A: %w", err)
	}
	return nil
}

// sanitizeCommitMessage removes newlines that could create fake trailers
// and caps length to prevent malformed git history.
func sanitizeCommitMessage(msg string) string {
	msg = strings.ReplaceAll(msg, "\n", " ")
	msg = strings.ReplaceAll(msg, "\r", "")
	if len(msg) > 200 {
		msg = msg[:197] + "..."
	}
	return strings.TrimSpace(msg)
}

// Commit stages all changes and creates a commit with the given message.
// WARNING: This stages the entire worktree. Use CommitWithFiles or
// CommitStaged for scoped commits.
func (g *Git) Commit(message string) error {
	if err := g.AddAll(); err != nil {
		return err
	}
	_, err := g.run("commit", "--message="+sanitizeCommitMessage(message))
	if err != nil {
		return fmt.Errorf("git commit: %w", err)
	}
	return nil
}

// CommitStaged creates a commit from already-staged changes only.
// Does not stage any additional files.
func (g *Git) CommitStaged(message string) (string, error) {
	_, err := g.run("commit", "--message="+sanitizeCommitMessage(message))
	if err != nil {
		return "", fmt.Errorf("git commit: %w", err)
	}
	hash, _ := g.HeadHash()
	return hash, nil
}

// CommitWithFiles stages and commits only the given paths with the given message.
// Returns the commit hash on success.
func (g *Git) CommitWithFiles(message string, paths ...string) (string, error) {
	if len(paths) > 0 {
		if err := g.Add(paths...); err != nil {
			return "", err
		}
	}
	if _, err := g.run("commit", "--message="+sanitizeCommitMessage(message)); err != nil {
		return "", fmt.Errorf("git commit: %w", err)
	}
	hash, _ := g.HeadHash()
	return hash, nil
}

// LogAll returns full commit history (newest first).
func (g *Git) LogAll() ([]CommitInfo, error) {
	return g.logInternal(false, "")
}

// Log returns the lastN commits (newest first). If lastN <= 0, returns all.
func (g *Git) Log(lastN int) ([]CommitInfo, error) {
	if lastN <= 0 {
		return g.LogAll()
	}
	args := []string{"log", "--format=%H§%h§%an§%s§%aI", "-n", strconv.Itoa(lastN)}
	return g.runLog(args)
}

// LogSince returns commits since the given time (newest first).
func (g *Git) LogSince(since time.Time) ([]CommitInfo, error) {
	args := []string{"log", "--format=%H§%h§%an§%s§%aI", "--since=" + since.Format(time.RFC3339)}
	return g.runLog(args)
}

// logInternal is the shared implementation for LogAll/LogSince.
func (g *Git) logInternal(oneline bool, since string) ([]CommitInfo, error) {
	format := "--format=%H§%h§%an§%s§%aI"
	if oneline {
		format = "--format=%h§%s"
	}
	args := []string{"log", format}
	if since != "" {
		args = append(args, "--since="+since)
	}
	return g.runLogWithOneline(args, oneline)
}

func (g *Git) runLog(args []string) ([]CommitInfo, error) {
	return g.runLogWithOneline(args, false)
}

func (g *Git) runLogWithOneline(args []string, oneline bool) ([]CommitInfo, error) {
	out, err := g.run(args...)
	if err != nil {
		if strings.Contains(err.Error(), "exit status 1") && strings.Contains(err.Error(), "does not have any commits") {
			return []CommitInfo{}, nil
		}
		return nil, fmt.Errorf("git log: %w", err)
	}
	return parseLog(out, oneline)
}

func parseLog(out string, oneline bool) ([]CommitInfo, error) {
	lines := strings.Split(strings.TrimSpace(out), "\n")
	if len(lines) == 0 || (len(lines) == 1 && lines[0] == "") {
		return []CommitInfo{}, nil
	}

	var commits []CommitInfo
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}

		if oneline {
			parts := strings.SplitN(line, "§", 2)
			if len(parts) < 2 {
				continue
			}
			commits = append(commits, CommitInfo{
				ShortHash: parts[0],
				Hash:      parts[0],
				Message:   parts[1],
			})
			continue
		}

		parts := strings.SplitN(line, "§", 5)
		if len(parts) < 5 {
			continue
		}

		ts, err := time.Parse(time.RFC3339, parts[4])
		if err != nil {
			slog.Warn("git log: malformed timestamp, using zero time", "raw", parts[4], "error", err)
		}

		ci := CommitInfo{
			Hash:      parts[0],
			ShortHash: parts[1],
			Author:    parts[2],
			Message:   parts[3],
			Timestamp: ts,
		}
		commits = append(commits, ci)
	}

	return commits, nil
}

// validateGitRef checks that a string is a safe git ref name.
func validateGitRef(ref string) bool {
	if ref == "" {
		return true
	}
	if strings.HasPrefix(ref, "--") {
		return false
	}
	if strings.Contains(ref, "..") {
		return false
	}
	if strings.ContainsAny(ref, "\x00\n\r") {
		return false
	}
	return true
}

// DiffRefs returns the diff between two refs. If both refs are empty, it runs
// plain `git diff` to show unstaged working-tree changes.
func (g *Git) DiffRefs(ref1, ref2 string) (string, error) {
	var args []string
	if ref1 == "" && ref2 == "" {
		args = []string{"diff"}
	} else {
		if !validateGitRef(ref1) || !validateGitRef(ref2) {
			return "", fmt.Errorf("git diff: invalid ref name")
		}
		args = []string{"diff", ref1 + ".." + ref2}
	}
	out, err := g.run(args...)
	if err != nil {
		return out, fmt.Errorf("git diff: %w", err)
	}
	return out, nil
}

// Diff returns git diff output based on the number of target refs provided:
//   - 0 args: unstaged working-tree diff (plain `git diff`)
//   - 1 arg:  diff between target and HEAD
//   - 2 args: diff between the two refs
func (g *Git) Diff(target ...string) (string, error) {
	switch len(target) {
	case 0:
		return g.DiffRefs("", "")
	case 1:
		return g.DiffRefs(target[0], "HEAD")
	default:
		return g.DiffRefs(target[0], target[1])
	}
}

// DiffStat returns diff stat (summary) based on the number of target refs provided.
func (g *Git) DiffStat(target ...string) (string, error) {
	var args []string
	switch len(target) {
	case 0:
		args = []string{"diff", "--stat"}
	case 1:
		args = []string{"diff", "--stat", target[0], "HEAD"}
	default:
		args = []string{"diff", "--stat", target[0], target[1]}
	}
	out, err := g.run(args...)
	if err != nil {
		return out, fmt.Errorf("git diff --stat: %w", err)
	}
	return out, nil
}

// DiffStaged returns the diff of staged but uncommitted changes.
func (g *Git) DiffStaged() (string, error) {
	out, err := g.run("diff", "--cached")
	if err != nil {
		return out, fmt.Errorf("git diff --cached: %w", err)
	}
	return out, nil
}

// DiffFile returns the diff for a single file. For untracked files ("?"),
// it returns the file contents as an "added" diff. For staged files it
// uses --cached; otherwise it shows unstaged working-tree changes.
func (g *Git) DiffFile(path string, status string) (string, error) {
	if status == "?" {
		out, err := g.run("diff", "--no-index", "/dev/null", "--", path)
		if err != nil {
			// git diff --no-index exits 1 when files differ, which is expected
			if len(out) > 0 {
				return out, nil
			}
			return "", fmt.Errorf("git diff file: %w", err)
		}
		return out, nil
	}
	// Check if staged changes exist for this file
	staged, _ := g.run("diff", "--cached", "--", path)
	if staged != "" {
		return staged, nil
	}
	// Fall back to unstaged working-tree changes
	out, err := g.run("diff", "--", path)
	if err != nil {
		return "", fmt.Errorf("git diff file: %w", err)
	}
	return out, nil
}

// FileStatus represents a single file's git status.
type FileStatus struct {
	Status    string // e.g. "M", "A", "D", "R", "C", "U", "?"
	Path      string
	OldPath   string // for renames
	Additions int
	Deletions int
}

// StatusPorcelain returns structured git status for the working directory.
// Uses git status --porcelain (v1 format): "XY path" per line.
func (g *Git) StatusPorcelain() ([]FileStatus, error) {
	statusOut, err := g.run("status", "--porcelain")
	if err != nil {
		return nil, fmt.Errorf("git status: %w", err)
	}

	// Get diff stats via --numstat
	numstatOut, numstatErr := g.run("diff", "--numstat", "HEAD")
	if numstatErr != nil {
		numstatOut = ""
	}
	numstatMap := make(map[string]struct{ add, del int })
	for _, line := range strings.Split(strings.TrimSpace(numstatOut), "\n") {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, "\t", 3)
		if len(parts) < 3 {
			continue
		}
		add := 0
		del := 0
		n1, _ := fmt.Sscanf(parts[0], "%d", &add)
		n2, _ := fmt.Sscanf(parts[1], "%d", &del)
		if n1 != 1 || n2 != 1 {
			continue // skip malformed line
		}
		numstatMap[parts[2]] = struct{ add, del int }{add, del}
	}

	var statuses []FileStatus
	for _, line := range strings.Split(strings.TrimSpace(statusOut), "\n") {
		if len(line) < 4 {
			continue
		}
		// Format: "XY path" where XY are 2 chars, then space, then path
		xy := line[:2]
		path := strings.TrimSpace(line[3:])
		// Remove quotes if present
		if strings.HasPrefix(path, "\"") {
			path, _ = strconv.Unquote(path)
		}

		fs := FileStatus{Path: path}

		// Determine status from XY codes
		x, y := xy[0], xy[1]
		switch {
		case x == 'A' && y == ' ':
			fs.Status = "A"
		case x == 'M' && y == ' ':
			fs.Status = "M"
		case x == 'D' && y == ' ':
			fs.Status = "D"
		case x == 'R' && y == ' ':
			fs.Status = "R"
		case x == 'C' && y == ' ':
			fs.Status = "C"
		case x == '?' && y == '?':
			fs.Status = "?"
		case y == 'M':
			fs.Status = "M"
		case y == 'D' && x != ' ':
			fs.Status = "D"
		case x == 'D' && y == ' ':
			fs.Status = "D"
		case y == 'A':
			fs.Status = "A"
		default:
			fs.Status = string(y)
			if fs.Status == " " {
				fs.Status = string(x)
			}
		}

		// Handle renames: "R old -> new"
		if fs.Status == "R" && strings.Contains(path, " -> ") {
			parts := strings.SplitN(path, " -> ", 2)
			if len(parts) == 2 {
				fs.OldPath = strings.TrimSpace(parts[0])
				fs.Path = strings.TrimSpace(parts[1])
			}
		}

		// Add diff stats
		if stats, ok := numstatMap[fs.Path]; ok {
			fs.Additions = stats.add
			fs.Deletions = stats.del
		}

		statuses = append(statuses, fs)
	}

	return statuses, nil
}

// Status returns the git status output.
func (g *Git) Status() (string, error) {
	out, err := g.run("status")
	if err != nil {
		return out, fmt.Errorf("git status: %w", err)
	}
	return out, nil
}

// HeadHash returns the current HEAD commit hash.
func (g *Git) HeadHash() (string, error) {
	out, err := g.run("rev-parse", "HEAD")
	if err != nil {
		return "", fmt.Errorf("git rev-parse HEAD: %w", err)
	}
	return strings.TrimSpace(out), nil
}

// CreateBranch creates a new branch with the given name.
func (g *Git) CreateBranch(name string) error {
	_, err := g.run("branch", "--", name)
	if err != nil {
		return fmt.Errorf("git branch: %w", err)
	}
	return nil
}

// CurrentBranch returns the name of the current branch.
func (g *Git) CurrentBranch() (string, error) {
	out, err := g.run("branch", "--show-current")
	if err != nil {
		return "", fmt.Errorf("git branch --show-current: %w", err)
	}
	return strings.TrimSpace(out), nil
}

// RevParse resolves a git ref to its full SHA hash.
func (g *Git) RevParse(ref string) (string, error) {
	out, err := g.run("rev-parse", "--", ref)
	if err != nil {
		return "", fmt.Errorf("git rev-parse %s: %w", ref, err)
	}
	return strings.TrimSpace(out), nil
}

// IsDirty returns true if the working tree has any uncommitted changes.
func (g *Git) IsDirty() (bool, error) {
	dirty, err := g.HasUncommittedChanges()
	if err != nil {
		return false, fmt.Errorf("is dirty: %w", err)
	}
	return dirty, nil
}

// HasUncommittedChanges returns true if the working tree has uncommitted changes
// (modified, staged, untracked, or deleted files).
func (g *Git) HasUncommittedChanges() (bool, error) {
	status, err := g.run("status", "--porcelain")
	if err != nil {
		return false, fmt.Errorf("has uncommitted changes: %w", err)
	}
	return strings.TrimSpace(status) != "", nil
}

// RemoteTracking returns the remote tracking branch, e.g. "origin/main".
// Returns empty string if no upstream is configured.
func (g *Git) RemoteTracking() (string, error) {
	out, err := g.run("rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}")
	if err != nil {
		return "", nil // no upstream configured — not an error
	}
	return strings.TrimSpace(out), nil
}

// ResetSoft resets HEAD to the given commit, keeping changes staged.
// Creates a backup branch for rollback safety.
func (g *Git) ResetSoft(commit string) error {
	if _, err := g.run("branch", "--force", "m31a-backup-pre-reset"); err != nil {
		return fmt.Errorf("create backup branch: %w", err)
	}
	_, err := g.run("reset", "--soft", commit)
	if err != nil {
		return fmt.Errorf("git reset --soft: %w", err)
	}
	return nil
}

// ResetHard resets HEAD to the given commit, discarding all changes.
// Creates a backup branch for rollback safety.
func (g *Git) ResetHard(commit string) error {
	if _, err := g.run("branch", "--force", "m31a-backup-pre-reset"); err != nil {
		return fmt.Errorf("create backup branch: %w", err)
	}
	_, err := g.run("reset", "--hard", commit)
	if err != nil {
		return fmt.Errorf("git reset --hard: %w", err)
	}
	return nil
}

// StashPush saves current changes to the stash with the given message.
func (g *Git) StashPush(message string) error {
	_, err := g.run("stash", "push", "--message="+message)
	if err != nil {
		return fmt.Errorf("git stash push: %w", err)
	}
	return nil
}

// StashPop applies the most recent stash entry.
func (g *Git) StashPop() error {
	_, err := g.run("stash", "pop")
	if err != nil {
		return fmt.Errorf("git stash pop: %w", err)
	}
	return nil
}

// ConfigUser sets the git user name and email for the repository.
func (g *Git) ConfigUser(name, email string) error {
	if _, err := g.run("config", "user.name", name); err != nil {
		return err
	}
	if _, err := g.run("config", "user.email", email); err != nil {
		return err
	}
	return nil
}

// WorkDir returns the working directory path.
func (g *Git) WorkDir() string {
	return g.workDir
}

// AbsPath returns the absolute path for a relative path within the working directory.
func (g *Git) AbsPath(rel string) string {
	return filepath.Join(g.workDir, rel)
}

// Fetch fetches from a remote repository.
func (g *Git) Fetch(remote string) error {
	if remote == "" {
		remote = "origin"
	}
	if _, err := g.run("fetch", remote); err != nil {
		return fmt.Errorf("git fetch %s: %w", remote, err)
	}
	return nil
}

// Pull pulls from a remote, optionally rebasing.
func (g *Git) Pull(remote, branch string, rebase bool) error {
	if remote == "" {
		remote = "origin"
	}
	args := []string{"pull", remote}
	if branch != "" {
		args = append(args, branch)
	}
	if rebase {
		args = append(args, "--rebase")
	}
	if _, err := g.run(args...); err != nil {
		return fmt.Errorf("git pull: %w", err)
	}
	return nil
}

// Push pushes to a remote repository.
func (g *Git) Push(remote, branch string) error {
	if remote == "" {
		remote = "origin"
	}
	args := []string{"push", remote}
	if branch != "" {
		args = append(args, branch)
	}
	if _, err := g.run(args...); err != nil {
		return fmt.Errorf("git push: %w", err)
	}
	return nil
}

// CheckoutBranch checks out a branch, optionally creating it.
func (g *Git) CheckoutBranch(name string, create bool) error {
	args := []string{"checkout"}
	if create {
		args = append(args, "-b")
	}
	args = append(args, "--", name)
	if _, err := g.run(args...); err != nil {
		return fmt.Errorf("git checkout: %w", err)
	}
	return nil
}

// StashList returns a list of stash entries.
func (g *Git) StashList() ([]string, error) {
	out, err := g.run("stash", "list")
	if err != nil {
		return nil, fmt.Errorf("git stash list: %w", err)
	}
	if out == "" {
		return nil, nil
	}
	return strings.Split(strings.TrimSpace(out), "\n"), nil
}

// StashApply applies a stash entry by index (0 = most recent).
func (g *Git) StashApply(index int) error {
	if _, err := g.run("stash", "apply", fmt.Sprintf("stash@{%d}", index)); err != nil {
		return fmt.Errorf("git stash apply: %w", err)
	}
	return nil
}

// Merge merges a branch into the current branch.
func (g *Git) Merge(branch string) error {
	if _, err := g.run("merge", "--", branch); err != nil {
		return fmt.Errorf("git merge %s: %w", branch, err)
	}
	return nil
}

// Tag creates a lightweight or annotated tag.
func (g *Git) Tag(name, msg string) error {
	if msg != "" {
		if _, err := g.run("tag", "-a", "--", name, "--message="+msg); err != nil {
			return fmt.Errorf("git tag: %w", err)
		}
	} else {
		if _, err := g.run("tag", "--", name); err != nil {
			return fmt.Errorf("git tag: %w", err)
		}
	}
	return nil
}

// BranchList returns a list of local branches.
func (g *Git) BranchList() ([]string, error) {
	out, err := g.run("branch", "--format=%(refname:short)")
	if err != nil {
		return nil, fmt.Errorf("git branch: %w", err)
	}
	if out == "" {
		return nil, nil
	}
	return strings.Split(strings.TrimSpace(out), "\n"), nil
}
