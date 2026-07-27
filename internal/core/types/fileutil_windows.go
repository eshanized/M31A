//go:build windows

package types

import (
	"fmt"
	"os"
	"path/filepath"
)

// AtomicWrite writes data to path atomically using a temp file in the same
// directory followed by a rename. If the target file already exists, its
// permissions are preserved; otherwise 0644 is used.
func AtomicWrite(path string, data []byte) error {
	return AtomicWriteWithPerm(path, data, 0644)
}

// AtomicWriteWithPerm writes data to path atomically with the given fallback
// permission. If the target already exists, its current permissions are preserved.
func AtomicWriteWithPerm(path string, data []byte, perm os.FileMode) error {
	dir := filepath.Dir(path)

	// Preserve original file permissions when overwriting
	if info, err := os.Stat(path); err == nil {
		perm = info.Mode().Perm()
	}

	tmpFile, err := os.CreateTemp(dir, ".m31a_tmp_*")
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	tmpPath := tmpFile.Name()
	if chmodErr := os.Chmod(tmpPath, perm); chmodErr != nil {
		_ = tmpFile.Close()
		return fmt.Errorf("chmod temp file: %w", chmodErr)
	}
	defer os.Remove(tmpPath) //nolint:errcheck

	if _, err := tmpFile.Write(data); err != nil {
		_ = tmpFile.Close()
		return fmt.Errorf("write temp file: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		_ = tmpFile.Close()
		return fmt.Errorf("sync temp file: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("close temp file: %w", err)
	}

	if err := os.Rename(tmpPath, path); err != nil {
		return fmt.Errorf("rename temp file: %w", err)
	}

	return nil
}
