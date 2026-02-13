package tui

import (
	"fmt"
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/types"
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
		// The welcome screen composes its own starfield + panel layout with
		// its own vertical centering, so return it directly without wrapping
		// in the outer rounded box (which adds an unpredictable border when
		// the composed content is wider than Width(centerW-4)).
		return fr.renderWelcome()
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

const firstRunStepCount = 4

func (fr *FirstRunModel) renderStepDots(current int) string {
	t := fr.theme
	var parts []string
	for i := 0; i < firstRunStepCount; i++ {
		if i == current {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Brand).Render("●"))
		} else {
			parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("○"))
		}
	}
	return strings.Join(parts, " ")
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

// renderWelcome renders the welcome step with a responsive layout.
//
// Wide terminals (≥60 cols, ≥10 rows) show the welcome panel floating over a
// deterministic starfield backdrop. Narrow terminals show a clean bordered
// panel stacked vertically. The panel always contains the brand logo,
// tagline, step dots, and keyboard hints; feature cards appear only when
// there is enough room (panel inner width ≥56).
func (fr *FirstRunModel) renderWelcome() string {
	availW := fr.effectiveWidth()
	availH := fr.height
	if availH < 1 {
		availH = 24
	}

	// Panel width budget: cap at 68 so the starfield peeks through on wide
	// terminals; shrink gracefully on narrow ones.
	panelW := availW - 4
	if panelW > 68 {
		panelW = 68
	}
	if panelW < 20 {
		panelW = 20
	}
	panelInnerW := panelW - 2
	if panelInnerW < 1 {
		panelInnerW = 18
	}

	panel := fr.renderWelcomePanel(panelInnerW, panelW)
	panelLines := strings.Split(panel, "\n")
	panelH := len(panelLines)

	// Degenerate case: terminal too cramped for the starfield composition.
	if availH < 8 || availW < 30 {
		if panelH > availH {
			panelLines = panelLines[:availH]
		}
		return centerScreen(strings.Join(panelLines, "\n"), availW, availH)
	}

	// Layout: starfield rows fill the space above and below the centered
	// panel. The panel occupies the middle rows as-is, so its border is never
	// spliced and any lipgloss-vs-rune-count width discrepancy is irrelevant.
	topMargin := (availH - panelH) / 2
	if topMargin < 0 {
		topMargin = 0
	}
	bottomMargin := availH - panelH - topMargin
	if bottomMargin < 0 {
		bottomMargin = 0
	}

	var out []string
	if topMargin > 0 {
		var starTop string
		if fr.starfieldCacheW == availW && fr.starfieldCacheH == topMargin && fr.starfieldCache != "" {
			starTop = fr.starfieldCache
		} else {
			starTop = components.RenderStarfieldPlain(availW, topMargin, 31)
			fr.starfieldCache = starTop
			fr.starfieldCacheW = availW
			fr.starfieldCacheH = topMargin
		}
		out = append(out, strings.Split(starTop, "\n")...)
	}
	out = append(out, panelLines...)
	if bottomMargin > 0 {
		starBot := components.RenderStarfieldPlain(availW, bottomMargin, 31+int64(topMargin))
		out = append(out, strings.Split(starBot, "\n")...)
	}

	// Trim any excess rows (defensive: panel may be taller than availH).
	if len(out) > availH {
		out = out[:availH]
	}

	// Center vertically by adding empty rows top/bottom. Skip horizontal
	// centering entirely because lipgloss.Place has historically added
	// unpredictable per-line padding that pushed the rendered width past availW.
	topPad := (availH - len(out)) / 2
	if topPad < 0 {
		topPad = 0
	}
	bottomPad := availH - len(out) - topPad
	if bottomPad < 0 {
		bottomPad = 0
	}
	var finalRows []string
	for i := 0; i < topPad; i++ {
		finalRows = append(finalRows, "")
	}
	finalRows = append(finalRows, out...)
	for i := 0; i < bottomPad; i++ {
		finalRows = append(finalRows, "")
	}

	result := strings.Join(finalRows, "\n")
	return result
}

