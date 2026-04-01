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
		if i < current {
			// Completed step: checkmark
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Success).Render("✓"))
		} else if i == current {
			// Current step: filled dot
			parts = append(parts, lipgloss.NewStyle().Foreground(t.Brand).Render("●"))
		} else {
			// Future step: empty dot
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

	// Compose branded header tab above the panel's top border edge.
	tabContent := fr.renderWelcomeHeaderRow(panelW)
	tabCentered := lipgloss.Place(panelW, 1, lipgloss.Center, lipgloss.Center, tabContent)
	allLines := append([]string{tabCentered}, panelLines...)
	panelH := len(allLines)

	// Degenerate case: terminal too cramped for the starfield composition.
	if availH < 8 || availW < 30 {
		if panelH > availH {
			allLines = allLines[:availH]
		}
		return centerScreen(strings.Join(allLines, "\n"), availW, availH)
	}

	// Layout: starfield rows fill the space above and below the centered
	// panel. allLines includes the branded header tab followed by the panel
	// border, so the tab appears to float above the double-bordered frame.
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
	out = append(out, allLines...)
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

// renderWelcomePanel builds the redesigned "Command Center" welcome panel.
//
// Layout (wide ≥56 inner width):
//
//	╭──────── M 3 1 A ────────╮  ← double border + branded header tab
//	│                          │
//	│      ▂▃▅▆▇▇▆▅▃▂         │  ← accent line above logo
//	│      [  M31A logo  ]     │
//	│      ▂▃▅▆▇▇▆▅▃▂         │  ← accent line below logo
//	│   ░▒▓████████████▓▒░    │  ← gradient separator
//	│                          │
//	│   AI pair programmer     │  ← tagline
//	│   in the terminal        │
//	│                          │
//	│   ⚡          🔧        ⎇│  ← feature pipeline
//	│   Workflows──▶Tools──▶Git│
//	│   Plan→Ship   Bash   Live│
//	│                          │
//	│   [ 1 ] Fix tests        │  ← quick-start prompts
//	│   [ 2 ] Refactor API     │
//	│   [ 3 ] Explain arch     │
//	│                          │
//	│   ●○○○          [↵] [q] │  ← step dots + key hints
//	╰──────────────────────────╯
//
// Narrow (<56): features collapse to compact labeled rows.
func (fr *FirstRunModel) renderWelcomePanel(innerW, panelW int) string {
	t := fr.theme

	logoBlock := components.RenderLogo("", true, t.Brand)
	accent := fr.renderAccentLine(innerW)
	separator := fr.renderGradientSeparator(innerW)

	tagline := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Italic(true).
		Render("Your AI pair programmer in the terminal")

	var features string
	if innerW >= 56 {
		features = fr.renderFeaturePipeline(innerW)
	} else {
		features = fr.renderCompactFeatures()
	}

	quickStart := fr.renderQuickStart(innerW)
	hints := fr.renderWelcomeHints()

	var body string
	if innerW >= 56 {
		body = lipgloss.JoinVertical(lipgloss.Center,
			accent,
			"",
			logoBlock,
			"",
			accent,
			"",
			separator,
			"",
			tagline,
			"",
			features,
			"",
			quickStart,
			"",
			hints,
		)
	} else {
		body = lipgloss.JoinVertical(lipgloss.Center,
			logoBlock,
			"",
			separator,
			"",
			tagline,
			"",
			features,
			"",
			quickStart,
			"",
			hints,
		)
	}

	body = padLines(body, innerW)

	return lipgloss.NewStyle().
		Border(lipgloss.DoubleBorder()).
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

// ─── Welcome panel section renderers ────────────────────────────────────────

// renderWelcomeHeaderRow builds a branded tab that sits on the top border edge.
// Result: ══╡ M 3 1 A ╞═════════════════════
func (fr *FirstRunModel) renderWelcomeHeaderRow(panelW int) string {
	t := fr.theme
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(" M 3 1 A ")
	leftW := (panelW - lipgloss.Width(title)) / 2
	if leftW < 2 {
		leftW = 2
	}
	rightW := panelW - lipgloss.Width(title) - leftW
	if rightW < 2 {
		rightW = 2
	}
	barStyle := lipgloss.NewStyle().Foreground(t.Brand)
	left := barStyle.Render(strings.Repeat("═", leftW-1) + "╡")
	right := barStyle.Render("╞" + strings.Repeat("═", rightW-1))
	return left + title + right
}

// renderAccentLine renders a thin decorative line using tapered block characters.
// Used above and below the logo for a subtle glow frame.
func (fr *FirstRunModel) renderAccentLine(width int) string {
	t := fr.theme
	if width < 12 {
		return lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("▂", width))
	}
	chars := []string{"▂", "▃", "▅", "▆", "▇"}
	var left, right strings.Builder
	for _, ch := range chars {
		styled := lipgloss.NewStyle().Foreground(t.Brand).Render(ch)
		left.WriteString(styled)
		right.WriteString(styled)
	}
	centerW := width - 2*len(chars)
	if centerW < 2 {
		centerW = 2
	}
	center := lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("▇", centerW))
	return left.String() + center + right.String()
}

