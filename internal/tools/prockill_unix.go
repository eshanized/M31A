//go:build !windows

package tools

import (
	"syscall"
)

// getProcessGroup returns the process group ID for the given PID.
func getProcessGroup(pid int) (int, error) {
	return syscall.Getpgid(pid)
}

// killProcessGroup sends SIGTERM to the given process group.
func killProcessGroup(pgid int) error {
	return syscall.Kill(-pgid, syscall.SIGTERM)
}