// renderWelcomePanel builds the welcome panel content (logo, tagline, step
// dots, hints, and optional feature cards). panelW is the total panel width
// including its own padding; innerW is the usable content width inside it.
func (fr *FirstRunModel) renderWelcomePanel(innerW, panelW int) string {
	t := fr.theme

	// Brand logo, styled consistently with the rest of the app.
	logoBlock := components.RenderLogo("", true, t.Brand)

	// Gradient glow row beneath the logo — matches the REPL welcome aesthetic.
	glow := fr.renderGlowRow(innerW)

	// Italic tagline.
	tagline := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Italic(true).
		Render("Your AI pair programmer in the terminal")

	// Step progress dots (4 total, first active).
	dots := fr.renderStepDots(0)

	// Keyboard hints row — bordered badges when there is room, plain text when
	// the panel is very narrow (the badges add ~16 cells of decoration that
	// cause the row to overflow and break the border via lipgloss wrapping).
	var hints string
	if innerW >= 38 {
		enterBadge := keyBadge("↵", "Continue", t.Brand, t.TextMuted)
		quitBadge := keyBadge("q", "Quit", t.TextMuted, t.TextMuted)
		hints = lipgloss.JoinHorizontal(lipgloss.Center, enterBadge, "   ", quitBadge)
	} else {
		hints = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("[enter] Continue   [q] Quit")
	}

	var body string
	if innerW >= 56 {
		// Wide layout: three feature cards laid out horizontally.
		// (innerW - 4) / 3 accounts for the two 2-cell gaps between cards.
		cardW := (innerW - 4) / 3
		if cardW < 16 {
			cardW = 16
		}
		c1 := fr.featureCard("⚡ Workflows", "Plan → Execute → Verify → Ship", cardW)
		c2 := fr.featureCard("🔧 Tools", "Bash · FileRead · FileWrite · Grep", cardW)
		c3 := fr.featureCard("⎇ Git Aware", "Live repo status in sidebar", cardW)
		cards := lipgloss.JoinHorizontal(lipgloss.Top, c1, "  ", c2, "  ", c3)

		body = lipgloss.JoinVertical(lipgloss.Center,
			logoBlock, "", glow, "", tagline, "", cards, "", dots, "", hints,
		)
	} else {
		// Narrow layout: cards collapse to short, border-safe bullets.
		features := lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render("- Workflows"),
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render("- Tools"),
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render("- Git aware"),
		)

		body = lipgloss.JoinVertical(lipgloss.Center,
			logoBlock, "", glow, "", tagline, "", features, "", dots, "", hints,
		)
	}

	// Pad every body line to exactly innerW visible cells. JoinVertical leaves
	// lines at their natural widths, which would produce a panel whose border
	// tracks the widest line — an unpredictable value. Padding to a fixed
	// width gives the panel a deterministic shape.
	body = padLines(body, innerW)

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Width(panelW).
		Render(body)
}

// padLines pads (or truncates) every line of s to exactly targetW visible
// cells. ANSI escape sequences are preserved and do not count toward the
// visible width.
func padLines(s string, targetW int) string {
	lines := strings.Split(s, "\n")
	for i, line := range lines {
		w := lipgloss.Width(line)
		if w < targetW {
			lines[i] = line + strings.Repeat(" ", targetW-w)
		} else if w > targetW {
			// Truncate by visible cells, preserving ANSI resets.
			lines[i] = truncateStyled(line, targetW)
		}
	}
	return strings.Join(lines, "\n")
}

// truncateStyled truncates styled text to at most maxW visible cells,
// preserving escape sequences and appending a reset so the terminal does not
// leak styling into subsequent output.
func truncateStyled(s string, maxW int) string {
	var out strings.Builder
	visible := 0
	inEsc := false
	esc := strings.Builder{}
	for _, r := range s {
		if inEsc {
			esc.WriteRune(r)
			if r == 'm' {
				out.WriteString(esc.String())
				esc.Reset()
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			esc.WriteRune(r)
			continue
		}
		if visible >= maxW {
			break
		}
		out.WriteRune(r)
		visible++
	}
	out.WriteString("\x1b[0m")
	return out.String()
}

// renderGlowRow renders a horizontal gradient-bar glow row used beneath the logo.
func (fr *FirstRunModel) renderGlowRow(width int) string {
	t := fr.theme
	chars := []string{"█", "▓", "▒", "░"}
	var edge strings.Builder
	for _, ch := range chars {
		edge.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(ch))
	}
	edgeStr := edge.String()
	edgeW := len(chars)
	center := width - 2*edgeW
	if center < 2 {
		center = 2
	}
	fill := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", center))
	return edgeStr + fill + edgeStr
}

// ─── Overlay helpers (starfield + panel composition) ────────────────────────

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
		Render("Step 1/3 — Choose your providers")
	dots := fr.renderStepDots(1)
	header := lipgloss.JoinVertical(lipgloss.Left, title, "", dots)

	subtitle := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Select one or more providers. You can switch between them anytime.")

	// Provider cards
	var cards []string
	for i, p := range providerCatalog {
		cards = append(cards, fr.renderProviderCard(p, i == fr.providerCursor, contentW))
	}
	cardsBlock := strings.Join(cards, "\n\n")

	// Navigation hints
	navBadge := keyBadge("↑↓", "Navigate", t.Brand, t.TextMuted)
	toggleBadge := keyBadge("space", "Toggle", t.Brand, t.TextMuted)
	selBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	skipBadge := keyBadge("s", "Skip", t.Warning, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center, navBadge, "  ", toggleBadge, "  ", selBadge, "  ", skipBadge, "  ", backBadge)

	return lipgloss.JoinVertical(lipgloss.Left,
		header, "", subtitle, "", cardsBlock, "", "", hints,
	)
}

