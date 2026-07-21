package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/types"
)

// handler_runtime.go — runtime, health, and cache refresh message handling
// extracted from Update().

// handleHealthCheckTickMsg processes health check timer ticks: runs a health
// check against the active provider and schedules the next tick.
func handleHealthCheckTickMsg(m *AppState, msg HealthCheckTickMsg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd
	if m.activeProvider != "" && m.registry != nil {
		if p, err := m.registry.Get(m.activeProvider); err == nil {
			cmds = append(cmds, HealthCheckCmd(m.shutdownCtx, p, 10*time.Second))
		}
	}
	cmds = append(cmds, NextHealthTick(m.shutdownCtx, types.HealthCheckInterval))
	return m, tea.Batch(cmds...)
}

// handleHealthCheckResultMsg processes health check results.
func handleHealthCheckResultMsg(m *AppState, msg HealthCheckResultMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleHealthCheckResult(msg)...)
}

// handleRefreshCacheMsg processes cache refresh requests.
func handleRefreshCacheMsg(m *AppState, msg RefreshCacheMsg) (tea.Model, tea.Cmd) {
	if m.registry != nil {
		providerName := msg.ProviderName
		if providerName == "" {
			providerName = m.activeProvider
		}
		return m, CacheRefreshCmd(m.shutdownCtx, m.registry, providerName)
	}
	return m, nil
}

// handleCacheRefreshResultMsg processes cache refresh completion.
func handleCacheRefreshResultMsg(m *AppState, msg CacheRefreshResultMsg) (tea.Model, tea.Cmd) {
	if msg.NextCmd != nil {
		return m, msg.NextCmd
	}
	return m, nil
}
