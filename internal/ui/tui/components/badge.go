package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ─── BadgeType-based badge (plan 30-06) ─────────────────────────────────────
// BadgeType represents the type/purpose of a badge.
type BadgeType int

const (
	BadgeBrand BadgeType = iota
	BadgeSuccess
	BadgeError
	BadgeWarning
	BadgeInfo
	BadgeNeutral
)

// SimpleBadge is a lightweight badge that renders colored text without backgrounds.
type SimpleBadge struct {
	Text    string
	Type    BadgeType
	Compact bool // compact = no padding, for inline use
	Theme   theme.Theme
}

// Render returns the badge as a styled string.
func (b SimpleBadge) Render() string {
	s := theme.BuildSemanticStyles(b.Theme)
	style := lipgloss.NewStyle().Bold(true)
	if !b.Compact {
		style = style.PaddingLeft(1).PaddingRight(1)
	}
	switch b.Type {
	case BadgeBrand:
		return style.Foreground(b.Theme.Brand).Render(b.Text)
	case BadgeSuccess:
		return s.BadgeSuccess.Render("✓ " + b.Text)
	case BadgeError:
		return s.BadgeError.Render("✗ " + b.Text)
	case BadgeWarning:
		return s.BadgeWarning.Render("⚠ " + b.Text)
	case BadgeInfo:
		return s.BadgeInfo.Render(b.Text)
	default:
		return style.Foreground(b.Theme.TextMuted).Render(b.Text)
	}
}

// ─── Existing badge system (preserved for backward compat) ───────────────────

// Badge renders a small colored pill label.
type Badge struct {
	Label string
	Style lipgloss.Style
}

// Render returns the legacy badge as a styled string.
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
	BadgeSuccessPreset BadgePreset = iota
	BadgeWarningPreset
	BadgeErrorPreset
	BadgeInfoPreset
	BadgeBrandPreset
	BadgeMutedPreset
)

// NewBadge creates a badge from a preset using the provided theme.
func NewBadge(label string, preset BadgePreset, t theme.Theme) Badge {
	s := theme.BuildSemanticStyles(t)
	var style lipgloss.Style

	switch preset {
	case BadgeSuccessPreset:
		style = s.BadgeSuccess
	case BadgeWarningPreset:
		style = s.BadgeWarning
	case BadgeErrorPreset:
		style = s.BadgeError
	case BadgeInfoPreset:
		style = s.BadgeInfo
	case BadgeBrandPreset:
		style = s.BadgeBrand
	case BadgeMutedPreset:
		style = s.BadgeMuted
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
	s := theme.BuildSemanticStyles(t)
	return Badge{Label: capability, Style: s.SecondaryText.Padding(0, 1)}
}

// StatusBadge returns a badge for a task/operation status.
func StatusBadge(status string, t theme.Theme) Badge {
	switch status {
	case "done", "pass", "complete":
		return NewBadge(status, BadgeSuccessPreset, t)
	case "running", "thinking", "pending":
		return NewBadge(status, BadgeInfoPreset, t)
	case "failed", "error":
		return NewBadge(status, BadgeErrorPreset, t)
	case "warning", "skipped":
		return NewBadge(status, BadgeWarningPreset, t)
	default:
		return NewBadge(status, BadgeMutedPreset, t)
	}
}
