package tui

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// maxVisibleToasts caps the number of toasts rendered simultaneously.
// Set via SetMaxVisibleToasts during TUI initialization from config.
var maxVisibleToasts = 3

// toastOverrides holds config-based toast type overrides.
// Set via SetToastOverrides during TUI initialization from config.
var toastOverrides map[string]config.ToastTypeConfig

// SetMaxVisibleToasts updates the maximum number of visible toasts.
// Called during TUI initialization with cfg.UI.ToastMaxVisible.
func SetMaxVisibleToasts(n int) {
	if n > 0 {
		maxVisibleToasts = n
	}
}

// SetToastOverrides sets custom toast type icon/title overrides from config.
// Called during TUI initialization with cfg.UI.ToastTypeOverrides.
func SetToastOverrides(overrides map[string]config.ToastTypeConfig) {
	toastOverrides = overrides
}

// getToastConfig returns the icon and title for a toast type,
// applying defaults first, then overlaying any config overrides.
func getToastConfig(toastType string) (icon, title string) {
	// Start with defaults
	switch toastType {
	case "success":
		icon, title = "✓", "Success"
	case "error":
		icon, title = "✗", "Error"
	case "warning":
		icon, title = "⚠", "Warning"
	default:
		icon, title = "ℹ", "Info"
	}

	// Apply overrides (partial overrides work correctly)
	if override, ok := toastOverrides[toastType]; ok {
		if override.Icon != "" {
			icon = override.Icon
		}
		if override.Title != "" {
			title = override.Title
		}
	}
	return icon, title
}

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
	icon, title := getToastConfig(toast.Type)
	switch toast.Type {
	case "success":
		borderColor = t.Success
		icon = lipgloss.NewStyle().Foreground(t.Success).Render(icon)
	case "error":
		borderColor = t.Error
		icon = lipgloss.NewStyle().Foreground(t.Error).Render(icon)
	case "warning":
		borderColor = t.Warning
		icon = lipgloss.NewStyle().Foreground(t.Warning).Render(icon)
	default:
		borderColor = t.Brand
		icon = lipgloss.NewStyle().Foreground(t.Brand).Render(icon)
	}

	offset := 0
	if toast.Frame < 2 {
		offset = (2 - toast.Frame) * 10
	}

	// Card width: all toasts use the same width (no shrinking for stacked toasts)
	contentWidth := 50
	if toastWidth > 0 {
		contentWidth = toastWidth / 3
	}
	if contentWidth > 50 {
		contentWidth = 50
	}
	if contentWidth < 25 {
		contentWidth = 25
	}

	// Build content with icon, title, and message
	titleStyle := lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Bold(true)
	msgStyle := lipgloss.NewStyle().
		Foreground(t.TextSecondary)

	content := icon + " " + titleStyle.Render(title)
	if toast.Text != "" {
		msgText := toast.Text
		if lipgloss.Width(msgText) > contentWidth-6 {
			msgText = TruncateWithEllipsis(msgText, contentWidth-6)
		}
		content += "\n" + msgStyle.Render("  "+msgText)
	}

	// Add action button if present
	if toast.Action != nil && toast.ActionLabel != "" {
		actionStyle := lipgloss.NewStyle().
			Foreground(t.Brand).
			Bold(true)
		content += "\n" + actionStyle.Render("  ["+toast.ActionLabel+"]")
	}

	// Rounded card with surface background and shadow hint
	toastContent := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Background(t.SurfaceElevated).
		Width(contentWidth).
		Render(content)

	if offset > 0 {
		toastContent = lipgloss.NewStyle().PaddingLeft(offset).Render(toastContent)
	}

	return toastContent
}
