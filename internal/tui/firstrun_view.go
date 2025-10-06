package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
)

func (m *FirstRunModel) View() string {
	switch m.state {
	case FirstRunWelcome:
		return m.viewWelcome()
	case FirstRunProviderSelect:
		return m.viewProviderSelect()
	case FirstRunKeyInput:
		return m.viewKeyInput()
	case FirstRunValidating:
		return m.viewValidating()
	case FirstRunKeychainPrompt:
		return m.viewKeychainPrompt()
	case FirstRunComplete:
		return m.viewComplete()
	}
	return ""
}

func (m *FirstRunModel) viewWelcome() string {
	t := m.theme
	w := m.width
	h := m.height

	if w == 0 || h == 0 {
		return ""
	}

	starfield := components.RenderStarfield(w, h, 42, t)
	logo := m.renderWelcomeLogo()

	subtitle := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("Terminal AI Coding Agent  " + m.versionLabel())

	features := m.renderFeatureCards()

	ctaText := "▶  Press Enter to begin setup"
	ctaBox := lipgloss.NewStyle().
		Background(t.SurfaceElevated).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 3).
		Foreground(t.Brand).
		Bold(true).
		Render(ctaText)

	footer := m.renderLaunchpadFooter()

	content := lipgloss.JoinVertical(lipgloss.Center,
		logo,
		"",
		subtitle,
		"",
		features,
		"",
		ctaBox,
		"",
		footer,
	)

	_ = starfield
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content,
		lipgloss.WithWhitespaceChars("·"),
		lipgloss.WithWhitespaceForeground(t.TextMuted))
}

func (m *FirstRunModel) renderWelcomeLogo() string {
	t := m.theme
	logo := `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/`

	lines := strings.Split(logo, "\n")
	styled := make([]string, len(lines))
	for i, line := range lines {
		styled[i] = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(line)
	}
	return lipgloss.JoinVertical(lipgloss.Top, styled...)
}

func (m *FirstRunModel) versionLabel() string {
	if m.version == "" {
		return "v1.x"
	}
	return "v" + m.version
}

func (m *FirstRunModel) renderFeatureCards() string {
	t := m.theme

	type feature struct {
		icon  string
		title string
		desc  string
	}

	features := []feature{
		{"⚡", "Fast Execution", "Parallel task runner with dependency graph"},
		{"🤖", "AI-Powered Coding", "Natural language goals → production code"},
		{"🔄", "Self-Healing", "Auto-retry & rollback via commit bisect"},
		{"📦", "Git Native", "Atomic commits per task with rollback browser"},
	}

	cardWidth := 30
	if m.width >= 120 {
		cardWidth = 35
	}
	if m.width < 60 {
		cardWidth = m.width - 4
	}

	cards := make([]string, len(features))
	for i, f := range features {
		iconStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
		descStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)

		cardContent := lipgloss.JoinVertical(lipgloss.Left,
			iconStyle.Render(f.icon+"  "+f.title),
			descStyle.Render(f.desc),
		)

		card := lipgloss.NewStyle().
			Background(t.SurfaceElevated).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(1, 2).
			Width(cardWidth).
			Render(cardContent)

		cards[i] = card
	}

	if m.width >= 80 {
		row1 := lipgloss.JoinHorizontal(lipgloss.Top, cards[0], " ", cards[1])
		row2 := lipgloss.JoinHorizontal(lipgloss.Top, cards[2], " ", cards[3])
		return lipgloss.JoinVertical(lipgloss.Center, row1, row2)
	}

	return lipgloss.JoinVertical(lipgloss.Center, cards...)
}

func (m *FirstRunModel) renderLaunchpadFooter() string {
	t := m.theme

	shortcuts := []struct {
		key   string
		label string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"/help", "help"},
		{"MIT License", ""},
	}

	parts := make([]string, 0, len(shortcuts))
	for _, s := range shortcuts {
		if s.label == "" {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(s.key))
		} else {
			keyStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
			labelStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
			parts = append(parts, keyStyle.Render(s.key)+" "+labelStyle.Render(s.label))
		}
	}

	return lipgloss.JoinHorizontal(lipgloss.Center, parts...)
}

