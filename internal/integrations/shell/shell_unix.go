//go:build !windows

// Package shell provides platform-aware shell command execution.
package shell

import (
	"context"
	"os/exec"
)

// CommandContext returns an *exec.Cmd that runs command via the platform's
// default shell. On Unix, this is "sh -c <command>".
func CommandContext(ctx context.Context, command string) *exec.Cmd {
	return exec.CommandContext(ctx, "sh", "-c", command)
}

// Command returns an *exec.Cmd that runs command via the platform's
// default shell. On Unix, this is "sh -c <command>".
func Command(command string) *exec.Cmd {
	return exec.Command("sh", "-c", command)
}
