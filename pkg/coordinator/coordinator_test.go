package coordinator

import (
	"sync"
	"testing"
	"time"
)

func TestNew(t *testing.T) {
	c := New[string]()
	if c == nil {
		t.Fatal("New() returned nil")
	}
	if c.entries == nil {
		t.Fatal("entries map is nil")
	}
}

func TestCoordinator_Run_Basic(t *testing.T) {
	c := New[string]()
	ctx := c.Run("key1")

	if ctx == nil {
		t.Fatal("Run() returned nil context")
	}
	if ctx.Err() != nil {
		t.Errorf("expected nil context error, got %v", ctx.Err())
	}

	c.Complete("key1")
}

func TestCoordinator_Run_Coalesces(t *testing.T) {
	c := New[string]()

	// Start first run
	ctx1 := c.Run("key1")
	if ctx1 == nil {
		t.Fatal("first Run() returned nil")
	}

	// Second run should coalesce - returns a context that waits for done
	ctx2 := c.Run("key1")
	if ctx2 == nil {
		t.Fatal("second Run() returned nil")
	}

	// Complete the first run
	c.Complete("key1")

	// ctx2 (from coalesced run) should be cancelled after Complete
	select {
	case <-ctx2.Done():
		// expected
	case <-time.After(100 * time.Millisecond):
		t.Error("ctx2 not cancelled after Complete")
	}

	// ctx1 is the original context; it doesn't auto-cancel on Complete
	// That's expected - the caller manages ctx1's lifecycle
}

func TestCoordinator_Wake(t *testing.T) {
	c := New[string]()

	// Wake on non-existent key should be no-op
	c.Wake("nonexistent")

	// Wake on idle key should be no-op
	c.Run("key1")
	c.Complete("key1")
	c.Wake("key1")

	// Wake on running key
	ctx := c.Run("key1")
	c.Wake("key1")

	if !c.HasPending("key1") {
		t.Error("expected pending after Wake")
	}

	c.Complete("key1")
	_ = ctx
}

func TestCoordinator_Interrupt(t *testing.T) {
	c := New[string]()

	// Interrupt on non-existent key should be no-op
	c.Interrupt("nonexistent")

	// Interrupt on running key
	ctx := c.Run("key1")
	c.Interrupt("key1")

	select {
	case <-ctx.Done():
		// expected
	case <-time.After(100 * time.Millisecond):
		t.Error("context not cancelled after Interrupt")
	}

	c.Complete("key1")
}

func TestCoordinator_Interrupt_IdleKey(t *testing.T) {
	c := New[string]()
	c.Run("key1")
	c.Complete("key1")
	c.Interrupt("key1")
}

func TestCoordinator_AwaitIdle(t *testing.T) {
	c := New[string]()

	// AwaitIdle on non-existent key should return immediately
	c.AwaitIdle("nonexistent")

	// AwaitIdle on idle key
	c.Run("key1")
	c.Complete("key1")
	c.AwaitIdle("key1")
}

func TestCoordinator_AwaitIdle_Running(t *testing.T) {
	c := New[string]()
	c.Run("key1")

	done := make(chan struct{})
	go func() {
		c.AwaitIdle("key1")
		close(done)
	}()

	// Complete after a short delay
	time.Sleep(10 * time.Millisecond)
	c.Complete("key1")

	select {
	case <-done:
		// expected
	case <-time.After(100 * time.Millisecond):
		t.Error("AwaitIdle did not return after Complete")
	}
}

func TestCoordinator_Complete_NotRunning(t *testing.T) {
	c := New[string]()
	demand := c.Complete("nonexistent")
	if demand != 0 {
		t.Errorf("Complete on nonexistent key returned demand %d, want 0", demand)
	}
}

func TestCoordinator_Complete_ReturnsPendingDemand(t *testing.T) {
	c := New[string]()
	c.Run("key1")
	c.Wake("key1")
	demand := c.Complete("key1")
	if demand != DemandWake {
		t.Errorf("Complete returned demand %d, want %d", demand, DemandWake)
	}
}

func TestCoordinator_HasPending(t *testing.T) {
	c := New[string]()

	if c.HasPending("nonexistent") {
		t.Error("HasPending on nonexistent key should be false")
	}

	c.Run("key1")
	if c.HasPending("key1") {
		t.Error("HasPending should be false before Wake")
	}

	c.Wake("key1")
	if !c.HasPending("key1") {
		t.Error("HasPending should be true after Wake")
	}

	c.Complete("key1")
	if c.HasPending("key1") {
		t.Error("HasPending should be false after Complete")
	}
}

func TestCoordinator_Concurrent(t *testing.T) {
	c := New[int]()
	var wg sync.WaitGroup

	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func(key int) {
			defer wg.Done()
			ctx := c.Run(key)
			time.Sleep(5 * time.Millisecond)
			c.Wake(key)
			c.Complete(key)
			_ = ctx
		}(i)
	}

	wg.Wait()
}

func TestCoordinator_DemandType_Constants(t *testing.T) {
	if DemandRun != 0 {
		t.Errorf("DemandRun = %d, want 0", DemandRun)
	}
	if DemandWake != 1 {
		t.Errorf("DemandWake = %d, want 1", DemandWake)
	}
}

func TestCoordinator_ContextCancellation(t *testing.T) {
	c := New[string]()
	ctx := c.Run("key1")

	// Verify context is not cancelled yet
	if ctx.Err() != nil {
		t.Errorf("expected nil context error, got %v", ctx.Err())
	}

	// Complete should close the done channel
	c.Complete("key1")

	// For the first run, Complete doesn't auto-cancel the context
	// but it should mark the entry as not running
	if c.HasPending("key1") {
		t.Error("HasPending should be false after Complete")
	}
}

func TestCoordinator_getOrCreate(t *testing.T) {
	c := New[string]()

	e1 := c.getOrCreate("key1")
	if e1 == nil {
		t.Fatal("getOrCreate returned nil")
	}

	e2 := c.getOrCreate("key1")
	if e1 != e2 {
		t.Error("getOrCreate should return same entry")
	}

	e3 := c.getOrCreate("key2")
	if e1 == e3 {
		t.Error("getOrCreate should return different entry for different key")
	}
}
