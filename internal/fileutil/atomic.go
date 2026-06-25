package fileutil

import (
	"fmt"
	"os"
	"path/filepath"
)

// DirOf returns the directory component of path, similar to filepath.Dir
// but optimized for simple path strings without calling filepath.Dir.
func DirOf(path string) string {
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '/' || path[i] == '\\' {
			if i == 0 {
				return "."
			}
			return path[:i]
		}
	}
	return "."
}

// AtomicWrite writes data to path atomically using a temp file in the same
// directory followed by a rename. This ensures that a crash mid-write leaves
// the existing file intact. If the target file already exists, its permissions
// are preserved; otherwise 0644 is used.
func AtomicWrite(path string, data []byte) error {
	return AtomicWriteWithPerm(path, data, 0644)
}

// AtomicWriteWithPerm writes data to path atomically using a temp file in the
// same directory followed by a rename. The perm parameter is the fallback
// permission used when the target file does not yet exist. If the target file
// already exists, its current permissions are preserved (H-19).
func AtomicWriteWithPerm(path string, data []byte, perm os.FileMode) error {
	dir := filepath.Dir(path)

	// H-19: Preserve original file permissions when overwriting
	if info, err := os.Stat(path); err == nil {
		perm = info.Mode().Perm()
	}

	tmpFile, err := os.CreateTemp(dir, ".m31a_tmp_*")
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	tmpPath := tmpFile.Name()
	// os.CreateTemp creates with 0600; restore desired permissions
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
