package provider

import (
	"context"
	"net/http"
	"strconv"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
)

type FallbackEvent struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

// maxRetryAfter is the maximum duration to wait for Retry-After header (60s cap).
const maxRetryAfter = 60 * time.Second

func FindFallbackProvider(registry *Registry, currentProvider string) (string, *FallbackEvent, error) {
	names := registry.List()

	for _, name := range names {
		if name == currentProvider {
			continue
		}

		p, err := registry.Get(name)
		if err != nil {
			continue
		}

		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		status := p.HealthCheck(ctx)
		cancel()

		if status.Status == "live" || status.Status == "slow" {
			reason := "rate_limited"
			if status.Status == "slow" {
				reason = "unavailable"
			}

			if err := registry.SetActive(name); err != nil {
				continue
			}

			return name, &FallbackEvent{
				From:   currentProvider,
				To:     name,
				Reason: reason,
			}, nil
		}
	}

	return "", nil, m31errors.ErrProviderUnreachable
}

// FindFallbackWithRetryAfter attempts fallback with Retry-After awareness.
// When the current provider returns 429 with a Retry-After header, it waits
// up to the specified duration (capped at 60s) before trying the other provider.
func FindFallbackWithRetryAfter(registry *Registry, currentProvider string, retryAfterHeader string) (string, *FallbackEvent, error) {
	if retryAfterHeader != "" {
		if seconds, err := strconv.Atoi(retryAfterHeader); err == nil && seconds > 0 {
			wait := time.Duration(seconds) * time.Second
			if wait > maxRetryAfter {
				wait = maxRetryAfter
			}
			time.Sleep(wait)
		}
	}

	return FindFallbackProvider(registry, currentProvider)
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
