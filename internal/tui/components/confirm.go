package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
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

	confirmText := cd.ConfirmText
	if confirmText == "" {
		confirmText = "Yes"
	}
	cancelText := cd.CancelText
	if cancelText == "" {
		cancelText = "No"
	}

	body := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.Text).Width(cd.Width-4).Render(cd.Message),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			"[y] "+confirmText+"   [n/esc] "+cancelText,
		),
	)

	borderColor := t.Brand
	if cd.Danger {
		borderColor = t.Error
	} else if cd.Warning {
		borderColor = t.Warning
	}

	cardStyle := CardBrand
	if cd.Warning {
		cardStyle = CardWarning
	}

	card := Card{
		Title:   cd.Title,
		Content: body,
		Width:   cd.Width,
		Border:  theme.NormalBorder,
		Style:   cardStyle,
		Theme:   t,
	}.Render()

	if cd.Danger {
		card = lipgloss.NewStyle().
			Border(theme.NormalBorder).
			BorderForeground(borderColor).
			Padding(1, 2).
			Width(cd.Width).
			Render(body)
	}

	return lipgloss.Place(screenW, screenH, lipgloss.Center, lipgloss.Center, card)
}
