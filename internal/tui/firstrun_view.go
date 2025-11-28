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

	// Use contentWidth (set by parent to account for sidebar) if available.
	// Otherwise fall back to subtracting sidebarDefaultWidth for full-width terminals.
	centerW := w
	if fr.contentWidth > 0 {
		centerW = fr.contentWidth
	} else if w >= WidthFull {
		centerW = w - sidebarDefaultWidth
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
		content = fr.renderDone()
	}

	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 3).
		Width(centerW - 4).
		Render(content)

	return centerScreen(box, centerW, fr.height)
}

// ─── Step progress indicator ─────────────────────────────────────────────────

func (fr *FirstRunModel) renderStepDots(current int, total int) string {
	t := fr.theme
	var parts []string
	for i := 0; i < total; i++ {
		if i == current {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Brand).Render("●"))
		} else {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("○"))
		}
	}
	return strings.Join(parts, " ")
}

// ─── Decorative gradient divider ─────────────────────────────────────────────

func renderBlockDivider(width int) string {
	if width < 10 {
		return strings.Repeat("─", width)
	}
	half := width / 2
	if half > 4 {
		half = 4
	}
	chars := []string{"░", "▒", "▓", "█"}
	var b strings.Builder
	for i := 0; i < half && i < len(chars); i++ {
		b.WriteString(chars[i])
	}
	center := strings.Repeat("█", width-2*half)
	b.WriteString(center)
	for i := half - 1; i >= 0; i-- {
		if i < len(chars) {
			b.WriteString(chars[i])
		}
	}
	return b.String()
}

// ─── Styled key badge ────────────────────────────────────────────────────────

func keyBadge(key, desc string, brand lipgloss.Color, muted lipgloss.Color) string {
	k := lipgloss.NewStyle().
		Foreground(brand).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(brand).
		Padding(0, 1).
		Render(key)
	d := lipgloss.NewStyle().Foreground(muted).Render(" " + desc)
	return k + d
}

// ─── Welcome step ────────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderWelcome() string {
	t := fr.theme
	w := fr.width
	if w < 40 {
		w = 70
	}
	if fr.contentWidth > 0 {
		w = fr.contentWidth
	}
	contentW := w - 10

	// Logo with brand color
	logoLines := strings.Split(logo, "\n")
	styledLogo := make([]string, len(logoLines))
	for i, line := range logoLines {
		styledLogo[i] = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(line)
	}
	logoBlock := lipgloss.JoinVertical(lipgloss.Center, styledLogo...)

	// Tagline
	tagline := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Italic(true).
		Render("Your AI pair programmer in the terminal")

	// Gradient divider
	divider := lipgloss.NewStyle().
		Foreground(t.Brand).
		Render(renderBlockDivider(contentW))

	// Feature cards row
	card1 := fr.featureCard("⚡ Workflows", "Plan → Execute → Verify → Ship", contentW/3-2)
	card2 := fr.featureCard("🔧 Tools", "Bash · FileRead · FileWrite · Glob · Grep", contentW/3-2)
	card3 := fr.featureCard("⎇ Git Aware", "Live repo status in sidebar", contentW/3-2)

	cardsRow := lipgloss.JoinHorizontal(lipgloss.Top, card1, "  ", card2, "  ", card3)

	// Step dots
	dots := fr.renderStepDots(0, 4)

	// Action hints
	enterBadge := keyBadge("↵", "Continue", t.Brand, t.TextMuted)
	quitBadge := keyBadge("q", "Quit", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center, enterBadge, "   ", quitBadge)

	return lipgloss.JoinVertical(lipgloss.Center,
		logoBlock,
		"",
		tagline,
		"",
		divider,
		"",
		cardsRow,
		"",
		"",
		dots,
		"",
		hints,
	)
}

