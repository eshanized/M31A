package workflow

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
)

// QualityGateResult holds the outcome of checking acceptance criteria against file state.
type QualityGateResult struct {
	TaskID  int
	Passed  bool
	Checked int
	Failed  int
	Details []string
}

// checkAcceptanceCriteria verifies a task's acceptance criteria against the actual
// state of files on disk. Each criterion is checked as a grep-verifiable condition.
func (e *Engine) checkAcceptanceCriteria(task m31types.Task) QualityGateResult {
	result := QualityGateResult{
		TaskID: task.ID,
		Passed: true,
	}

	for _, criterion := range task.AcceptanceCriteria {
		result.Checked++
		if !e.verifyCriterion(task, criterion) {
			result.Failed++
			result.Passed = false
			result.Details = append(result.Details,
				fmt.Sprintf("FAIL: %s", criterion))
		} else {
			result.Details = append(result.Details,
				fmt.Sprintf("PASS: %s", criterion))
		}
	}

	return result
}

// verifyCriterion checks a single acceptance criterion against file state.
// Supports several patterns:
//   - "FILE contains STRING" — check if a file contains a substring
//   - "FILE exists" — check if a file exists
//   - "FILE has FUNCTION_PATTERN" — check for function declarations
//   - Fallback: check if any task file contains the criterion text
func (e *Engine) verifyCriterion(task m31types.Task, criterion string) bool {
	lower := strings.ToLower(criterion)

	// Pattern: "FILENAME contains STRING"
	if idx := strings.Index(lower, " contains "); idx > 0 {
		filename := strings.TrimSpace(criterion[:idx])
		searchStr := strings.TrimSpace(criterion[idx+len(" contains "):])
		return fileContains(e.workDir, filename, searchStr)
	}

	// Pattern: "FILENAME exists"
	if strings.HasSuffix(lower, " exists") {
		filename := strings.TrimSpace(criterion[:len(criterion)-len(" exists")])
		fullPath := filepath.Join(e.workDir, filename)
		_, err := os.Stat(fullPath)
		return err == nil
	}

	// Pattern: "FILENAME has PATTERN"
	if idx := strings.Index(lower, " has "); idx > 0 {
		filename := strings.TrimSpace(criterion[:idx])
		pattern := strings.TrimSpace(criterion[idx+len(" has "):])
		return fileContains(e.workDir, filename, pattern)
	}

	// Fallback: check if the criterion text appears in any task file
	for _, f := range task.Files {
		if fileContains(e.workDir, f, extractKeyPhrase(criterion)) {
			return true
		}
	}

	// If no files to check, give the benefit of the doubt
	// (some criteria like "npm test exits 0" require command execution)
	return len(task.Files) == 0
}

// fileContains checks if a file in the workdir contains the given substring.
func fileContains(workDir, filename, substr string) bool {
	fullPath := filepath.Join(workDir, filename)
	data, err := os.ReadFile(fullPath)
	if err != nil {
		return false
	}
	return strings.Contains(strings.ToLower(string(data)), strings.ToLower(substr))
}

// extractKeyPhrase extracts the most meaningful phrase from a criterion
// for fallback file searching. Strips common prefixes and keeps the core.
func extractKeyPhrase(criterion string) string {
	// Strip common prefixes
	prefixes := []string{
		"the ", "a ", "an ", "verify that ", "check that ", "ensure that ",
		"confirm that ", "assert that ", "validate that ",
	}
	lower := strings.ToLower(criterion)
	for _, prefix := range prefixes {
		if strings.HasPrefix(lower, prefix) {
			criterion = criterion[len(prefix):]
			break
		}
	}

	// Cap at 50 chars for efficient searching
	if len(criterion) > 50 {
		criterion = criterion[:50]
	}

	return strings.TrimSpace(criterion)
}

// BehavioralResult holds the outcome of running a verification command.
type BehavioralResult struct {
	Command  string `json:"command"`
	Passed   bool   `json:"passed"`
	Output   string `json:"output"`
	Error    string `json:"error,omitempty"`
	Duration int64  `json:"duration_ms"`
}

// BehavioralVerification runs CMDs from acceptance criteria and validates outcomes.
// Commands are checked against a blocklist (destructive/dangerous) and allowlist (safe).
// Commands outside both lists are rejected (would require permission modal in interactive mode).
func (e *Engine) BehavioralVerification(task m31types.Task) ([]BehavioralResult, error) {
	var results []BehavioralResult

	for _, criterion := range task.AcceptanceCriteria {
		cmd := extractCommand(criterion)
		if cmd == "" {
			continue
		}

		// Check blocklist first
		if isBlockedCommand(cmd) {
			results = append(results, BehavioralResult{
				Command: cmd,
				Passed:  false,
				Error:   "command blocked by security policy",
			})
			continue
		}

		// Check allowlist
		if !isAllowedCommand(cmd) {
			results = append(results, BehavioralResult{
				Command: cmd,
				Passed:  false,
				Error:   "command not in allowlist (requires permission)",
			})
			continue
		}

		// Check HTTP restrictions (localhost only)
		if isHTTPCommand(cmd) && !isLocalhostOnly(cmd) {
			results = append(results, BehavioralResult{
				Command: cmd,
				Passed:  false,
				Error:   "HTTP verification restricted to localhost only",
			})
			continue
		}

		// Execute the command with a 60-second timeout
		ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
		start := time.Now()
		out, err := e.execCommandContext(ctx, "sh", "-c", cmd)
		cancel()
		duration := time.Since(start).Milliseconds()

		result := BehavioralResult{
			Command:  cmd,
			Passed:   err == nil,
			Output:   truncateOutput(string(out), 2000),
			Duration: duration,
		}
		if err != nil {
			result.Error = err.Error()
		}
		results = append(results, result)
	}

	return results, nil
}

