package tui

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/pkg/types"
)

// app_handlers_provider.go — provider and health event handling extracted from Update().

// handleHealthCheckResult processes HealthCheckResultMsg: updates health state
// and displays the result in the REPL.
func (m *AppState) handleHealthCheckResult(msg HealthCheckResultMsg) []tea.Cmd {
	m.healthStatus = msg.Result
	m.lastHealth = time.Now()

	if m.replModel != nil {
		result := msg.Result
		var emoji, status string
		switch result.Status {
		case types.HealthStatusLive:
			emoji = "✓"
			status = "healthy"
		case types.HealthStatusSlow:
			emoji = "⚠"
			status = "slow"
		case types.HealthStatusOffline:
			emoji = "✗"
			status = "offline"
		case types.HealthStatusDegraded:
			emoji = "⚠"
			status = "degraded"
		default:
			emoji = "?"
			status = result.Status
		}
		var text string
		if result.Error != "" {
			text = fmt.Sprintf("%s Health check: %s (%s) — %s", emoji, status, fmt.Sprintf("%dms", result.LatencyMs), result.Error)
		} else {
			text = fmt.Sprintf("%s Health check: %s (%s)", emoji, status, fmt.Sprintf("%dms", result.LatencyMs))
		}
		m.replModel.AddMessage(makeAssistantMsg(text))
	}

	return nil
}

// handleFallbackEvent processes FallbackEventMsg: switches the active provider
// and persists the change to config.
func (m *AppState) handleFallbackEvent(msg FallbackEventMsg) []tea.Cmd {
	m.activeProvider = msg.To
	slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)
	m.addToast(fmt.Sprintf("Switched from %s to %s (provider unavailable)", msg.From, msg.To), "warning")

	if m.config != nil {
		m.config.Provider.Default = msg.To
		if m.configPath != "" {
			if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
				slog.Warn("failed to save provider to config", "error", err)
			}
		}
	}

	if m.registry != nil && m.activeModel != nil {
		if p, err := m.registry.Get(msg.To); err == nil && p != nil {
			if info, err := p.GetModel(m.activeModel.ID); err == nil && info != nil {
				m.activeModel = info
			}
		}
	}

	return nil
}

// handleProviderModelsFetched processes ProviderModelsFetchedMsg.
func (m *AppState) handleProviderModelsFetched(msg ProviderModelsFetchedMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if msg.Err != nil {
		slog.Warn("failed to fetch provider models", "error", msg.Err)
		cmds = append(cmds, m.addToastCmd("Could not load model catalog: "+fmt.Sprintf("%v", msg.Err), "warning", 5*time.Second))
	}
	if m.replModel != nil {
		m.replModel.handleProviderModelsFetched(msg)
		if m.replModel.activeModel != nil {
			m.activeModel = m.replModel.activeModel
		}
	}
	return cmds
}

// handleModelSelected processes ModelSelectedMsg: switches the active provider/model
// and syncs with the REPL.
func (m *AppState) handleModelSelected(msg ModelSelectedMsg) []tea.Cmd {
	var cmds []tea.Cmd

	m.activeModel = &msg.Model
	m.activeProvider = msg.Provider
	if m.registry != nil {
		if err := m.registry.SetActive(msg.Provider); err != nil {
			slog.Warn("failed to set active provider", "provider", msg.Provider, "error", err)
			cmds = append(cmds, m.addToastCmd("Could not switch to "+msg.Provider+": "+fmt.Sprintf("%v", err), "warning", 5*time.Second))
		}
	}
	if m.replModel != nil {
		m.replModel.activeModel = &msg.Model
		m.replModel.activeProvider = msg.Provider
		providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
		cmds = append(cmds, providerCmd)
	}
	cmds = append(cmds, m.popScreen())

	return cmds
}
