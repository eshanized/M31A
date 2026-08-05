//go:build !windows

package exec

import (
	"syscall"
)

// getProcessGroup returns the process group ID for the given PID.
func getProcessGroup(pid int) (int, error) {
	return syscall.Getpgid(pid)
}

// killProcessGroup sends SIGKILL to the given process group to ensure
// immediate termination. Per D-07, cancellation uses SIGKILL (not SIGTERM)
// to guarantee the process tree is killed promptly.
func killProcessGroup(pgid int) error {
	return syscall.Kill(-pgid, syscall.SIGKILL)
}
