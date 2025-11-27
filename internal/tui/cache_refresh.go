package tui

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
)

// CacheRefreshTicker returns a tea.Cmd that emits a RefreshCacheMsg after d.
func CacheRefreshTicker(providerName string, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return RefreshCacheMsg{ProviderName: providerName}
	})
}

// NextCacheRefreshTick returns a tea.Cmd for the next cache refresh tick.
func NextCacheRefreshTick(d time.Duration) tea.Cmd {
	return tea.Tick(d, func(time.Time) tea.Msg {
		return RefreshCacheMsg{}
	})
}

// CacheRefreshCmd runs FetchModels in a goroutine and emits CacheRefreshResultMsg.
func CacheRefreshCmd(ctx context.Context, registry *provider.Registry, providerName string) tea.Cmd {
	return func() tea.Msg {
		if registry == nil {
			return CacheRefreshResultMsg{ErrMsg: "no registry"}
		}
		p, err := registry.Get(providerName)
		if err != nil {
			return CacheRefreshResultMsg{ErrMsg: err.Error()}
		}
		fetchCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		defer cancel()
		if _, err := p.FetchModels(fetchCtx); err != nil {
			return CacheRefreshResultMsg{
				ErrMsg:  err.Error(),
				NextCmd: NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
			}
		}
		return CacheRefreshResultMsg{
			NextCmd: NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
		}
	}
}