func (fr *FirstRunModel) renderProviderCard(p providerInfo, selected bool, w int) string {
	t := fr.theme
	if w < 20 {
		w = 40
	}

	checked := fr.providerChecked[p.ID]

	// Checkbox indicator
	checkBox := "[ ] "
	if checked {
		checkBox = lipgloss.NewStyle().Foreground(t.Success).Render("[✓] ")
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

	// Default badge for first checked provider
	defaultBadge := ""
	if checked && len(fr.selectedProviders) > 0 && fr.selectedProviders[0] == p.ID {
		defaultBadge = " " + lipgloss.NewStyle().
			Foreground(t.Brand).
			Render("(default)")
	}

	titleRow := checkBox + icon + "  " + name + recBadge + defaultBadge

	// Description
	desc := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(p.Description)

	// Selection indicator (cursor arrow)
	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▶ ")
	}

	content := lipgloss.JoinVertical(lipgloss.Left, titleRow, "", desc)

	borderColor := t.Border
	if selected {
		borderColor = t.Brand
	}
	if checked && !selected {
		borderColor = t.Success
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

	total := len(fr.selectedProviders)
	current := fr.keyProviderIndex + 1
	provName := ""
	if fr.keyProviderIndex < total {
		provName = titleCase(fr.selectedProviders[fr.keyProviderIndex])
	}

	var title string
	if total > 1 {
		title = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
			Render(fmt.Sprintf("Step 2/3 — Enter %s API key (%d/%d)", provName, current, total))
	} else {
		title = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
			Render(fmt.Sprintf("Step 2/3 — Enter %s API key", provName))
	}
	dots := fr.renderStepDots(2)

	desc := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Your key is stored securely and never sent anywhere else.")

	keyBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Render(fr.keyInput.View())

	saveLabel := "  ○ Save to system keychain  "
	if fr.opts.SaveKeychain {
		saveLabel = "  " + lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render("✓ Save to system keychain")
	} else {
		saveLabel = "  " + lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Render("○ Save to system keychain")
	}
	saveLabel += lipgloss.NewStyle().Foreground(t.TextMuted).Render("  (tab to toggle)")

	errLine := ""
	if fr.keyValidationErr != "" {
		errLine = lipgloss.NewStyle().Foreground(t.Error).Render("  ✗ " + fr.keyValidationErr)
	} else if fr.keyErr != "" {
		errLine = lipgloss.NewStyle().Foreground(t.Error).Render("  ⚠ " + fr.keyErr)
	}

	validatingLine := ""
	if fr.keyValidating {
		validatingLine = lipgloss.NewStyle().Foreground(t.Brand).Render("  ⠋ Validating API key...")
	}

	// Navigation hints
	confirmBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	keychainBadge := keyBadge("tab", "Keychain", t.Brand, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", keychainBadge, "  ", backBadge)

	parts := []string{title, "", dots, "", desc, "", keyBox, ""}
	// UX-04: keychain toggle with focus highlight
	parts = append(parts, saveLabel)
	if validatingLine != "" {
		parts = append(parts, validatingLine)
	}
	if errLine != "" {
		parts = append(parts, errLine)
	}
	parts = append(parts, "", "", hints)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// ─── Model pick step ────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderModelPickStep() string {
	t := fr.theme
	w := fr.width
	if w < 40 {
		w = 70
	}
	if fr.contentWidth > 0 {
		w = fr.contentWidth
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 3/3 — Choose your default model")
	dots := fr.renderStepDots(3)

	confirmBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)

	var parts []string
	parts = append(parts, title, "", dots, "")

	// ── Loading ───────────────────────────────────────────────────────────────
	if fr.modelsLoading {
		spinner := lipgloss.NewStyle().Foreground(t.Brand).Render("⠋")
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Fetching all models from " + titleCase(fr.opts.DefaultProvider) + "...")
		parts = append(parts, spinner+" "+desc)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// ── No provider configured ────────────────────────────────────────────────
	if fr.opts.DefaultProvider == "" {
		inputBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Render(fr.modelInput.View())
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No provider configured. Enter a model ID or press ↵ to continue.")
		hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", backBadge)
		parts = append(parts, desc, "", inputBox, "", hints)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// ── Full categorized browser ──────────────────────────────────────────────
	b := fr.browser
	if b == nil || len(b.categories) == 0 {
		inputBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Render(fr.modelInput.View())
		warnLine := lipgloss.NewStyle().Foreground(t.Warning).
			Render("⚠  Could not fetch models. Enter a model ID manually.")
		hints := lipgloss.JoinHorizontal(lipgloss.Center, confirmBadge, "  ", backBadge)
		parts = append(parts, warnLine, "", inputBox, "", hints)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// Category tab bar
	var tabParts []string
	for i, cat := range b.categories {
		count := fmt.Sprintf("%d", len(cat.Models))
		label := cat.Icon + " " + cat.Title + " (" + count + ")"
		if i == b.catCursor {
			tabParts = append(tabParts, lipgloss.NewStyle().
				Foreground(t.Brand).Bold(true).
				Border(lipgloss.RoundedBorder()).BorderForeground(t.Brand).
				Padding(0, 1).Render(label))
		} else {
			tabParts = append(tabParts, lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Border(lipgloss.RoundedBorder()).BorderForeground(t.Border).
				Padding(0, 1).Render(label))
		}
		if i < len(b.categories)-1 {
			tabParts = append(tabParts, " ")
		}
	}
	tabBar := lipgloss.JoinHorizontal(lipgloss.Top, tabParts...)
	parts = append(parts, tabBar, "")

	// Model list
	cat := b.activeCat()
	listW := w - 8
	if listW < 30 {
		listW = 30
	}
	if cat != nil && len(cat.Models) > 0 {
		listH := fr.height - 22
		if listH < 5 {
			listH = 5
		}
		// Clamp scroll
		if b.modelCursor >= b.scrollOffset+listH {
			b.scrollOffset = b.modelCursor - listH + 1
		}
		if b.scrollOffset < 0 {
			b.scrollOffset = 0
		}
		end := b.scrollOffset + listH
		if end > len(cat.Models) {
			end = len(cat.Models)
		}

		var rows []string
		for i := b.scrollOffset; i < end; i++ {
			rows = append(rows, fr.renderModelRow(cat.Models[i], i == b.modelCursor, listW))
		}
		if len(cat.Models) > listH {
			rows = append(rows, lipgloss.NewStyle().Foreground(t.TextMuted).
				Render(fmt.Sprintf("  %d–%d of %d  (↑/↓ scroll)",
					b.scrollOffset+1, end, len(cat.Models))))
		}
		listBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Width(listW).
			Render(strings.Join(rows, "\n"))
		parts = append(parts, listBox)
	}

	// Selected model ID input
	inputLabel := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render("Selected model (edit to override):")
	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Render(fr.modelInput.View())
	parts = append(parts, "", inputLabel, inputBox)

	// Hints
	navBadge := keyBadge("↑↓", "Navigate", t.Brand, t.TextMuted)
	tabNavBadge := keyBadge("tab", "Category", t.Brand, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center,
		navBadge, "  ", tabNavBadge, "  ", confirmBadge, "  ", backBadge)
	parts = append(parts, "", hints)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderModelRow renders a single model row in the categorized browser list.
func (fr *FirstRunModel) renderModelRow(m types.ModelInfo, selected bool, maxW int) string {
	t := fr.theme

	cursor := "  "
	nameStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
		nameStyle = nameStyle.Bold(true).Foreground(t.Brand)
	}

	name := m.Name
	if name == "" {
		name = m.ID
	}
	maxName := maxW - 38
	if maxName < 12 {
		maxName = 12
	}
	nameStr := nameStyle.Render(TruncateWithEllipsis(name, maxName))

	// Pricing badge
	var priceBadge string
	if m.Pricing.InputPerMToken == 0 && m.Pricing.OutputPerMToken == 0 {
		priceBadge = lipgloss.NewStyle().Foreground(t.Success).Bold(true).Render("FREE")
	} else {
		priceBadge = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("$%.2f/M", m.Pricing.InputPerMToken))
	}

	// Capability icons
	var caps []string
	if m.Capabilities.Reasoning {
		caps = append(caps, lipgloss.NewStyle().Foreground(t.Warning).Render("💡"))
	}
	if m.Capabilities.Tools {
		caps = append(caps, lipgloss.NewStyle().Foreground(t.Brand).Render("🔧"))
	}
	if m.Capabilities.Vision {
		caps = append(caps, lipgloss.NewStyle().Foreground(t.Brand).Render("👁"))
	}
	capsStr := strings.Join(caps, " ")

	// Context window
	ctxStr := ""
	if m.ContextLength >= 1000000 {
		ctxStr = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("%dM", m.ContextLength/1000000))
	} else if m.ContextLength >= 1000 {
		ctxStr = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("%dK", m.ContextLength/1000))
	}

	row := cursor + nameStr
	if priceBadge != "" {
		row += "  " + priceBadge
	}
	if ctxStr != "" {
		row += "  " + ctxStr
	}
	if capsStr != "" {
		row += "  " + capsStr
	}
	return row
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

// titleCase uppercases the first rune of s.
func titleCase(s string) string {
	if s == "" {
		return s
	}
	runes := []rune(s)
	runes[0] = unicode.ToUpper(runes[0])
	return string(runes)
}
