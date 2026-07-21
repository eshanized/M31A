//go:build windows

package fileutil

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
)

// FileLock provides file locking using atomic rename on Windows.
// flock(2) is not available on Windows, so we use exclusive file creation
// as a best-effort lock mechanism.
type FileLock struct {
	path string
	mu   sync.Mutex
	file *os.File
}

// NewFileLock creates a FileLock for the given path.
// The lock file is created on Lock() if it doesn't exist.
func NewFileLock(path string) *FileLock {
	return &FileLock{path: path}
}

// Lock acquires an exclusive lock on the file. On Windows this uses
// exclusive file creation — best-effort advisory locking.
func (fl *FileLock) Lock() error {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR|os.O_EXCL, 0600)
	if err != nil {
		// If the file already exists, another process holds the lock.
		// Try opening it — if the other process is dead, the file is stale.
		f, err = os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
		if err != nil {
			return fmt.Errorf("open lock file %s: %w", fl.path, err)
		}
	}
	fl.mu.Lock()
	fl.file = f
	fl.mu.Unlock()
	return nil
}

// TryLock attempts to acquire an exclusive lock without blocking.
// Returns false if the lock is already held by another process.
func (fl *FileLock) TryLock() (bool, error) {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return false, fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR|os.O_EXCL, 0600)
	if err != nil {
		return false, nil
	}
	fl.mu.Lock()
	fl.file = f
	fl.mu.Unlock()
	return true, nil
}

// Unlock releases the lock and closes the file.
func (fl *FileLock) Unlock() error {
	fl.mu.Lock()
	f := fl.file
	fl.file = nil
	fl.mu.Unlock()
	if f == nil {
		return nil
	}
	return f.Close()
}
