package git

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// CommitInfo represents metadata about a git commit.
type CommitInfo struct {
	Hash      string    `json:"hash"`
	ShortHash string    `json:"short_hash"`
	Author    string    `json:"author"`
	Message   string    `json:"message"`
	Timestamp time.Time `json:"timestamp"`
}

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

// Commit stages all changes and creates a commit with the given message.
func (g *Git) Commit(message string) error {
	if err := g.AddAll(); err != nil {
		return err
	}
	_, err := g.run("commit", "-m", message)
	if err != nil {
		return fmt.Errorf("git commit: %w", err)
	}
	return nil
}

// CommitWithFiles stages and commits only the given paths with the given message.
// Returns the commit hash on success.
func (g *Git) CommitWithFiles(message string, paths ...string) (string, error) {
	if len(paths) > 0 {
		if err := g.Add(paths...); err != nil {
			return "", err
		}
	}
	if _, err := g.run("commit", "-m", message); err != nil {
		return "", fmt.Errorf("git commit: %w", err)
	}
	hash, _ := g.HeadHash()
	return hash, nil
}

// Log returns commit history. If oneline is true, returns short format.
// If since is non-empty, only commits since that date are returned.
func (g *Git) Log(oneline bool, since string) ([]CommitInfo, error) {
	args := []string{"log", "--format=%H|%h|%an|%s|%aI"}
	if since != "" {
		args = append(args, "--since="+since)
	}

	out, err := g.run(args...)
	if err != nil {
		// git log fails with exit 1 if no commits yet
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

		parts := strings.SplitN(line, "|", 5)
		if len(parts) < 5 {
			continue
		}

		ts, err := time.Parse(time.RFC3339, parts[4])
		if err != nil {
			// Malformed timestamp — use zero time and continue
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

// Diff returns the diff between two refs. If both refs are empty, it runs
// plain `git diff` to show unstaged working-tree changes.
func (g *Git) Diff(ref1, ref2 string) (string, error) {
	var args []string
	if ref1 == "" && ref2 == "" {
		args = []string{"diff"}
	} else {
		args = []string{"diff", ref1 + ".." + ref2}
	}
	out, err := g.run(args...)
	if err != nil {
		return out, fmt.Errorf("git diff: %w", err)
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
		fmt.Sscanf(parts[0], "%d", &add)
		fmt.Sscanf(parts[1], "%d", &del)
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
	_, err := g.run("branch", name)
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
	_, err := g.run("stash", "push", "-m", message)
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
