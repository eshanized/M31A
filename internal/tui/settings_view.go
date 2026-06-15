package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
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
