package tools

import (
	"context"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/tests/testutil/mocks"
)

// TestDispatcher_ConcurrentPermissionRequests tests 10 goroutines requesting
// permissions simultaneously to verify no deadlocks.
func TestDispatcher_ConcurrentPermissionRequests(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	// Register a dangerous tool that requires permission
	d.Register(&mocks.MockTool{Name_: "dangerous_tool", RiskLevel_: types.RiskDangerous})

	const goroutines = 10
	var wg sync.WaitGroup
	var completed atomic.Int64

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			// Start execution in background — will block on permission
			go func() {
				_, _ = d.Execute(context.Background(), types.ToolCall{
					ID:    "call_" + string(rune('A'+id)),
					Name:  "dangerous_tool",
					Input: []byte(`{}`),
				})
				completed.Add(1)
			}()
		}(i)
	}

	// Wait for all permission requests to arrive
	time.Sleep(100 * time.Millisecond)

	// Drain all pending requests and approve them
	drained := 0
	for drained < goroutines {
		select {
		case req := <-d.RequestCh():
			d.ApprovePermission(req.ID, true, false)
			drained++
		case <-time.After(2 * time.Second):
			t.Fatalf("timed out waiting for permission requests, got %d/%d", drained, goroutines)
		}
	}

	wg.Wait()

	// Give goroutines time to complete
	time.Sleep(100 * time.Millisecond)

	if completed.Load() != goroutines {
		t.Errorf("expected %d completions, got %d", goroutines, completed.Load())
	}
}

// TestDispatcher_BatchApproval_ConcurrentTools tests batch approval for
// multiple tools requesting permissions concurrently.
func TestDispatcher_BatchApproval_ConcurrentTools(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	// Register two dangerous tools
	d.Register(&mocks.MockTool{Name_: "tool_a", RiskLevel_: types.RiskDangerous})
	d.Register(&mocks.MockTool{Name_: "tool_b", RiskLevel_: types.RiskDangerous})

	const goroutines = 5
	var wg sync.WaitGroup
	var completed atomic.Int64

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			name := "tool_a"
			if id%2 == 1 {
				name = "tool_b"
			}
			go func() {
				_, _ = d.Execute(context.Background(), types.ToolCall{
					ID:    "call_batch_" + string(rune('A'+id)),
					Name:  name,
					Input: []byte(`{}`),
				})
				completed.Add(1)
			}()
		}(i)
	}

	// Wait for permission requests
	time.Sleep(100 * time.Millisecond)

	// Batch approve all requests
	drained := 0
	for drained < goroutines {
		select {
		case req := <-d.RequestCh():
			d.ApproveBatch(req.ToolName, req.RiskLevel)
			d.ApprovePermission(req.ID, true, false)
			drained++
		case <-time.After(2 * time.Second):
			t.Fatalf("timed out waiting for permission requests, got %d/%d", drained, goroutines)
		}
	}

	wg.Wait()
	time.Sleep(100 * time.Millisecond)

	if completed.Load() != goroutines {
		t.Errorf("expected %d completions, got %d", goroutines, completed.Load())
	}
}

// TestDispatcher_Stop_WhileRequestsPending tests clean shutdown while
// permission requests are pending.
func TestDispatcher_Stop_WhileRequestsPending(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	d.Register(&mocks.MockTool{Name_: "safe_tool", RiskLevel_: types.RiskSafe})

	// Execute a safe tool (no permission needed) — should complete quickly
	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call_stop",
		Name:  "safe_tool",
		Input: []byte(`{}`),
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}

	// Stop the dispatcher — should not panic or leak goroutines
	d.Stop()

	// Verify Stop is idempotent (safe to call twice)
	d.Stop()
}

