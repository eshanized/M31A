// Package testtimeout provides helper functions for per-test timeouts.
package testtimeout

import (
	"context"
	"testing"
	"time"
)

func TestWithTimeout(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name          string
		duration      time.Duration
		workDuration  time.Duration
		expectTimeout bool
	}{
		{
			name:          "timeout fires before work completes",
			duration:      50 * time.Millisecond,
			workDuration:  100 * time.Millisecond,
			expectTimeout: true,
		},
		{
			name:          "work completes before timeout",
			duration:      200 * time.Millisecond,
			workDuration:  50 * time.Millisecond,
			expectTimeout: false,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()

			ctx, cancel := WithTimeout(t, tc.duration)
			defer cancel()

			done := make(chan struct{})
			go func() {
				time.Sleep(tc.workDuration)
				close(done)
			}()

			select {
			case <-done:
				if tc.expectTimeout {
					t.Error("expected timeout but work completed")
				}
			case <-ctx.Done():
				if !tc.expectTimeout {
					t.Errorf("unexpected timeout: %v", ctx.Err())
				}
				if ctx.Err() != context.DeadlineExceeded {
					t.Errorf("expected DeadlineExceeded, got %v", ctx.Err())
				}
			}
		})
	}
}

func TestWithTimeoutContext(t *testing.T) {
	t.Parallel()

	parentCtx, parentCancel := context.WithTimeout(context.Background(), 200*time.Millisecond)
	defer parentCancel()

	ctx, cancel := WithTimeoutContext(t, parentCtx, 50*time.Millisecond)
	defer cancel()

	done := make(chan struct{})
	go func() {
		time.Sleep(100 * time.Millisecond)
		close(done)
	}()

	select {
	case <-done:
		t.Error("expected timeout but work completed")
	case <-ctx.Done():
		if ctx.Err() != context.DeadlineExceeded {
			t.Errorf("expected DeadlineExceeded, got %v", ctx.Err())
		}
	}
}

func TestDefaultTestTimeout(t *testing.T) {
	t.Parallel()

	if DefaultTestTimeout != 30*time.Second {
		t.Errorf("DefaultTestTimeout = %v, want 30s", DefaultTestTimeout)
	}
}
