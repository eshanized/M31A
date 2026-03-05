package tui

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// HealthCheckTicker returns a tea.Cmd that emits a HealthCheckTickMsg after the
// given duration. The app re-schedules it in response to HealthCheckResultMsg.
func HealthCheckTicker(ctx context.Context, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}

// NextHealthTick returns a tea.Cmd for the next health check tick.
func NextHealthTick(ctx context.Context, d time.Duration) tea.Cmd {
	return HealthCheckTicker(ctx, d)
}

// HealthCheckCmd runs a health check against the given provider in a goroutine
// and emits a HealthCheckResultMsg when it completes.
func HealthCheckCmd(ctx context.Context, p provider.LLMProvider, timeout time.Duration) tea.Cmd {
	return func() tea.Msg {
		if timeout <= 0 {
			timeout = types.HealthCheckInterval
		}
		hCtx, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()
		status := p.HealthCheck(hCtx)
		return HealthCheckResultMsg{Result: status}
	}
}

// SidebarRefreshTicker returns a tea.Cmd that emits a SidebarRefreshTickMsg after the
// given duration. The sidebar re-schedules it in response to the tick.
func SidebarRefreshTicker(ctx context.Context, d time.Duration) tea.Cmd {
	return tea.Tick(d, func(t time.Time) tea.Msg {
		return SidebarRefreshTickMsg{}
	})
}

// NextSidebarRefreshTick returns a tea.Cmd for the next sidebar refresh tick.
func NextSidebarRefreshTick(ctx context.Context, d time.Duration) tea.Cmd {
	return SidebarRefreshTicker(ctx, d)
}
