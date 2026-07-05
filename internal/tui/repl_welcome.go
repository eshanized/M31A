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

	// Workflow explanation (per D-33)
	workflowExplanation := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Describe a task below. M31A will:\n  1. Discuss your requirements\n  2. Create a plan\n  3. Execute it\n  4. Verify the results\n\n  Type ? for help, Esc to go back.")

	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		workflowExplanation,
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

	s := theme.BuildSemanticStyles(t)
	brandStyle := s.BrandText

	fadeChars := []rune{'█', '▓', '▒', '░', '·'}
	halfW := width / 2
	var left strings.Builder
	for i := 0; i < halfW; i++ {
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		left.WriteString(brandStyle.Render(string(fadeChars[idx])))
	}
	var right strings.Builder
	for i := halfW - 1; i >= 0; i-- {
		idx := i * len(fadeChars) / halfW
		if idx >= len(fadeChars) {
			idx = len(fadeChars) - 1
		}
		right.WriteString(brandStyle.Render(string(fadeChars[idx])))
	}

	return left.String() + right.String()
}

// renderProviderCard shows current model/provider status or a setup prompt.
func (m *ReplModel) renderProviderCard(cardWidth int) string {
	t := m.theme
	s := theme.BuildSemanticStyles(t)

	if m.activeModel == nil || m.activeProvider == "" {
		title := s.WarningText.Render("⚠") + " " + s.Heading.Render("No provider configured")
		subtitle := s.SecondaryText.Render("Run /settings to get started")
		content := lipgloss.JoinVertical(lipgloss.Left, title, subtitle)
		return components.Card{
			Content: content,
			Width:   cardWidth,
			Border:  lipgloss.RoundedBorder(),
			Style:   components.CardWarning,
			Theme:   t,
		}.Render()
	}

	modelBadge := s.Heading.Render(m.activeModel.Name)
	providerBadge := components.NewBadge(m.activeProvider, components.BadgeBrandPreset, t).Render()

	pricingText := ""
	if m.activeModel.Pricing.InputPerMToken > 0 || m.activeModel.Pricing.OutputPerMToken > 0 {
		pricingText = s.Muted.Render(
			"in $" + formatFloat(m.activeModel.Pricing.InputPerMToken) + "/M  " +
				"out $" + formatFloat(m.activeModel.Pricing.OutputPerMToken) + "/M",
		)
	}

	contextText := ""
	if m.activeModel.ContextLength > 0 {
		contextText = s.Muted.Render("ctx " + components.FormatMetric(int(m.activeModel.ContextLength)))
	}

	var caps []string
	if m.activeModel.Capabilities.Tools {
		caps = append(caps, s.SuccessText.Border(lipgloss.RoundedBorder()).BorderForeground(t.Success).Padding(0, 1).Render("tools"))
	}
	if m.activeModel.Capabilities.Vision {
		caps = append(caps, s.InfoBanner.Border(lipgloss.RoundedBorder()).BorderForeground(t.Info).Padding(0, 1).Render("vision"))
	}
	if m.activeModel.Capabilities.Reasoning {
		caps = append(caps, s.SecondaryText.Border(lipgloss.RoundedBorder()).BorderForeground(t.Secondary).Padding(0, 1).Render("reasoning"))
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
		parts = append(parts, s.SecondaryText.Render(m.sessionSparkline))
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
	s := theme.BuildSemanticStyles(t)

	projectName := filepath.Base(m.cwd)
	if projectName == "" {
		projectName = "project"
	}

	nameRow := s.Heading.Render("▸ " + projectName)

	branchRow := ""
	if m.sidebarBranch != "" {
		branchRow = s.SecondaryText.Render("⎇  " + m.sidebarBranch)
	}

	var changedRow string
	if m.changedFiles > 0 {
		changedRow = s.WarningText.Render(fmt.Sprintf("● %d file(s) changed", m.changedFiles))
	} else {
		changedRow = s.SuccessText.Render("✓ working tree clean")
	}

	lang := detectProjectLanguage(m.cwd)
	langRow := ""
	if lang != "" {
		langRow = s.Muted.Render("  " + lang)
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
	s := theme.BuildSemanticStyles(t)

	title := s.BrandBold.Render("Getting started")

	prompts := m.welcomePrompts()

	nums := []string{"1.", "2.", "3."}
	var lines []string
	for i, p := range prompts {
		revealed := i < m.welcomeRevealCount
		var numStyle, body, hint string
		if revealed {
			numStyle = s.BrandBold.Render(nums[i])
			body = s.Body.Render(" " + p.prompt)
			hint = s.Muted.Italic(true).Render("  ·" + p.hint)
		} else {
			numStyle = s.Muted.Render(nums[i])
			body = s.Muted.Render(" " + p.prompt)
			hint = s.Muted.Italic(true).Render("  ·" + p.hint)
		}
		lines = append(lines, "  "+numStyle+body+hint)
	}

	sepWidth := cardWidth - 4
	if sepWidth < 10 {
		sepWidth = 10
	}
	sep := s.SeparatorH.Render(strings.Repeat("─", sepWidth))
	footer := s.Muted.Render("  Type a message, @file, /command, or goal…")

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
	s := theme.BuildSemanticStyles(t)
	hints := []string{"ctrl+p commands", "ctrl+b sidebar", "/help"}
	parts := make([]string, len(hints))
	for i, h := range hints {
		parts[i] = s.KeyboardHint.Render(h)
	}
	sep := s.BrandText.Render(" · ")
	return strings.Join(parts, sep)
}

// renderBottomBar renders a bottom bar showing cwd basename and version with │ separator.
func (m *ReplModel) renderBottomBar() string {
	s := theme.BuildSemanticStyles(m.theme)

	cwdLabel := s.FooterCwd.Render("⌂ " + filepath.Base(m.cwd))

	versionLabel := ""
	if m.version != "" {
		versionLabel = "v" + m.version
	}
	versionWidth := lipgloss.Width(versionLabel)
	sep := s.SeparatorV.Render(" │ ")

	spacer := m.replWidth() - lipgloss.Width(cwdLabel) - lipgloss.Width(sep) - versionWidth - 4
	if spacer < 0 {
		spacer = 0
	}

	if versionLabel != "" {
		return lipgloss.JoinHorizontal(lipgloss.Left,
			"  "+cwdLabel,
			strings.Repeat(" ", spacer),
			sep+s.FooterCwd.Render(versionLabel)+"  ",
		)
	}
	return "  " + cwdLabel
}

// formatFloat formats a float64 to 2 decimal places.
func formatFloat(f float64) string {
	return fmt.Sprintf("%.2f", f)
}