// renderGradientSeparator renders a flowing gradient bar using ░▒▓█ characters.
func (fr *FirstRunModel) renderGradientSeparator(width int) string {
	t := fr.theme
	if width <= 0 {
		return ""
	}
	if width <= 8 {
		return lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("█", width))
	}
	edge := []string{"░", "▒", "▓"}
	var b strings.Builder
	for _, ch := range edge {
		b.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(ch))
	}
	centerW := width - 2*len(edge)
	if centerW < 1 {
		centerW = 1
	}
	b.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(strings.Repeat("█", centerW)))
	for i := len(edge) - 1; i >= 0; i-- {
		b.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(edge[i]))
	}
	return b.String()
}

// renderFeaturePipeline renders three feature blocks connected with ──▶ arrows.
func (fr *FirstRunModel) renderFeaturePipeline(width int) string {
	t := fr.theme
	arrowW := 3
	cardW := (width - 2*arrowW) / 3
	if cardW < 12 {
		cardW = 12
	}

	c1 := fr.renderFeatureBlock("⚡", "Workflows", "Plan → Execute → Ship", cardW)
	c2 := fr.renderFeatureBlock("🔧", "Tools", "Bash · Read · Write · Grep", cardW)
	c3 := fr.renderFeatureBlock("⎇", "Git Aware", "Live repo status", cardW)

	arrow := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("─▶")

	return lipgloss.JoinHorizontal(lipgloss.Top, c1, " "+arrow+" ", c2, " "+arrow+" ", c3)
}

// renderFeatureBlock renders a single feature block: icon header, bold title, muted desc.
func (fr *FirstRunModel) renderFeatureBlock(icon, title, desc string, w int) string {
	t := fr.theme
	iconRow := lipgloss.NewStyle().Foreground(t.Brand).Render(icon)
	titleRow := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render(title)
	descRow := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(desc)
	content := lipgloss.JoinVertical(lipgloss.Left, iconRow, titleRow, descRow)
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Width(w).
		Render(content)
}

// renderQuickStart renders numbered suggestion prompts.
func (fr *FirstRunModel) renderQuickStart(width int) string {
	t := fr.theme

	title := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		Render("QUICK START")

	type prompt struct {
		num  int
		text string
	}
	prompts := []prompt{
		{1, "Fix the failing tests in this repo"},
		{2, "Add error handling to the API layer"},
		{3, "Explain this codebase architecture"},
	}

	var lines []string
	for _, p := range prompts {
		numBadge := lipgloss.NewStyle().
			Foreground(t.Brand).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Brand).
			Padding(0, 1).
			Render(fmt.Sprintf("%d", p.num))
		text := lipgloss.NewStyle().Foreground(t.TextPrimary).Render(" " + p.text)
		lines = append(lines, numBadge+text)
	}

	return lipgloss.JoinVertical(lipgloss.Left, title, "", strings.Join(lines, "\n"))
}

// renderCompactFeatures renders features as compact labeled rows for narrow terminals.
func (fr *FirstRunModel) renderCompactFeatures() string {
	t := fr.theme
	type feat struct {
		icon  string
		label string
	}
	feats := []feat{
		{"⚡", "Workflows: Plan → Execute → Ship"},
		{"🔧", "Tools: Bash · Read · Write · Grep"},
		{"⎇", "Git Aware: Live repo status"},
	}
	var lines []string
	for _, f := range feats {
		icon := lipgloss.NewStyle().Foreground(t.Brand).Render(f.icon)
		label := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(" " + f.label)
		lines = append(lines, icon+label)
	}
	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}

// renderWelcomeHints renders a dual-section footer: step dots + separator + key badges.
func (fr *FirstRunModel) renderWelcomeHints() string {
	t := fr.theme
	dots := fr.renderStepDots(0)

	enterBadge := keyBadge("↵", "Continue", t.Brand, t.TextMuted)
	quitBadge := keyBadge("q", "Quit", t.TextMuted, t.TextMuted)

	return lipgloss.JoinHorizontal(lipgloss.Center,
		dots,
		lipgloss.NewStyle().Foreground(t.Border).Render("   │   "),
		enterBadge,
		"  ",
		quitBadge,
	)
}

