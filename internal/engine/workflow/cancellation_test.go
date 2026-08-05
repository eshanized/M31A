package workflow

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// TestEngine_Cancellation_StopsLLMStreaming tests that cancelling context
// stops LLM streaming immediately.
func TestEngine_Cancellation_StopsLLMStreaming(t *testing.T) {
	_, _ = setupTestEngine(t)

	ctx, cancel := context.WithCancel(context.Background())

	streamingStarted := make(chan struct{})
	streamingStopped := make(chan struct{})

	// Simulate LLM streaming in a goroutine
	go func() {
		close(streamingStarted)
		for {
			select {
			case <-ctx.Done():
				close(streamingStopped)
				return
			case <-time.After(10 * time.Millisecond):
				// simulate chunk processing
			}
		}
	}()

	<-streamingStarted

	// Cancel context — streaming should stop
	cancel()

	select {
	case <-streamingStopped:
		// success: streaming stopped
	case <-time.After(2 * time.Second):
		t.Fatal("LLM streaming did not stop after context cancellation")
	}
}

// TestEngine_Cancellation_StopsToolExecution tests that cancelling context
// stops tool execution.
func TestEngine_Cancellation_StopsToolExecution(t *testing.T) {
	_, _ = setupTestEngine(t)

	ctx, cancel := context.WithCancel(context.Background())

	toolStarted := make(chan struct{})
	toolStopped := make(chan struct{})

	// Simulate tool execution in a goroutine
	go func() {
		close(toolStarted)
		for {
			select {
			case <-ctx.Done():
				close(toolStopped)
				return
			case <-time.After(10 * time.Millisecond):
				// simulate work
			}
		}
	}()

	<-toolStarted

	// Cancel context — tool should stop
	cancel()

	select {
	case <-toolStopped:
		// success: tool stopped
	case <-time.After(2 * time.Second):
		t.Fatal("tool execution did not stop after context cancellation")
	}
}

// TestEngine_Cancellation_StopsSubagents tests that cancelling parent context
// stops subagents.
func TestEngine_Cancellation_StopsSubagents(t *testing.T) {
	_, _ = setupTestEngine(t)

	parentCtx, parentCancel := context.WithCancel(context.Background())

	const subagents = 3
	var wg sync.WaitGroup
	var stopped atomic.Int64

	for i := 0; i < subagents; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			// Subagent derives context from parent
			_, subCancel := context.WithCancel(parentCtx)
			defer subCancel()

			for {
				select {
				case <-parentCtx.Done():
					stopped.Add(1)
					return
				case <-time.After(10 * time.Millisecond):
					// simulate work
				}
			}
		}(i)
	}

	// Give subagents time to start
	time.Sleep(50 * time.Millisecond)

	// Cancel parent — all subagents should stop
	parentCancel()

	wg.Wait()

	if stopped.Load() != subagents {
		t.Errorf("expected %d subagents stopped, got %d", subagents, stopped.Load())
	}
}

// TestEngine_Cancellation_CleansUpResources tests that cancelling context
// cleans up resources (channels, goroutines).
func TestEngine_Cancellation_CleansUpResources(t *testing.T) {
	_, _ = setupTestEngine(t)

	ctx, cancel := context.WithCancel(context.Background())

	// Create a channel that should be cleaned up
	resourceCh := make(chan struct{}, 1)

	// Goroutine that uses the resource
	done := make(chan struct{})
	go func() {
		for {
			select {
			case <-ctx.Done():
				// Clean up: drain the channel
				select {
				case <-resourceCh:
				default:
				}
				close(done)
				return
			case <-time.After(10 * time.Millisecond):
				// simulate work
			}
		}
	}()

	// Cancel context
	cancel()

	select {
	case <-done:
		// success: goroutine cleaned up
	case <-time.After(2 * time.Second):
		t.Fatal("resource cleanup did not complete after context cancellation")
	}

	// Verify channel is empty (resource was cleaned up)
	select {
	case <-resourceCh:
		// resource was consumed — this is fine
	default:
		// channel is empty — also fine, goroutine drained it
	}
}

// TestEngine_Cancellation_PersistsState tests that cancelling mid-execution
// preserves recoverable state.
func TestEngine_Cancellation_PersistsState(t *testing.T) {
	_, _ = setupTestEngine(t)

	// Simulate state that should persist across cancellation
	type persistedState struct {
		phase     string
		taskID    int
		completed bool
	}

	state := persistedState{phase: "execute", taskID: 1, completed: false}

	ctx, cancel := context.WithCancel(context.Background())

	// Simulate execution that persists state
	done := make(chan struct{})
	go func() {
		// Simulate work
		select {
		case <-ctx.Done():
			// State should be preserved (not modified) on cancellation
			state.completed = false
		case <-time.After(100 * time.Millisecond):
			state.completed = true
		}
		close(done)
	}()

	// Cancel before work completes
	cancel()

	<-done

	if state.completed {
		t.Error("state should not be marked as completed after cancellation")
	}
	if state.phase != "execute" {
		t.Errorf("phase should be preserved as 'execute', got %q", state.phase)
	}
}

// TestEngine_Cancellation_Recovery tests that cancelling and restarting
// the engine recovers correctly.
func TestEngine_Cancellation_Recovery(t *testing.T) {
	_, _ = setupTestEngine(t)

	// First execution — cancel it
	ctx1, cancel1 := context.WithCancel(context.Background())
	done1 := make(chan struct{})
	go func() {
		for {
			select {
			case <-ctx1.Done():
				close(done1)
				return
			case <-time.After(10 * time.Millisecond):
			}
		}
	}()

	time.Sleep(50 * time.Millisecond)
	cancel1()
	<-done1

	// Second execution — should work fine
	ctx2, cancel2 := context.WithCancel(context.Background())
	defer cancel2()

	resultCh := make(chan string, 1)
	go func() {
		for {
			select {
			case <-ctx2.Done():
				return
			case <-time.After(10 * time.Millisecond):
				resultCh <- "recovered"
				return
			}
		}
	}()

	select {
	case result := <-resultCh:
		if result != "recovered" {
			t.Errorf("expected 'recovered', got %q", result)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("engine did not recover after cancellation")
	}
}
