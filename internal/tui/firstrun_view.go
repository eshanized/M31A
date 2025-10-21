package tui

import (
	"fmt"
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
)

// renderFirstRun renders the appropriate wizard step.
func (fr *FirstRunModel) renderFirstRun() string {
	t := fr.theme
	w := fr.width
	if w < 40 {
		w = 80
	}

	var content string
	switch fr.step {
	case stepWelcome:
		content = fr.renderWelcome()
	case stepProviderSelect:
		content = fr.renderProviderSelect()
	case stepAPIKey:
		content = fr.renderAPIKeyStep()
	case stepModelPick:
		content = fr.renderModelPickStep()
	case stepDone:
		content = lipgloss.NewStyle().
			Foreground(t.Success).Bold(true).
			Render("✓ Setup complete! Starting M31A...")
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 3).
		Width(w - 4).
		Render(content)

	return centerScreen(box, w, fr.height)
}

// renderWelcome renders the welcome step.
func (fr *FirstRunModel) renderWelcome() string {
	t := fr.theme
	title := lipgloss.NewStyle().
		Foreground(t.Brand).Bold(true).
		Render("Welcome to M31A")

	subtitle := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render("A terminal AI coding agent")

	body := lipgloss.NewStyle().Foreground(t.Text).
		Render("This wizard will guide you through the initial setup.\n" +
			"You'll need an API key for OpenRouter or Zen gateway.")

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Press  Enter  to continue   q to quit")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, "", subtitle, "", body, "", hint)
}

// renderProviderSelect renders the provider selection step.
func (fr *FirstRunModel) renderProviderSelect() string {
	t := fr.theme
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 1/3 — Choose your provider")

	var items []string
	for i, p := range fr.providers {
		prefix := "  "
		style := lipgloss.NewStyle().Foreground(t.Text)
		if i == fr.providerCursor {
			prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
			style = style.Foreground(t.Brand).Bold(true)
		}
		items = append(items, prefix+style.Render(titleCase(p)))
	}

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↑↓/jk navigate  ↵ select  esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, "", strings.Join(items, "\n"), "", hint)
}

// renderAPIKeyStep renders the API key entry step.
func (fr *FirstRunModel) renderAPIKeyStep() string {
	t := fr.theme
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render(fmt.Sprintf("Step 2/3 — Enter %s API key", titleCase(fr.opts.Provider)))

	desc := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Your API key is stored securely and never sent anywhere else.")

	keyBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Render(fr.keyInput.View())

	saveLabel := "  [ ] Save to system keychain (tab to toggle)"
	if fr.opts.SaveKeychain {
		saveLabel = lipgloss.NewStyle().Foreground(t.Success).
			Render("  [✓] Save to system keychain (tab to toggle)")
	} else {
		saveLabel = lipgloss.NewStyle().Foreground(t.TextMuted).Render(saveLabel)
	}

	errLine := ""
	if fr.keyErr != "" {
		errLine = lipgloss.NewStyle().Foreground(t.Error).Render("! " + fr.keyErr)
	}

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↵ confirm  tab toggle keychain  esc back")

	parts := []string{title, "", desc, "", keyBox, "", saveLabel}
	if errLine != "" {
		parts = append(parts, errLine)
	}
	parts = append(parts, "", hint)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderModelPickStep renders the model pick step.
func (fr *FirstRunModel) renderModelPickStep() string {
	t := fr.theme
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 3/3 — Enter default model ID")

	desc := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("e.g. anthropic/claude-3-5-sonnet  (leave blank to choose later)")

	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Render(fr.modelInput.View())

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↵ confirm  esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title, "", desc, "", inputBox, "", hint)
}

// titleCase uppercases the first rune of s.
func titleCase(s string) string {
	if s == "" {
		return s
	}
	runes := []rune(s)
	runes[0] = unicode.ToUpper(runes[0])
	return string(runes)
}
