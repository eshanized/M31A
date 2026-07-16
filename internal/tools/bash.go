package tools

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

// Compile-time interface check
var _ types.Tool = (*Bash)(nil)

type Bash struct {
	workDir                       string
	maxTimeout                    time.Duration
	additionalBlockedCommands     []string
	additionalObfuscationPatterns []string
}

// NewBash creates a new Bash tool instance.
func NewBash(workDir string, maxTimeoutSecs int, additionalBlockedCommands []string, additionalObfuscationPatterns []string) *Bash {
	maxTimeout := types.BashTimeout
	if maxTimeoutSecs > 0 {
		maxTimeout = time.Duration(maxTimeoutSecs) * time.Second
	}
	return &Bash{
		workDir:                       workDir,
		maxTimeout:                    maxTimeout,
		additionalBlockedCommands:     additionalBlockedCommands,
		additionalObfuscationPatterns: additionalObfuscationPatterns,
	}
}

func (t *Bash) Name() string {
	return "Bash"
}

func (t *Bash) Description() string {
	return "Execute a shell command with output capping, timeout, and working directory support. Use the workdir parameter to run commands in a different directory without chaining cd commands."
}

func (t *Bash) RiskLevel() types.RiskLevel {
	return types.RiskDangerous
}

// ParameterSchema returns the JSON Schema for Bash tool parameters.
func (t *Bash) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"command": {
				"type": "string",
				"description": "The shell command to execute"
			},
			"timeout": {
				"type": "integer",
				"description": "Timeout in seconds (default 1800, max 1800)",
				"minimum": 1,
				"maximum": 1800
			},
			"workdir": {
				"type": "string",
				"description": "Working directory for the command (overrides default). If relative, resolved from the default working directory."
			}
		},
		"required": ["command"]
	}`
}

func (t *Bash) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	commandRaw, ok := input.Params["command"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: command: %w", m31errors.ErrToolExecution)
	}
	command, ok := commandRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter command must be a string: %w", m31errors.ErrToolExecution)
	}

	// Check for dangerous commands before execution
	if reason, blocked := checkDangerousCommand(command, t.additionalBlockedCommands, t.additionalObfuscationPatterns); blocked {
		return types.ToolResult{}, types.NewToolError(
			fmt.Errorf("command blocked: %s", reason),
			"Modify the command to avoid destructive patterns. If this is intentional, use a more specific command.",
		)
	}

	// Validate command syntax (unbalanced quotes, etc.)
	if err := validateCommandSyntax(command); err != nil {
		return types.ToolResult{}, types.NewToolError(
			fmt.Errorf("invalid command syntax: %w", err),
			"Fix the command syntax. Ensure all quotes are properly closed.",
		)
	}

	timeoutSec := int(t.maxTimeout.Seconds())
	if customRaw, ok := input.Params["timeout"]; ok {
		if customFloat, ok := customRaw.(float64); ok {
			timeoutSec = int(customFloat)
		}
	}
	if timeoutSec <= 0 {
		return types.ToolResult{}, fmt.Errorf("timeout must be positive: %w", m31errors.ErrInvalidTimeout)
	}
	if timeoutSec > int(t.maxTimeout.Seconds()) {
		return types.ToolResult{}, fmt.Errorf("timeout exceeds max %s: %w", t.maxTimeout, m31errors.ErrInvalidTimeout)
	}

	ctx, cancel := context.WithTimeout(ctx, time.Duration(timeoutSec)*time.Second)
	defer cancel()

	cmd := newShellCmd(ctx, command)
	// Determine working directory: explicit workdir parameter overrides default
	cmd.Dir = t.workDir
	if workdirRaw, ok := input.Params["workdir"].(string); ok && workdirRaw != "" {
		// Validate and clean the workdir path
		cleaned := filepath.Clean(workdirRaw)

		// If relative, resolve against t.workDir
		var absWorkdir string
		if filepath.IsAbs(cleaned) {
			absWorkdir = cleaned
		} else {
			absWorkdir = filepath.Join(t.workDir, cleaned)
		}

		// Ensure the path is within the project directory
		cleanTWorkDir := filepath.Clean(t.workDir)
		rel, err := filepath.Rel(cleanTWorkDir, absWorkdir)
		if err != nil || strings.HasPrefix(rel, "..") {
			return types.ToolResult{}, fmt.Errorf("workdir must be within the project directory: %w", m31errors.ErrToolExecution)
		}

		// Verify path exists
		if _, err := os.Stat(absWorkdir); err != nil {
			return types.ToolResult{}, fmt.Errorf("workdir does not exist: %s: %w", absWorkdir, m31errors.ErrToolExecution)
		}

		cmd.Dir = absWorkdir
	}

	setupProcessGroup(cmd)

	// Apply platform-specific sandboxing: scrubs sensitive env vars,
	// applies OS-level restrictions where available (Landlock on Linux,
	// sandbox-exec on macOS). Falls back to env-scrub-only on other platforms.
	if err := applyBashSandbox(cmd, cmd.Dir); err != nil {
		slog.Warn("bash sandbox setup failed, proceeding without sandbox",
			"error", err)
	}

	// Explicitly close stdin so child processes reading from it get EOF
	// immediately instead of blocking. This prevents hangs from CLIs that
	// open /dev/tty directly or fall back to stdin for interactive prompts.
	cmd.Stdin = strings.NewReader("")

	stdoutR, stdoutW := io.Pipe()
	stderrR, stderrW := io.Pipe()

	// Use limitWriter that gates the pipe writer — output is bounded at BashOutputLimit
	stdoutLimit := &limitWriter{limit: int64(types.BashOutputLimit), w: stdoutW}
	stderrLimit := &limitWriter{limit: int64(types.BashOutputLimit), w: stderrW}

	cmd.Stdout = stdoutLimit
	cmd.Stderr = stderrLimit

	if err := cmd.Start(); err != nil {
		// Close pipe ends to unblock the goroutines that will read from them
		_ = stdoutW.Close()
		_ = stderrW.Close()
		_ = stdoutR.Close()
		errRead, _ := io.ReadAll(io.LimitReader(stderrR, types.BashOutputLimit))
		_ = stderrR.Close()
		extra := ""
		if len(errRead) > 0 {
			extra = ": " + strings.TrimSpace(string(errRead))
		}
		return types.ToolResult{}, fmt.Errorf("%w%s", m31errors.ErrToolExecution, extra)
	}

	// Signal forwarding on cancellation
	var killOnce sync.Once
	cmdDone := make(chan struct{})
	var termMu sync.Mutex
	var terminationMsg string
	go func() {
		defer func() {
			if r := recover(); r != nil {
				slog.Error("bash signal forwarder panic", "error", r)
			}
		}()
		select {
		case <-ctx.Done():
			if cmd.Process != nil {
				killOnce.Do(func() {
					termMu.Lock()
					terminationMsg = "Terminating process..."
					termMu.Unlock()
					processKill(cmd.Process.Pid, sigInt)
				})
				killTimer := time.AfterFunc(BashKillGracePeriod, func() {
					killOnce.Do(func() {
						termMu.Lock()
						terminationMsg = "Force killing process..."
						termMu.Unlock()
						processKill(cmd.Process.Pid, sigKill)
					})
				})
				// Wait for the process to finish so we can cancel the timer
				// if it exits during the grace period
				<-cmdDone
				killTimer.Stop()
			}
		case <-cmdDone:
			// Command finished — goroutine exits immediately
		}
	}()

	// Use error channel instead of shared variable to avoid data race
	waitCh := make(chan error, 1)
	go func() {
		defer func() {
			if r := recover(); r != nil {
				slog.Error("bash wait goroutine panic", "error", r)
			}
		}()
		waitCh <- cmd.Wait()
		_ = stdoutW.Close()
		_ = stderrW.Close()
		close(cmdDone)
	}()

	// Read from both pipes concurrently with per-stream buffers
	var outMu sync.Mutex
	var outStr strings.Builder

	var wg sync.WaitGroup
	wg.Add(2)

	go func() {
		defer wg.Done()
		defer func() {
			if r := recover(); r != nil {
				slog.Error("bash stdout copy panic", "error", r)
			}
		}()
		var stdoutBuf strings.Builder
		_, _ = io.Copy(&stdoutBuf, stdoutR)
		outMu.Lock()
		outStr.WriteString(stdoutBuf.String())
		outMu.Unlock()
	}()

	go func() {
		defer wg.Done()
		defer func() {
			if r := recover(); r != nil {
				slog.Error("bash stderr copy panic", "error", r)
			}
		}()
		var stderrBuf strings.Builder
		_, _ = io.Copy(&stderrBuf, stderrR)
		outMu.Lock()
		if stderrBuf.Len() > 0 {
			if outStr.Len() > 0 {
				outStr.WriteString("\n")
			}
			outStr.WriteString(stderrBuf.String())
		}
		outMu.Unlock()
	}()

	wg.Wait()

	// Close pipe readers explicitly (L-11)
	_ = stdoutR.Close()
	_ = stderrR.Close()

	output := outStr.String()

	// Check if output was truncated
	truncated := stdoutLimit.Written() >= types.BashOutputLimit || stderrLimit.Written() >= types.BashOutputLimit
	if truncated {
		output += "\n[... output truncated by 50K char cap]"
	}

	// Binary detection
	if isBinary(output) {
		output = fmt.Sprintf("[binary output, %d bytes]", len(output))
	}

	elapsed := time.Since(start).Milliseconds()

	// Read wait error from channel (avoids data race)
	var waitErr error
	select {
	case waitErr = <-waitCh:
	case <-time.After(BashWaitTimeout):
		waitErr = fmt.Errorf("wait timeout")
	}

	var exitCode int
	if cmd.ProcessState != nil {
		exitCode = cmd.ProcessState.ExitCode()
	}

	if waitErr != nil {
		if ctx.Err() == context.DeadlineExceeded {
			output = fmt.Sprintf("[TIMEOUT: command exceeded %ds limit]\n%s", timeoutSec, output)
			return types.ToolResult{
				Output:     output,
				DurationMs: elapsed,
				Truncated:  truncated,
				Error:      fmt.Sprintf("timeout after %ds", timeoutSec),
			}, nil
		}
		if ctx.Err() == context.Canceled {
			// Add termination message if available
			termMu.Lock()
			msg := terminationMsg
			termMu.Unlock()
			if msg != "" {
				output = msg + "\n" + output
			}
			return types.ToolResult{
				Output:     output,
				DurationMs: elapsed,
				Truncated:  truncated,
				Error:      "context cancelled",
			}, nil
		}
		return types.ToolResult{
			Output:     output,
			DurationMs: elapsed,
			Truncated:  truncated,
			Error:      fmt.Sprintf("exit code %d", exitCode),
		}, nil
	}

	return types.ToolResult{
		Output:     output,
		DurationMs: elapsed,
		Truncated:  truncated,
	}, nil
}

// limitWriter writes up to limit bytes through w and then silently drops further writes.
type limitWriter struct {
	limit   int64
	written int64
	mu      sync.Mutex
	w       io.Writer
}

func (lw *limitWriter) Write(p []byte) (int, error) {
	lw.mu.Lock()
	remaining := lw.limit - lw.written
	if remaining <= 0 {
		lw.mu.Unlock()
		return len(p), nil
	}
	if int64(len(p)) > remaining {
		p = p[:remaining]
	}
	n, err := lw.w.Write(p)
	lw.written += int64(n)
	lw.mu.Unlock()
	return n, err
}

func (lw *limitWriter) Written() int64 {
	lw.mu.Lock()
	defer lw.mu.Unlock()
	return lw.written
}

// isBinary checks if a string contains binary content.
// Uses multiple detection methods: null bytes, high ratio of non-printable
// characters, and common binary file signatures.
func isBinary(s string) bool {
	if len(s) == 0 {
		return false
	}

	// Check for null bytes (most reliable indicator)
	checkLen := len(s)
	if checkLen > 1024 {
		checkLen = 1024
	}
	for i := 0; i < checkLen; i++ {
		if s[i] == 0 {
			return true
		}
	}

	// Check for high ratio of non-printable characters
	nonPrintable := 0
	total := 0
	for i := 0; i < checkLen && i < len(s); i++ {
		b := s[i]
		// Allow common whitespace and printable ASCII
		if b >= 32 && b < 127 || b == '\n' || b == '\r' || b == '\t' || b == '\f' || b == '\v' {
			continue
		}
		// Allow common UTF-8 continuation bytes
		if b >= 0x80 && b < 0xC0 {
			continue
		}
		nonPrintable++
		total++
	}

	// If more than 10% non-printable in first 1KB, likely binary
	if total > 0 && float64(nonPrintable)/float64(total) > 0.1 {
		return true
	}

	// Check for common binary file signatures (magic bytes)
	if len(s) >= 4 {
		// ELF executable
		if s[0] == '\x7f' && s[1] == 'E' && s[2] == 'L' && s[3] == 'F' {
			return true
		}
		// PDF
		if s[0] == '%' && s[1] == 'P' && s[2] == 'D' && s[3] == 'F' {
			return true
		}
		// ZIP/JAR/APK
		if s[0] == 'P' && s[1] == 'K' && s[2] == '\x03' && s[3] == '\x04' {
			return true
		}
		// PNG
		if s[0] == '\x89' && s[1] == 'P' && s[2] == 'N' && s[3] == 'G' {
			return true
		}
		// GIF
		if s[0] == 'G' && s[1] == 'I' && s[2] == 'F' {
			return true
		}
		// JPEG
		if s[0] == '\xff' && s[1] == '\xd8' && s[2] == '\xff' {
			return true
		}
	}

	return false
}

// dangerousCommandPatterns lists shell command patterns that should be blocked
// as a defense-in-depth measure. These are patterns that could cause catastrophic
// damage if executed accidentally or by a compromised LLM.
var dangerousCommandPatterns = []struct {
	pattern string
	reason  string
}{
	{"rm -rf /", "recursive delete of root filesystem"},
	{"rm -rf /*", "recursive delete of root filesystem"},
	{"rm -fr /", "recursive delete of root filesystem"},
	{"rm -fr /*", "recursive delete of root filesystem"},
	{":(){ :|:& };:", "fork bomb"},
	{"mkfs", "filesystem formatting"},
	{"mkfs.ext4", "ext4 filesystem formatting"},
	{"mkfs.xfs", "XFS filesystem formatting"},
	{"dd if=", "raw disk write"},
	{"dd of=/dev/", "raw disk write to device"},
	{"> /dev/sda", "raw disk overwrite"},
	{"chmod -r 777 /", "recursive permission change on root"},
	{"chmod -r 777 /*", "recursive permission change on root"},
	{"chown -r", "recursive ownership change"},
	{"curl | sh", "piping remote code to shell"},
	{"curl | bash", "piping remote code to shell"},
	{"wget | sh", "piping remote code to shell"},
	{"wget | bash", "piping remote code to shell"},
	{"shutdown", "system shutdown"},
	{"reboot", "system reboot"},
	{"halt", "system halt"},
	{"init 0", "system shutdown"},
	{"init 6", "system reboot"},
	{"systemctl stop", "stopping system services"},
	{"killall", "killing all processes"},
	{"pkill -9", "force killing processes"},
	{"> /etc/", "writing to system config directory"},
	{"mv / ", "moving to root filesystem"},
	{"mv /* ", "moving from root filesystem"},
	{"fdisk", "disk partitioning"},
	{"wipefs", "filesystem signature wiping"},
	{"shred", "secure file deletion"},
	{"nc -l", "netcat listener"},
	{"ncat -l", "netcat listener"},
}

// dangerousObfuscationPatterns catches commands that use shell features
// to hide destructive intent: variable expansion, command substitution,
// base64/hex decode piping, and eval/exec with dangerous payloads.
var dangerousObfuscationPatterns = []struct {
	pattern string
	reason  string
}{
	{"base64 -d", "base64 decode (potential payload obfuscation)"},
	{"base64 --decode", "base64 decode (potential payload obfuscation)"},
	{"| eval", "piping to eval (arbitrary code execution)"},
	{"| exec", "piping to exec (arbitrary code execution)"},
	{"eval $", "eval with variable expansion"},
	{"eval \"$", "eval with variable expansion"},
	{"$(eval", "command substitution with eval"},
	{"xargs rm", "xargs with rm (batch deletion)"},
	{"xargs -0 rm", "xargs with rm (batch deletion)"},
}

// ansiEscapePattern matches ANSI escape sequences (color codes, cursor movement, etc.)
// that could be used to obfuscate command content from pattern matching.
var ansiEscapePattern = regexp.MustCompile(`\x1b\[[0-9;]*m`)

// normalizeCommand normalizes a shell command for security pattern matching.
// It removes ANSI escape codes, collapses whitespace, trims, and lowercases
// to ensure obfuscation techniques cannot bypass dangerous command detection.
func normalizeCommand(cmd string) string {
	// Remove ANSI escape codes
	cmd = ansiEscapePattern.ReplaceAllString(cmd, "")
	// Collapse multiple spaces/tabs/newlines into a single space
	cmd = regexp.MustCompile(`\s+`).ReplaceAllString(cmd, " ")
	// Remove leading/trailing whitespace
	cmd = strings.TrimSpace(cmd)
	// Convert to lowercase for case-insensitive matching
	cmd = strings.ToLower(cmd)
	return cmd
}

// containsVariableExpansion detects shell variable expansion patterns that could
// be used for command injection: $VARIABLE, ${VARIABLE}, $((expression)),
// $(command), and backtick command substitution.
func containsVariableExpansion(cmd string) bool {
	patterns := []string{
		"$[A-Za-z_]",
		"${",
		"$(",
		"`",
	}
	for _, p := range patterns {
		if strings.Contains(cmd, p) {
			return true
		}
	}
	return false
}

