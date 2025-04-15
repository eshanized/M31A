//go:build windows

package tools

import (
	"context"
	"os/exec"
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
	c := exec.Command("taskkill", "/F", "/PID", string(rune(pid)))
	_ = c.Run()
}

func newShellCmd(ctx context.Context, command string) *exec.Cmd {
	return exec.CommandContext(ctx, "cmd", "/C", command)
}

