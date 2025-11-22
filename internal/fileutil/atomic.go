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
// the existing file intact. Temp files use 0600 permissions.
func AtomicWrite(path string, data []byte) error {
	dir := filepath.Dir(path)

	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("generate temp name: %w", err)
	}
	tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))

	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0600)
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