func (fr *FirstRunModel) featureCard(title, desc string, w int) string {
	t := fr.theme
	if w < 12 {
		w = 18
	}
	titleS := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(title)
	descS := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(desc)

	content := lipgloss.JoinVertical(lipgloss.Left, titleS, "", descS)

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Width(w).
		Render(content)
}

// ─── Provider selection step ─────────────────────────────────────────────────

func (fr *FirstRunModel) renderProviderSelect() string {
	t := fr.theme
	w := fr.width
	if w < 40 {
		w = 70
	}
	if fr.contentWidth > 0 {
		w = fr.contentWidth
	}
	contentW := w - 10

	// Title + step dots
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 1/3 — Choose your provider")
	dots := fr.renderStepDots(1, 4)
	header := lipgloss.JoinVertical(lipgloss.Left, title, "", dots)

	// Provider cards
	var cards []string
	for i, p := range providerCatalog {
		cards = append(cards, fr.renderProviderCard(p, i == fr.providerCursor, contentW))
	}
	cardsBlock := strings.Join(cards, "\n\n")

	// Navigation hints
	navBadge := keyBadge("↑↓", "Navigate", t.Brand, t.TextMuted)
	selBadge := keyBadge("↵", "Select", t.Brand, t.TextMuted)
	skipBadge := keyBadge("s", "Skip", t.Warning, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center, navBadge, "  ", selBadge, "  ", skipBadge, "  ", backBadge)

	return lipgloss.JoinVertical(lipgloss.Left,
		header, "", cardsBlock, "", "", hints,
	)
}

func (fr *FirstRunModel) renderProviderCard(p providerInfo, selected bool, w int) string {
	t := fr.theme
	if w < 20 {
		w = 40
	}

	// Icon + Name row
	iconStyle := lipgloss.NewStyle().Foreground(t.Brand)
	nameStyle := lipgloss.NewStyle().Foreground(t.Text).Bold(true)
	if selected {
		iconStyle = iconStyle.Foreground(t.Brand)
		nameStyle = nameStyle.Foreground(t.Brand)
	}

	icon := iconStyle.Render(p.Icon)
	name := nameStyle.Render(p.Name)

	// Recommended badge
	recBadge := ""
	if p.Recommended {
		recBadge = " " + lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render("Recommended")
	}

	titleRow := icon + "  " + name + recBadge

	// Description
	desc := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(p.Description)

	// Selection indicator
	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▶ ")
	}

	content := lipgloss.JoinVertical(lipgloss.Left, titleRow, "", desc)

	borderColor := t.Border
	if selected {
		borderColor = t.Brand
	}

	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Width(w - 4).
		Render(content)

	return prefix + card
}

