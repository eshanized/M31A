package settings

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// settings_view.go — view helpers for the Settings screen.
// The main View() method and tab content renderers are in settings_model.go.

// renderSettingCard wraps tab content in a ThinBorder card with the given title.
func renderSettingCard(t theme.Theme, title string, content string, width int) string {
	divWidth := width - 6
	if divWidth < 4 {
		divWidth = 4
	}
	divider := components.SectionDivider{
		Title: title,
		Width: divWidth,
		Theme: t,
	}.Render()
	body := lipgloss.NewStyle().PaddingLeft(2).Render(content)
	inner := lipgloss.JoinVertical(lipgloss.Left, "", divider, "", body)

	return lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(t.Border).
		Padding(0, 1).
		Width(max(4, width-4)).
		Render(inner)
}

// maskedKey returns a masked version of an API key for display.
// Shows only the last 4 characters, e.g. "••••••••abcd".
func maskedKey(key string) string {
	if key == "" {
		return "(not set)"
	}
	tail := key
	if len(tail) > 4 {
		tail = key[len(key)-4:]
	}
	return fmt.Sprintf("••••••••%s", tail)
}

// renderLeftNav renders the left navigation sidebar for settings tabs.
func (s *SettingsModel) renderLeftNav() string {
	t := s.theme
	tabNames := []string{"Provider", "Model", "UI", "Keys", "Workflow", "About"}
	var items []string
	for i, name := range tabNames {
		style := lipgloss.NewStyle().PaddingLeft(2).PaddingRight(2)
		if s.activeTab == SettingsTab(i) {
			style = style.Background(t.Brand).Foreground(t.TextPrimary)
		} else {
			style = style.Foreground(t.TextMuted)
		}
		items = append(items, style.Render(name))
	}
	return lipgloss.JoinVertical(lipgloss.Left, items...)
}

// renderSectionHeader renders a section header for settings content.
func renderSectionHeader(title string, width int) string {
	t := theme.Default()
	return lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(2).
		Width(width - 4).
		Render(title)
}
