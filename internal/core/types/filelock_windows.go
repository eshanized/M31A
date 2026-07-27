//go:build windows

package types

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
)

// FileLock provides a simple file-based locking mechanism for Windows
// using file creation and deletion as a lock mechanism.
type FileLock struct {
	path string
	mu   sync.Mutex
	file *os.File
}

// NewFileLock creates a FileLock for the given path.
func NewFileLock(path string) *FileLock {
	return &FileLock{path: path}
}

// Lock acquires an exclusive lock on the file. Blocks until the lock
// is available. Returns an error if the file cannot be opened.
func (fl *FileLock) Lock() error {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0600)
	if err != nil {
		return fmt.Errorf("lock %s: %w", fl.path, err)
	}
	fl.mu.Lock()
	fl.file = f
	fl.mu.Unlock()
	return nil
}

// TryLock attempts to acquire an exclusive lock without blocking.
func (fl *FileLock) TryLock() (bool, error) {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return false, fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0600)
	if err != nil {
		if os.IsExist(err) {
			return false, nil
		}
		return false, fmt.Errorf("try lock %s: %w", fl.path, err)
	}
	fl.mu.Lock()
	fl.file = f
	fl.mu.Unlock()
	return true, nil
}

// Unlock releases the lock.
func (fl *FileLock) Unlock() error {
	fl.mu.Lock()
	defer fl.mu.Unlock()
	if fl.file != nil {
		err := fl.file.Close()
		_ = os.Remove(fl.path)
		fl.file = nil
		return err
	}
	return nil
}