// ─── API key step ────────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderAPIKeyStep() string {
	t := fr.theme

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render(fmt.Sprintf("Step 2/3 — Enter %s API key", titleCase(fr.opts.Provider)))
	dots := fr.renderStepDots(2, 4)

	desc := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Your key is stored securely and never sent anywhere else.")

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
		errLine = lipgloss.NewStyle().Foreground(t.Error).Render("  ⚠ " + fr.keyErr)
	}

	// Navigation hints
	confirmBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	keychainBadge := keyBadge("tab", "Keychain", t.Brand, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", keychainBadge, "  ", backBadge)

	parts := []string{title, "", dots, "", desc, "", keyBox, "", saveLabel}
	if errLine != "" {
		parts = append(parts, errLine)
	}
	parts = append(parts, "", "", hints)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// ─── Model pick step ─────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderModelPickStep() string {
	t := fr.theme
	w := fr.width
	if w < 40 {
		w = 70
	}
	if fr.contentWidth > 0 {
		w = fr.contentWidth
	}
	contentW := w - 10

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 3/3 — Default model")
	dots := fr.renderStepDots(3, 4)

	// Custom input (always shown)
	inputLabel := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).
		Render("Model ID")
	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Render(fr.modelInput.View())

	// Navigation hints
	confirmBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)

	// Build the content based on state
	var parts []string
	parts = append(parts, title, "", dots, "")

	if fr.opts.Provider == "" {
		// Skipped provider — no suggestions, just text input
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No provider configured. Enter a model ID or leave blank.")
		parts = append(parts, desc, "", inputLabel, "", inputBox, "")
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("  ⓘ  You can choose a model later via /models."))
		parts = append(parts, "", confirmBadge, "  ", backBadge)
	} else if fr.modelsLoading {
		// Fetching models from provider
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Fetching models from " + titleCase(fr.opts.Provider) + "...")
		spinner := lipgloss.NewStyle().Foreground(t.Brand).Render("⠋")
		parts = append(parts, spinner+" "+desc, "", inputLabel, "", inputBox, "")
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("  ⓘ  Type a model ID while models are loading."))
		hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", backBadge)
		parts = append(parts, "", hints)
	} else if len(fr.suggestedModels) > 0 {
		// Dynamic suggestions available
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Pick a model or type a custom model ID.")
		parts = append(parts, desc, "")

		// Quick-pick chips
		var chips []string
		for i, s := range fr.suggestedModels {
			chips = append(chips, fr.renderModelChip(s, i == fr.suggestedCursor, contentW))
		}
		chipsRow := lipgloss.JoinHorizontal(lipgloss.Top, chips[0], "  ", chips[1], "  ", chips[2])
		parts = append(parts, chipsRow, "")

		parts = append(parts, inputLabel, "", inputBox, "")

		quickBadge := keyBadge("1-3", "Quick pick", t.Brand, t.TextMuted)
		hints := lipgloss.JoinHorizontal(lipgloss.Center, quickBadge, "  ", confirmBadge, "  ", backBadge)
		parts = append(parts, hints)
	} else {
		// Provider configured but no models fetched (error or empty)
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Could not fetch models. Enter a model ID manually.")
		parts = append(parts, desc, "", inputLabel, "", inputBox, "")
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("  ⓘ  Leave blank to choose later via /models."))
		hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", backBadge)
		parts = append(parts, "", hints)
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

func (fr *FirstRunModel) renderModelChip(s suggestedModel, selected bool, totalW int) string {
	t := fr.theme
	chipW := totalW/3 - 3
	if chipW < 14 {
		chipW = 14
	}

	numLabel := ""
	for i, sm := range fr.suggestedModels {
		if sm.ID == s.ID {
			numLabel = fmt.Sprintf("%d", i+1)
			break
		}
	}

	name := lipgloss.NewStyle().Foreground(t.Text).Bold(true).Render(s.Label)
	tag := lipgloss.NewStyle().Foreground(t.Success).Render(s.Tag)
	num := lipgloss.NewStyle().Foreground(t.TextMuted).Render("[" + numLabel + "]")

	header := num + " " + name
	content := lipgloss.JoinVertical(lipgloss.Left, header, tag)

	borderColor := t.Border
	if selected {
		borderColor = t.Brand
		content = lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▶ "+s.Label+" "+num),
			lipgloss.NewStyle().Foreground(t.Success).Render(s.Tag),
		)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Width(chipW).
		Render(content)
}

// ─── Done step ───────────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderDone() string {
	t := fr.theme
	return lipgloss.JoinVertical(lipgloss.Center,
		lipgloss.NewStyle().Foreground(t.Success).Bold(true).Render("✓ Setup complete!"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("Starting M31A..."),
	)
}

// ─── Logo constant (duplicated from components for wizard use) ───────────────

const logo = `  __  _______  __
 /  |/  / __ \/ _/
 / /|_/ / /_/ / _/
 /_/  /_/\____/_/ `

// titleCase uppercases the first rune of s.
func titleCase(s string) string {
	if s == "" {
		return s
	}
	runes := []rune(s)
	runes[0] = unicode.ToUpper(runes[0])
	return string(runes)
}
