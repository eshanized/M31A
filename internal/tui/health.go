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
func NextHealthTick(d time.Duration) tea.Cmd {
	return HealthCheckTicker(context.Background(), d)
}

// HealthCheckCmd runs a health check against the given provider in a goroutine
// and emits a HealthCheckResultMsg when it completes.
func HealthCheckCmd(p provider.LLMProvider, timeout time.Duration) tea.Cmd {
	return func() tea.Msg {
		if timeout <= 0 {
			timeout = types.HealthCheckInterval
		}
		ctx, cancel := context.WithTimeout(context.Background(), timeout)
		defer cancel()
		status := p.HealthCheck(ctx)
		return HealthCheckResultMsg{Result: status}
	}
}
