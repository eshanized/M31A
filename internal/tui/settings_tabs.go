package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
)

// settings_tabs.go — left sidebar tab navigation for the Settings screen.

// renderLeftNav renders a vertical left sidebar with tab names.
// The active tab is highlighted with a branded badge.
func (s *SettingsModel) renderLeftNav() string {
	t := s.theme
	var items []string
	for i, name := range settingsTabNames {
		tab := SettingsTab(i)
		if tab == s.activeTab {
			badge := components.SimpleBadge{
				Text:    "▍ " + name,
				Type:    components.BadgeBrand,
				Compact: true,
				Theme:   t,
			}
			items = append(items, badge.Render())
		} else {
			items = append(items, lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3).
				Render(name))
		}
	}
	return strings.Join(items, "\n")
}
