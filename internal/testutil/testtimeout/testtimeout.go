// Package testtimeout provides helper functions for per-test timeouts.
package testtimeout

import (
	"context"
	"testing"
	"time"
)

// DefaultTestTimeout is the default timeout for individual tests.
const DefaultTestTimeout = 30 * time.Second

// WithTimeout returns a context with the given timeout and a cancel function.
// The cancel function is registered with t.Cleanup for automatic cleanup.
// t.Helper() is called to mark this function as a test helper.
func WithTimeout(t *testing.T, duration time.Duration) (context.Context, context.CancelFunc) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), duration)
	t.Cleanup(cancel)
	return ctx, cancel
}

// WithTimeoutContext returns a context derived from the parent context with the given timeout.
// The cancel function is registered with t.Cleanup for automatic cleanup.
// t.Helper() is called to mark this function as a test helper.
func WithTimeoutContext(t *testing.T, parent context.Context, duration time.Duration) (context.Context, context.CancelFunc) {
	t.Helper()
	ctx, cancel := context.WithTimeout(parent, duration)
	t.Cleanup(cancel)
	return ctx, cancel
}