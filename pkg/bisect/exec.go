package bisect

import (
	"fmt"
	"os/exec"
	"strings"
)

// execGit runs a git command directly via exec.Command. Used as a fallback
// when the project's git wrapper (*git.Git) is not wired into Bisect.
// Prefer wiring the git wrapper to benefit from typed errors and logging.
func execGit(workDir string, args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = workDir
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
	}
	return strings.TrimSpace(string(out)), nil
}
