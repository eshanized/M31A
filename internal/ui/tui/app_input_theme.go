package tui

import (
	"log/slog"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// applyTheme switches the theme and propagates it to all sub-models.
// M31A ships with a single dark theme. Light/auto themes are not supported.
func (m *AppState) applyTheme(themeName string) {
	if themeName != "dark" {
		// Light/auto themes are not supported; always use dark.
		themeName = "dark"
	}
	m.themeManager = theme.NewManager(theme.ModeDark)

	// Persist theme selection to config so it survives restarts.
	if m.config != nil {
		m.config.UI.Theme = themeName
		if m.configPath != "" {
			if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
				slog.Warn("failed to save theme to config", "error", err)
			}
		}
	}

	t := m.themeManager.Current()
	if m.replModel != nil {
		m.replModel.SetTheme(t)
	}
	if m.sidebarModel != nil {
		m.sidebarModel.SetTheme(t)
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetTheme(t)
	}
	if m.settingsModel != nil {
		m.settingsModel.SetTheme(t)
	}
	if m.planModel != nil {
		m.planModel.theme = t
	}
	if m.executeModel != nil {
		m.executeModel.theme = t
	}
	if m.verifyModel != nil {
		m.verifyModel.theme = t
	}
	if m.shipModel != nil {
		m.shipModel.theme = t
	}
	if m.discussModel != nil {
		m.discussModel.SetTheme(t)
	}
	if m.metricsModel != nil {
		m.metricsModel.SetTheme(t)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetTheme(t)
	}
	if m.diffModel != nil {
		m.diffModel.SetTheme(t)
	}
	if m.goalInput != nil {
		m.goalInput.SetTheme(t)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetTheme(t)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetTheme(t)
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetTheme(t)
	}
	if m.msModel != nil {
		m.msModel.SetTheme(t)
	}
	if m.helpModel != nil {
		m.helpModel.SetTheme(t)
	}
	if m.configModel != nil {
		m.configModel.theme = t
	}
	if m.bisectModel != nil {
		m.bisectModel.SetTheme(t)
	}
	if m.notifModel != nil {
		m.notifModel.SetTheme(t)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetTheme(t)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetTheme(t)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetTheme(t)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetTheme(t)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetTheme(t)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetTheme(t)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetTheme(t)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetTheme(t)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetTheme(t)
	}
}
