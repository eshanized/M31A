package tui

import (
	"strings"
	"time"

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

// renderSingleToast renders one toast with a ThinBorder and colored left border.
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

	// Calculate slide-in offset based on frame
	offset := 0
	if toast.Frame < 2 {
		offset = (2 - toast.Frame) * 10
	}

	// Create progress bar for auto-dismiss
	progressBar := renderToastProgress(toast, t)

	// Stack offset: each toast 2 cols narrower than one above
	// Responsive width: scale with terminal, clamped to usable bounds
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

	content := icon + toast.Text
	if len(content) > contentWidth {
		content = content[:contentWidth-3] + "..."
	}

	toastContent := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(borderColor).
		Padding(0, 1).
		Foreground(t.TextPrimary).
		Width(contentWidth).
		Render(content)

	// Add progress bar below
	if progressBar != "" {
		toastContent += "\n" + progressBar
	}

	// Apply slide-in offset
	if offset > 0 {
		toastContent = lipgloss.NewStyle().PaddingLeft(offset).Render(toastContent)
	}

	toastContent = theme.RenderWithShadow(toastContent, t.ShadowColor, 1, 1)

	return toastContent
}

// renderToastProgress renders a progress bar for auto-dismiss
func renderToastProgress(toast Toast, t theme.Theme) string {
	elapsed := time.Since(toast.CreatedAt)
	duration := toast.Duration
	if duration <= 0 {
		duration = 5 * time.Second
	}

	progress := float64(elapsed) / float64(duration)
	if progress > 1 {
		progress = 1
	}
	if progress < 0 {
		progress = 0
	}

	// Create progress bar
	barWidth := 20
	filledWidth := int(progress * float64(barWidth))
	emptyWidth := barWidth - filledWidth

	// Choose color based on toast type
	var barColor lipgloss.Color
	switch toast.Type {
	case "success":
		barColor = t.Success
	case "error":
		barColor = t.Error
	case "warning":
		barColor = t.Warning
	default:
		barColor = t.Brand
	}

	// Render progress bar
	filled := lipgloss.NewStyle().
		Foreground(barColor).
		Render(strings.Repeat("━", filledWidth))
	empty := lipgloss.NewStyle().
		Foreground(t.Border).
		Render(strings.Repeat("─", emptyWidth))

	return lipgloss.NewStyle().
		PaddingLeft(2).
		Render(filled + empty)
}
