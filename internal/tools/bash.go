package tools

import (
	"context"
	"fmt"
	"io"
	"os/exec"
	"runtime"
	"strings"
	"sync"
	"syscall"
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
		return types.ToolResult{}, fmt.Errorf("missing parameter: command")
	}
	command, ok := commandRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter command must be a string")
	}

	timeoutSec := 1800
	if customRaw, ok := input.Params["timeout"]; ok {
		if customFloat, ok := customRaw.(float64); ok {
			timeoutSec = int(customFloat)
		}
	}

	ctx, cancel := context.WithTimeout(ctx, time.Duration(timeoutSec)*time.Second)
	defer cancel()

	var cmd *exec.Cmd
	if runtime.GOOS == "windows" {
		cmd = exec.CommandContext(ctx, "cmd", "/C", command)
	} else {
		cmd = exec.CommandContext(ctx, "bash", "-c", command)
	}
	cmd.Dir = t.workDir

	if runtime.GOOS != "windows" {
		cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	}

	// Use limited writers that cap output at BashOutputLimit
	stdoutLimit := &limitWriter{limit: types.BashOutputLimit}
	stderrLimit := &limitWriter{limit: types.BashOutputLimit}

	stdoutR, stdoutW := io.Pipe()
	stderrR, stderrW := io.Pipe()

	cmd.Stdout = io.MultiWriter(stdoutW, stdoutLimit)
	cmd.Stderr = io.MultiWriter(stderrW, stderrLimit)

	if err := cmd.Start(); err != nil {
		return types.ToolResult{}, m31errors.ErrToolExecution
	}

	// Signal forwarding on cancellation
	var killOnce sync.Once
	go func() {
		select {
		case <-ctx.Done():
			if cmd.Process != nil {
				killOnce.Do(func() {
					pgid := cmd.Process.Pid
					if runtime.GOOS != "windows" {
						pgid = -cmd.Process.Pid
					}
					syscall.Kill(pgid, syscall.SIGINT)
				})
				time.AfterFunc(5*time.Second, func() {
					killOnce.Do(func() {
						pgid := cmd.Process.Pid
						if runtime.GOOS != "windows" {
							pgid = -cmd.Process.Pid
						}
						syscall.Kill(pgid, syscall.SIGKILL)
					})
				})
			}
		case <-time.After(time.Minute):
			// goroutine cleanup after command finishes
		}
	}()

	// Close write ends when command finishes
	var waitErr error
	go func() {
		waitErr = cmd.Wait()
		stdoutW.Close()
		stderrW.Close()
	}()

	// Read from both pipes concurrently
	var outMu sync.Mutex
	var outStr strings.Builder

	var wg sync.WaitGroup
	wg.Add(2)

	go func() {
		defer wg.Done()
		io.Copy(&outStr, stdoutR)
	}()

	go func() {
		defer wg.Done()
		var stderrBuf strings.Builder
		io.Copy(&stderrBuf, stderrR)
		if stderrBuf.Len() > 0 {
			outMu.Lock()
			defer outMu.Unlock()
			if outStr.Len() > 0 {
				outStr.WriteString("\n")
			}
			outStr.WriteString(stderrBuf.String())
		}
	}()

	wg.Wait()

	output := outStr.String()

	// Check if output was truncated
	truncated := stdoutLimit.written >= types.BashOutputLimit || stderrLimit.written >= types.BashOutputLimit

	// Binary detection
	if isBinary(output) {
		output = fmt.Sprintf("[binary output, %d bytes]", len(output))
	}

	elapsed := time.Since(start).Milliseconds()

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
	limit int
	written int
}

func (lw *limitWriter) Write(p []byte) (int, error) {
	remaining := lw.limit - lw.written
	if remaining <= 0 {
		return len(p), nil
	}
	if len(p) > remaining {
		p = p[:remaining]
	}
	n := len(p)
	lw.written += n
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
