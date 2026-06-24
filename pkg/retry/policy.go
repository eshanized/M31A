package retry

import (
	"context"
	"fmt"
	"log/slog"
	"math"
	"net/http"
	"strconv"
	"strings"
	"time"
)

// DefaultPolicy returns a retry policy with sensible defaults for LLM APIs:
// 3 attempts, 1s initial delay, 30s max delay, 2x backoff factor.
func DefaultPolicy() *Policy {
	return &Policy{
		MaxAttempts:   3,
		InitialDelay:  1 * time.Second,
		MaxDelay:      30 * time.Second,
		BackoffFactor: 2.0,
	}
}

// Policy configures retry behavior with exponential backoff and
// retry-after header support.
type Policy struct {
	MaxAttempts   int
	InitialDelay  time.Duration
	MaxDelay      time.Duration
	BackoffFactor float64
}

// Delay computes the wait duration for the given attempt number (1-based).
// If headers contain retry-after or retry-after-ms, those take precedence.
func (p *Policy) Delay(attempt int, headers http.Header) time.Duration {
	if headers != nil {
		if d := parseRetryAfter(headers); d > 0 {
			return d
		}
	}
	delay := float64(p.InitialDelay) * math.Pow(p.BackoffFactor, float64(attempt-1))
	if delay > float64(p.MaxDelay) {
		delay = float64(p.MaxDelay)
	}
	return time.Duration(delay)
}

// parseRetryAfter extracts a wait duration from Retry-After or Retry-After-Ms headers.
func parseRetryAfter(headers http.Header) time.Duration {
	if v := headers.Get("Retry-After-Ms"); v != "" {
		if ms, err := strconv.ParseInt(v, 10, 64); err == nil && ms > 0 {
			return time.Duration(ms) * time.Millisecond
		}
	}
	if v := headers.Get("Retry-After"); v != "" {
		if seconds, err := strconv.Atoi(v); err == nil && seconds > 0 {
			return time.Duration(seconds) * time.Second
		}
		if t, err := time.Parse(time.RFC1123, v); err == nil {
			d := time.Until(t)
			if d > 0 {
				return d
			}
		}
	}
	return 0
}

// ErrorClass categorizes an error for retry decisions.
type ErrorClass int

const (
	ErrorClassUnknown ErrorClass = iota
	ErrorClassContextOverflow
	ErrorClassRateLimit
	ErrorClassOverloaded
	ErrorClassServerError
	ErrorClassNetwork
)

// ClassifyError examines an error and returns its class and a human-readable reason.
func ClassifyError(err error) (ErrorClass, string) {
	if err == nil {
		return ErrorClassUnknown, ""
	}
	msg := strings.ToLower(err.Error())

	if strings.Contains(msg, "context") && (strings.Contains(msg, "overflow") || strings.Contains(msg, "too long") || strings.Contains(msg, "exceeds")) {
		return ErrorClassContextOverflow, "context window exceeded — retry will not help"
	}

	if strings.Contains(msg, "rate limit") || strings.Contains(msg, "too many requests") || strings.Contains(msg, "rate increased too quickly") || strings.Contains(msg, "429") {
		return ErrorClassRateLimit, "rate limited — backing off"
	}

	if strings.Contains(msg, "overloaded") || strings.Contains(msg, "529") {
		return ErrorClassOverloaded, "server overloaded — backing off"
	}

	if strings.Contains(msg, "500") || strings.Contains(msg, "502") || strings.Contains(msg, "503") || strings.Contains(msg, "504") || strings.Contains(msg, "internal server error") {
		return ErrorClassServerError, "server error — retrying"
	}

	if strings.Contains(msg, "connection") || strings.Contains(msg, "timeout") || strings.Contains(msg, "eof") || strings.Contains(msg, "reset") {
		return ErrorClassNetwork, "network error — retrying"
	}

	return ErrorClassUnknown, "unknown error"
}

// IsRetryable returns true if the error class warrants a retry.
func IsRetryable(class ErrorClass) bool {
	switch class {
	case ErrorClassContextOverflow:
		return false
	case ErrorClassRateLimit, ErrorClassOverloaded, ErrorClassServerError, ErrorClassNetwork:
		return true
	default:
		return false
	}
}

// Retry executes fn with exponential backoff. It respects context cancellation
// and classifies errors to decide whether to retry. Returns the last error
// if all attempts fail, or nil on success.
func (p *Policy) Retry(ctx context.Context, fn func() error) error {
	return p.RetryWithHeaders(ctx, func() (http.Header, error) {
		return nil, fn()
	})
}

// RetryWithHeaders is like Retry but the function also returns HTTP response
// headers for retry-after parsing.
func (p *Policy) RetryWithHeaders(ctx context.Context, fn func() (http.Header, error)) error {
	var lastErr error
	for attempt := 1; attempt <= p.MaxAttempts; attempt++ {
		headers, err := fn()
		if err == nil {
			return nil
		}
		lastErr = err

		class, reason := ClassifyError(err)
		if !IsRetryable(class) {
			slog.Debug("retry: non-retryable error", "attempt", attempt, "reason", reason, "error", err)
			return err
		}

		if attempt >= p.MaxAttempts {
			break
		}

		delay := p.Delay(attempt, headers)
		slog.Debug("retry: backing off", "attempt", attempt, "max", p.MaxAttempts, "delay", delay, "reason", reason)

		timer := time.NewTimer(delay)
		select {
		case <-ctx.Done():
			timer.Stop()
			return fmt.Errorf("%w: %v", ctx.Err(), lastErr)
		case <-timer.C:
		}
	}
	return lastErr
}
