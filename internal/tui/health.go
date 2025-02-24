package tui

import (
	"context"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
)

const HealthCheckInterval = 60 * time.Second

func HealthCheckTicker(ctx context.Context, registry *provider.Registry,
	activeProvider string, interval time.Duration) tea.Cmd {

	if ctx == nil {
		return nil
	}

	if interval <= 0 {
		interval = HealthCheckInterval
	}

	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}

func NextHealthTick(interval time.Duration) tea.Cmd {
	if interval <= 0 {
		interval = HealthCheckInterval
	}
	return tea.Tick(interval, func(t time.Time) tea.Msg {
		return HealthCheckTickMsg{Time: t}
	})
}
