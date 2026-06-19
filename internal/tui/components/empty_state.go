package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Action represents a suggested action in an empty state.
type Action struct {
	Label string
	Hint  string
}

// EmptyState renders a branded, illustrated empty state with icon, title,
// description, and actionable suggestions.
type EmptyState struct {
	Icon     string   // Unicode icon (e.g., "◈")
	Title    string
	Subtitle string
	Actions  []Action
	Theme    theme.Theme
	Width    int
	Height   int
}

// Render returns the empty state as a centered string.
func (e EmptyState) Render() string {
	t := e.Theme
	if t.Brand == "" {
		t = theme.Default()
	}

	w := e.Width
	if w < 20 {
		w = 40
	}
	h := e.Height
	if h < 3 {
		h = 10
	}

	icon := e.Icon
	if icon == "" {
		icon = "◈"
	}

	// Icon + brand
	iconLine := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render(icon + "  M 3 1 A")

	// Title
	titleLine := ""
	if e.Title != "" {
		titleLine = lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).
			Render(e.Title)
	}

	// Subtitle
	subtitleLine := ""
	if e.Subtitle != "" {
		subtitleLine = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(e.Subtitle)
	}

	// Actions
	var actionLines []string
	if len(e.Actions) > 0 {
		// Build action card
		var items []string
		for _, a := range e.Actions {
			label := lipgloss.NewStyle().Foreground(t.TextPrimary).Render("▸ " + a.Label)
			hint := ""
			if a.Hint != "" {
				hint = lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).
					Render("  · " + a.Hint)
			}
			items = append(items, "  "+label+hint)
		}

		innerW := w - 6
		if innerW < 20 {
			innerW = 20
		}
		if innerW > 50 {
			innerW = 50
		}

		actionsBlock := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.BorderSubtle).
			Width(innerW).
			Padding(0, 1).
			Render(strings.Join(items, "\n"))
		actionLines = append(actionLines, actionsBlock)
	}

	// Keyboard hint
	hintLine := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Type a message or press ctrl+p for commands")

	// Compose vertically
	var parts []string
	parts = append(parts, iconLine)
	if titleLine != "" {
		parts = append(parts, "")
		parts = append(parts, titleLine)
	}
	if subtitleLine != "" {
		parts = append(parts, "")
		parts = append(parts, subtitleLine)
	}
	if len(actionLines) > 0 {
		parts = append(parts, "")
		parts = append(parts, actionLines...)
	}
	parts = append(parts, "")
	parts = append(parts, hintLine)

	content := lipgloss.JoinVertical(lipgloss.Center, parts...)
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}
