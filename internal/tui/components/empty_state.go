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
	// OnActivate is an optional callback invoked when the user presses Enter
	// on this action. If nil, the action is display-only.
	OnActivate func()
}

// EmptyState renders a branded, illustrated empty state with icon, title,
// description, and actionable suggestions. Actions are keyboard-navigable
// using j/k or up/down to move and Enter to activate (D-18, D-44).
type EmptyState struct {
	Icon         string // Unicode icon (e.g., "◈")
	Title        string
	Subtitle     string
	Actions      []Action
	FocusedIndex int // which action is focused (-1 = none)
	Theme        theme.Theme
	Width        int
	Height       int
}

// MoveFocusDown moves the focus to the next action.
func (e *EmptyState) MoveFocusDown() {
	if len(e.Actions) == 0 {
		return
	}
	if e.FocusedIndex < 0 {
		e.FocusedIndex = 0
	} else if e.FocusedIndex < len(e.Actions)-1 {
		e.FocusedIndex++
	}
}

// MoveFocusUp moves the focus to the previous action.
func (e *EmptyState) MoveFocusUp() {
	if len(e.Actions) == 0 {
		return
	}
	if e.FocusedIndex > 0 {
		e.FocusedIndex--
	}
}

// ActivateFocused invokes the OnActivate callback of the currently focused
// action. Returns true if an action was activated.
func (e *EmptyState) ActivateFocused() bool {
	if e.FocusedIndex < 0 || e.FocusedIndex >= len(e.Actions) {
		return false
	}
	a := e.Actions[e.FocusedIndex]
	if a.OnActivate != nil {
		a.OnActivate()
		return true
	}
	return false
}

// Render returns the empty state as a centered string.
func (e EmptyState) Render() string {
	t := e.Theme
	if t.Brand == "" {
		t = theme.Default()
	}
	s := theme.BuildSemanticStyles(t)

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
	iconLine := s.EmptyStateIcon.Render(icon + "  M 3 1 A")

	// Title
	titleLine := ""
	if e.Title != "" {
		titleLine = s.EmptyStateTitle.Render(e.Title)
	}

	// Subtitle
	subtitleLine := ""
	if e.Subtitle != "" {
		subtitleLine = s.EmptyStateHint.Render(e.Subtitle)
	}

	// Actions with keyboard focus indicator
	var actionLines []string
	if len(e.Actions) > 0 {
		var items []string
		for i, a := range e.Actions {
			// Show focus indicator: arrow prefix for focused, space for others
			prefix := "  "
			if i == e.FocusedIndex {
				prefix = "→ "
			}

			var label string
			if i == e.FocusedIndex {
				label = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
					Render(prefix + a.Label)
			} else {
				label = s.EmptyStateAction.Render(prefix + a.Label)
			}

			hint := ""
			if a.Hint != "" {
				hint = s.EmptyStateHint.Italic(true).Render("  · " + a.Hint)
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

	// Keyboard hint — shows navigation instructions when actions exist
	hintText := "Type a message or press ctrl+p for commands"
	if len(e.Actions) > 0 {
		hintText = "j/k navigate  Enter select  Esc close"
	}
	hintLine := s.EmptyStateHint.Render(hintText)

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
