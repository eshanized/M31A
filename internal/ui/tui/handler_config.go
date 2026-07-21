package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
)

// handler_config.go — configuration, settings, and first-run message handling
// extracted from Update().

// handleFirstRunCompleteMsg processes first-run wizard completion.
func handleFirstRunCompleteMsg(m *AppState, msg FirstRunCompleteMsg) (tea.Model, tea.Cmd) {
	return m, m.handleFirstRunComplete(msg)
}

// handleSettingsSavedMsg processes settings save completion.
func handleSettingsSavedMsg(m *AppState, msg SettingsSavedMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleSettingsSaved()...)
}

// handleResetCompleteMsg processes factory reset completion.
func handleResetCompleteMsg(m *AppState, msg ResetCompleteMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleResetComplete()...)
}

// handleConfigSavedMsg processes config editor save completion.
func handleConfigSavedMsg(m *AppState, msg ConfigSavedMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleConfigSaved()...)
}

// handleConfigReloadMsg processes config file hot-reload.
func handleConfigReloadMsg(m *AppState, msg config.ConfigReloadMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleConfigReload(msg)...)
}
