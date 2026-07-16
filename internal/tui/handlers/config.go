package handlers

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
)

// ─── config.go — configuration and settings event handling for AppState ──────────

// HandleConfigReload processes config.ConfigReloadMsg: applies reloaded config,
// re-registers providers, and updates permissions.
func HandleConfigReload(m *AppState, msg config.ConfigReloadMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if msg.Error != nil {
		slog.Warn("config reload failed", "error", msg.Error)
		cmds = append(cmds, AddToastCmd(m, "Config reload failed: "+m31errors.UserMessage(msg.Error), "error", 5*time.Second))
		return cmds
	}

	if msg.Config != nil {
		if err := msg.Config.ResolveAPIKeys(m.keychain); err != nil {
			slog.Warn("failed to resolve API keys on reload", "error", err)
		}
		m.config = msg.Config
		m.reRegisterProvidersFromConfig()
		if m.dispatcher != nil {
			m.dispatcher.UpdatePermissions(&m.config.Permissions)
		}
		if m.configModel != nil {
			m.configModel.cfg = m.config
			m.configModel.buildContent()
		}
		if m.settingsModel != nil {
			m.settingsModel.config = m.config
		}
		cmds = append(cmds, AddToastCmd(m, "Config reloaded from disk", "info", 3*time.Second))
	}

	return cmds
}

// HandleSettingsSaved processes SettingsSavedMsg: rebuilds the config model
// and re-registers providers.
func HandleSettingsSaved(m *AppState) []tea.Cmd {
	var cmds []tea.Cmd

	if m.configModel != nil {
		m.configModel.buildContent()
	} else {
		cw, ch := m.contentDimensions()
		m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
	}
	m.reRegisterProvidersFromConfig()
	cmds = append(cmds, PopScreen(m))

	return cmds
}

// HandleConfigSaved processes ConfigSavedMsg: re-registers providers and
// updates permissions after a config editor save.
func HandleConfigSaved(m *AppState) []tea.Cmd {
	var cmds []tea.Cmd

	m.reRegisterProvidersFromConfig()
	if m.configModel != nil {
		m.configModel.buildContent()
	}
	if m.dispatcher != nil && m.config != nil {
		m.dispatcher.UpdatePermissions(&m.config.Permissions)
	}
	cmds = append(cmds, AddToastCmd(m, "Config saved to disk", "success", 3*time.Second))

	return cmds
}

// HandleResetComplete processes ResetCompleteMsg: resets all state to defaults
// and navigates to the first-run wizard.
func HandleResetComplete(m *AppState) []tea.Cmd {
	var cmds []tea.Cmd

	m.config = config.DefaultConfig()
	m.registry = provider.NewRegistry()
	m.activeProvider = ""
	m.activeModel = nil
	m.firstRunModel = nil
	m.replModel = nil
	m.configModel = nil
	m.screenStack = m.screenStack[:0]
	cmds = append(cmds, NavigateToScreen(m, ScreenFirstRun))
	cmds = append(cmds, AddToastCmd(m, "M31A reset to factory state", "success", 3*time.Second))

	return cmds
}

// HandleProviderModelsFetched processes ProviderModelsFetchedMsg.
func HandleProviderModelsFetched(m *AppState, msg ProviderModelsFetchedMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if msg.Err != nil {
		slog.Warn("failed to fetch provider models", "error", msg.Err)
		cmds = append(cmds, AddToastCmd(m, "Could not load model catalog: "+fmt.Sprintf("%v", msg.Err), "warning", 5*time.Second))
	}
	if m.replModel != nil {
		m.replModel.handleProviderModelsFetched(msg)
		if m.replModel.activeModel != nil {
			m.activeModel = m.replModel.activeModel
		}
	}
	return cmds
}

// HandleModelSelected processes ModelSelectedMsg: switches the active provider/model
// and syncs with the REPL.
func HandleModelSelected(m *AppState, msg ModelSelectedMsg) []tea.Cmd {
	var cmds []tea.Cmd

	m.activeModel = &msg.Model
	m.activeProvider = msg.Provider
	if m.registry != nil {
		if err := m.registry.SetActive(msg.Provider); err != nil {
			slog.Warn("failed to set active provider", "provider", msg.Provider, "error", err)
			cmds = append(cmds, AddToastCmd(m, "Could not switch to "+msg.Provider+": "+fmt.Sprintf("%v", err), "warning", 5*time.Second))
		}
	}
	if m.replModel != nil {
		m.replModel.activeModel = &msg.Model
		m.replModel.activeProvider = msg.Provider
		providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
		cmds = append(cmds, providerCmd)
	}
	cmds = append(cmds, PopScreen(m))

	return cmds
}

// HandleFallbackEvent processes FallbackEventMsg: switches the active provider
// and persists the change to config.
func HandleFallbackEvent(m *AppState, msg FallbackEventMsg) []tea.Cmd {
	m.activeProvider = msg.To
	slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)
	AddToast(m, fmt.Sprintf("Switched from %s to %s (provider unavailable)", msg.From, msg.To), "warning")

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

// HandleHealthCheckResult processes HealthCheckResultMsg: updates health state
// and displays the result in the REPL.
func HandleHealthCheckResult(m *AppState, msg HealthCheckResultMsg) []tea.Cmd {
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