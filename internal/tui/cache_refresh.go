package tui

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
)

// CacheRefreshTicker returns a tea.Cmd that emits RefreshCacheMsg on the
// given interval for the named provider. Uses tea.Every (available in
// Bubble Tea v1.3.0+).
func CacheRefreshTicker(providerName string, interval time.Duration) tea.Cmd {
	if interval <= 0 {
		interval = provider.DefaultCacheRefreshInterval
	}

	return tea.Every(interval, func(t time.Time) tea.Msg {
		return RefreshCacheMsg{ProviderName: providerName}
	})
}

// NextCacheRefreshTick returns a one-shot tea.Cmd that schedules the next
// cache refresh at the given interval. Used by AppState.Update() to
// reschedule after a manual or on-demand refresh.
func NextCacheRefreshTick(interval time.Duration) tea.Cmd {
	if interval <= 0 {
		interval = provider.DefaultCacheRefreshInterval
	}

	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return RefreshCacheMsg{ProviderName: ""}
	})
}

// handleCacheRefresh performs the actual model cache refresh for the given
// provider name via the registry. Returns error as string (empty on success)
// and the next tea.Cmd to continue the refresh cycle.
func handleCacheRefresh(ctx context.Context, registry *provider.Registry, providerName string) (string, tea.Cmd) {
	if registry == nil {
		return "Provider registry not available", nil
	}

	if providerName == "" {
		providerName = registry.Active()
	}

	p, err := registry.Get(providerName)
	if err != nil || p == nil {
		return "Provider not found: " + providerName, nil
	}

	models, err := p.FetchModels(ctx)
	if err != nil {
		return "Cache refresh failed: " + err.Error(), NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
	}

	if len(models) == 0 {
		return "Cache refresh returned no models", NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
	}

	return "", NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
}

// CacheRefreshCmd returns a tea.Cmd that performs the cache refresh in a
// goroutine and emits CacheRefreshResultMsg when complete. C-2 fix: the
// HTTP call no longer blocks Bubble Tea's Update() loop.
func CacheRefreshCmd(registry *provider.Registry, providerName string) tea.Cmd {
	return func() tea.Msg {
		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()
		errMsg, nextCmd := handleCacheRefresh(ctx, registry, providerName)
		return CacheRefreshResultMsg{ErrMsg: errMsg, NextCmd: nextCmd}
	}
}
