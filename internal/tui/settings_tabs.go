package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// renderGeneralTab renders the General tab.
func (m SettingsModel) renderGeneralTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("General Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabGeneral] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  Theme: \"auto\", \"dark\", or \"light\""),
	)

	return strings.Join(lines, "\n")
}

// renderProviderTab renders the Provider tab.
func (m SettingsModel) renderProviderTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Provider Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabProvider] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  API keys are masked by default. Press Enter to reveal."),
	)
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  Store keys in OS keychain via Settings on save."),
	)

	return strings.Join(lines, "\n")
}

// renderModelTab renders the Model tab.
func (m SettingsModel) renderModelTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Model Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabModel] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	return strings.Join(lines, "\n")
}

// renderPermissionsTab renders the Permissions tab.
func (m SettingsModel) renderPermissionsTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Permissions Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabPermissions] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	// Read-only rules count
	ruleText := fmt.Sprintf("%d rule(s) configured", len(m.config.Permissions.Rules))
	if len(m.config.Permissions.Rules) == 0 {
		ruleText = "No permission rules configured"
	}
	lines = append(lines, "  "+lipgloss.NewStyle().
		Width(30).Align(lipgloss.Right).PaddingRight(1).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Render("Permission Rules")+"  "+
		lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render(ruleText),
	)

	return strings.Join(lines, "\n")
}

// renderFeaturesTab renders the Features tab.
func (m SettingsModel) renderFeaturesTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Features Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabFeatures] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	return strings.Join(lines, "\n")
}

// renderLedgerTab renders the Ledger tab with stats.
func (m SettingsModel) renderLedgerTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Ledger Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabLedger] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	// Stats section
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true).
		Render("  Ledger Statistics"),
	)

	if m.ledger != nil {
		stats := m.ledger.Stats()
		lines = append(lines, fmt.Sprintf("  Total Sessions:    %d", stats.TotalSessions))
		lines = append(lines, fmt.Sprintf("  Avg Tasks/Session: %.1f", stats.AvgTaskCount))
		lines = append(lines, fmt.Sprintf("  Avg Duration:      %.0f min", stats.AvgDurationMinutes))
		lines = append(lines, fmt.Sprintf("  Avg Cost:          $%.4f", stats.AvgCost))
		lines = append(lines, fmt.Sprintf("  Total Failed:      %d", stats.TotalFailedTasks))

		if len(stats.TopFrameworks) > 0 {
			lines = append(lines, fmt.Sprintf("  Top Frameworks:    %s", strings.Join(stats.TopFrameworks, ", ")))
		}
		if len(stats.TopFailures) > 0 {
			lines = append(lines, fmt.Sprintf("  Top Failures:      %s", strings.Join(stats.TopFailures, ", ")))
		}
		if len(stats.ByProjectType) > 0 {
			lines = append(lines, "")
			lines = append(lines, lipgloss.NewStyle().
				Foreground(lipgloss.Color(m.theme.TextSecondary)).
				Render("  By Project Type:"),
			)
			for ptype, count := range stats.ByProjectType {
				if ptype == "" {
					ptype = "(unknown)"
				}
				lines = append(lines, fmt.Sprintf("    %s: %d", ptype, count))
			}
		}
	} else {
		lines = append(lines, "  "+lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render("Ledger not initialized"),
		)

	}

	return strings.Join(lines, "\n")
}

// renderActiveTab dispatches to the correct tab renderer.
func (m SettingsModel) renderActiveTab() string {
	switch m.activeTab {
	case tabGeneral:
		return m.renderGeneralTab()
	case tabProvider:
		return m.renderProviderTab()
	case tabModel:
		return m.renderModelTab()
	case tabPermissions:
		return m.renderPermissionsTab()
	case tabFeatures:
		return m.renderFeaturesTab()
	case tabLedger:
		return m.renderLedgerTab()
	default:
		return ""
	}
}

// renderFieldColumn renders the field list for the current tab.
func (m SettingsModel) renderFieldColumn() string {
	var lines []string
	lines = append(lines, m.sectionHeader(tabNames[m.activeTab]+" Settings"))
	lines = append(lines, "")

	fields := m.fields[m.activeTab]
	for i, f := range fields {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	return strings.Join(lines, "\n")
}

// renderField renders a single editable field with label and value.
func (m SettingsModel) renderField(f *editableField, isFocused bool) string {
	labelStyle := lipgloss.NewStyle().Width(30).Align(lipgloss.Right).PaddingRight(1).
		Foreground(lipgloss.Color(m.theme.TextPrimary))

	label := labelStyle.Render(f.label)

	var value string
	if isFocused && f.editing {
		// Editing mode — show current value with cursor marker
		editingStyle := lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.Success)).
			Background(lipgloss.Color(m.theme.Surface))
		val := f.value
		if val == "" {
			val = " "
		}
		value = editingStyle.Render(">" + val + "<")
	} else if f.masked {
		// Masked display
		mutedStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
		if f.value == "" {
			value = mutedStyle.Render("(not set)")
		} else {
			masked := strings.Repeat(f.maskChar, 8)
			value = mutedStyle.Render(masked)
			if isFocused {
				value += " " + mutedStyle.Render("(Enter to reveal)")
			}
		}
	} else if isFocused {
		// Focused but not editing
		focusedStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Brand))
		if f.value == "" {
			value = focusedStyle.Render("(not set)")
		} else {
			value = focusedStyle.Render(f.value)
		}
	} else {
		// Normal display
		valStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextPrimary))
		if f.value == "" {
			value = lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary)).Render("(not set)")
		} else {
			value = valStyle.Render(f.value)
		}
	}

	// Apply dirty indicator
	if isFocused && m.dirty {
		dirtyStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Warning)).Bold(true)
		return dirtyStyle.Render("▶ ") + label + "  " + value
	}

	if isFocused {
		return "▶ " + label + "  " + value
	}
	return "  " + label + "  " + value
}
