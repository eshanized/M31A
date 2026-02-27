package bisect

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"
)

// execGit runs a git command directly via exec.Command. Used as a fallback
// when the project's git wrapper (*git.Git) is not wired into Bisect.
// Prefer wiring the git wrapper to benefit from typed errors and logging.
// H-26: Uses a 30-second timeout to prevent indefinite blocking.
func execGit(workDir string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "git", args...)
	cmd.Dir = workDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
	}
	return strings.TrimSpace(string(out)), nil
}
