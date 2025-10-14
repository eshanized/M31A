package tui

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func (m *ReplModel) renderWelcome() string {
	if m.width == 0 || m.height == 0 {
		return "Welcome to M31A"
	}

	// 1. Logo (compact, 4 lines)
	logo := m.renderLogo()

	// 2. Provider status card
	providerCard := m.renderProviderCard()

	// 3. Keyboard hints
	hints := renderKeyboardHints(m.theme)

	// Stack vertically, centered (no input box — the real textarea is below the viewport)
	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		providerCard,
		"",
		hints,
	)

	// Center in available space (account for sidebar width)
	availableWidth := m.width - m.sidebarWidth
	if availableWidth < 20 {
		availableWidth = 20
	}
	return lipgloss.Place(availableWidth, m.height, lipgloss.Center, lipgloss.Center, content)
}

// renderProviderCard shows current model/provider status or setup prompt.
func (m *ReplModel) renderProviderCard() string {
	t := m.theme

	if m.activeModel == nil || m.activeProvider == "" {
		// Not configured - show setup prompt
		style := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Warning).
			Padding(0, 2).
			Width(40)

		warningDot := lipgloss.NewStyle().Foreground(t.Warning).Render("●")
		title := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("No provider configured")
		subtitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Run /settings to get started")

		content := lipgloss.JoinVertical(lipgloss.Left,
			warningDot+" "+title,
			subtitle,
		)

		return style.Render(content)
	}

	// Configured - show provider info
	style := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Success).
		Padding(0, 2).
		Width(40)

	// Model name
	modelStyle := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	modelBadge := modelStyle.Render(m.activeModel.Name)

	// Provider badge
	providerBadge := components.NewBadge(m.activeProvider, components.BadgeBrand, m.theme).Render()

	// Pricing info
	pricingText := ""
	if m.activeModel.Pricing.InputPerMToken > 0 || m.activeModel.Pricing.OutputPerMToken > 0 {
		pricingText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			fmt.Sprintf("in $%.2f/M  out $%.2f/M",
				m.activeModel.Pricing.InputPerMToken,
				m.activeModel.Pricing.OutputPerMToken))
	}

	// Context window
	contextText := ""
	if m.activeModel.ContextLength > 0 {
		contextText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			fmt.Sprintf("ctx %s", components.FormatMetric(int(m.activeModel.ContextLength))))
	}

	parts := []string{modelBadge + " " + providerBadge}
	if pricingText != "" {
		parts = append(parts, pricingText)
	}
	if contextText != "" {
		parts = append(parts, contextText)
	}

	// Recent session activity sparkline (optional, shown when we have history)
	if m.sessionSparkline != "" {
		sparkStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
		parts = append(parts, sparkStyle.Render(m.sessionSparkline))
	}

	return style.Render(lipgloss.JoinVertical(lipgloss.Left, parts...))
}

// renderLogo renders a clean ASCII art logo for M31A.
func (m *ReplModel) renderLogo() string {
	version := m.version
	if version == "" {
		version = "dev"
	}
	logo := `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/ ` + version

	lines := strings.Split(logo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(m.theme.Brand).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

// renderInputBox renders the input area with placeholder text.
func (m *ReplModel) renderInputBox() string {
	t := m.theme

	style := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Brand).
		Background(t.Surface).
		Padding(0, 2).
		Width(50)

	placeholder := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("Type a message, /command, or goal...")

	// Context line (model · provider)
	var contextParts []string
	if m.activeModel != nil {
		contextParts = append(contextParts, m.activeModel.Name)
	}
	if m.activeProvider != "" {
		contextParts = append(contextParts, m.activeProvider)
	}
	contextLine := ""
	if len(contextParts) > 0 {
		contextLine = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render(strings.Join(contextParts, " · "))
	} else {
		contextLine = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render("M31A")
	}

	content := lipgloss.JoinVertical(lipgloss.Top, placeholder, contextLine)
	return style.Render(content)
}

// renderKeyboardHints renders keyboard shortcut hints.
func renderKeyboardHints(t theme.Theme) string {
	hints := []struct {
		key   string
		label string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"ctrl+x", "leader"},
	}

	parts := make([]string, 0, len(hints)*2)
	for i, h := range hints {
		keyStyle := lipgloss.NewStyle().
			Foreground(t.Brand).
			Bold(true)
		labelStyle := lipgloss.NewStyle().
			Foreground(t.TextMuted)

		parts = append(parts, keyStyle.Render(h.key)+" "+labelStyle.Render(h.label))
		if i < len(hints)-1 {
			parts = append(parts, "  ")
		}
	}

	return lipgloss.JoinHorizontal(lipgloss.Center, parts...)
}

// renderBottomBar renders the bottom bar with cwd and version.
func (m *ReplModel) renderBottomBar() string {
	t := m.theme

	cwdLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(filepath.Base(m.cwd))

	version := m.version
	if version == "" {
		version = "dev"
	}
	versionLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(version)

	// Right-align version
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
