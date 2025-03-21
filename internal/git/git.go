package git

import (
	"fmt"
	"os/exec"
	"path/filepath"
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
	args := append([]string{"add"}, paths...)
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

		ts, _ := time.Parse(time.RFC3339, parts[4])

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

// Diff returns the diff between two refs.
func (g *Git) Diff(ref1, ref2 string) (string, error) {
	ref := ref1 + ".." + ref2
	out, err := g.run("diff", ref)
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

// ResetSoft resets HEAD to the given commit, keeping changes staged.
func (g *Git) ResetSoft(commit string) error {
	_, err := g.run("reset", "--soft", commit)
	if err != nil {
		return fmt.Errorf("git reset --soft: %w", err)
	}
	return nil
}

// ResetHard resets HEAD to the given commit, discarding all changes.
func (g *Git) ResetHard(commit string) error {
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
