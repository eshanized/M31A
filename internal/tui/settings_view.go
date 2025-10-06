package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// View renders the settings screen.
func (m SettingsModel) View() string {
	if m.width == 0 || m.height == 0 {
		return m.spinner.View() + " Loading..."
	}

	tabBar := m.renderTabBar()
	content := m.renderTwoColumnContent()
	footer := m.renderFooter()

	mainView := lipgloss.JoinVertical(lipgloss.Top, tabBar, "", content, "", footer)

	if m.showUnsavedWarning {
		return m.renderUnsavedWarning(mainView)
	}

	return mainView
}

// renderUnsavedWarning renders an unsaved changes confirmation modal over the settings view.
func (m SettingsModel) renderUnsavedWarning(bg string) string {
	title := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true).
		Render("Unsaved Changes")

	hint := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[Y] Save  [N] Discard  [Esc] Cancel")

	warningBox := lipgloss.NewStyle().
		Border(lipgloss.DoubleBorder()).
		BorderForeground(m.theme.Warning).
		Padding(1, 2).
		Width(50).
		Render(lipgloss.JoinVertical(lipgloss.Left, title, "", hint))

	// Center the modal
	centeredModal := lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, warningBox)

	return lipgloss.JoinVertical(lipgloss.Top, bg, centeredModal)
}

// renderTabBar renders the tab headers row with icons and underline.
func (m SettingsModel) renderTabBar() string {
	var tabs []string
	for i := 0; i < int(tabCount); i++ {
		tab := settingsTab(i)
		name := tabNames[tab]
		icon := tabIcons[tab]

		var style lipgloss.Style
		if tab == m.activeTab {
			style = lipgloss.NewStyle().
				Foreground(lipgloss.Color(m.theme.Brand)).
				Padding(0, 1).
				Bold(true)
		} else {
			style = lipgloss.NewStyle().
				Foreground(lipgloss.Color(m.theme.TextSecondary)).
				Padding(0, 1)
		}
		tabs = append(tabs, style.Render(icon+" "+name))
	}

	tabBar := lipgloss.JoinHorizontal(lipgloss.Top, tabs...)

	// Add underline for active tab
	underlineStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand))
	underline := underlineStyle.Render(strings.Repeat("━", m.width-4))

	return lipgloss.JoinVertical(lipgloss.Left, tabBar, underline)
}

// renderTwoColumnContent renders the two-column layout: fields on left, description pane on right.
func (m SettingsModel) renderTwoColumnContent() string {
	leftColumn := m.renderFieldColumn()
	rightColumn := m.renderDescriptionPane()

	leftWidth := m.width * 60 / 100
	rightWidth := m.width - leftWidth - 4

	leftStyled := lipgloss.NewStyle().
		Width(leftWidth).
		Render(leftColumn)
	rightStyled := lipgloss.NewStyle().
		Width(rightWidth).
		Render(rightColumn)

	return lipgloss.JoinHorizontal(lipgloss.Top, leftStyled, rightStyled)
}

// renderFieldColumn is defined in settings_tabs.go

// renderDescriptionPane renders the description pane for the focused field.
func (m SettingsModel) renderDescriptionPane() string {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return ""
	}

	f := fields[m.focusedField]
	var lines []string

	// Field name in bold
	fieldNameStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true)
	lines = append(lines, fieldNameStyle.Render(f.label))
	lines = append(lines, "")

	// Separator
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Border)).
		Render(strings.Repeat("─", 30)))

	// Description text
	desc := fieldDescriptions[f.key]
	if desc == "" {
		desc = "No description available."
	}
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render(desc))
	lines = append(lines, "")

	// Current value
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Bold(true).
		Render("Current:"))
	val := f.value
	if f.masked && f.value != "" {
		val = "••••••••"
	}
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Render("  "+val))

	// Mini dropdown for enum fields
	if f.fieldType == "string" && f.key == "ui.theme" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render("Options:"))
		options := []string{"dark", "light", "auto"}
		for _, opt := range options {
			if opt == f.value {
				lines = append(lines, lipgloss.NewStyle().
					Foreground(lipgloss.Color(m.theme.Brand)).
					Render("  ◆ "+opt+" ←"))
			} else {
				lines = append(lines, lipgloss.NewStyle().
					Foreground(lipgloss.Color(m.theme.TextSecondary)).
					Render("  ○ "+opt))
			}
		}
	}

	if f.fieldType == "string" && f.key == "provider.default" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render("Options:"))
		options := []string{"openrouter", "zen"}
		for _, opt := range options {
			if opt == f.value {
				lines = append(lines, lipgloss.NewStyle().
					Foreground(lipgloss.Color(m.theme.Brand)).
					Render("  ◆ "+opt+" ←"))
			} else {
				lines = append(lines, lipgloss.NewStyle().
					Foreground(lipgloss.Color(m.theme.TextSecondary)).
					Render("  ○ "+opt))
			}
		}
	}

	content := strings.Join(lines, "\n")

	boxStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(m.theme.Border)).
		Padding(1, 1).
		Width(m.width*40/100 - 4)

	return boxStyle.Render(content)
}

// renderActiveTab and renderField are defined in settings_tabs.go

// renderFooter renders the footer with key hints, unsaved indicator, and status.
func (m SettingsModel) renderFooter() string {
	helpStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary)).Faint(true)
	footer := helpStyle.Render("[Tab/←→] Navigate  [↑↓] Select  [Enter] Edit/Toggle  [Esc] Back  [Ctrl+S] Save")

	// Unsaved indicator
	if m.dirty {
		dirtyStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Warning)).Bold(true)
		// Count dirty fields
		dirtyCount := 0
		for _, fields := range m.fields {
			for _, f := range fields {
				if f.value != f.original {
					dirtyCount++
				}
			}
		}
		footer = dirtyStyle.Render(fmt.Sprintf("● %d unsaved", dirtyCount)) + "  " + footer
	}
	if m.statusMsg != "" {
		statusStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Success))
		footer = statusStyle.Render(m.statusMsg) + "\n" + footer
	}
	if m.err != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		footer = errStyle.Render(m.err) + "\n" + footer
	}

	return footer
}

// sectionHeader renders a section title with theme styling.
func (m SettingsModel) sectionHeader(label string) string {
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true).
		Render(label)
}
