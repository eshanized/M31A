package tui

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

func HealthCheckTicker(ctx context.Context, interval time.Duration) tea.Cmd {

	if ctx == nil {
		return nil
	}

	if interval <= 0 {
		interval = types.HealthCheckInterval
	}

	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}

func NextHealthTick(interval time.Duration) tea.Cmd {
	if interval <= 0 {
		interval = types.HealthCheckInterval
	}
	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}

// HealthCheckCmd returns a tea.Cmd that performs the health check in a
// goroutine and emits HealthCheckResultMsg when complete. C-1 fix: the
// HTTP call no longer blocks Bubble Tea's Update() loop.
func HealthCheckCmd(p provider.LLMProvider, timeout time.Duration) tea.Cmd {
	return func() tea.Msg {
		if timeout <= 0 {
			timeout = 10 * time.Second
		}
		ctx, cancel := context.WithTimeout(context.Background(), timeout)
		defer cancel()
		result := p.HealthCheck(ctx)
		return HealthCheckResultMsg{Result: result}
	}
}