// ─── Provider selection step ─────────────────────────────────────────────────

func (fr *FirstRunModel) renderProviderSelect() string {
	t := fr.theme
	// Use effectiveWidth (which accounts for the sidebar and the parent's
	// contentWidth) rather than fr.width directly. The parent wraps this
	// output in a rounded box with Width(w - 4) and Padding(1, 3), so the
	// visible inner content area is (w - 4) - 2 (border) - 6 (pad) = w - 12.
	// Add 2 back because lipgloss's Width includes the border in its outer
	// measurement, giving a practical inner budget of w - 10.
	w := fr.effectiveWidth()
	if w < 32 {
		w = 70
	}
	innerW := w - 10
	if innerW < 22 {
		innerW = 60
	}
	// Vertical budget: outer box adds border (2) + vertical padding (1+1) = 4,
	// plus a 2-row safety margin for centerScreen.
	h := fr.height
	if h < 1 {
		h = 24 // sane default if dimensions haven't been pushed yet
	}
	availH := h - 6
	if availH < 10 {
		availH = 10
	}

	// ── Row 1: step title + progress dots ─────────────────────────────────
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
		Render("Step 1 of 4 — Choose your providers")
	dots := fr.renderStepDots(1)
	stepLine := lipgloss.JoinHorizontal(lipgloss.Center,
		title,
		lipgloss.NewStyle().Foreground(t.Border).Render("   "),
		dots,
	)

	// ── Row 2: subtitle or validation error (single status row) ───────────
	var statusRow string
	if fr.keyErr != "" {
		statusRow = lipgloss.NewStyle().Foreground(t.Error).
			Render("⚠ " + fr.keyErr)
	} else {
		statusRow = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Select one or more providers. You can switch between them anytime.")
	}

	// ── Provider cards ────────────────────────────────────────────────────
	// Always render the full catalog. The catalog is small (2 providers in
	// V1), so the viewport heuristic is premature optimization — and it was
	// silently dropping cards on some terminal sizes. The hard line clamp
	// below handles overflow on genuinely tiny terminals.
	fr.clampProviderScroll()
	var cards []string
	for i, p := range providerCatalog {
		cards = append(cards, fr.renderProviderCard(p, i == fr.providerCursor, innerW-2))
	}
	cardsBlock := strings.Join(cards, "\n\n")

	// ── Summary row: count + selected chips ──────────────────────────────
	summary := fr.renderProviderSummary(innerW)

	// ── Hints row ─────────────────────────────────────────────────────────
	navBadge := keyBadge("↑↓", "Navigate", t.Brand, t.TextMuted)
	toggleBadge := keyBadge("space", "Toggle", t.Brand, t.TextMuted)
	selBadge := keyBadge("↵", "Confirm", t.Brand, t.TextMuted)
	skipBadge := keyBadge("s", "Skip", t.Warning, t.TextMuted)
	backBadge := keyBadge("esc", "Back", t.TextMuted, t.TextMuted)
	hints := lipgloss.JoinHorizontal(lipgloss.Center,
		navBadge, "  ", toggleBadge, "  ", selBadge, "  ", skipBadge, "  ", backBadge)

	// Compose, pad each line to innerW, and hard-clamp to availH rows so the
	// outer box never overflows the terminal — even on very small sizes.
	parts := []string{stepLine, "", statusRow, "", cardsBlock, "", summary, "", hints}
	body := lipgloss.JoinVertical(lipgloss.Left, parts...)
	body = padLines(body, innerW)

	lines := strings.Split(body, "\n")
	if len(lines) > availH {
		lines = lines[:availH]
	}
	return strings.Join(lines, "\n")
}

// plural returns "s" when n != 1, for use in count-bearing status strings.
func plural(n int) string {
	if n == 1 {
		return ""
	}
	return "s"
}

