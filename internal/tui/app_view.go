package tui

import (
	"fmt"
	"time"

	"github.com/charmbracelet/lipgloss"
)

func (m *AppState) View() string {
	if !m.initialized || m.width == 0 {
		return "M31A — starting..."
	}

	if m.width < 40 || m.height < 10 {
		return fmt.Sprintf("Terminal too small: %dx%d (minimum 40x10)", m.width, m.height)
	}

	// Check if toast has expired
	if m.toastText != "" && time.Now().After(m.toastExpires) {
		m.toastText = ""
		m.toastType = ""
	}

	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel != nil {
			return m.firstRunModel.View()
		}
		return "Loading..."

	case ScreenPermission:
		if m.permissionModal != nil {
			return m.permissionModal.Render(m.width, m.height)
		}
		return "Permission screen error"

	case ScreenREPL:
		if m.replModel == nil {
			return "Loading..."
		}

		// Update REPL model with current app state for View() rendering
		m.replModel.SetKeyRegistry(m.keyRegistry)
		m.replModel.SetLastActivity(m.lastActivity)

		var mainContent string
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			sidebar := m.sidebarModel.View()
			replView := m.replModel.View()
			mainContent = lipgloss.JoinHorizontal(lipgloss.Top, replView, sidebar)
		} else {
			mainContent = m.replModel.View()
		}

		return m.renderWithPalette(mainContent)

	case ScreenSettings:
		if m.settingsModel != nil {
			return m.settingsModel.View()
		}
		return "Loading..."

	case ScreenResume:
		if m.resumeModel != nil {
			return m.resumeModel.View()
		}
		return "Loading..."

	case ScreenModelSelector:
		return m.modelSelector.View()

	case ScreenPlan:
		if m.planModel != nil {
			return m.planModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Plan screen — driven by workflow engine")

	case ScreenExecute:
		if m.executeModel != nil {
			return m.executeModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Execute screen — driven by workflow engine")

	case ScreenVerify:
		if m.verifyModel != nil {
			return m.verifyModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Verify screen — driven by workflow engine")

	case ScreenShip:
		if m.shipModel != nil {
			return m.shipModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Ship screen — driven by workflow engine")

	case ScreenDiff:
		if m.diffModel.lines != nil || m.diffModel.diff != "" {
			return m.renderToast(m.diffModel.View())
		}
		return m.renderToast(lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Loading diff..."))

	default:
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Unknown screen")
	}
}

func (m *AppState) renderToast(content string) string {
	if m.toastText == "" {
		return content
	}
	var color lipgloss.Color
	switch m.toastType {
	case "success":
		color = m.themeManager.Current().Success
	case "warning":
		color = m.themeManager.Current().Warning
	case "error":
		color = m.themeManager.Current().Error
	default:
		color = m.themeManager.Current().Thinking
	}
	toastStyle := lipgloss.NewStyle().
		Foreground(color).
		Background(m.themeManager.Current().Surface).
		Padding(0, 2).
		Bold(true)
	toast := toastStyle.Render(m.toastText)
	return lipgloss.JoinVertical(lipgloss.Center,
		lipgloss.PlaceHorizontal(m.width, lipgloss.Center, toast),
		content,
	)
}

func (m *AppState) renderWithPalette(base string) string {
	if m.cmdPaletteOpen && m.cmdPalette != nil {
		palette := m.cmdPalette.View()
		if palette != "" {
			return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, palette)
		}
	}
	return base
}
