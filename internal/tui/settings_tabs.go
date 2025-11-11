package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// settings_tabs.go — left sidebar tab navigation for the Settings screen.

// renderLeftNav renders a vertical left sidebar with tab names.
// The active tab is highlighted with a brand-colored ▍ indicator.
func (s *SettingsModel) renderLeftNav() string {
	t := s.theme
	var items []string
	for i, name := range settingsTabNames {
		tab := SettingsTab(i)
		if tab == s.activeTab {
			items = append(items, lipgloss.NewStyle().
				Foreground(t.Brand).
				Bold(true).
				Render("▍ "+name))
		} else {
			items = append(items, lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3).
				Render(name))
		}
	}
	return strings.Join(items, "\n")
}
