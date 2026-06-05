//go:build windows

package commands

import (
	"os"
	"path/filepath"
)

// diskUsageBytes returns the total disk usage in bytes for the given path
// by walking the directory tree and summing file sizes.
// Used on Windows where `du` is not available.
func diskUsageBytes(path string) (int64, error) {
	var total int64
	err := filepath.Walk(path, func(_ string, info os.FileInfo, err error) error {
		if err != nil {
			// Skip unreadable files/directories gracefully
			return nil
		}
		if !info.IsDir() {
			total += info.Size()
		}
		return nil
	})
	return total, err
}
