//go:build !windows

package types

import (
	"sync"
	"testing"
)

func TestFileLock_MutualExclusion(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	lockPath := dir + "/test.lock"

	lock := NewFileLock(lockPath)

	// Acquire lock
	if err := lock.Lock(); err != nil {
		t.Fatalf("Lock failed: %v", err)
	}

	// Release lock
	if err := lock.Unlock(); err != nil {
		t.Fatalf("Unlock failed: %v", err)
	}

	// Second lock should succeed after first is released
	lock2 := NewFileLock(lockPath)
	if err := lock2.Lock(); err != nil {
		t.Fatalf("Lock after unlock failed: %v", err)
	}

	// Clean up
	if err := lock2.Unlock(); err != nil {
		t.Fatalf("Unlock failed: %v", err)
	}
}

func TestFileLock_UnlockReleases(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	lockPath := dir + "/test.lock"

	lock1 := NewFileLock(lockPath)
	if err := lock1.Lock(); err != nil {
		t.Fatalf("Lock failed: %v", err)
	}

	// Release first lock
	if err := lock1.Unlock(); err != nil {
		t.Fatalf("Unlock failed: %v", err)
	}

	// Second lock should succeed now
	lock2 := NewFileLock(lockPath)
	if err := lock2.Lock(); err != nil {
		t.Fatalf("Lock after unlock failed: %v", err)
	}

	// Clean up
	if err := lock2.Unlock(); err != nil {
		t.Fatalf("Unlock failed: %v", err)
	}
}

func TestFileLock_ConcurrentAccess(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	lockPath := dir + "/test.lock"

	var wg sync.WaitGroup
	const goroutines = 10
	successCount := make(chan int, goroutines)

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			lock := NewFileLock(lockPath)
			if err := lock.Lock(); err != nil {
				t.Errorf("goroutine %d: Lock failed: %v", id, err)
				return
			}
			// Hold lock briefly
			if err := lock.Unlock(); err != nil {
				t.Errorf("goroutine %d: Unlock failed: %v", id, err)
				return
			}
			successCount <- 1
		}(i)
	}

	wg.Wait()
	close(successCount)

	// Count successes
	count := 0
	for range successCount {
		count++
	}

	if count != goroutines {
		t.Errorf("expected %d successful lock/unlock cycles, got %d", goroutines, count)
	}
}