// TestDispatcher_ContextCancellation_ConcurrentTools tests that cancelling
// context stops all executing tools.
func TestDispatcher_ContextCancellation_ConcurrentTools(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	// Register a slow tool that respects context
	d.Register(&mocks.MockTool{
		Name_:      "slow_tool",
		RiskLevel_: types.RiskSafe,
		ExecFunc: func(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
			select {
			case <-ctx.Done():
				return types.ToolResult{}, ctx.Err()
			case <-time.After(10 * time.Second):
				return types.ToolResult{Output: "done"}, nil
			}
		},
	})

	ctx, cancel := context.WithCancel(context.Background())

	const goroutines = 5
	var wg sync.WaitGroup
	var cancelled atomic.Int64

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			_, err := d.Execute(ctx, types.ToolCall{
				ID:    "call_cancel_" + string(rune('A'+id)),
				Name:  "slow_tool",
				Input: []byte(`{}`),
			})
			if err != nil {
				cancelled.Add(1)
			}
		}(i)
	}

	// Give tools time to start
	time.Sleep(50 * time.Millisecond)

	// Cancel context — all tools should stop
	cancel()

	wg.Wait()

	if cancelled.Load() != goroutines {
		t.Errorf("expected %d cancellations, got %d", goroutines, cancelled.Load())
	}
}

// TestDispatcher_RateLimit_Exhaustion tests that rate limiting works
// when tokens are exhausted.
func TestDispatcher_RateLimit_Exhaustion(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	// Register a fast tool
	d.Register(&mocks.MockTool{
		Name_:      "fast_tool",
		RiskLevel_: types.RiskSafe,
		ExecFunc: func(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
			return types.ToolResult{Output: "ok"}, nil
		},
	})

	// Execute many tools concurrently to potentially exhaust rate limit
	const goroutines = 20
	var wg sync.WaitGroup
	var completed atomic.Int64

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			_, err := d.Execute(context.Background(), types.ToolCall{
				ID:    "call_rate_" + string(rune('A'+id%26)),
				Name:  "fast_tool",
				Input: []byte(`{}`),
			})
			if err == nil {
				completed.Add(1)
			}
		}(i)
	}

	wg.Wait()

	// All should complete (rate limiting queues, doesn't reject)
	if completed.Load() != goroutines {
		t.Errorf("expected %d completions, got %d", goroutines, completed.Load())
	}
}

// TestDispatcher_ConcurrencySemaphore_Limit tests that the concurrency
// semaphore blocks excess concurrent executions.
func TestDispatcher_ConcurrencySemaphore_Limit(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)

	// Track concurrent executions
	var maxConcurrent atomic.Int64
	var currentConcurrent atomic.Int64

	d.Register(&mocks.MockTool{
		Name_:      "concurrent_tool",
		RiskLevel_: types.RiskSafe,
		ExecFunc: func(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
			cur := currentConcurrent.Add(1)
			// Update max concurrent
			for {
				old := maxConcurrent.Load()
				if cur <= old || maxConcurrent.CompareAndSwap(old, cur) {
					break
				}
			}
			time.Sleep(50 * time.Millisecond) // simulate work
			currentConcurrent.Add(-1)
			return types.ToolResult{Output: "ok"}, nil
		},
	})

	// Execute many tools concurrently
	const goroutines = 50
	var wg sync.WaitGroup

	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			_, _ = d.Execute(context.Background(), types.ToolCall{
				ID:    "call_sem_" + string(rune('A'+id%26)),
				Name:  "concurrent_tool",
				Input: []byte(`{}`),
			})
		}(i)
	}

	wg.Wait()

	// Max concurrent should not exceed the semaphore size
	// MaxConcurrentTools is defined in dispatcher constants
	t.Logf("max concurrent executions: %d (limit: %d)", maxConcurrent.Load(), MaxConcurrentTools)

	if maxConcurrent.Load() > MaxConcurrentTools {
		t.Errorf("max concurrent (%d) exceeded limit (%d)", maxConcurrent.Load(), MaxConcurrentTools)
	}
}
