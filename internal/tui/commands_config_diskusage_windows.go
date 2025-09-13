//go:build windows

package tui

import "errors"

// diskUsage returns the total disk space in bytes for the filesystem
// containing the given path. Not supported on Windows.
func diskUsage(path string) (uint64, error) {
	return 0, errors.New("disk usage query not supported on Windows")
}