// checkDangerousCommand checks if a command matches any dangerous patterns.
// Returns the reason and true if blocked, or empty string and false if allowed.
// The compiled baseline is always checked first (security baseline — never bypassable).
// Additional patterns from config are checked after the baseline.
func checkDangerousCommand(command string, additionalBlocked []string, additionalObfuscation []string) (string, bool) {
	normalized := normalizeCommand(command)

	// Block if variable expansion detected (prevents injection via env vars)
	if containsVariableExpansion(normalized) {
		return "command contains variable expansion (potential injection)", true
	}

	// Check compiled baseline (security baseline — never bypassable)
	for _, dp := range dangerousCommandPatterns {
		if strings.Contains(normalized, dp.pattern) {
			return fmt.Sprintf("blocked dangerous command: %s (pattern: %q)", dp.reason, dp.pattern), true
		}
	}
	for _, dp := range dangerousObfuscationPatterns {
		if strings.Contains(normalized, dp.pattern) {
			return fmt.Sprintf("blocked dangerous command: %s (pattern: %q)", dp.reason, dp.pattern), true
		}
	}

	// Check user-added blocked commands (can be bypassed by removing from config)
	for _, pattern := range additionalBlocked {
		if strings.Contains(normalized, pattern) {
			return fmt.Sprintf("blocked by user-configured command: %q", pattern), true
		}
	}
	// Check user-added obfuscation patterns
	for _, pattern := range additionalObfuscation {
		if strings.Contains(normalized, pattern) {
			return fmt.Sprintf("blocked by user-configured obfuscation pattern: %q", pattern), true
		}
	}

	return "", false
}

// validateCommandSyntax performs basic syntax validation on shell commands.
// Returns an error if the command contains unbalanced quotes or other syntax issues.
func validateCommandSyntax(command string) error {
	inSingleQuote := false
	inDoubleQuote := false
	escaped := false

	for _, ch := range command {
		if escaped {
			escaped = false
			continue
		}
		if ch == '\\' && !inSingleQuote {
			escaped = true
			continue
		}
		if ch == '\'' && !inDoubleQuote {
			inSingleQuote = !inSingleQuote
			continue
		}
		if ch == '"' && !inSingleQuote {
			inDoubleQuote = !inDoubleQuote
			continue
		}
	}

	if inSingleQuote {
		return fmt.Errorf("unbalanced single quotes in command")
	}
	if inDoubleQuote {
		return fmt.Errorf("unbalanced double quotes in command")
	}
	return nil
}
