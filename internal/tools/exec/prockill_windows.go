//go:build windows

package exec

// getProcessGroup is not applicable on Windows.
// Returns 0, nil (no process group support).
func getProcessGroup(pid int) (int, error) {
	return 0, nil
}

// killProcessGroup is not applicable on Windows.
// Returns nil (no-op).
func killProcessGroup(pgid int) error {
	return nil
}
