package tools

import (
	"context"
	"fmt"
	"io"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/types"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

type Bash struct {
	workDir string
}

func NewBash(workDir string) *Bash {
	return &Bash{workDir: workDir}
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

	timeoutSec := 1800
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

	// Use limited writers that cap output at BashOutputLimit
	stdoutLimit := &limitWriter{limit: int64(types.BashOutputLimit)}
	stderrLimit := &limitWriter{limit: int64(types.BashOutputLimit)}

	stdoutR, stdoutW := io.Pipe()
	stderrR, stderrW := io.Pipe()

	cmd.Stdout = io.MultiWriter(stdoutW, stdoutLimit)
	cmd.Stderr = io.MultiWriter(stderrW, stderrLimit)

	if err := cmd.Start(); err != nil {
		// Close pipe ends to unblock the goroutines that will read from them
		stdoutW.Close()
		stderrW.Close()
		stdoutR.Close()
		errRead, _ := io.ReadAll(stderrR)
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
	go func() {
		select {
		case <-ctx.Done():
			if cmd.Process != nil {
				killOnce.Do(func() {
					processKill(cmd.Process.Pid, sigInt)
				})
				time.AfterFunc(5*time.Second, func() {
					killOnce.Do(func() {
						processKill(cmd.Process.Pid, sigKill)
					})
				})
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

	output := outStr.String()

	// Check if output was truncated
	truncated := atomic.LoadInt64(&stdoutLimit.written) >= types.BashOutputLimit || atomic.LoadInt64(&stderrLimit.written) >= types.BashOutputLimit
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
	case <-time.After(30 * time.Second):
		waitErr = fmt.Errorf("wait timeout")
	}

	var exitCode int
	if cmd.ProcessState != nil {
		exitCode = cmd.ProcessState.ExitCode()
	}

	if waitErr != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return types.ToolResult{
				Output:     output,
				DurationMs: elapsed,
				Truncated:  truncated,
				Error:      fmt.Sprintf("timeout after %ds", timeoutSec),
			}, nil
		}
		if ctx.Err() == context.Canceled {
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

// limitWriter writes up to limit bytes and then silently drops further writes.
type limitWriter struct {
	limit   int64
	written int64
}

func (lw *limitWriter) Write(p []byte) (int, error) {
	remaining := atomic.LoadInt64(&lw.limit) - atomic.LoadInt64(&lw.written)
	if remaining <= 0 {
		return len(p), nil
	}
	if int64(len(p)) > remaining {
		p = p[:remaining]
	}
	n := len(p)
	atomic.AddInt64(&lw.written, int64(n))
	return n, nil
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
