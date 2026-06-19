package components

import (
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ShortcutTip renders a floating tooltip showing keyboard shortcuts.
// It auto-dismisses after a configurable duration.
type ShortcutTip struct {
	Binding   string // e.g. "ctrl+p"
	Label     string // e.g. "command palette"
	CreatedAt time.Time
	Duration  time.Duration
	Theme     theme.Theme
	Width     int
}

// IsVisible returns true if the tip hasn't expired yet.
func (s ShortcutTip) IsVisible() bool {
	if s.Duration <= 0 {
		s.Duration = 3 * time.Second
	}
	return time.Since(s.CreatedAt) < s.Duration
}

// Render returns the shortcut tip as a styled string.
func (s ShortcutTip) Render() string {
	if !s.IsVisible() {
		return ""
	}

	t := s.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	binding := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(s.Binding)
	label := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" — " + s.Label)

	content := binding + label

	w := s.Width
	if w < 20 {
		w = lipgloss.Width(content) + 4
	}

	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.BorderSubtle).
		Background(t.SurfaceElevated).
		Padding(0, 1).
		Width(w)

	return style.Render(content)
}

// RenderShortcutBar renders a horizontal bar of shortcut hints.
func RenderShortcutBar(shortcuts []struct{ Key, Label string }, width int, t theme.Theme) string {
	if len(shortcuts) == 0 {
		return ""
	}

	var parts []string
	for _, s := range shortcuts {
		key := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(s.Key)
		label := lipgloss.NewStyle().Foreground(t.TextMuted).Render(s.Label)
		parts = append(parts, key+" "+label)
	}

	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render("  │  ")
	result := strings.Join(parts, sep)

	resultW := lipgloss.Width(result)
	if resultW < width {
		result += strings.Repeat(" ", width-resultW)
	}

	return result
}
