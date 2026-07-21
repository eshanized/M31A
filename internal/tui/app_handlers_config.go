package tui

import (
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// app_handlers_config.go — configuration and settings event handling extracted from Update().

// handleConfigReload processes config.ConfigReloadMsg: applies reloaded config,
// re-registers providers, and updates permissions.
func (m *AppState) handleConfigReload(msg config.ConfigReloadMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if msg.Error != nil {
		slog.Warn("config reload failed", "error", msg.Error)
		cmds = append(cmds, m.addToastCmd("Config reload failed: "+m31errors.UserMessage(msg.Error), "error", 5*time.Second))
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
		cmds = append(cmds, m.addToastCmd("Config reloaded from disk", "info", 3*time.Second))
	}

	return cmds
}

// handleSettingsSaved processes SettingsSavedMsg: rebuilds the config model
// and re-registers providers.
func (m *AppState) handleSettingsSaved() []tea.Cmd {
	var cmds []tea.Cmd

	if m.configModel != nil {
		m.configModel.buildContent()
	} else {
		cw, ch := m.contentDimensions()
		m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
	}
	m.reRegisterProvidersFromConfig()
	cmds = append(cmds, m.popScreen())

	return cmds
}

// handleConfigSaved processes ConfigSavedMsg: re-registers providers and
// updates permissions after a config editor save.
func (m *AppState) handleConfigSaved() []tea.Cmd {
	var cmds []tea.Cmd

	m.reRegisterProvidersFromConfig()
	if m.configModel != nil {
		m.configModel.buildContent()
	}
	if m.dispatcher != nil && m.config != nil {
		m.dispatcher.UpdatePermissions(&m.config.Permissions)
	}
	cmds = append(cmds, m.addToastCmd("Config saved to disk", "success", 3*time.Second))

	return cmds
}

// handleResetComplete processes ResetCompleteMsg: resets all state to defaults
// and navigates to the first-run wizard.
func (m *AppState) handleResetComplete() []tea.Cmd {
	var cmds []tea.Cmd

	m.config = config.DefaultConfig()
	m.registry = provider.NewRegistry()
	m.activeProvider = ""
	m.activeModel = nil
	m.firstRunModel = nil
	m.replModel = nil
	m.configModel = nil
	m.screenStack = m.screenStack[:0]
	cmds = append(cmds, m.navigateToScreen(ScreenFirstRun))
	cmds = append(cmds, m.addToastCmd("M31A reset to factory state", "success", 3*time.Second))

	return cmds
}