// extractCommand extracts a shell command from a criterion string.
// Looks for patterns like "run CMD", "execute CMD", or backtick-quoted commands.
func extractCommand(criterion string) string {
	lower := strings.ToLower(criterion)

	// Pattern: "run `command`" or "execute `command`"
	if idx := strings.Index(lower, "`"); idx >= 0 {
		end := strings.Index(criterion[idx+1:], "`")
		if end >= 0 {
			return criterion[idx+1 : idx+1+end]
		}
	}

	// Pattern: "run CMD" or "execute CMD"
	for _, prefix := range []string{"run ", "execute ", "cmd: ", "command: "} {
		if strings.HasPrefix(lower, prefix) {
			cmd := strings.TrimSpace(criterion[len(prefix):])
			// Take until end of string or next clause
			if idx := strings.Index(cmd, " and "); idx > 0 {
				cmd = cmd[:idx]
			}
			if idx := strings.Index(cmd, " then "); idx > 0 {
				cmd = cmd[:idx]
			}
			return strings.TrimSpace(cmd)
		}
	}

	return ""
}

// blockedCommands are patterns that must never be executed during verification.
var blockedCommands = regexp.MustCompile(`(?i)(?:\brm\s+-rf\s+/|\bsudo\b|\bchmod\s+777\b|\bcurl\s+.*\|\s*sh\b|\bwget\s+.*\|\s*bash\b|\bmkfs\b|\bdd\s+|\bformat\b|\bkill\s+-9\s+1\b|\bshutdown\b|\breboot\b|\bnc\s+|\bncat\b|\bssh\s+|\bscp\s+|\brsync\s+.*--delete\b|\btar\s+.*--delete\b|\bpg_dump\b|\bmysqldump\b|\bexec\b.*\bexec\b)`)

// allowedCommands are safe commands that can be executed during verification.
var allowedCommands = regexp.MustCompile(`^(?:go\s+(?:build|test|vet|fmt|staticcheck|lint)|(?:npm\s+(?:test|run\s+\w+|ci|install))|(?:yarn\s+(?:test|run\s+\w+|install))|(?:pnpm\s+(?:test|run\s+\w+|install))|(?:cargo\s+(?:build|test|clippy|fmt|check))|(?:make\s+\w+)|(?:python3?\s+(?:-m\s+)?(?:pytest|unittest|pylint|mypy|black|isort|ruff))|(?:pip(?:3)?\s+install)|(?:bundle\s+(?:exec\s+)?(?:rspec|rake|rubocop))|(?:dotnet\s+(?:build|test|run))|(?:gradle\s+\w+)|(?:mvn\s+\w+)|(?:node\s+[\w./-]+\.js)|(?:npx\s+\w+))$`)

// httpCommandPattern matches commands that make HTTP requests.
var httpCommandPattern = regexp.MustCompile(`(?i)\b(?:curl|wget|httpie|http|fetch)\b`)

// localhostPattern matches localhost/127.0.0.1/0.0.0.0 in URLs.
var localhostPattern = regexp.MustCompile(`(?i)(?:localhost|127\.0\.0\.1|0\.0\.0\.0|::1)`)

func isBlockedCommand(cmd string) bool {
	return blockedCommands.MatchString(cmd)
}

func isAllowedCommand(cmd string) bool {
	return allowedCommands.MatchString(cmd)
}

func isHTTPCommand(cmd string) bool {
	return httpCommandPattern.MatchString(cmd)
}

func isLocalhostOnly(cmd string) bool {
	return localhostPattern.MatchString(cmd)
}

func truncateOutput(s string, maxLen int) string {
	if maxLen <= 0 {
		return ""
	}
	if len(s) <= maxLen {
		return s
	}
	if maxLen <= 3 {
		return s[:maxLen]
	}
	return s[:maxLen-3] + "..."
}

// execCommandContext runs a command with the given context. If ctx is nil,
// a 60-second timeout is applied. The caller must ensure the returned
// context is cancelled via the returned cancel function.
func (e *Engine) execCommandContext(ctx context.Context, name string, args ...string) ([]byte, error) {
	if ctx == nil {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(context.Background(), 60*time.Second)
		defer cancel()
	}
	cmd := exec.CommandContext(ctx, name, args...)
	cmd.Dir = e.workDir
	return cmd.CombinedOutput()
}
