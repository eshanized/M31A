package provider

import (
	"context"
	"net/http"
	"strconv"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

type FallbackEvent struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

// maxRetryAfter is the maximum duration to wait for Retry-After header (120s cap).
const maxRetryAfter = types.MaxRetryAfterWait

// FindFallbackProvider searches for an alternative provider when the current
// one fails. fallbackPriority is an ordered list of provider names to try;
// when non-empty, candidates are checked in this order. When empty, falls
// back to alphabetical sort (existing behavior). healthCheckTimeoutSecs
// controls the per-provider health check deadline; 0 uses the default (10s).
func FindFallbackProvider(registry *Registry, currentProvider string, fallbackPriority []string, healthCheckTimeoutSecs int) (string, *FallbackEvent, error) {
	// Build candidate list in the requested order
	type candidate struct {
		name     string
		provider LLMProvider
	}
	var candidates []candidate

	if len(fallbackPriority) > 0 {
		// Use config-specified order
		for _, name := range fallbackPriority {
			if name == currentProvider {
				continue
			}
			p, err := registry.Get(name)
			if err != nil {
				continue
			}
			candidates = append(candidates, candidate{name: name, provider: p})
		}
	} else {
		// Fallback to alphabetical sort (existing behavior)
		names := registry.ListAll()
		for _, name := range names {
			if name == currentProvider {
				continue
			}
			p, err := registry.Get(name)
			if err != nil {
				continue
			}
			candidates = append(candidates, candidate{name: name, provider: p})
		}
	}

	if len(candidates) == 0 {
		return "", nil, m31errors.ErrProviderUnreachable
	}

	// Parallel health checks (PV-14 fix): run all candidates concurrently
	// and pick the first healthy one instead of serial 10s×N worst case.
	type result struct {
		name   string
		status types.HealthStatus
	}
	ch := make(chan result, len(candidates))

	timeout := 10 * time.Second // default
	if healthCheckTimeoutSecs > 0 {
		timeout = time.Duration(healthCheckTimeoutSecs) * time.Second
	}
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	for _, c := range candidates {
		go func(c candidate) {
			status := c.provider.HealthCheck(ctx)
			ch <- result{name: c.name, status: status}
		}(c)
	}

	// Collect results as they arrive, short-circuiting on the first live
	// provider in priority order. Falls back to a slow provider if no live
	// one is found after all results arrive.
	results := make(map[string]types.HealthStatus, len(candidates))
	var slowFallback string
	priorityIdx := 0
	for i := 0; i < len(candidates); i++ {
		r := <-ch
		results[r.name] = r.status

		// Advance through priority list only for newly arrived results
		for priorityIdx < len(candidates) {
			c := candidates[priorityIdx]
			status, ok := results[c.name]
			if !ok {
				break // haven't received this candidate's result yet
			}
			priorityIdx++
			if status.Status == "live" {
				cancel()
				if _, err := registry.TrySetActive(c.name); err != nil {
					continue
				}
				return c.name, &FallbackEvent{
					From:   currentProvider,
					To:     c.name,
					Reason: "fallback_live",
				}, nil
			}
			if status.Status == "slow" && slowFallback == "" {
				slowFallback = c.name
			}
		}
	}

	// No live provider found; use the first slow one if available
	if slowFallback != "" {
		if _, err := registry.TrySetActive(slowFallback); err == nil {
			return slowFallback, &FallbackEvent{
				From:   currentProvider,
				To:     slowFallback,
				Reason: "fallback_slow",
			}, nil
		}
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
func FindFallbackWithRetryAfter(registry *Registry, currentProvider string, retryAfterHeader string, fallbackPriority []string, healthCheckTimeoutSecs int) FallbackAfterWait {
	wait := time.Duration(0)
	if retryAfterHeader != "" {
		if seconds, err := strconv.Atoi(retryAfterHeader); err == nil && seconds > 0 {
			wait = time.Duration(seconds) * time.Second
			if wait > maxRetryAfter {
				wait = maxRetryAfter
			}
		}
	}

	_, event, err := FindFallbackProvider(registry, currentProvider, fallbackPriority, healthCheckTimeoutSecs)
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