func (m *FirstRunModel) viewProviderSelect() string {
	t := m.theme

	title := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("M31A"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › "),
		lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render("Provider Setup"),
	)

	subtitle := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("Which AI gateway will power M31A?")

	subHint := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("You can add more later via /settings → Provider")

	type providerOpt struct {
		key        string
		icon       string
		name       string
		desc       string
		coverage   int
		recommended bool
		isCard     bool
	}

	providers := []providerOpt{
		{key: "1", icon: "◆", name: "OpenRouter", desc: "100+ models · pay-per-use", coverage: 94, isCard: true},
		{key: "2", icon: "◈", name: "Zen", desc: "Fast inference · competitive pricing", coverage: 61, isCard: true},
		{key: "3", icon: "◆◈", name: "Both", desc: "OpenRouter + Zen with automatic failover", coverage: 98, recommended: true, isCard: true},
		{key: "4", icon: "○", name: "Skip", desc: "configure via /settings later", isCard: false},
	}

	var lines []string
	for i, p := range providers {
		if p.isCard {
			card := m.renderProviderCard(p, i == m.cursor)
			lines = append(lines, card)
		} else {
			marker := "  "
			if i == m.cursor {
				marker = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
			}
			skipLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Render(fmt.Sprintf("%s%s  %s  %s", marker, lipgloss.NewStyle().Foreground(t.Brand).Render("["+p.key+"]"), p.icon, p.name+" — "+p.desc))
			lines = append(lines, skipLine)
		}
	}

	hints := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("↑↓ navigate  ·  Enter select  ·  1-4 jump")

	content := lipgloss.JoinVertical(lipgloss.Center,
		title,
		subtitle,
		subHint,
		"",
		strings.Join(lines, "\n"),
		"",
		hints,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) renderProviderCard(p struct {
	key        string
	icon       string
	name       string
	desc       string
	coverage   int
	recommended bool
	isCard     bool
}, isActive bool) string {
	t := m.theme

	marker := "  "
	if isActive {
		marker = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}

	iconStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)
	nameStyle := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	descStyle := lipgloss.NewStyle().Foreground(t.TextSecondary)

	headerLine := fmt.Sprintf("%s%s %s  %s",
		marker,
		lipgloss.NewStyle().Foreground(t.Brand).Render("["+p.key+"]"),
		iconStyle.Render(p.icon),
		nameStyle.Render(p.name),
	)

	descLine := lipgloss.NewStyle().PaddingLeft(5).Render(descStyle.Render(p.desc))

	barWidth := 30
	filled := p.coverage * barWidth / 100
	empty := barWidth - filled
	bar := strings.Repeat("█", filled) + strings.Repeat("░", empty)
	coverageText := fmt.Sprintf("  Coverage: %d%%", p.coverage)
	barLine := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.Success).Render(bar),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(coverageText),
	)

	var badgeLine string
	if p.recommended {
		badge := t.SuccessBadge.Render(" Recommended")
		badgeLine = lipgloss.NewStyle().PaddingLeft(5).Render(badge)
	}

	content := lipgloss.JoinVertical(lipgloss.Left,
		headerLine,
		descLine,
		barLine,
	)
	if badgeLine != "" {
		content += "\n" + badgeLine
	}

	borderColor := t.Border
	if isActive {
		borderColor = t.Brand
	}

	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Background(t.Surface).
		Padding(0, 1).
		Width(m.width - 8).
		Render(content)

	return card
}

func (m *FirstRunModel) viewKeyInput() string {
	var parts []string

	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Render("🔑 Enter your API key")

	parts = append(parts, header)

	providerInfo := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Foreground(m.theme.TextSecondary).
		Padding(0, 2).
		Render(fmt.Sprintf("Configuring: %s", strings.Join(m.providers, " + ")))

	parts = append(parts, providerInfo)
	parts = append(parts, "")

	inputContainer := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(1, 2).
		Render(m.apiKeyInput.View())

	parts = append(parts, inputContainer)

	if m.validationErr != "" {
		errBox := lipgloss.NewStyle().
			Background(m.theme.Surface).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Error).
			Padding(0, 2).
			Foreground(m.theme.Error).
			Render(" " + m.validationErr)
		parts = append(parts, "")
		parts = append(parts, errBox)
	}

	footer := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Enter to confirm  ·  Esc to go back")

	parts = append(parts, "")
	parts = append(parts, footer)

	content := lipgloss.JoinVertical(lipgloss.Center, parts...)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewValidating() string {
	spinner := m.theme.Spinner.Render("⟳")
	text := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render("Validating API key...")

	subtitle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("This may take a few seconds")

	content := lipgloss.JoinVertical(lipgloss.Center,
		spinner,
		"",
		text,
		subtitle,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewKeychainPrompt() string {
	title := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Bold(true).
		Render(" Store API key in system keychain?")

	desc := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("This keeps your key secure and avoids re-entering it")

	yes := lipgloss.NewStyle().
		Background(m.theme.Success).
		Foreground(m.theme.BadgeForeground).
		Bold(true).
		Padding(0, 2).
		Render("Y / Enter = Yes")

	no := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Foreground(m.theme.TextSecondary).
		Padding(0, 2).
		Render("N = No")

	content := lipgloss.JoinVertical(lipgloss.Center,
		title,
		"",
		desc,
		"",
		lipgloss.JoinHorizontal(lipgloss.Center, yes, "   ", no),
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}

func (m *FirstRunModel) viewComplete() string {
	checkmark := lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Bold(true).
		Render(" Setup Complete!")

	text := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary).
		Render("Launching M31A...")

	content := lipgloss.JoinVertical(lipgloss.Center,
		checkmark,
		"",
		text,
	)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, content)
}
