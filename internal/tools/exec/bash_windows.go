//go:build windows

package exec

import (
	"context"
	"os/exec"
	"strconv"
)

func setupProcessGroup(cmd *exec.Cmd) {
	// No process groups on Windows
}

// Signal constants (ignored on Windows)
const (
	sigInt  = 2
	sigKill = 9
)

func processKill(pid int, sig int) {
	// On Windows, use taskkill to terminate the process
	_ = sig // ignored on Windows
	c := exec.Command("taskkill", "/F", "/PID", strconv.Itoa(pid))
	_ = c.Run()
}

func newShellCmd(ctx context.Context, command string) *exec.Cmd {
	return exec.CommandContext(ctx, "cmd", "/C", command)
}