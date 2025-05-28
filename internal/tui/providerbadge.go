package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ProviderBadge returns the badge text and styled lipgloss.Style for a provider.
func ProviderBadge(t theme.Theme, provider string) (text string, style lipgloss.Style) {
	short := ProviderShortName(provider)
	var color lipgloss.Color
	switch provider {
	case "openrouter":
		color = t.Warning
	case "zen":
		color = t.Thinking
	default:
		color = t.TextSecondary
	}
	style = t.ModelBadge.Foreground(color)
	text = short
	return
}

// ProviderShortName returns a short uppercase identifier for a provider.
func ProviderShortName(provider string) string {
	switch provider {
	case "openrouter":
		return "OR"
	case "zen":
		return "ZEN"
	default:
		return strings.ToUpper(provider[:min(len(provider), 3)])
	}
}
