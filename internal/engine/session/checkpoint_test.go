package session

import (
	"sync"
	"testing"
	"time"
)

func TestSaveCheckpoint_Concurrent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	var wg sync.WaitGroup
	const goroutines = 10

	// Concurrent checkpoint saves
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			cp := Checkpoint{
				Phase:     "plan",
				Goal:      "test goal",
				Timestamp: time.Now().Add(time.Duration(id) * time.Millisecond),
			}
			if err := mgr.SaveCheckpoint(s.ID, cp); err != nil {
				t.Errorf("goroutine %d: SaveCheckpoint failed: %v", id, err)
			}
		}(i)
	}

	wg.Wait()

	// Verify checkpoints were saved
	checkpoints, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}

	// Should have at most 2 checkpoints (trimmed)
	if len(checkpoints) > 2 {
		t.Errorf("expected at most 2 checkpoints, got %d", len(checkpoints))
	}
}

func TestCheckpoint_ConcurrentReadWrite(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})

	// Create a session
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Add initial checkpoint
	cp := Checkpoint{
		Phase:     "initialize",
		Goal:      "initial",
		Timestamp: time.Now(),
	}
	if err := mgr.SaveCheckpoint(s.ID, cp); err != nil {
		t.Fatalf("SaveCheckpoint failed: %v", err)
	}

	var wg sync.WaitGroup
	const writers = 5
	const readers = 10

	// Concurrent writers
	for i := 0; i < writers; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			cp := Checkpoint{
				Phase:     "plan",
				Goal:      "writer goal",
				Timestamp: time.Now().Add(time.Duration(id) * time.Millisecond),
			}
			if err := mgr.SaveCheckpoint(s.ID, cp); err != nil {
				t.Errorf("writer %d: SaveCheckpoint failed: %v", id, err)
			}
		}(i)
	}

	// Concurrent readers
	for i := 0; i < readers; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			_, err := mgr.LoadCheckpoints(s.ID)
			if err != nil {
				t.Errorf("reader %d: LoadCheckpoints failed: %v", id, err)
			}
		}(i)
	}

	wg.Wait()
}
