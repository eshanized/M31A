package fileutil

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestNewFileLock(t *testing.T) {
	fl := NewFileLock("/tmp/test.lock")
	if fl == nil {
		t.Fatal("NewFileLock() returned nil")
	}
	if fl.path != "/tmp/test.lock" {
		t.Errorf("path = %q, want %q", fl.path, "/tmp/test.lock")
	}
}

func TestFileLock_LockUnlock(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "test.lock")

	fl := NewFileLock(lockPath)
	if err := fl.Lock(); err != nil {
		t.Fatalf("Lock() error = %v", err)
	}

	// Verify lock file was created
	if _, err := os.Stat(lockPath); os.IsNotExist(err) {
		t.Error("lock file was not created")
	}

	if err := fl.Unlock(); err != nil {
		t.Fatalf("Unlock() error = %v", err)
	}
}

func TestFileLock_TryLock(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "test.lock")

	fl := NewFileLock(lockPath)
	ok, err := fl.TryLock()
	if err != nil {
		t.Fatalf("TryLock() error = %v", err)
	}
	if !ok {
		t.Error("TryLock() returned false, want true")
	}

	if err := fl.Unlock(); err != nil {
		t.Fatalf("Unlock() error = %v", err)
	}
}

func TestFileLock_TryLock_Contention(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "test.lock")

	fl1 := NewFileLock(lockPath)
	if err := fl1.Lock(); err != nil {
		t.Fatalf("Lock() error = %v", err)
	}

	// Second lock attempt should not succeed immediately
	fl2 := NewFileLock(lockPath)
	ok, err := fl2.TryLock()
	if err != nil {
		t.Fatalf("TryLock() error = %v", err)
	}
	if ok {
		t.Error("TryLock() returned true when lock is held")
	}

	if err := fl1.Unlock(); err != nil {
		t.Fatalf("Unlock() error = %v", err)
	}
}

func TestFileLock_Unlock_NoLock(t *testing.T) {
	fl := NewFileLock("/tmp/nonexistent.lock")

	// Unlock without lock should not error
	if err := fl.Unlock(); err != nil {
		t.Errorf("Unlock() error = %v", err)
	}
}

func TestFileLock_Lock_CreatesDirectory(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "subdir", "test.lock")

	fl := NewFileLock(lockPath)
	if err := fl.Lock(); err != nil {
		t.Fatalf("Lock() error = %v", err)
	}

	// Verify directory was created
	if _, err := os.Stat(filepath.Dir(lockPath)); os.IsNotExist(err) {
		t.Error("lock directory was not created")
	}

	if err := fl.Unlock(); err != nil {
		t.Fatalf("Unlock() error = %v", err)
	}
}

func TestFileLock_Concurrent(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "test.lock")

	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			fl := NewFileLock(lockPath)
			if err := fl.Lock(); err != nil {
				return
			}
			// Hold lock briefly
			_ = fl.Unlock()
		}()
	}
	wg.Wait()
}

func TestFileLock_Lock_LockFileExists(t *testing.T) {
	dir := t.TempDir()
	lockPath := filepath.Join(dir, "test.lock")

	// Pre-create the lock file
	if err := os.WriteFile(lockPath, []byte(""), 0600); err != nil {
		t.Fatal(err)
	}

	fl := NewFileLock(lockPath)
	if err := fl.Lock(); err != nil {
		t.Fatalf("Lock() error = %v", err)
	}

	if err := fl.Unlock(); err != nil {
		t.Fatalf("Unlock() error = %v", err)
	}
}
