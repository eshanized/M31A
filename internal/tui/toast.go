package tui

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// maxVisibleToasts caps the number of toasts rendered simultaneously.
const maxVisibleToasts = 3

// renderToastStack renders up to 3 most recent toasts stacked.
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
	for i, toast := range show {
		rendered = append(rendered, renderSingleToast(toast, t, i, termWidth))
	}
	stack := lipgloss.JoinVertical(lipgloss.Right, rendered...)

	// Return raw stack — overlayToastOnContent handles right-alignment positioning
	return stack
}

// renderSingleToast renders one toast as a card with rounded border and colored accent.
func renderSingleToast(toast Toast, t theme.Theme, index int, toastWidth int) string {
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

	offset := 0
	if toast.Frame < 2 {
		offset = (2 - toast.Frame) * 10
	}

	// Card width: front toast widest, each subsequent 2 cols narrower
	contentWidth := 40 - (index * 2)
	if toastWidth > 0 {
		contentWidth = toastWidth/3 - (index * 2)
	}
	if contentWidth > 45 {
		contentWidth = 45
	}
	if contentWidth < 20 {
		contentWidth = 20
	}

	content := toast.Text
	if lipgloss.Width(content) > contentWidth-2 {
		content = content[:contentWidth-5] + "..."
	}

	// Rounded card with surface background
	toastContent := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Foreground(t.TextPrimary).
		Background(t.SurfaceElevated).
		Width(contentWidth).
		Render(content)

	if offset > 0 {
		toastContent = lipgloss.NewStyle().PaddingLeft(offset).Render(toastContent)
	}

	return toastContent
}
