package tui

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// renderWelcome renders the welcome screen content placed inside the viewport.
//
// Layout adapts based on terminal width:
//   - Wide (≥88 cols): two-column top row (provider card + project card)
//   - Narrow: single-column stack (provider card only)
//
// CRITICAL: content is centered within viewportHeight, NOT the full terminal height,
// to prevent the double-input visual bug.
func (m *ReplModel) renderWelcome() string {
	if m.width == 0 || m.height == 0 {
		return "Welcome to M31A"
	}

	availWidth := m.replWidth()
	vpHeight := m.viewport.Height
	if vpHeight < 4 {
		vpHeight = contentViewportHeight(m.height)
	}

	cardW := welcomeCardWidth(availWidth)

	// 1. Logo with glow
	logo := m.renderLogoWithGlow()

	// 2. Top row — provider card always shown; project card when wide enough
	var topRow string
	if availWidth >= replWelcomeTwoColThreshold {
		provCard := m.renderProviderCard(cardW)
		projCard := m.renderProjectCard(cardW)
		topRow = lipgloss.JoinHorizontal(lipgloss.Top, provCard, "  ", projCard)
	} else {
		topRow = m.renderProviderCard(cardW)
	}

	// 3. Getting-started with context-aware prompts
	gettingStarted := m.renderGettingStarted(cardW)

	// 4. Keyboard hints
	hints := renderKeyboardHints(m.theme)

	// 5. Bottom bar (cwd + version)
	bottomBar := m.renderBottomBar()

	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		topRow,
		"",
		gettingStarted,
		"",
		hints,
		"",
		bottomBar,
	)

	return lipgloss.Place(availWidth, vpHeight, lipgloss.Center, lipgloss.Center, content)
}

// welcomeCardWidth computes the responsive card width for the welcome screen.
// Two-column layout requires availWidth >= replWelcomeTwoColThreshold.
const replWelcomeTwoColThreshold = 88

func welcomeCardWidth(availWidth int) int {
	if availWidth >= replWelcomeTwoColThreshold {
		w := (availWidth - 6) / 2
		if w > 60 { // allow wider cards on ultra-wide terminals
			w = 60
		}
		if w < 20 {
			w = 20
		}
		return w
	}
	w := availWidth - 4
	if w > 60 {
		w = 60
	}
	if w < 20 {
		w = 20
	}
	return w
}

// renderLogoWithGlow renders the M31A logo with a gradient glow row beneath.
func (m *ReplModel) renderLogoWithGlow() string {
	logoBlock := components.RenderBigLogo(m.theme.Brand, true, m.replWidth())
	gradientSep := renderGradientSeparator(m.replWidth(), m.theme)

	return lipgloss.JoinVertical(lipgloss.Center, logoBlock, gradientSep)
}

// renderGradientSeparator renders a clean gradient separator line
func renderGradientSeparator(width int, t theme.Theme) string {
	if width <= 0 {
		return ""
	}

	// Clean gradient: fade from center outward
	fadeChars := []rune{'█', '▓', '▒', '░', '·'}
	halfW := width / 2
	var left strings.Builder
	for i := 0; i < halfW; i++ {
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		left.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(string(fadeChars[idx])))
	}
	var right strings.Builder
	for i := halfW - 1; i >= 0; i-- {
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		right.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(string(fadeChars[idx])))
	}

	return left.String() + right.String()
}

// renderProviderCard shows current model/provider status or a setup prompt.
func (m *ReplModel) renderProviderCard(cardWidth int) string {
	t := m.theme

	if m.activeModel == nil || m.activeProvider == "" {
		warningDot := lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("⚠")
		title := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render(" No provider configured")
		subtitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Run /settings to get started")
		content := lipgloss.JoinVertical(lipgloss.Left, warningDot+title, subtitle)
		return components.Card{
			Content: content,
			Width:   cardWidth,
			Border:  lipgloss.RoundedBorder(),
			Style:   components.CardWarning,
			Theme:   t,
		}.Render()
	}

	modelBadge := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render(m.activeModel.Name)
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

	var caps []string
	if m.activeModel.Capabilities.Tools {
		caps = append(caps, lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render("tools"))
	}
	if m.activeModel.Capabilities.Vision {
		caps = append(caps, lipgloss.NewStyle().
			Foreground(t.Info).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Info).
			Padding(0, 1).
			Render("vision"))
	}
	if m.activeModel.Capabilities.Reasoning {
		caps = append(caps, lipgloss.NewStyle().
			Foreground(t.Secondary).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Secondary).
			Padding(0, 1).
			Render("reasoning"))
	}

	parts := []string{modelBadge + " " + providerBadge}
	if pricingText != "" {
		parts = append(parts, pricingText)
	}
	if contextText != "" {
		parts = append(parts, contextText)
	}
	if len(caps) > 0 {
		parts = append(parts, strings.Join(caps, " "))
	}
	if m.sessionSparkline != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(m.sessionSparkline))
	}

	return components.Card{
		Content: lipgloss.JoinVertical(lipgloss.Left, parts...),
		Width:   cardWidth,
		Border:  lipgloss.RoundedBorder(),
		Style:   components.CardSuccess,
		Theme:   t,
	}.Render()
}

