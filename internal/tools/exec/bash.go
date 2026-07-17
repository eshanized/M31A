package exec

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*Bash)(nil)

// Constants from original constants.go (copied to avoid circular imports)
const (
	BashKillGracePeriod = time.Duration(types.DefaultBashKillGraceSecs) * time.Second
	BashWaitTimeout     = 30 * time.Second

	MaxBackupsPerFile = types.DefaultMaxBackupsPerFile
	DirPermission     = types.DirPermission
	FilePermission    = types.FilePermission

	MaxGlobResults = types.DefaultMaxGlobResults

	MaxGrepPatternLength  = 1024
	DefaultMaxGrepResults = types.DefaultMaxGrepResults

	MaxRedirects       = types.DefaultWebfetchMaxRedirects
	DefaultTimeoutSecs = 30
	MaxTimeoutSecs     = 120
	DNSCacheTTL        = 5 * time.Minute

	MinLinesForFuzzy     = 3
	LevenshteinThreshold = 0.7

	PermissionChannelBuffer = 8
	QuestionChannelBuffer   = 4
	DefaultAgentName        = "default"

	// Rate limiting: token bucket for tool execution.
	ToolRateLimitBurst  = 20
	ToolRateLimitPerSec = 10

	// Per-risk-level rate limits (M6): dangerous tools get stricter limits.
	DangerousRateLimitBurst  = 5
	DangerousRateLimitPerSec = 2

	// Max concurrent tool executions (M5): prevents resource exhaustion.
	MaxConcurrentTools = 8

	DefaultSearchBaseURL    = "https://search.sagibo.net"
	MaxSearchQueryLength    = 500
	DefaultMaxSearchResults = 5
	MaxSearchResults        = 10

	DefaultOutputMaxLines = 2000
	DefaultOutputMaxBytes = 51200
	OutputRetentionDays   = 7
)

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

// WorkDir returns the working directory for this Bash tool.
func (t *Bash) WorkDir() string {
	return t.workDir
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
	if reason, blocked := CheckDangerousCommand(command, t.additionalBlockedCommands, t.additionalObfuscationPatterns); blocked {
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
					terminationMsg = " (terminated by signal)"
					termMu.Unlock()
					_ = killProcessGroup(cmd.Process.Pid)
				})
			}
		case <-cmdDone:
		}
	}()

	// Read stdout and stderr
	var stdoutBuf, stderrBuf strings.Builder
	var wg sync.WaitGroup
	wg.Add(2)
	go func() {
		defer wg.Done()
		_, _ = io.Copy(&stdoutBuf, io.LimitReader(stdoutR, types.BashOutputLimit))
		_ = stdoutR.Close()
	}()
	go func() {
		defer wg.Done()
		_, _ = io.Copy(&stderrBuf, io.LimitReader(stderrR, types.BashOutputLimit))
		_ = stderrR.Close()
	}()

	err := cmd.Wait()
	close(cmdDone)
	wg.Wait()

	elapsed := time.Since(start).Milliseconds()

	termMu.Lock()
	output := stdoutBuf.String() + stderrBuf.String() + terminationMsg
	termMu.Unlock()

	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return types.ToolResult{
				Output:     output,
				Error:      fmt.Sprintf("command exited with code %d", exitErr.ExitCode()),
				DurationMs: elapsed,
			}, nil
		}
		return types.ToolResult{
			Output:     output,
			Error:      err.Error(),
			DurationMs: elapsed,
		}, nil
	}

	return types.ToolResult{
		Output:     output,
		DurationMs: elapsed,
	}, nil
}

// limitWriter wraps an io.Writer and limits the total bytes written.
type limitWriter struct {
	limit int64
	w     io.Writer
	written int64
}

func (lw *limitWriter) Write(p []byte) (n int, err error) {
	remaining := lw.limit - lw.written
	if remaining <= 0 {
		return 0, nil // silently drop excess
	}
	if int64(len(p)) > remaining {
		p = p[:remaining]
	}
	n, err = lw.w.Write(p)
	lw.written += int64(n)
	return n, err
}

// Regex-based command patterns for dangerous command detection
var (
	dangerousCommandPatterns = []struct {
		pattern string
		reason  string
	}{
		{"rm -rf /", "recursive root deletion"},
		{"rm -rf /*", "recursive root deletion with wildcard"},
		{"mkfs", "filesystem formatting"},
		{"dd if=", "disk imaging/overwriting"},
		{"> /dev/sd", "direct disk write"},
		{"fdisk", "disk partitioning"},
		{"parted", "disk partitioning"},
		{":(){ :|:& };:", "fork bomb"},
		{"chmod -R 777 /", "recursive permission change on root"},
		{"chown -R", "recursive ownership change"},
		{"shutdown", "system shutdown"},
		{"reboot", "system reboot"},
		{"halt", "system halt"},
		{"poweroff", "system power off"},
		{"init 0", "system halt via init"},
		{"init 6", "system reboot via init"},
		{"killall", "kill all processes by name"},
		{"pkill -9", "force kill all processes"},
		{"mv / /", "move root directory"},
		{"cp /dev/zero /dev/sd", "zeroing disk device"},
		{"truncate -s 0 /dev/sd", "truncating disk device"},
	}

	dangerousObfuscationPatterns = []struct {
		pattern string
		reason  string
	}{
		{"base64 -d", "base64 decode (potential obfuscation)"},
		{"echo.*|.*sh", "piped shell execution"},
		{"curl.*|.*sh", "curl to shell pipe"},
		{"wget.*|.*sh", "wget to shell pipe"},
		{"eval.*", "eval command execution"},
		{"exec.*", "exec replacement"},
		{"source /dev/stdin", "source from stdin"},
		{". /dev/stdin", "dot source from stdin"},
	}
)

func CheckDangerousCommand(command string, additionalBlocked []string, additionalObfuscation []string) (string, bool) {
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

func normalizeCommand(cmd string) string {
	// Remove comments
	cmd = regexp.MustCompile(`#[^\n]*`).ReplaceAllString(cmd, "")
	// Collapse whitespace
	cmd = regexp.MustCompile(`\s+`).ReplaceAllString(cmd, " ")
	// Convert to lowercase for case-insensitive matching
	return strings.ToLower(strings.TrimSpace(cmd))
}

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