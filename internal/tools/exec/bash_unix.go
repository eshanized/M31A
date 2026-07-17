//go:build !windows

package exec

import (
	"context"
	"os/exec"
	"syscall"
)

// Signal constants for cross-platform process control
const (
	sigInt  = 2 // SIGINT
	sigKill = 9 // SIGKILL
)

func setupProcessGroup(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func processKill(pid int, sig int) {
	_ = syscall.Kill(-pid, syscall.Signal(sig))
}

func newShellCmd(ctx context.Context, command string) *exec.Cmd {
	return exec.CommandContext(ctx, "bash", "-c", command)
}
