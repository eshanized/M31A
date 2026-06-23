//go:build windows

package tools

import (
	"os"
)

// getProcessGroup returns 0 on Windows (process groups not supported).
func getProcessGroup(pid int) (int, error) {
	return 0, nil
}

// killProcessGroup kills the process by PID on Windows using os.Process.Kill.
func killProcessGroup(pid int) error {
	p, err := os.FindProcess(pid)
	if err != nil {
		return err
	}
	return p.Kill()
}
