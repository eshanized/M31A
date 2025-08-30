package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Badge renders a small colored pill label.
type Badge struct {
	Label string
	Style lipgloss.Style
}

// Render returns the badge as a styled string.
// Format: [ Label ]
func (b Badge) Render() string {
	if b.Label == "" {
		return ""
	}

	label := " " + b.Label + " "
	return b.Style.Render("[" + label + "]")
}

// BadgePreset returns common badge configurations.
type BadgePreset int

const (
	BadgeSuccess BadgePreset = iota
	BadgeWarning
	BadgeError
	BadgeInfo
	BadgeBrand
	BadgeMuted
)

// NewBadge creates a badge from a preset using the provided theme.
func NewBadge(label string, preset BadgePreset, t theme.Theme) Badge {
	var style lipgloss.Style

	switch preset {
	case BadgeSuccess:
		style = t.SuccessBadge.Copy()
	case BadgeWarning:
		style = t.WarningBadge.Copy()
	case BadgeError:
		style = t.ErrorBadge.Copy()
	case BadgeInfo:
		style = lipgloss.NewStyle().
			Background(t.Thinking).
			Foreground(t.BadgeForeground).
			Padding(0, 1).
			Bold(true)
	case BadgeBrand:
		style = t.ModelBadge.Copy()
	case BadgeMuted:
		style = lipgloss.NewStyle().
			Background(t.Border).
			Foreground(t.TextSecondary).
			Padding(0, 1).
			Bold(true)
	default:
		style = lipgloss.NewStyle().
			Background(t.Surface).
			Foreground(t.TextPrimary).
			Padding(0, 1)
	}

	return Badge{Label: label, Style: style}
}

// RenderBadges renders multiple badges separated by a space.
func RenderBadges(badges []Badge) string {
	parts := make([]string, len(badges))
	for i, b := range badges {
		parts[i] = b.Render()
	}
	return strings.Join(parts, " ")
}

// CapabilityBadge returns a badge for a model capability.
func CapabilityBadge(capability string, t theme.Theme) Badge {
	style := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Padding(0, 1)
	return Badge{Label: capability, Style: style}
}

// StatusBadge returns a badge for a task/operation status.
func StatusBadge(status string, t theme.Theme) Badge {
	switch status {
	case "done", "pass", "complete":
		return NewBadge(status, BadgeSuccess, t)
	case "running", "thinking", "pending":
		return NewBadge(status, BadgeInfo, t)
	case "failed", "error":
		return NewBadge(status, BadgeError, t)
	case "warning", "skipped":
		return NewBadge(status, BadgeWarning, t)
	default:
		return NewBadge(status, BadgeMuted, t)
	}
}
