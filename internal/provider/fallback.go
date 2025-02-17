package provider

import (
	"context"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
)

type FallbackEvent struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Reason string `json:"reason"`
}

func ShouldFallback(statusCode int) bool {
	return statusCode == 429 || statusCode == 503
}

func FindFallbackProvider(registry *Registry, currentProvider string) (string, *FallbackEvent, error) {
	names := registry.List()

	var reason string
	for _, name := range names {
		if name == currentProvider {
			continue
		}

		p, err := registry.Get(name)
		if err != nil {
			continue
		}

		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		status := p.HealthCheck(ctx)
		cancel()

		if status.Status == "live" || status.Status == "slow" {
			reason = "rate_limited"
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
