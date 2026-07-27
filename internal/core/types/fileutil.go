//go:build !windows

package types

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"
	"syscall"
)

// FileLock provides kernel-managed file locking using fcntl(F_SETLK).
// Used to prevent concurrent M31A instances from corrupting
// shared session files in the same project directory.
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
	f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
	if err != nil {
		return fmt.Errorf("open lock file %s: %w", fl.path, err)
	}
	lock := syscall.Flock_t{
		Type:   syscall.F_WRLCK,
		Whence: int16(io.SeekStart),
		Start:  0,
		Len:    0,
	}
	if err := syscall.FcntlFlock(f.Fd(), syscall.F_SETLK, &lock); err != nil {
		_ = f.Close()
		return fmt.Errorf("fcntl lock %s: %w", fl.path, err)
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
	lock := syscall.Flock_t{
		Type:   syscall.F_WRLCK,
		Whence: int16(io.SeekStart),
		Start:  0,
		Len:    0,
	}
	if err := syscall.FcntlFlock(f.Fd(), syscall.F_SETLK, &lock); err != nil {
		_ = f.Close()
		if err == syscall.EAGAIN || err == syscall.EACCES {
			return false, nil
		}
		return false, fmt.Errorf("fcntl trylock %s: %w", fl.path, err)
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
	lock := syscall.Flock_t{
		Type:   syscall.F_UNLCK,
		Whence: int16(io.SeekStart),
		Start:  0,
		Len:    0,
	}
	err := syscall.FcntlFlock(f.Fd(), syscall.F_SETLK, &lock)
	closeErr := f.Close()
	if err != nil {
		return fmt.Errorf("unlock %s: %w", fl.path, err)
	}
	return closeErr
}

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
