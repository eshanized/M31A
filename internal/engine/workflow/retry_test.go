package workflow

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestRetryWithBackoff_Success(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		return nil
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if calls != 1 {
		t.Fatalf("expected 1 call, got %d", calls)
	}
}

func TestRetryWithBackoff_RetryOnTransientError(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		if calls < 3 {
			return errors.New("connection reset by peer")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if calls != 3 {
		t.Fatalf("expected 3 calls, got %d", calls)
	}
}

func TestRetryWithBackoff_MaxAttemptsExceeded(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 2, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		return errors.New("connection timeout")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	// Initial attempt + MaxAttempts-1 retries = MaxAttempts total
	if calls != 2 {
		t.Fatalf("expected 2 calls, got %d", calls)
	}
}

func TestRetryWithBackoff_NonRetryableError(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		return errors.New("context window exceeded: too long")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	// Should not retry on context overflow
	if calls != 1 {
		t.Fatalf("expected 1 call (no retries), got %d", calls)
	}
}

func TestRetryWithBackoff_ContextCancellation(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 5, BaseDelay: 50 * time.Millisecond}
	ctx, cancel := context.WithCancel(context.Background())
	var calls int
	err := RetryWithBackoff(ctx, cfg, func() error {
		calls++
		if calls == 2 {
			cancel() // Cancel during retry backoff
		}
		return errors.New("connection refused")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if calls != 2 {
		t.Fatalf("expected 2 calls before cancellation, got %d", calls)
	}
}

func TestRetryWithBackoff_ExponentialBackoff(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 4, BaseDelay: 10 * time.Millisecond, BackoffMultiplier: 2.0}
	var calls int
	start := time.Now()
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		return errors.New("internal server error 500")
	})
	elapsed := time.Since(start)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	// Should have retried 3 times with delays: ~10ms, ~20ms, ~40ms = ~70ms total minimum
	if elapsed < 40*time.Millisecond {
		t.Fatalf("expected at least 40ms of backoff delays, got %v", elapsed)
	}
}

func TestRetryWithResult_Success(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	result, err := RetryWithResult[string](context.Background(), cfg, func() (string, error) {
		calls++
		if calls < 2 {
			return "", errors.New("connection reset by peer")
		}
		return "success", nil
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if result != "success" {
		t.Fatalf("expected 'success', got %q", result)
	}
	if calls != 2 {
		t.Fatalf("expected 2 calls, got %d", calls)
	}
}

func TestRetryWithResult_MaxAttemptsExceeded(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 2, BaseDelay: time.Millisecond}
	result, err := RetryWithResult[int](context.Background(), cfg, func() (int, error) {
		return 0, errors.New("connection timeout")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if result != 0 {
		t.Fatalf("expected zero value, got %d", result)
	}
}

func TestRetryWithResult_ContextCancellation(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 5, BaseDelay: 50 * time.Millisecond}
	ctx, cancel := context.WithCancel(context.Background())
	var calls int
	_, err := RetryWithResult[int](ctx, cfg, func() (int, error) {
		calls++
		if calls == 1 {
			cancel()
		}
		return 0, errors.New("connection refused")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if calls != 1 {
		t.Fatalf("expected 1 call before cancellation, got %d", calls)
	}
}

func TestRetryConfig_Defaults(t *testing.T) {
	cfg := RetryConfig{}
	d := cfg.defaults()
	if d.MaxAttempts != 3 {
		t.Fatalf("expected MaxAttempts=3, got %d", d.MaxAttempts)
	}
	if d.BaseDelay != 100*time.Millisecond {
		t.Fatalf("expected BaseDelay=100ms, got %v", d.BaseDelay)
	}
	if d.MaxDelay != 10*time.Second {
		t.Fatalf("expected MaxDelay=10s, got %v", d.MaxDelay)
	}
	if d.BackoffMultiplier != 2.0 {
		t.Fatalf("expected BackoffMultiplier=2.0, got %f", d.BackoffMultiplier)
	}
}

func TestRetryConfig_CustomValues(t *testing.T) {
	cfg := RetryConfig{
		MaxAttempts:       5,
		BaseDelay:         200 * time.Millisecond,
		MaxDelay:          30 * time.Second,
		BackoffMultiplier: 3.0,
	}
	d := cfg.defaults()
	if d.MaxAttempts != 5 {
		t.Fatalf("expected MaxAttempts=5, got %d", d.MaxAttempts)
	}
	if d.BaseDelay != 200*time.Millisecond {
		t.Fatalf("expected BaseDelay=200ms, got %v", d.BaseDelay)
	}
}

func TestRetryWithBackoff_RateLimitError(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		return errors.New("rate limit exceeded: too many requests 429")
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	// Rate limit is retryable, so should exhaust all attempts
	if calls != 3 {
		t.Fatalf("expected 3 calls (all retries), got %d", calls)
	}
}

func TestRetryWithBackoff_ServerErrorThenSuccess(t *testing.T) {
	cfg := RetryConfig{MaxAttempts: 3, BaseDelay: time.Millisecond}
	var calls int
	err := RetryWithBackoff(context.Background(), cfg, func() error {
		calls++
		if calls == 1 {
			return errors.New("internal server error 500")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("expected nil error, got %v", err)
	}
	if calls != 2 {
		t.Fatalf("expected 2 calls, got %d", calls)
	}
}
