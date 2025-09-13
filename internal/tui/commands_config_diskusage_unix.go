//go:build !windows

package tui

import "syscall"

// diskUsage returns the total disk space in bytes for the filesystem
// containing the given path. It uses syscall.Statfs on Unix systems.
func diskUsage(path string) (uint64, error) {
	var stat syscall.Statfs_t
	if err := syscall.Statfs(path, &stat); err != nil {
		return 0, err
	}
	return stat.Blocks * uint64(stat.Bsize), nil
}