// renderProviderSummary renders a single-line summary showing selected count
// and chip row for chosen providers. The output is truncated to innerW so it
// never breaks the layout.
func (fr *FirstRunModel) renderProviderSummary(innerW int) string {
	t := fr.theme

	n := len(fr.selectedProviders)
	var countBadge string
	switch {
	case n == 0:
		countBadge = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Render("none selected")
	case n == 1:
		countBadge = lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render("1 selected")
	default:
		countBadge = lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render(fmt.Sprintf("%d selected", n))
	}

	if n == 0 {
		hint := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  press space to select")
		return truncateStyled(countBadge+hint, innerW)
	}

	// Build chip row. Stop appending chips once we'd exceed innerW.
	sep := lipgloss.NewStyle().Foreground(t.Border).Render(" ")
	prefix := countBadge + lipgloss.NewStyle().Foreground(t.Border).Render(" │")
	used := lipgloss.Width(prefix)
	var chips []string
	for i, pid := range fr.selectedProviders {
		info := lookupProviderInfo(pid)
		label := info.Icon + " " + info.Name
		var chip string
		if i == 0 {
			chip = lipgloss.NewStyle().
				Foreground(t.Brand).
				Border(lipgloss.RoundedBorder()).
				BorderForeground(t.Brand).
				Padding(0, 1).
				Render("★ " + label)
		} else {
			chip = lipgloss.NewStyle().
				Foreground(t.TextSecondary).
				Border(lipgloss.RoundedBorder()).
				BorderForeground(t.Border).
				Padding(0, 1).
				Render(label)
		}
		chipW := lipgloss.Width(chip) + lipgloss.Width(sep)
		if used+chipW+4 > innerW { // leave 4 cells headroom
			chips = append(chips, lipgloss.NewStyle().Foreground(t.TextMuted).Render("…"))
			break
		}
		chips = append(chips, chip)
		used += chipW
	}

	row := prefix + sep + strings.Join(chips, sep)
	return truncateStyled(row, innerW)
}

// lookupProviderInfo returns catalog metadata for a provider ID, falling back
// to a minimal entry if the ID isn't in the catalog.
func lookupProviderInfo(id string) providerInfo {
	for _, p := range providerCatalog {
		if p.ID == id {
			return p
		}
	}
	return providerInfo{ID: id, Name: id, Icon: "◆"}
}

func (fr *FirstRunModel) renderProviderCard(p providerInfo, selected bool, w int) string {
	t := fr.theme
	if w < 20 {
		w = 40
	}

	checked := fr.providerChecked[p.ID]

	// Checkbox — always rendered with an explicit color so it's visible on
	// both light and dark terminal themes.
	var checkBox string
	if checked {
		box := lipgloss.NewStyle().Foreground(t.Success).Bold(true).Render("▣")
		checkBox = lipgloss.NewStyle().Foreground(t.Success).Render("[") +
			box +
			lipgloss.NewStyle().Foreground(t.Success).Render("] ")
	} else {
		checkBox = lipgloss.NewStyle().Foreground(t.TextMuted).Render("[ ] ")
	}

	// Icon + Name row
	var iconStyle, nameStyle lipgloss.Style
	if checked {
		iconStyle = lipgloss.NewStyle().Foreground(t.Success).Bold(true)
	} else if selected {
		iconStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	} else {
		iconStyle = lipgloss.NewStyle().Foreground(t.Brand)
	}
	if selected {
		nameStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	} else {
		nameStyle = lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	}

	icon := iconStyle.Render(p.Icon)
	name := nameStyle.Render(p.Name)

	// Badges
	var badges string
	if p.Recommended {
		badges += " " + lipgloss.NewStyle().
			Foreground(t.Success).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Success).
			Padding(0, 1).
			Render("Recommended")
	}
	if checked && len(fr.selectedProviders) > 0 && fr.selectedProviders[0] == p.ID {
		badges += " " + lipgloss.NewStyle().
			Foreground(t.Brand).
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Brand).
			Padding(0, 1).
			Render("★ default")
	}

	titleRow := checkBox + icon + "  " + name + badges
	desc := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(p.Description)

	// Fixed-height body: exactly 3 content rows (title, blank, desc). No
	// conditional footer — selected/checked state is conveyed by prefix +
	// border + color only. This keeps every card the same height.
	content := lipgloss.JoinVertical(lipgloss.Left, titleRow, "", desc)

	// Border reflects state: brand = focused, success = checked, muted = idle.
	borderColor := t.Border
	if selected {
		borderColor = t.Brand
	} else if checked {
		borderColor = t.Success
	}

	card := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Padding(0, 1).
		Width(w - 4).
		Render(content)

	// Cursor prefix — bold arrow on the focused card, subtle tick on checked
	// cards. Both are a single character so card height is unaffected.
	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▶ ")
	} else if checked {
		prefix = lipgloss.NewStyle().Foreground(t.Success).Render("✓ ")
	}

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
			Render(fmt.Sprintf("Step 3/4 — Enter %s API key (%d/%d)", provName, current, total))
	} else {
		title = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).
			Render(fmt.Sprintf("Step 3/4 — Enter %s API key", provName))
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
		Render("Step 4/4 — Choose your default model")
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
