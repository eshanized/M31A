package provider

import (
	"context"
	"net/http"
	"strconv"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type FallbackEvent struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

// maxRetryAfter is the maximum duration to wait for Retry-After header (120s cap).
const maxRetryAfter = types.MaxRetryAfterWait

func FindFallbackProvider(registry *Registry, currentProvider string) (string, *FallbackEvent, error) {
	names := registry.ListAll()

	for _, name := range names {
		if name == currentProvider {
			continue
		}

		p, err := registry.TrySetActive(name)
		if err != nil {
			continue
		}

		// Use defer cancel() to ensure cleanup even if HealthCheck panics
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		status := func() types.HealthStatus {
			defer cancel()
			return p.HealthCheck(ctx)
		}()

		if status.Status == "live" || status.Status == "slow" {
			// Reason describes the fallback's health, since the caller lost the
			// original provider's failure context by the time we get here.
			reason := "fallback_live"
			if status.Status == "slow" {
				reason = "fallback_slow"
			}

			return name, &FallbackEvent{
				From:   currentProvider,
				To:     name,
				Reason: reason,
			}, nil
		}

		// Health check failed — rollback to the original provider
		registry.RollbackActive(name, currentProvider)
	}

	return "", nil, m31errors.ErrProviderUnreachable
}

// RetryInfo holds Retry-After metadata extracted from an HTTP response.
// Wait is zero when no retry header is present.
type RetryInfo struct {
	RateLimited bool
	RetryAfter  string
	Wait        time.Duration
}

// InspectResponse extracts rate-limit and Retry-After metadata from an
// HTTP response using IsRateLimited and GetRetryAfter. Returns a zero
// RetryInfo for nil responses. Callers use this to decide whether to
// wait before falling back to another provider.
func InspectResponse(resp *http.Response) RetryInfo {
	info := RetryInfo{
		RateLimited: IsRateLimited(resp),
		RetryAfter:  GetRetryAfter(resp),
	}
	if info.RetryAfter != "" {
		if seconds, err := strconv.Atoi(info.RetryAfter); err == nil && seconds > 0 {
			info.Wait = time.Duration(seconds) * time.Second
			if info.Wait > maxRetryAfter {
				info.Wait = maxRetryAfter
			}
		}
	}
	return info
}

// FallbackAfterWait describes a pending fallback: the provider to switch to,
// the event describing the transition, and the delay to apply before
// committing the switch. Returned by FindFallbackWithRetryAfter so the
// caller (typically the TUI layer) can schedule the wait asynchronously
// via a tea.Cmd instead of blocking the event loop.
type FallbackAfterWait struct {
	Event *FallbackEvent
	Wait  time.Duration
	Err   error
}

// FindFallbackWithRetryAfter attempts fallback with Retry-After awareness.
// Instead of blocking the caller (which would stall the Bubble Tea event
// loop), it returns a FallbackAfterWait describing the delay the caller
// should schedule asynchronously. The caller is responsible for waiting
// the returned Wait duration before applying the provider switch.
//
// When the current provider returned a 429 with a Retry-After header,
// Wait is capped at maxRetryAfter (120s).
func FindFallbackWithRetryAfter(registry *Registry, currentProvider string, retryAfterHeader string) FallbackAfterWait {
	wait := time.Duration(0)
	if retryAfterHeader != "" {
		if seconds, err := strconv.Atoi(retryAfterHeader); err == nil && seconds > 0 {
			wait = time.Duration(seconds) * time.Second
			if wait > maxRetryAfter {
				wait = maxRetryAfter
			}
		}
	}

	_, event, err := FindFallbackProvider(registry, currentProvider)
	if err != nil {
		return FallbackAfterWait{Err: err}
	}
	if event != nil && wait > 0 {
		event.Reason = "rate_limited"
	}
	return FallbackAfterWait{Event: event, Wait: wait, Err: err}
}

// IsRateLimited checks if an HTTP response indicates rate limiting (429).
func IsRateLimited(resp *http.Response) bool {
	return resp != nil && resp.StatusCode == http.StatusTooManyRequests
}

// GetRetryAfter extracts the Retry-After header value from an HTTP response.
func GetRetryAfter(resp *http.Response) string {
	if resp == nil {
		return ""
	}
	return resp.Header.Get("Retry-After")
}
