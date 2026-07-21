package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ConfirmDialog renders a reusable Y/N confirmation modal.
type ConfirmDialog struct {
	Title       string
	Message     string
	ConfirmText string
	CancelText  string
	Danger      bool
	Warning     bool
	Theme       theme.Theme
	Width       int
}

// View renders the confirmation dialog centered in the given dimensions.
func (cd ConfirmDialog) View(screenW, screenH int) string {
	t := cd.Theme
	s := theme.BuildSemanticStyles(t)

	confirmText := cd.ConfirmText
	if confirmText == "" {
		confirmText = "Yes"
	}
	cancelText := cd.CancelText
	if cancelText == "" {
		cancelText = "No"
	}

	body := lipgloss.JoinVertical(lipgloss.Left,
		s.Body.Width(cd.Width-4).Render(cd.Message),
		"",
		s.KeyboardHint.Render(
			"[y] "+confirmText+"   [n/esc] "+cancelText,
		),
	)

	var card lipgloss.Style
	switch {
	case cd.Danger:
		card = s.DialogDanger
	case cd.Warning:
		card = s.DialogWarning
	default:
		card = s.Dialog
	}

	return lipgloss.Place(screenW, screenH, lipgloss.Center, lipgloss.Center,
		card.Width(cd.Width).Render(body))
}
