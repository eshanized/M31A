//go:build windows

package fileutil

import (
	"errors"
	"fmt"
	"os"
	"sync"
)

// ErrLockHeld is returned when a lock is already acquired by this instance or another process.
var ErrLockHeld = errors.New("lock already held")

// ErrLockNotHeld is returned when Unlock is called on an unacquired lock.
var ErrLockNotHeld = errors.New("lock not held")

// FileLock manages advisory file locking for Windows platform.
type FileLock struct {
	path string
	file *os.File
	mu   sync.Mutex
}

// NewFileLock creates a FileLock for the given path.
// The lock file is created on Lock() if it doesn't exist.
func NewFileLock(path string) *FileLock {
	return &FileLock{path: path}
}

// Lock acquires an exclusive lock on the file. On Windows this uses
// exclusive file creation — best-effort advisory locking.
func (fl *FileLock) Lock() error {
	fl.mu.Lock()
	defer fl.mu.Unlock()

	if fl.file != nil {
		return ErrLockHeld
	}

	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_EXCL|os.O_RDWR, 0600)
	if err != nil {
		if os.IsExist(err) {
			return fmt.Errorf("%w: %v", ErrLockHeld, err)
		}
		return err
	}

	fl.file = f
	return nil
}

// Unlock releases the lock by closing and removing the lock file.
func (fl *FileLock) Unlock() error {
	fl.mu.Lock()
	defer fl.mu.Unlock()

	if fl.file == nil {
		return ErrLockNotHeld
	}

	closeErr := fl.file.Close()
	fl.file = nil

	removeErr := os.Remove(fl.path)
	if removeErr != nil && !os.IsNotExist(removeErr) {
		if closeErr == nil {
			return removeErr
		}
	}

	return closeErr
}

// Path returns the path of the lock file.
func (fl *FileLock) Path() string {
	return fl.path
}
