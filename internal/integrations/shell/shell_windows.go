//go:build windows

// Package shell provides platform-aware shell command execution.
package shell

import (
	"context"
	"os/exec"
)

// CommandContext returns an *exec.Cmd that runs command via the platform's
// default shell. On Windows, this is "cmd /C <command>".
func CommandContext(ctx context.Context, command string) *exec.Cmd {
	return exec.CommandContext(ctx, "cmd", "/C", command)
}

// Command returns an *exec.Cmd that runs command via the platform's
// default shell. On Windows, this is "cmd /C <command>".
func Command(command string) *exec.Cmd {
	return exec.Command("cmd", "/C", command)
}
