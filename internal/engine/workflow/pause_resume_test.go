package workflow

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// TestEngine_PauseResume_BasicFlow tests basic pause/resume lifecycle.
func TestEngine_PauseResume_BasicFlow(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initially not paused
	if engine.IsPaused() {
		t.Fatal("engine should not be paused initially")
	}

	// Pause
	if !engine.PauseExecution() {
		t.Fatal("PauseExecution should return true on first call")
	}
	if !engine.IsPaused() {
		t.Fatal("engine should be paused after PauseExecution")
	}

	// Double pause returns false
	if engine.PauseExecution() {
		t.Fatal("PauseExecution should return false when already paused")
	}

	// Resume
	if !engine.ResumeExecution() {
		t.Fatal("ResumeExecution should return true when paused")
	}
	if engine.IsPaused() {
		t.Fatal("engine should not be paused after ResumeExecution")
	}

	// Double resume returns false
	if engine.ResumeExecution() {
		t.Fatal("ResumeExecution should return false when not paused")
	}
}

// TestEngine_PauseResume_WhileExecuting tests that waitForPause blocks
// while paused and unblocks on resume.
func TestEngine_PauseResume_WhileExecuting(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Pause first, then start goroutine — ensures waitForPause blocks immediately
	engine.PauseExecution()

	resumed := make(chan bool, 1)
	go func() {
		resumed <- engine.waitForPause(context.Background())
	}()

	// Verify goroutine hasn't returned yet (should be blocked on paused channel)
	select {
	case <-resumed:
		t.Fatal("waitForPause should block while paused")
	case <-time.After(100 * time.Millisecond):
		// expected: still blocked
	}

	// Resume — goroutine should unblock
	engine.ResumeExecution()

	select {
	case ok := <-resumed:
		if !ok {
			t.Fatal("waitForPause should return true on resume")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("waitForPause did not unblock after resume")
	}
}

// TestEngine_PauseResume_SkipTask tests sending a skip command while paused.
func TestEngine_PauseResume_SkipTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.PauseExecution()

	// Send skip command
	engine.SkipCurrentTask(42)

	// Consume the command
	skipID, cancelID, cancelledGroup, ok := engine.consumeSkipOrCancel(context.Background())
	if !ok {
		t.Fatal("consumeSkipOrCancel should return ok")
	}
	if skipID != 42 {
		t.Errorf("expected skipID=42, got %d", skipID)
	}
	if cancelID != 0 {
		t.Errorf("expected cancelID=0, got %d", cancelID)
	}
	if cancelledGroup {
		t.Error("expected cancelledGroup=false")
	}

	engine.ResumeExecution()
}

// TestEngine_PauseResume_CancelTask tests sending a cancel command while paused.
func TestEngine_PauseResume_CancelTask(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.PauseExecution()

	// Send cancel command
	engine.CancelCurrentTask(7)

	// Consume the command
	skipID, cancelID, cancelledGroup, ok := engine.consumeSkipOrCancel(context.Background())
	if !ok {
		t.Fatal("consumeSkipOrCancel should return ok")
	}
	if skipID != 0 {
		t.Errorf("expected skipID=0, got %d", skipID)
	}
	if cancelID != 7 {
		t.Errorf("expected cancelID=7, got %d", cancelID)
	}
	if cancelledGroup {
		t.Error("expected cancelledGroup=false")
	}

	engine.ResumeExecution()
}

// TestEngine_PauseResume_CancelGroup tests cancelling an entire group while paused.
func TestEngine_PauseResume_CancelGroup(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.PauseExecution()

	// Start goroutine that will consume the cancel group signal
	resultCh := make(chan struct {
		skipID        int
		cancelID      int
		cancelledGroup bool
		ok            bool
	}, 1)
	go func() {
		skipID, cancelID, cancelledGroup, ok := engine.consumeSkipOrCancel(context.Background())
		resultCh <- struct {
			skipID        int
			cancelID      int
			cancelledGroup bool
			ok            bool
		}{skipID, cancelID, cancelledGroup, ok}
	}()

	// Give goroutine time to enter select
	time.Sleep(50 * time.Millisecond)

	// Cancel group — goroutine should unblock
	engine.CancelGroup()

	select {
	case res := <-resultCh:
		if !res.ok {
			t.Fatal("consumeSkipOrCancel should return ok")
		}
		if res.skipID != 0 {
			t.Errorf("expected skipID=0, got %d", res.skipID)
		}
		if res.cancelID != 0 {
			t.Errorf("expected cancelID=0, got %d", res.cancelID)
		}
		if !res.cancelledGroup {
			t.Error("expected cancelledGroup=true")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("consumeSkipOrCancel did not unblock after CancelGroup")
	}
}

// TestEngine_PauseResume_ContextCancellation tests that waitForPause returns
// false when context is cancelled.
func TestEngine_PauseResume_ContextCancellation(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.PauseExecution()

	ctx, cancel := context.WithCancel(context.Background())

	resumed := make(chan bool, 1)
	go func() {
		resumed <- engine.waitForPause(ctx)
	}()

	// Give goroutine time to enter select
	time.Sleep(50 * time.Millisecond)

	// Cancel context — should unblock waitForPause with false
	cancel()

	select {
	case ok := <-resumed:
		if ok {
			t.Fatal("waitForPause should return false on context cancellation")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("waitForPause did not unblock after context cancellation")
	}

	// Cleanup
	engine.ResumeExecution()
}

// TestEngine_PauseResume_ConcurrentAccess tests concurrent access to pause/resume
// from multiple goroutines without races.
func TestEngine_PauseResume_ConcurrentAccess(t *testing.T) {
	engine, _ := setupTestEngine(t)

	const goroutines = 10
	const iterations = 100
	var wg sync.WaitGroup
	var pauseCount, resumeCount atomic.Int64

	// Concurrent pause/resume
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				if engine.PauseExecution() {
					pauseCount.Add(1)
				}
				if engine.ResumeExecution() {
					resumeCount.Add(1)
				}
			}
		}(i)
	}

	wg.Wait()

	// Every successful pause should have a matching resume
	if pauseCount.Load() != resumeCount.Load() {
		t.Errorf("pause count (%d) != resume count (%d)", pauseCount.Load(), resumeCount.Load())
	}

	// Engine should not be paused at the end
	if engine.IsPaused() {
		t.Error("engine should not be paused after all goroutines complete")
	}
}

// TestEngine_PauseResume_ConcurrentSkipCancel tests concurrent skip/cancel
// commands while paused.
func TestEngine_PauseResume_ConcurrentSkipCancel(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.PauseExecution()

	const goroutines = 10
	var wg sync.WaitGroup

	// Concurrent skip/cancel commands (but NOT CancelGroup to avoid nil channel issue)
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			engine.SkipCurrentTask(id)
			engine.CancelCurrentTask(id)
		}(i)
	}

	wg.Wait()

	// Drain any pending commands via consumeSkipOrCancel with timeout
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()

	drained := 0
	for {
		_, _, _, ok := engine.consumeSkipOrCancel(ctx)
		if !ok {
			break
		}
		drained++
		if drained > goroutines*2 {
			break // safety limit
		}
	}

	t.Logf("drained %d commands from concurrent senders", drained)

	engine.ResumeExecution()
}
