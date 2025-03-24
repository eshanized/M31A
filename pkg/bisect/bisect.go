package bisect

import (
	"fmt"
	"log/slog"
	"os/exec"
	"strings"
)

// CommitInfo represents metadata about a git commit.
type CommitInfo struct {
	Hash      string `json:"hash"`
	ShortHash string `json:"short_hash"`
	Author    string `json:"author"`
	Message   string `json:"message"`
}

// BisectResult holds the offending commit and its diff.
type BisectResult struct {
	OffendingCommit CommitInfo
	Diff            string
}

// Bisect wraps git bisect operations.
type Bisect struct {
	workDir string
	logger  *slog.Logger
}

// New creates a Bisect instance.
func New(workDir string, logger *slog.Logger) *Bisect {
	return &Bisect{workDir: workDir, logger: logger}
}

// run executes a git command.
func (b *Bisect) run(args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = b.workDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
	}
	return strings.TrimSpace(string(out)), nil
}

// Run performs a git bisect between sessionStartHash (good) and headHash (bad)
// using checkFn to determine pass/fail at each step.
// checkFn returns true if the commit passes verification, false if it fails.
func (b *Bisect) Run(sessionStartHash, headHash string, checkFn func() bool) (*BisectResult, error) {
	if b.logger != nil {
		b.logger.Info("bisect starting", "good", sessionStartHash, "bad", headHash)
	}

	// Ensure we always reset
	defer func() {
		b.run("bisect", "reset")
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
		OffendingCommit: CommitInfo{
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
