package first_run

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// renderFirstRun renders the appropriate wizard step.
func (fr *FirstRunModel) renderFirstRun() string {
	t := fr.theme
	w := fr.width
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	// Use contentWidth (set by parent to account for sidebar) if available.
	// Otherwise fall back to subtracting sidebarDefaultWidth for full-width terminals.
	centerW := w
	if fr.contentWidth > 0 {
		centerW = fr.contentWidth
	} else if w >= tuitypes.WidthFull {
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

	boxW := centerW - 4
	box := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 3).
		Width(boxW).
		Render(content)

	return centerScreen(box, boxW, fr.height)
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

// renderStepBar renders a labeled horizontal rule: "─── Step N of 4 · Label ───".
func renderStepBar(current, total int, label string, w int, brand, muted lipgloss.Color) string {
	tag := fmt.Sprintf("Step %d of %d · %s", current, total, label)
	styled := lipgloss.NewStyle().Foreground(brand).Bold(true).Render(tag)
	pad := w - lipgloss.Width(styled) - 6 // 3 chars each side for the rule
	if pad < 4 {
		pad = 4
	}
	left := strings.Repeat("─", pad/2+1)
	right := strings.Repeat("─", pad-pad/2+1)
	rule := lipgloss.NewStyle().Foreground(muted).Render(left + " " + styled + " " + right)
	return rule
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

	// Responsive panel width: scale with terminal width, leaving margin for
	// the starfield to peek through on wide terminals.
	panelW := availW - 4
	if panelW > availW {
		panelW = availW
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

	// Center vertically by adding empty rows top/bottom.
	topPad := (availH - len(out)) / 2
	if topPad < 0 {
		topPad = 0
	}
	bottomPad := availH - len(out) - topPad
	if bottomPad < 0 {
		bottomPad = 0
	}

	// Center each line horizontally within availW.
	leftPad := (availW - panelW) / 2
	if leftPad < 0 {
		leftPad = 0
	}
	padStr := strings.Repeat(" ", leftPad)

	var finalRows []string
	for i := 0; i < topPad; i++ {
		finalRows = append(finalRows, strings.Repeat(" ", availW))
	}
	for _, line := range out {
		// Starfield lines are already availW wide; pad panel lines.
		if lipgloss.Width(line) < availW {
			finalRows = append(finalRows, padStr+line)
		} else {
			finalRows = append(finalRows, line)
		}
	}
	for i := 0; i < bottomPad; i++ {
		finalRows = append(finalRows, strings.Repeat(" ", availW))
	}

	result := strings.Join(finalRows, "\n")
	return result
}

// renderWelcomePanel builds the welcome panel with a large logo, glow effect,
// feature cards, and quick-start prompts.
//
// Layout (wide ≥56 inner width):
//
//	╭──────── M 3 1 A ────────╮  ← double border + branded header tab
//	│                          │
//	│   █████╗  ██████╗  ...   │  ← big block logo
//	│   ...                    │
//	│   ▓▓▓▒▒▒░░░··  ···░░░▒▒▓ │  ← glow fade
//	│                          │
//	│   Your AI pair programmer│  ← tagline
//	│   in the terminal        │
//	│                          │
//	│  ┌─ Workflows ─┐  ...   │  ← feature cards
//	│  │  Plan→Ship   │        │
//	│  └─────────────┘        │
//	│                          │
//	│  › Fix the failing tests │  ← quick-start prompts
//	│  › Add error handling    │
//	│  › Explain architecture  │
//	│                          │
//	│   ●○○○          [↵] [q] │  ← step dots + key hints
//	╰──────────────────────────╯
//
// Narrow (<56): features collapse to compact labeled rows.
func (fr *FirstRunModel) renderWelcomePanel(innerW, panelW int) string {
	t := fr.theme

	logoBlock := components.RenderBigLogo(t.Brand, true, fr.effectiveWidth(), resolveLogoText(fr.config.UI))

	tagline := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Italic(true).
		Render("Your AI pair programmer in the terminal")

	var features string
	if innerW >= 56 {
		features = fr.renderFeatureCards(innerW)
	} else {
		features = fr.renderCompactFeatures()
	}

	quickStart := fr.renderQuickStart(innerW)
	hints := fr.renderWelcomeHints()

	var body string
	if innerW >= 56 {
		body = lipgloss.JoinVertical(lipgloss.Center,
			"",
			logoBlock,
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

// renderFeatureCards renders three feature cards with accent top borders.
func (fr *FirstRunModel) renderFeatureCards(width int) string {
	t := fr.theme
	gapW := 2
	cardW := (width - 2*gapW) / 3
	if cardW < 12 {
		cardW = 12
	}
	// Clamp card width so 3 cards + gaps don't exceed available width
	maxCardW := (width - 2*gapW) / 3
	if cardW > maxCardW && maxCardW > 0 {
		cardW = maxCardW
	}

	c1 := fr.renderFeatureCard("⚡", "Workflows", "Plan → Execute → Ship", cardW, t.Brand)
	c2 := fr.renderFeatureCard("🔧", "Tools", "Bash · Read · Write · Grep", cardW, t.Secondary)
	c3 := fr.renderFeatureCard("⎇", "Git Aware", "Live repo status", cardW, t.Success)

	return lipgloss.JoinHorizontal(lipgloss.Top, c1, " ", c2, " ", c3)
}

// renderFeatureCard renders a single feature card with a colored top accent line.
func (fr *FirstRunModel) renderFeatureCard(icon, title, desc string, w int, accent lipgloss.Color) string {
	t := fr.theme
	topLine := lipgloss.NewStyle().Foreground(accent).Render(strings.Repeat("─", w-2))
	iconRow := lipgloss.NewStyle().Foreground(accent).Render(icon + " ")
	titleRow := lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true).Render(title)
	descRow := lipgloss.NewStyle().Foreground(t.TextSecondary).Render(desc)
	content := lipgloss.JoinVertical(lipgloss.Left, topLine, "", iconRow+titleRow, descRow)
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Border).
		Padding(0, 1).
		Width(w).
		Render(content)
}

// renderQuickStart renders numbered suggestion prompts with a brand-colored index.
func (fr *FirstRunModel) renderQuickStart(width int) string {
	t := fr.theme

	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Render("Quick start")

	prompts := []string{
		"Fix the failing tests in this repo",
		"Add error handling to the API layer",
		"Explain this codebase architecture",
	}
	nums := []string{"1.", "2.", "3."}

	var lines []string
	for i, text := range prompts {
		num := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(nums[i])
		body := lipgloss.NewStyle().Foreground(t.TextPrimary).Render(" " + text)
		lines = append(lines, "  "+num+body)
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
		lines = append(lines, "  "+icon+label)
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
	w := fr.effectiveWidth()
	if w <= 0 {
		w = 80
	}
	innerW := w - 10
	if innerW < 22 {
		innerW = 22
	}
	// Clamp to actual available interior width
	if boxInner := w - 10; innerW > boxInner && boxInner > 0 {
		innerW = boxInner
	}
	h := fr.height
	if h < 1 {
		h = 24
	}
	availH := h - 6
	if availH < 10 {
		availH = 10
	}

	// ── Step bar ──────────────────────────────────────────────────────────
	stepBar := renderStepBar(1, firstRunStepCount, "Choose your providers", innerW, t.Brand, t.Border)

	// ── Subtitle or validation error ──────────────────────────────────────
	var statusRow string
	if fr.keyErr != "" {
		statusRow = lipgloss.NewStyle().Foreground(t.Error).
			Render("⚠ " + fr.keyErr)
	} else {
		statusRow = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Select one or more providers. You can switch between them anytime.")
	}

	// ── Provider cards (vertical, full-width) ─────────────────────────────
	fr.clampProviderScroll()
	var cards []string
	for i, p := range providerCatalog {
		if i > 0 {
			// Thin separator between cards
			sep := lipgloss.NewStyle().Foreground(t.Border).
				Render(strings.Repeat("─", innerW))
			cards = append(cards, sep)
		}
		cards = append(cards, fr.renderProviderCard(p, i == fr.providerCursor, innerW))
	}
	cardsBlock := lipgloss.JoinVertical(lipgloss.Left, cards...)

	// ── Summary: count + horizontal rule ──────────────────────────────────
	summary := fr.renderProviderSummary(innerW)

	// ── Hints (compact single line) ───────────────────────────────────────
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	keyStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	hints := lipgloss.JoinHorizontal(lipgloss.Center,
		keyStyle.Render("↑↓"), hintStyle.Render(" navigate  "),
		keyStyle.Render("space"), hintStyle.Render(" select  "),
		keyStyle.Render("↵"), hintStyle.Render(" confirm  "),
		keyStyle.Render("s"), hintStyle.Render(" skip  "),
		keyStyle.Render("esc"), hintStyle.Render(" back"),
	)

	// ── Compose ───────────────────────────────────────────────────────────
	parts := []string{stepBar, "", statusRow, "", cardsBlock, "", summary, "", hints}
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

// renderProviderSummary renders a compact count line: "N selected ────────".
func (fr *FirstRunModel) renderProviderSummary(innerW int) string {
	t := fr.theme
	n := len(fr.selectedProviders)

	var label string
	switch n {
	case 0:
		label = lipgloss.NewStyle().Foreground(t.TextMuted).Render("none selected")
	case 1:
		label = lipgloss.NewStyle().Foreground(t.Success).Render("1 selected")
	default:
		label = lipgloss.NewStyle().Foreground(t.Success).
			Render(fmt.Sprintf("%d selected", n))
	}

	rule := lipgloss.NewStyle().Foreground(t.Border).
		Render(strings.Repeat("─", innerW-lipgloss.Width(label)-2))
	return label + " " + rule
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
	checked := fr.providerChecked[p.ID]

	// ── Line 1: cursor + icon + name + status pill (right-aligned) ────────
	var cursor string
	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("▸ ")
	} else {
		cursor = "  "
	}

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
	} else if checked {
		nameStyle = lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	} else {
		nameStyle = lipgloss.NewStyle().Foreground(t.TextPrimary).Bold(true)
	}

	icon := iconStyle.Render(p.Icon)
	name := nameStyle.Render(p.Name)

	// Status pill (right side of line 1)
	var statusPill string
	if checked {
		statusPill = lipgloss.NewStyle().
			Foreground(t.Success).Bold(true).
			Render("● selected")
	} else if selected {
		statusPill = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render("○ press space")
	}

	// Badge tags after name (Recommended, default)
	var tags string
	if p.Recommended {
		tags += " " + lipgloss.NewStyle().
			Foreground(t.Warning).
			Render("★ recommended")
	}
	if checked && len(fr.selectedProviders) > 0 && fr.selectedProviders[0] == p.ID {
		tags += " " + lipgloss.NewStyle().
			Foreground(t.Brand).
			Render("★ default")
	}

	// Compose line 1: left side + pad + right side
	leftSide := cursor + icon + "  " + name + tags
	leftW := lipgloss.Width(leftSide)
	rightW := lipgloss.Width(statusPill)
	gap := w - leftW - rightW
	if gap < 2 {
		gap = 2
	}
	line1 := leftSide + strings.Repeat(" ", gap) + statusPill
	line1 = truncateStyled(line1, w)

	// ── Line 2: indented description ──────────────────────────────────────
	desc := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render("  " + p.Description)
	desc = truncateStyled(desc, w)

	return line1 + "\n" + desc
}
