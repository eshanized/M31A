package tools

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*Bash)(nil)

type Bash struct {
	workDir string
}

func NewBash(workDir string) *Bash {
	return &Bash{
		workDir: workDir,
	}
}

func (t *Bash) Name() string {
	return "Bash"
}

func (t *Bash) Description() string {
	return "Execute a shell command with output capping, timeout, and working directory support."
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
	if reason, blocked := checkDangerousCommand(command); blocked {
		return types.ToolResult{}, types.NewToolError(
			fmt.Errorf("command blocked: %s", reason),
			"Modify the command to avoid destructive patterns. If this is intentional, use a more specific command.",
		)
	}

	timeoutSec := int(types.BashTimeout.Seconds())
	if customRaw, ok := input.Params["timeout"]; ok {
		if customFloat, ok := customRaw.(float64); ok {
			timeoutSec = int(customFloat)
		}
	}
	if timeoutSec <= 0 {
		return types.ToolResult{}, fmt.Errorf("timeout must be positive: %w", m31errors.ErrInvalidTimeout)
	}
	if timeoutSec > int(types.BashTimeout.Seconds()) {
		return types.ToolResult{}, fmt.Errorf("timeout exceeds max %s: %w", types.BashTimeout, m31errors.ErrInvalidTimeout)
	}

	ctx, cancel := context.WithTimeout(ctx, time.Duration(timeoutSec)*time.Second)
	defer cancel()

	cmd := newShellCmd(ctx, command)
	cmd.Dir = t.workDir

	setupProcessGroup(cmd)

	// Inject non-interactive environment variables to prevent CLI tools from
	// hanging on stdin prompts (e.g., npx init, npm create, apt-get).
	cmd.Env = append(os.Environ(),
		"CI=true",
		"DEBIAN_FRONTEND=noninteractive",
		"npm_config_yes=true",
		"PIP_NO_INPUT=1",
		"YARN_ENABLE_IMMUTABLE_INSTALLS=false",
	)

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

// isBinary checks if a string contains null bytes (binary content).
func isBinary(s string) bool {
	if len(s) == 0 {
		return false
	}
	checkLen := len(s)
	if checkLen > 512 {
		checkLen = 512
	}
	for i := 0; i < checkLen; i++ {
		if s[i] == 0 {
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
	{"dd if=", "raw disk write"},
	{"dd of=/dev/", "raw disk write to device"},
	{"> /dev/sda", "raw disk overwrite"},
	{"chmod -R 777 /", "recursive permission change on root"},
	{"chmod -R 777 /*", "recursive permission change on root"},
	{"chown -R", "recursive ownership change"},
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
}

// checkDangerousCommand checks if a command matches any dangerous patterns.
// Returns the reason and true if blocked, or empty string and false if allowed.
func checkDangerousCommand(command string) (string, bool) {
	normalized := strings.ToLower(strings.TrimSpace(command))
	for _, dp := range dangerousCommandPatterns {
		if strings.Contains(normalized, dp.pattern) {
			return fmt.Sprintf("blocked dangerous command: %s (pattern: %q)", dp.reason, dp.pattern), true
		}
	}
	return "", false
}
