//go:build !windows

package fileutil

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"syscall"
)

// FileLock provides advisory file locking using flock(2).
// Used to prevent concurrent M31A instances from corrupting
// shared session files in the same project directory.
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

// Lock acquires an exclusive lock on the file. Blocks until the lock
// is available. Returns an error if the file cannot be opened.
func (fl *FileLock) Lock() error {
	if err := os.MkdirAll(filepath.Dir(fl.path), 0700); err != nil {
		return fmt.Errorf("create lock directory: %w", err)
	}
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return fmt.Errorf("open lock file %s: %w", fl.path, err)
	}
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil {
		_ = f.Close()
		return fmt.Errorf("flock %s: %w", fl.path, err)
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
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return false, fmt.Errorf("open lock file %s: %w", fl.path, err)
	}
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX|syscall.LOCK_NB); err != nil {
		_ = f.Close()
		if err == syscall.EWOULDBLOCK {
			return false, nil
		}
		return false, fmt.Errorf("flock %s: %w", fl.path, err)
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
	err := syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
	closeErr := f.Close()
	if err != nil {
		return fmt.Errorf("unlock %s: %w", fl.path, err)
	}
	return closeErr
}
