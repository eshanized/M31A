//go:build !windows

package fileutil_test

import (
	"path/filepath"
	"testing"

	"m31a/internal/infrastructure/fileutil"
)

func TestFileLock_Initialization(t *testing.T) {
	tmpDir := t.TempDir()
	lockPath := filepath.Join(tmpDir, "app.lock")

	lock := fileutil.NewFileLock(lockPath)
	if lock == nil {
		t.Fatal("expected non-nil FileLock")
	}
	if lock.Path() != lockPath {
		t.Errorf("expected path %s, got %s", lockPath, lock.Path())
	}
}

func TestFileLock_LockUnlockLifecycle(t *testing.T) {
	tmpDir := t.TempDir()
	lockPath := filepath.Join(tmpDir, "app.lock")

	lock := fileutil.NewFileLock(lockPath)

	// Lock
	if err := lock.Lock(); err != nil {
		t.Fatalf("failed to acquire lock: %v", err)
	}

	// Double lock should fail
	if err := lock.Lock(); err == nil {
		t.Error("expected error on double lock, got nil")
	}

	// Second instance lock should fail
	lock2 := fileutil.NewFileLock(lockPath)
	if err := lock2.Lock(); err == nil {
		t.Error("expected error when locking already locked path, got nil")
	}

	// Unlock
	if err := lock.Unlock(); err != nil {
		t.Fatalf("failed to unlock: %v", err)
	}

	// Second unlock should fail
	if err := lock.Unlock(); err == nil {
		t.Error("expected error on double unlock, got nil")
	}

	// Re-acquire should succeed after unlock
	if err := lock2.Lock(); err != nil {
		t.Fatalf("failed to acquire lock after previous instance unlocked: %v", err)
	}
	_ = lock2.Unlock()
}

func TestFileLock_InvalidDirectory(t *testing.T) {
	lockPath := filepath.Join(t.TempDir(), "nonexistent_subdir", "app.lock")
	lock := fileutil.NewFileLock(lockPath)

	if err := lock.Lock(); err == nil {
		t.Error("expected error when locking in non-existent directory, got nil")
	}
}
