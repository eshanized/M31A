//go:build !windows

package workflow

import (
	"syscall"
)

func getProcessGroup(pid int) (int, error) {
	return syscall.Getpgid(pid)
}

func killProcessGroup(pgid int) error {
	return syscall.Kill(-pgid, syscall.SIGTERM)
}
