package tools

import (
	"context"
	"fmt"
	"io"
	"os"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

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
		stdoutW.Close()
		stderrW.Close()
		stdoutR.Close()
		errRead, _ := io.ReadAll(io.LimitReader(stderrR, types.BashOutputLimit))
		stderrR.Close()
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
		waitCh <- cmd.Wait()
		stdoutW.Close()
		stderrW.Close()
		close(cmdDone)
	}()

	// Read from both pipes concurrently with per-stream buffers
	var outMu sync.Mutex
	var outStr strings.Builder

	var wg sync.WaitGroup
	wg.Add(2)

	go func() {
		defer wg.Done()
		var stdoutBuf strings.Builder
		io.Copy(&stdoutBuf, stdoutR)
		outMu.Lock()
		outStr.WriteString(stdoutBuf.String())
		outMu.Unlock()
	}()

	go func() {
		defer wg.Done()
		var stderrBuf strings.Builder
		io.Copy(&stderrBuf, stderrR)
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
	stdoutR.Close()
	stderrR.Close()

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
