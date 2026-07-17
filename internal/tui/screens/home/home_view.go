package home

import (
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// resolveLogoText returns the custom logo text from config.
// Priority: LogoText (inline) > LogoFile (file contents) > "" (use embedded default).
func resolveLogoText(ui config.UIConfig) string {
	if ui.LogoText != "" {
		return ui.LogoText
	}
	if ui.LogoFile != "" {
		data, err := os.ReadFile(ui.LogoFile)
		if err == nil {
			return string(data)
		}
	}
	return ""
}

// renderHome renders the home screen content (logo + prompt + tips).
// Returns content only — header, footer, and chrome are handled by PageLayout.
func (hm *HomeModel) renderHome() string {
	t := hm.theme
	w := hm.width
	if w <= 0 {
		w = 80
	}
	h := hm.height
	if h <= 0 {
		h = 24
	}

	// Logo without glow effect (simplified chrome)
	var customLogo string
	if hm.cfg != nil {
		customLogo = resolveLogoText(hm.cfg.UI)
	}
	logoBlock := components.RenderBigLogo(t.Brand, false, w, customLogo)

	glowBlock := logoBlock

	// Workflow explanation (per D-33, D-34)
	explanation := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("Describe a task. M31A will plan and execute it.")

	promptMaxW := w * 7 / 10
	if promptMaxW > 75 {
		promptMaxW = 75
	}
	if promptMaxW < 30 {
		promptMaxW = 30
	}
	hm.input.Width = promptMaxW - 4

	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.BorderActive).
		Padding(0, 1).
		Width(promptMaxW).
		Render(hm.input.View())

	// Apply visible focus ring when the input has keyboard focus.
	// The home screen input always has focus unless slash suggestions are open.
	inputFocused := !hm.slashVisible
	inputBox = components.RenderFocusRing(inputBox, inputFocused, t, promptMaxW)

	// Suggested prompts with categories (per D-31)
	suggestionsBlock := hm.renderSuggestions(w)

	// Getting-started hint for first-time users
	var hintBlock string
	if hm.firstVisit {
		hintBlock = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Faint(true).
			Render("First time? Type /help getting-started")
	}

	tipsBlock := hm.renderTips(w)

	// Tagline below logo (replaces version display)
	tagline := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Faint(true).
		Render("Autonomous coding agent")

	content := lipgloss.JoinVertical(lipgloss.Center,
		"",
		glowBlock,
		tagline,
		explanation,
		"",
		inputBox,
		"",
		suggestionsBlock,
		"",
		tipsBlock,
	)

	if hintBlock != "" {
		content = lipgloss.JoinVertical(lipgloss.Center,
			"",
			glowBlock,
			tagline,
			explanation,
			"",
			inputBox,
			"",
			suggestionsBlock,
			"",
			tipsBlock,
			"",
			hintBlock,
		)
	}

	// Overlay slash suggestions if visible
	if hm.slashVisible && len(hm.slashSuggestions) > 0 {
		suggestionsBox := hm.renderSlashSuggestions(promptMaxW)
		content = lipgloss.JoinVertical(lipgloss.Center,
			"",
			glowBlock,
			tagline,
			"",
			inputBox,
			suggestionsBox,
			"",
			tipsBlock,
		)
	}

	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}

// renderSlashSuggestions renders the slash command autocomplete dropdown.
func (hm *HomeModel) renderSlashSuggestions(width int) string {
	t := hm.theme
	var lines []string

	for i, cmd := range hm.slashSuggestions {
		slashStyle := lipgloss.NewStyle().Foreground(t.Brand)
		nameStyle := lipgloss.NewStyle().Foreground(t.Text)
		descStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

		if i == hm.slashSelected {
			nameStyle = nameStyle.Background(t.Brand).Foreground(t.Background)
			slashStyle = slashStyle.Background(t.Brand).Foreground(t.Background)
			descStyle = descStyle.Background(t.Brand).Foreground(t.Background)
		}

		slash := slashStyle.Render(cmd.Slash)
		desc := descStyle.Render("  " + cmd.Description)
		line := "  " + nameStyle.Render(slash) + desc
		if lipgloss.Width(line) > width {
			line = tuitypes.TruncateWithEllipsis(line, width)
		}
		lines = append(lines, line)
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Width(width - 2).
		Render(strings.Join(lines, "\n"))

	return box
}

// renderSuggestions renders categorized suggested prompts.
func (hm *HomeModel) renderSuggestions(w int) string {
	t := hm.theme

	type suggestion struct {
		category string
		text     string
	}

	suggestions := []suggestion{
		{"Code", "Fix failing tests"},
		{"Code", "Add error handling"},
		{"Explore", "Explain this codebase"},
		{"Debug", "Find the bug in..."},
		{"Explore", "Show the architecture"},
	}

	catStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	textStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

	var lines []string
	for _, s := range suggestions {
		cat := catStyle.Render(s.category)
		txt := textStyle.Render(s.text)
		lines = append(lines, "  "+cat+": "+txt)
	}

	return strings.Join(lines, "\n")
}

// renderTips renders the keyboard shortcut tips as a horizontal row.
func (hm *HomeModel) renderTips(w int) string {
	t := hm.theme

	var parts []string
	for _, tip := range hm.tips {
		key := lipgloss.NewStyle().
			Foreground(t.Brand).
			Bold(true).
			Render(tip.key)
		label := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render(tip.label)
		parts = append(parts, key+" "+label)
	}

	row := strings.Join(parts, lipgloss.NewStyle().Foreground(t.Border).Render("  ·  "))

	rowW := lipgloss.Width(row)
	if rowW > w-4 {
		var lines []string
		var current []string
		currentW := 0
		sep := lipgloss.NewStyle().Foreground(t.Border).Render("  ·  ")
		sepW := 3
		for i, part := range parts {
			pw := lipgloss.Width(part)
			addedW := pw
			if i > 0 {
				addedW += sepW
			}
			if currentW+addedW > w-4 && len(current) > 0 {
				lines = append(lines, strings.Join(current, sep))
				current = nil
				currentW = 0
				addedW = pw
			}
			current = append(current, part)
			currentW += addedW
		}
		if len(current) > 0 {
			lines = append(lines, strings.Join(current, sep))
		}
		return strings.Join(lines, "\n")
	}

	return row
}
