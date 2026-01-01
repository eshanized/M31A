package tui

import (
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Toast represents a transient notification overlay.
type Toast struct {
	ID        int
	Text      string
	Type      string // "success", "error", "warning", "info"
	CreatedAt time.Time
}

// maxVisibleToasts caps the number of toasts rendered simultaneously.
const maxVisibleToasts = 3

// renderToastStack renders up to 3 most recent toasts stacked top-right.
func renderToastStack(toasts []Toast, t theme.Theme, termWidth int) string {
	if len(toasts) == 0 {
		return ""
	}
	// Show at most maxVisibleToasts
	show := toasts
	if len(show) > maxVisibleToasts {
		show = show[len(show)-maxVisibleToasts:]
	}

	var rendered []string
	for _, toast := range show {
		rendered = append(rendered, renderSingleToast(toast, t))
	}
	stack := lipgloss.JoinVertical(lipgloss.Right, rendered...)

	// Position in top-right: right-align with 2-char right margin
	return lipgloss.PlaceHorizontal(termWidth, lipgloss.Right,
		lipgloss.NewStyle().MarginRight(2).Render(stack))
}

// renderSingleToast renders one toast with a ThinBorder and colored left border.
func renderSingleToast(toast Toast, t theme.Theme) string {
	var borderColor lipgloss.Color
	switch toast.Type {
	case "success":
		borderColor = t.Success
	case "error":
		borderColor = t.Error
	case "warning":
		borderColor = t.Warning
	default:
		borderColor = t.Brand
	}

	// Icon prefix
	icon := ""
	switch toast.Type {
	case "success":
		icon = "✓ "
	case "error":
		icon = "✗ "
	case "warning":
		icon = "⚠ "
	default:
		icon = "● "
	}

	return lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(borderColor).
		Padding(0, 1).
		Foreground(t.TextPrimary).
		Render(icon + toast.Text)
}