// renderProjectCard shows project name, git branch, changed-file count, and language.
func (m *ReplModel) renderProjectCard(cardWidth int) string {
	t := m.theme

	projectName := filepath.Base(m.cwd)
	if projectName == "" {
		projectName = "project"
	}

	// Project name row
	nameRow := lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Render("▸ " + projectName)

	// Git branch row
	branchRow := ""
	if m.sidebarBranch != "" {
		branchRow = lipgloss.NewStyle().Foreground(t.TextSecondary).
			Render("⎇  " + m.sidebarBranch)
	}

	// Changed files row
	var changedRow string
	if m.changedFiles > 0 {
		changedRow = lipgloss.NewStyle().Foreground(t.Warning).
			Render(fmt.Sprintf("● %d file(s) changed", m.changedFiles))
	} else {
		changedRow = lipgloss.NewStyle().Foreground(t.Success).Render("✓ working tree clean")
	}

	// Language detection
	lang := detectProjectLanguage(m.cwd)
	langRow := ""
	if lang != "" {
		langRow = lipgloss.NewStyle().Foreground(t.TextMuted).Render("  " + lang)
	}

	parts := []string{nameRow}
	if branchRow != "" {
		parts = append(parts, branchRow)
	}
	parts = append(parts, changedRow)
	if langRow != "" {
		parts = append(parts, langRow)
	}

	return components.Card{
		Content: lipgloss.JoinVertical(lipgloss.Left, parts...),
		Width:   cardWidth,
		Border:  lipgloss.RoundedBorder(),
		Style:   components.CardDefault,
		Theme:   t,
	}.Render()
}

// renderGettingStarted renders suggested prompts, context-aware when possible.
func (m *ReplModel) welcomePrompts() []struct{ prompt, hint string } {
	type suggestion = struct{ prompt, hint string }
	prompts := []suggestion{
		{"Fix the failing tests in this repo", "auto-fix"},
		{"Add error handling to the API layer", "refactor"},
		{"Explain this codebase architecture", "explore"},
	}
	if m.changedFiles > 0 {
		prompts[1] = suggestion{
			fmt.Sprintf("Review recent changes (%d files)", m.changedFiles),
			"review",
		}
	}
	if proj := filepath.Base(m.cwd); proj != "" {
		prompts[2] = suggestion{
			fmt.Sprintf("Explain the %s architecture", proj),
			"explore",
		}
	}
	return prompts
}

func (m *ReplModel) renderGettingStarted(cardWidth int) string {
	t := m.theme

	title := lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true).
		Render("Getting started")

	prompts := m.welcomePrompts()

	nums := []string{"1.", "2.", "3."}
	var lines []string
	for i, p := range prompts {
		revealed := i < m.welcomeRevealCount
		var numStyle, body, hint string
		if revealed {
			numStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(nums[i])
			body = lipgloss.NewStyle().Foreground(t.TextPrimary).Render(" " + p.prompt)
			hint = lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render("  ·" + p.hint)
		} else {
			numStyle = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render(nums[i])
			body = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render(" " + p.prompt)
			hint = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Italic(true).Render("  ·" + p.hint)
		}
		lines = append(lines, "  "+numStyle+body+hint)
	}

	// Separator width tied to card width, not hardcoded
	sepWidth := cardWidth - 4
	if sepWidth < 10 {
		sepWidth = 10
	}
	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render(strings.Repeat("─", sepWidth))
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("  Type a message, @file, /command, or goal…")

	content := lipgloss.JoinVertical(lipgloss.Left,
		title,
		"",
		strings.Join(lines, "\n"),
		"",
		sep,
		footer,
	)

	return components.Card{
		Content: content,
		Width:   cardWidth,
		Border:  lipgloss.RoundedBorder(),
		Style:   components.CardDefault,
		Theme:   t,
	}.Render()
}

// renderKeyboardHints renders keyboard shortcut hints with branded separator dots.
func renderKeyboardHints(t theme.Theme) string {
	hints := []string{"ctrl+p commands", "ctrl+b sidebar", "/help"}
	parts := make([]string, len(hints))
	for i, h := range hints {
		parts[i] = lipgloss.NewStyle().Foreground(t.TextMuted).Render(h)
	}
	sep := lipgloss.NewStyle().Foreground(t.Brand).Render(" · ")
	return strings.Join(parts, sep)
}

// renderBottomBar renders a bottom bar showing cwd basename and version with │ separator.
func (m *ReplModel) renderBottomBar() string {
	t := m.theme

	cwdLabel := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("⌂ " + filepath.Base(m.cwd))

	version := m.version
	if version == "" {
		version = "dev"
	}
	versionLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("v" + version)
	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).Render(" │ ")

	spacer := m.replWidth() - lipgloss.Width(cwdLabel) - lipgloss.Width(sep) - lipgloss.Width(versionLabel) - 4
	if spacer < 0 {
		spacer = 0
	}

	return lipgloss.JoinHorizontal(lipgloss.Left,
		"  "+cwdLabel,
		strings.Repeat(" ", spacer),
		sep+versionLabel+"  ",
	)
}

// formatFloat formats a float64 to 2 decimal places.
func formatFloat(f float64) string {
	return fmt.Sprintf("%.2f", f)
}
