package investigate

import (
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"
	"time"
)

// InvestigateGitRunner is the narrow interface for git operations in investigate.
// Mirrors the legacy GitRunner but adapted to this package.
type InvestigateGitRunner interface {
	Run(args ...string) (string, error)
}

// WorktreeManager manages a detached temporary worktree for bisect operations.
type WorktreeManager struct {
	git      InvestigateGitRunner
	repoDir  string
	wtDir    string
	prefix   string
	sigClean bool
}

// NewWorktreeManager creates a new WorktreeManager.
// git: the git runner to use (must implement InvestigateGitRunner)
// repoDir: the repository working directory
func NewWorktreeManager(git InvestigateGitRunner, repoDir string) *WorktreeManager {
	return &WorktreeManager{
		git:     git,
		repoDir: repoDir,
		prefix:  "m31a-investigate-",
	}
}

// Create creates a new temporary worktree at the given badRef.
// Returns the absolute path to the created worktree.
// badRef is validated through ValidateRef before any git invocation.
func (m *WorktreeManager) Create(badRef string) (string, error) {
	if !ValidateRef(badRef) {
		return "", fmt.Errorf("invalid ref: %q", badRef)
	}

	// Prune any orphaned worktrees from previous runs before creating new one
	m.PruneOrphans()

	// Create unique temp directory name with pid and timestamp component
	pid := os.Getpid()
	timestamp := fmt.Sprintf("%d", time.Now().UnixNano())
	wtDir := filepath.Join(os.TempDir(), fmt.Sprintf("%s%d-%s", m.prefix, pid, timestamp))
	m.wtDir = wtDir

	// Create the worktree with --detach at the badRef
	_, err := m.git.Run("worktree", "add", "--detach", wtDir, badRef)
	if err != nil {
		m.wtDir = ""
		return "", fmt.Errorf("git worktree add --detach %s %s: %w", wtDir, badRef, err)
	}

	// Install signal handler for cleanup on interrupt/terminate
	m.installSignalHandler()

	return wtDir, nil
}

// CheckoutTo detaches HEAD inside the temp worktree to the given SHA.
// sha must be a valid 40-hex or abbreviated hex commit hash.
func (m *WorktreeManager) CheckoutTo(sha string) error {
	if m.wtDir == "" {
		return fmt.Errorf("no worktree created")
	}
	if !isValidSHA(sha) {
		return fmt.Errorf("invalid SHA format: %q", sha)
	}

	// Use git.New(wtDir).Run equivalent - run git in the worktree directory
	wtGit := NewGitRunner(m.wtDir)
	_, err := wtGit.Run("checkout", "--detach", sha)
	if err != nil {
		return fmt.Errorf("git checkout --detach %s: %w", sha, err)
	}
	return nil
}

// Remove removes the temporary worktree.
// Safe to call multiple times.
func (m *WorktreeManager) Remove() error {
	if m.wtDir == "" {
		return nil
	}

	wtDir := m.wtDir
	m.wtDir = ""

	// Try worktree remove --force first
	_, err := m.git.Run("worktree", "remove", "--force", wtDir)
	if err != nil {
		// Fallback to worktree prune
		_, _ = m.git.Run("worktree", "prune")
	}

	// Best effort: remove the directory if it still exists
	_ = os.RemoveAll(wtDir)

	return nil
}

// WorktreeDir returns the current worktree directory path.
func (m *WorktreeManager) WorktreeDir() string {
	return m.wtDir
}

// PruneOrphans sweeps stale worktree registrations matching our prefix.
func (m *WorktreeManager) PruneOrphans() {
	// List worktrees and look for ones with our prefix
	out, _ := m.git.Run("worktree", "list", "--porcelain")
	if out == "" {
		return
	}

	lines := strings.Split(out, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "worktree ") {
			path := strings.TrimPrefix(line, "worktree ")
			if strings.Contains(path, m.prefix) {
				// Try to remove this orphaned worktree
				_, _ = m.git.Run("worktree", "remove", "--force", path)
			}
		}
	}
	// Final prune sweep
	_, _ = m.git.Run("worktree", "prune")
}

// installSignalHandler sets up signal handling for SIGINT/SIGTERM.
func (m *WorktreeManager) installSignalHandler() {
	if m.sigClean {
		return
	}
	m.sigClean = true

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)

	go func() {
		<-sigCh
		_ = m.Remove()
		os.Exit(130)
	}()
}

// InstallSignalCleanup is exported for report-level composition in plan 04-05.
// It installs the signal handler on the given manager.
func InstallSignalCleanup(m *WorktreeManager) {
	m.installSignalHandler()
}

// isValidSHA checks if a string is a valid SHA (40 hex chars or abbreviated).
func isValidSHA(s string) bool {
	if len(s) < 4 || len(s) > 40 {
		return false
	}
	for _, c := range s {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
			return false
		}
	}
	return true
}

// ValidateRef validates a git ref name for safety.
// Exported so other packages can use it (validated by plan 04-02 Task 1).
func ValidateRef(ref string) bool {
	if ref == "" {
		return true
	}
	if strings.HasPrefix(ref, "--") {
		return false
	}
	if strings.HasPrefix(ref, "-") {
		return false
	}
	if strings.Contains(ref, "..") {
		return false
	}
	if strings.ContainsAny(ref, "\x00\n\r") {
		return false
	}
	// Reject shell metacharacters that could enable injection
	if strings.ContainsAny(ref, ";|&$`<>(){}[]!#~") {
		return false
	}
	// Reject spaces and tabs (could split arguments)
	if strings.ContainsAny(ref, " \t") {
		return false
	}
	// Reject refs starting with dot (hidden path traversal)
	if strings.HasPrefix(ref, ".") {
		return false
	}
	// Reject refs containing path separators (could enable traversal)
	if strings.Contains(ref, "/") && !strings.HasPrefix(ref, "refs/") {
		return false
	}
	// Reject refs containing backslashes (Windows path issues)
	if strings.Contains(ref, "\\") {
		return false
	}
	// Reject refs with unicode control characters
	for _, r := range ref {
		if r < 32 || (r >= 0x7F && r < 0xA0) {
			return false
		}
	}
	// Reject extremely long refs (potential buffer overflow)
	if len(ref) > 256 {
		return false
	}
	return true
}

// GitRunner wraps git operations for a working directory.
// This is a local copy to avoid importing the internal/integrations/git package
// which would create a dependency cycle.
type GitRunner struct {
	workDir string
}

// NewGitRunner creates a GitRunner for the given working directory.
func NewGitRunner(workDir string) *GitRunner {
	return &GitRunner{workDir: workDir}
}

// Run executes a git command with the working directory set.
func (g *GitRunner) Run(args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = g.workDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
	}
	return string(out), nil
}

// WorkDir returns the working directory path.
func (g *GitRunner) WorkDir() string {
	return g.workDir
}