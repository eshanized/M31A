package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/layout"
)

// handleWindowResize resizes all sub-models using content dimensions
// (terminal minus unified chrome and sidebar).
func (m *AppState) handleWindowResize(msg tea.WindowSizeMsg) tea.Cmd {
	// Compute content dimensions: terminal minus unified chrome (header+footer)
	contentW := msg.Width
	contentH := msg.Height - layout.ChromeHeight
	if contentH < 1 {
		contentH = 1
	}

	// Subtract sidebar width when visible at full breakpoint
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(msg.Width) {
		sw = m.sidebarModel.GetWidth()
		contentW -= sw
	}
	if contentW < 1 {
		contentW = 1
	}

	// Build a content-sized WindowSizeMsg for sub-models
	contentMsg := tea.WindowSizeMsg{Width: contentW, Height: contentH}

	var cmd tea.Cmd
	if m.replModel != nil {
		m.replModel.width = contentW
		m.replModel.height = contentH
		// contentW already accounts for the sidebar; pass 0 to avoid double-subtracting.
		m.replModel.SetSidebarWidth(0)
		replM, replCmd := m.replModel.Update(contentMsg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		cmd = replCmd
	}

	if m.planModel != nil {
		m.planModel.SetDimensions(contentW, contentH)
	}
	if m.executeModel != nil {
		m.executeModel.width = contentW
		m.executeModel.height = contentH
	}
	if m.verifyModel != nil {
		m.verifyModel.width = contentW
		m.verifyModel.height = contentH
	}
	if m.metricsModel != nil {
		m.metricsModel.width = contentW
		m.metricsModel.height = contentH
	}
	if m.settingsModel != nil {
		m.settingsModel.width = contentW
		m.settingsModel.height = contentH
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetDimensions(contentW, contentH)
	}
	if m.msModel != nil {
		m.msModel.SetDimensions(contentW, contentH)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetDimensions(contentW, contentH)
	}
	if m.diffModel != nil {
		m.diffModel.SetDimensions(contentW, contentH)
	}
	if m.goalInput != nil {
		m.goalInput.SetDimensions(contentW, contentH)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetDimensions(contentW, contentH)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetDimensions(contentW, contentH)
	}
	if m.shipModel != nil {
		m.shipModel.width = contentW
		m.shipModel.height = contentH
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetDimensions(contentW, contentH)
	}
	if m.configModel != nil {
		m.configModel.width = contentW
		m.configModel.height = contentH
	}
	if m.discussModel != nil {
		m.discussModel.SetDimensions(contentW, contentH)
	}
	if m.helpModel != nil {
		m.helpModel.SetDimensions(contentW, contentH)
	}
	if m.bisectModel != nil {
		m.bisectModel.SetDimensions(contentW, contentH)
	}
	if m.notifModel != nil {
		m.notifModel.SetDimensions(contentW, contentH)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetDimensions(contentW, contentH)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetDimensions(contentW, contentH)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetDimensions(contentW, contentH)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetDimensions(contentW, contentH)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetDimensions(contentW, contentH)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetDimensions(contentW, contentH)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetDimensions(contentW, contentH)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetDimensions(contentW, contentH)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetDimensions(contentW, contentH)
	}
	if m.homeModel != nil {
		m.homeModel.SetDimensions(contentW, contentH)
	}

	// UX-38: Notify when sidebar auto-hides due to narrow terminal
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && msg.Width < WidthFull {
		if !m.sidebarAutoHideNotified {
			m.addToast("Sidebar hidden -- resize wider or press ctrl+b to toggle", "info")
			m.sidebarAutoHideNotified = true
		}
	} else if msg.Width >= WidthFull {
		m.sidebarAutoHideNotified = false
	}

	return cmd
}
