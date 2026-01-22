package bisect

import (
	"fmt"
	"log/slog"
	"strings"

	"github.com/eshanized/M31A/internal/git"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

// BisectResult holds the offending commit and its diff.
type BisectResult struct {
	OffendingCommit git.CommitInfo
	Diff            string
}

// GitRunner is the subset of *git.Git that Bisect uses. Using an interface
// lets callers inject test doubles without pulling in the full git wrapper.
type GitRunner interface {
	Run(args ...string) (string, error)
}

// Bisect wraps git bisect operations.
type Bisect struct {
	workDir string
	logger  *slog.Logger
	git     GitRunner
}

// New creates a Bisect instance backed by the project's git wrapper.
// If git is nil, a minimal exec-based fallback is used (kept for
// backward compatibility with callers that don't wire the wrapper).
func New(workDir string, logger *slog.Logger) *Bisect {
	return &Bisect{workDir: workDir, logger: logger}
}

// SetGit wires the project's git wrapper into the bisect instance.
func (b *Bisect) SetGit(g GitRunner) {
	b.git = g
}

// run executes a git command via the wired git wrapper. Falls back to
// the in-process exec.Command only when no wrapper was provided.
func (b *Bisect) run(args ...string) (string, error) {
	if b.git != nil {
		out, err := b.git.Run(args...)
		if err != nil {
			return out, fmt.Errorf("git %s: %w", strings.Join(args, " "), err)
		}
		return strings.TrimSpace(out), nil
	}
	// Fallback: no wrapper wired — use direct exec (preserves legacy callers).
	return execGit(b.workDir, args...)
}

// Run performs a git bisect between sessionStartHash (good) and headHash (bad)
// using checkFn to determine pass/fail at each step.
// checkFn returns true if the commit passes verification, false if it fails.
// On git bisect reset failure, returns an error wrapping ErrBisectResetFailed (M-29).
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (result *BisectResult, err error) {
	if b.logger != nil {
		b.logger.Info("bisect starting", "good", sessionStartHash, "bad", headHash)
	}

	// Ensure we always reset (wrap reset failure with typed error)
	defer func() {
		if _, resetErr := b.run("bisect", "reset"); resetErr != nil {
			if err == nil {
				err = fmt.Errorf("bisect reset: %w: %w", m31errors.ErrBisectResetFailed, resetErr)
			}
		}
	}()

	// Start bisect
	if _, err := b.run("bisect", "start"); err != nil {
		return nil, fmt.Errorf("bisect start: %w", err)
	}

	// Mark good and bad
	if _, err := b.run("bisect", "good", sessionStartHash); err != nil {
		return nil, fmt.Errorf("bisect good %s: %w", sessionStartHash, err)
	}
	if _, err := b.run("bisect", "bad", headHash); err != nil {
		return nil, fmt.Errorf("bisect bad %s: %w", headHash, err)
	}

	// Bisect loop
	for {
		// Check if bisect is complete
		logOut, _ := b.run("bisect", "log")
		if strings.Contains(logOut, "first bad commit") {
			break
		}

		// Get current bisect commit
		current, err := b.run("rev-parse", "HEAD")
		if err != nil {
			return nil, fmt.Errorf("bisect rev-parse HEAD: %w", err)
		}

		if b.logger != nil {
			b.logger.Info("bisect checking", "commit", current)
		}

		// Run check function
		if checkFn() {
			if _, err := b.run("bisect", "good"); err != nil {
				return nil, fmt.Errorf("bisect good: %w", err)
			}
		} else {
			if _, err := b.run("bisect", "bad"); err != nil {
				return nil, fmt.Errorf("bisect bad: %w", err)
			}
		}
	}

	// Parse result
	logOut, err := b.run("bisect", "log")
	if err != nil {
		return nil, fmt.Errorf("bisect log: %w", err)
	}

	offending := parseBisectLog(logOut)
	if offending == "" {
		return nil, fmt.Errorf("could not parse offending commit from bisect log")
	}

	// Get diff
	diff, _ := b.run("diff", offending+"^.."+offending)

	return &BisectResult{
		OffendingCommit: git.CommitInfo{
			Hash:      offending,
			ShortHash: offending,
		},
		Diff: diff,
	}, nil
}

// parseBisectLog extracts the offending commit hash from bisect log output.
func parseBisectLog(log string) string {
	lines := strings.Split(log, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		// Standard format: "# first bad commit: [abc123] commit message"
		if strings.HasPrefix(line, "# first bad commit:") {
			rest := strings.TrimSpace(strings.TrimPrefix(line, "# first bad commit:"))
			// Extract from [hash] format
			if strings.HasPrefix(rest, "[") {
				closeIdx := strings.Index(rest, "]")
				if closeIdx > 0 {
					return rest[1:closeIdx]
				}
			}
			// Or just take the first word
			parts := strings.Fields(rest)
			if len(parts) > 0 {
				return parts[0]
			}
		}
		// Some git versions use different format
		if strings.HasPrefix(line, "Bisecting:") {
			continue
		}
		// Try to parse: "[hash] message"
		if len(line) >= 7 && line[0] == '[' {
			parts := strings.SplitN(line, "]", 2)
			if len(parts) == 2 {
				return strings.TrimSpace(parts[0][1:])
			}
		}
	}
	return ""
}
