package fileutil

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
)

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

	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("generate temp name: %w", err)
	}
	tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))

	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, perm)
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}
	defer os.Remove(tmpPath)

	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return fmt.Errorf("write temp file: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
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
