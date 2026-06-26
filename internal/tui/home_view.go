package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
)

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

	// Logo with glow effect
	logoBlock := components.RenderBigLogo(t.Brand, true, w)

	// Add gradient glow line below logo
	glowWidth := w / 2
	if glowWidth > 40 {
		glowWidth = 40
	}
	glowStyle := lipgloss.NewStyle().
		Foreground(t.Brand).
		Faint(true)
	glowLine := glowStyle.Render(strings.Repeat("·", glowWidth))
	glowBlock := lipgloss.JoinVertical(lipgloss.Center, logoBlock, glowLine)

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

	tipsBlock := hm.renderTips(w)

	versionLine := ""
	if hm.version != "" {
		versionLine = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render("v" + hm.version)
	}

	content := lipgloss.JoinVertical(lipgloss.Center,
		"",
		glowBlock,
		"",
		"",
		inputBox,
		"",
		tipsBlock,
		"",
		versionLine,
	)

	// Overlay slash suggestions if visible
	if hm.slashVisible && len(hm.slashSuggestions) > 0 {
		suggestionsBox := hm.renderSlashSuggestions(promptMaxW)
		content = lipgloss.JoinVertical(lipgloss.Center,
			"",
			glowBlock,
			"",
			"",
			inputBox,
			suggestionsBox,
			"",
			tipsBlock,
			"",
			versionLine,
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
			line = TruncateWithEllipsis(line, width)
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
