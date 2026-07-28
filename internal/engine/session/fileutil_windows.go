//go:build windows

package session

import (
	"fmt"
	"os"
	"path/filepath"
)

// fileLock provides a simple file-based locking mechanism for Windows
// using file creation and deletion as a lock mechanism.
type fileLock struct {
	path string
	file *os.File
}

// newFileLock creates a file lock for the given path.
func newFileLock(path string) *fileLock {
	return &fileLock{path: path}
}

// Lock acquires an exclusive lock on the file.
func (fl *fileLock) Lock() error {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0600)
	if err != nil {
		return fmt.Errorf("lock %s: %w", fl.path, err)
	}
	fl.file = f
	return nil
}

// Unlock releases the lock.
func (fl *fileLock) Unlock() error {
	if fl.file != nil {
		err := fl.file.Close()
		_ = os.Remove(fl.path)
		fl.file = nil
		return err
	}
	return nil
}

// atomicWrite writes data to path atomically using a temp file then rename.
func atomicWrite(path string, data []byte) error {
	dir := filepath.Dir(path)

	// Preserve original file permissions when overwriting
	perm := os.FileMode(0644)
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
