package tui

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// renderWelcome renders the welcome screen content to be placed INSIDE the viewport.
//
// CRITICAL FIX: The welcome content must be centered within the viewport height,
// NOT the full terminal height. Using m.height (full terminal) causes the content
// to overflow the viewport boundary, creating the visual double-input bug.
//
// The viewport height = termHeight - topChrome - bottomChrome.
// We center content within viewportHeight(m.height), not m.height.
func (m *ReplModel) renderWelcome() string {
	if m.width == 0 || m.height == 0 {
		return "Welcome to M31A"
	}

	availWidth := m.replWidth()
	vpHeight := viewportHeight(m.height) // FIXED: use viewport height, not terminal height

	// 1. Logo (no starfield row — compact welcome)
	logo := m.renderLogo()

	// 2. Provider status card
	providerCard := m.renderProviderCard()

	// 3. Keyboard hints as muted single-line list
	hints := renderKeyboardHints(m.theme)

	// 4. Bottom bar with cwd and version
	bottomBar := m.renderBottomBar()

	// Stack vertically, centered horizontally
	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		providerCard,
		"",
		hints,
		"",
		bottomBar,
	)

	// Center in VIEWPORT space (not full terminal height)
	return lipgloss.Place(availWidth, vpHeight, lipgloss.Center, lipgloss.Center, content)
}

// renderProviderCard shows current model/provider status or setup prompt.
func (m *ReplModel) renderProviderCard() string {
	t := m.theme

	if m.activeModel == nil || m.activeProvider == "" {
		warningDot := lipgloss.NewStyle().Foreground(t.Warning).Render("●")
		title := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("No provider configured")
		subtitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Run /settings to get started")

		content := lipgloss.JoinVertical(lipgloss.Left,
			warningDot+" "+title,
			subtitle,
		)
		return components.Card{
			Content: content,
			Width:   42,
			Border:  theme.ThinBorder,
			Style:   components.CardWarning,
			Theme:   t,
		}.Render()
	}

	modelBadge := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).
		Render(m.activeModel.Name)
	providerBadge := components.NewBadge(m.activeProvider, components.BadgeBrandPreset, m.theme).Render()

	pricingText := ""
	if m.activeModel.Pricing.InputPerMToken > 0 || m.activeModel.Pricing.OutputPerMToken > 0 {
		pricingText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			"in $" + formatFloat(m.activeModel.Pricing.InputPerMToken) + "/M  " +
				"out $" + formatFloat(m.activeModel.Pricing.OutputPerMToken) + "/M",
		)
	}

	contextText := ""
	if m.activeModel.ContextLength > 0 {
		contextText = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("ctx " + components.FormatMetric(int(m.activeModel.ContextLength)))
	}

	parts := []string{modelBadge + " " + providerBadge}
	if pricingText != "" {
		parts = append(parts, pricingText)
	}
	if contextText != "" {
		parts = append(parts, contextText)
	}
	if m.sessionSparkline != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(m.sessionSparkline))
	}

	return components.Card{
		Content: lipgloss.JoinVertical(lipgloss.Left, parts...),
		Width:   42,
		Border:  theme.ThinBorder,
		Style:   components.CardSuccess,
		Theme:   t,
	}.Render()
}

// renderLogo renders the M31A ASCII art logo.
func (m *ReplModel) renderLogo() string {
	version := m.version
	if version == "" {
		version = "dev"
	}
	return components.RenderLogo(version, false, m.theme.Brand)
}

// renderKeyboardHints renders keyboard shortcut hints as a muted single-line list.
func renderKeyboardHints(t theme.Theme) string {
	hints := []string{"ctrl+p commands", "ctrl+b sidebar", "ctrl+x leader", "/help"}
	parts := make([]string, len(hints))
	for i, h := range hints {
		parts[i] = lipgloss.NewStyle().Foreground(t.TextMuted).Render(h)
	}
	return strings.Join(parts, "  ·  ")
}

// renderBottomBar renders a bottom bar with cwd and version.
func (m *ReplModel) renderBottomBar() string {
	t := m.theme

	cwdLabel := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(filepath.Base(m.cwd))

	version := m.version
	if version == "" {
		version = "dev"
	}
	versionLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render(version)

	spacer := m.width - lipgloss.Width(cwdLabel) - lipgloss.Width(versionLabel) - 4
	if spacer < 0 {
		spacer = 0
	}

	return lipgloss.JoinHorizontal(lipgloss.Left,
		"  "+cwdLabel,
		strings.Repeat(" ", spacer),
		versionLabel+"  ",
	)
}

// formatFloat formats a float64 to 2 decimal places.
func formatFloat(f float64) string {
	return fmt.Sprintf("%.2f", f)
}
