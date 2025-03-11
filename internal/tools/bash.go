package tools

import (
	"bytes"
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

	var stdout io.ReadCloser
	var stderrBuf bytes.Buffer
	var err error

	stdout, err = cmd.StdoutPipe()
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("stdout pipe failed: %w", err)
	}
	cmd.Stderr = &stderrBuf

	if err := cmd.Start(); err != nil {
		return types.ToolResult{}, m31errors.ErrToolExecution
	}

	var output strings.Builder
	var mu sync.Mutex
	truncated := false
	done := make(chan struct{})

	// Kill escalation goroutine
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
				// SIGKILL escalation after 5s grace
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
		case <-done:
		}
	}()

	// Read stdout
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		buf := make([]byte, 4096)
		for {
			n, err := stdout.Read(buf)
			if n > 0 {
				mu.Lock()
				if output.Len()+n > types.BashOutputLimit {
					toWrite := n
					if output.Len()+toWrite > types.BashOutputLimit {
						toWrite = types.BashOutputLimit - output.Len()
						truncated = true
					}
					if toWrite > 0 {
						output.Write(buf[:toWrite])
					}
					mu.Unlock()
				} else {
					output.Write(buf[:n])
					mu.Unlock()
				}
			}
			if err != nil {
				return
			}
		}
	}()

	waitErr := cmd.Wait()
	close(done)
	stdout.Close()
	wg.Wait()

	mu.Lock()
	outStr := output.String()
	mu.Unlock()

	// Append stderr (non-PTY mode)
	if stderrBuf.Len() > 0 {
		if outStr != "" {
			outStr += "\n"
		}
		outStr += stderrBuf.String()
	}

	// Binary detection
	if len(outStr) > 0 {
		firstBytes := []byte(outStr)
		checkLen := 512
		if len(firstBytes) > checkLen {
			firstBytes = firstBytes[:checkLen]
		}
		isBinary := false
		for _, b := range firstBytes {
			if b == 0 {
				isBinary = true
				break
			}
		}
		if isBinary {
			outStr = fmt.Sprintf("[binary output, %d bytes]", len(outStr))
		}
	}

	elapsed := time.Since(start).Milliseconds()

	var exitCode int
	if cmd.ProcessState != nil {
		exitCode = cmd.ProcessState.ExitCode()
	}

	if waitErr != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return types.ToolResult{
				Output:     outStr,
				DurationMs: elapsed,
				Truncated:  truncated,
				Error:      fmt.Sprintf("timeout after %ds", timeoutSec),
			}, nil
		}
		if ctx.Err() == context.Canceled {
			return types.ToolResult{
				Output:     outStr,
				DurationMs: elapsed,
				Truncated:  truncated,
				Error:      "context cancelled",
			}, nil
		}
		// Non-zero exit
		return types.ToolResult{
			Output:     outStr,
			DurationMs: elapsed,
			Truncated:  truncated,
			Error:      fmt.Sprintf("exit code %d", exitCode),
		}, nil
	}

	return types.ToolResult{
		Output:     outStr,
		DurationMs: elapsed,
		Truncated:  truncated,
	}, nil
}
