package workflow

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"github.com/eshanized/M31A/pkg/retry"
)

// RetryConfig configures retry behavior with exponential backoff.
// Zero values use sensible defaults (3 attempts, 100ms base, 10s max, 2x multiplier).
type RetryConfig struct {
	MaxAttempts       int
	BaseDelay         time.Duration
	MaxDelay          time.Duration
	BackoffMultiplier float64
}

// defaults fills zero-value fields with sensible defaults.
func (c RetryConfig) defaults() RetryConfig {
	if c.MaxAttempts <= 0 {
		c.MaxAttempts = 3
	}
	if c.BaseDelay <= 0 {
		c.BaseDelay = 100 * time.Millisecond
	}
	if c.MaxDelay <= 0 {
		c.MaxDelay = 10 * time.Second
	}
	if c.BackoffMultiplier <= 0 {
		c.BackoffMultiplier = 2.0
	}
	return c
}

// toPolicy converts RetryConfig to the underlying pkg/retry.Policy.
func (c RetryConfig) toPolicy() *retry.Policy {
	c = c.defaults()
	return &retry.Policy{
		MaxAttempts:   c.MaxAttempts,
		InitialDelay:  c.BaseDelay,
		MaxDelay:      c.MaxDelay,
		BackoffFactor: c.BackoffMultiplier,
	}
}

// RetryWithBackoff executes fn with exponential backoff, retrying on
// retryable errors. It respects context cancellation and logs each
// retry attempt. Returns the final error after all attempts are
// exhausted, or nil on success.
//
// Use this for side-effect-only operations where you don't need a
// return value. For operations that return a result, use
// RetryWithResult instead.
func RetryWithBackoff(ctx context.Context, cfg RetryConfig, fn func() error) error {
	policy := cfg.toPolicy()
	var lastErr error

	for attempt := 1; attempt <= policy.MaxAttempts; attempt++ {
		err := fn()
		if err == nil {
			return nil
		}
		lastErr = err

		class, reason := retry.ClassifyError(err)
		if !retry.IsRetryable(class) {
			slog.Debug("retry: non-retryable error", "attempt", attempt, "reason", reason, "error", err)
			return err
		}

		if attempt >= policy.MaxAttempts {
			break
		}

		delay := policy.Delay(attempt, nil)
		slog.Info("retrying operation", "attempt", attempt, "max", policy.MaxAttempts, "delay", delay, "reason", reason)

		select {
		case <-ctx.Done():
			return fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-time.After(delay):
		}
	}

	return lastErr
}

// RetryWithResult executes fn with exponential backoff, retrying on
// retryable errors. It returns the result of fn on success, or the
// zero value of T and the final error after all attempts are exhausted.
//
// Use this for operations that produce a value. For side-effect-only
// operations, use RetryWithBackoff instead.
func RetryWithResult[T any](ctx context.Context, cfg RetryConfig, fn func() (T, error)) (T, error) {
	var zero T
	policy := cfg.toPolicy()
	var lastErr error

	for attempt := 1; attempt <= policy.MaxAttempts; attempt++ {
		result, err := fn()
		if err == nil {
			return result, nil
		}
		lastErr = err

		class, reason := retry.ClassifyError(err)
		if !retry.IsRetryable(class) {
			slog.Debug("retry: non-retryable error", "attempt", attempt, "reason", reason, "error", err)
			return zero, err
		}

		if attempt >= policy.MaxAttempts {
			break
		}

		delay := policy.Delay(attempt, nil)
		slog.Info("retrying operation", "attempt", attempt, "max", policy.MaxAttempts, "delay", delay, "reason", reason)

		select {
		case <-ctx.Done():
			return zero, fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-time.After(delay):
		}
	}

	return zero, lastErr
}

// RetryWithHeaders executes fn with exponential backoff, retrying on
// retryable errors. The function returns HTTP response headers which
// are used for Retry-After header parsing. Returns the final error
// after all attempts are exhausted, or nil on success.
func RetryWithHeaders(ctx context.Context, cfg RetryConfig, fn func() (http.Header, error)) error {
	policy := cfg.toPolicy()
	var lastErr error

	for attempt := 1; attempt <= policy.MaxAttempts; attempt++ {
		headers, err := fn()
		if err == nil {
			return nil
		}
		lastErr = err

		class, reason := retry.ClassifyError(err)
		if !retry.IsRetryable(class) {
			slog.Debug("retry: non-retryable error", "attempt", attempt, "reason", reason, "error", err)
			return err
		}

		if attempt >= policy.MaxAttempts {
			break
		}

		delay := policy.Delay(attempt, headers)
		slog.Info("retrying operation", "attempt", attempt, "max", policy.MaxAttempts, "delay", delay, "reason", reason)

		select {
		case <-ctx.Done():
			return fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-time.After(delay):
		}
	}

	return lastErr
}
