package tui

import (
	"fmt"
	"strings"
	"unicode"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/core/types"
)

// ─── API key step ────────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderAPIKeyStep() string {
	t := fr.theme
	w := fr.effectiveWidth()
	if w <= 0 {
		w = 70
	}
	innerW := w - 10
	if innerW < 22 {
		innerW = 22
	}

	total := len(fr.selectedProviders)
	current := fr.keyProviderIndex + 1
	provName := ""
	if fr.keyProviderIndex < total {
		provName = titleCase(fr.selectedProviders[fr.keyProviderIndex])
	}

	// ── Step bar ──────────────────────────────────────────────────────────
	stepBar := renderStepBar(3, firstRunStepCount, "API key", innerW, t.Brand, t.Border)

	// ── Subtitle ──────────────────────────────────────────────────────────
	var subtitle string
	if total > 1 {
		subtitle = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("Enter %s API key (%d/%d)", provName, current, total))
	} else {
		subtitle = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("Enter %s API key", provName))
	}

	// ── Security note ─────────────────────────────────────────────────────
	desc := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("Your key is stored securely and never sent anywhere else.")

	// ── API key input ─────────────────────────────────────────────────────
	keyBox := lipgloss.NewStyle().
		Padding(0, 1).
		Render(fr.keyInput.View())

	// ── Keychain toggle (plain inline, no border) ─────────────────────────
	var toggleIcon string
	var toggleStyle lipgloss.Style
	if fr.opts.SaveKeychain {
		toggleIcon = "✓"
		toggleStyle = lipgloss.NewStyle().Foreground(t.Success)
	} else {
		toggleIcon = "○"
		toggleStyle = lipgloss.NewStyle().Foreground(t.TextMuted)
	}
	saveLabel := toggleStyle.Render(toggleIcon+" Save to system keychain") +
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  (tab to toggle)")

	// ── Status lines ──────────────────────────────────────────────────────
	var statusLines []string
	if fr.keyValidating {
		statusLines = append(statusLines,
			lipgloss.NewStyle().Foreground(t.Brand).Render("⠋ Validating API key..."))
	}
	if fr.keyValidationErr != "" {
		statusLines = append(statusLines,
			lipgloss.NewStyle().Foreground(t.Error).Render("✗ "+fr.keyValidationErr))
	} else if fr.keyErr != "" {
		statusLines = append(statusLines,
			lipgloss.NewStyle().Foreground(t.Error).Render("⚠ "+fr.keyErr))
	}

	// ── Compact hints ─────────────────────────────────────────────────────
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	keyStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	hints := lipgloss.JoinHorizontal(lipgloss.Center,
		keyStyle.Render("↵"), hintStyle.Render(" confirm  "),
		keyStyle.Render("tab"), hintStyle.Render(" keychain  "),
		keyStyle.Render("esc"), hintStyle.Render(" back"),
	)

	// ── Compose ───────────────────────────────────────────────────────────
	parts := []string{stepBar, "", subtitle, "", desc, "", keyBox, "", saveLabel}
	parts = append(parts, statusLines...)
	parts = append(parts, "", hints)
	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// ─── Model pick step ────────────────────────────────────────────────────────

func (fr *FirstRunModel) renderModelPickStep() string {
	t := fr.theme
	w := fr.width
	if w <= 0 {
		w = 80
	}
	if fr.contentWidth > 0 {
		w = fr.contentWidth
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

	// ── Step bar ──────────────────────────────────────────────────────────
	stepBar := renderStepBar(4, firstRunStepCount, "Choose your default model", innerW, t.Brand, t.Border)

	// ── Compact hints (reused across branches) ────────────────────────────
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	keyStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	baseHints := func() string {
		return lipgloss.JoinHorizontal(lipgloss.Center,
			keyStyle.Render("↑↓"), hintStyle.Render(" navigate  "),
			keyStyle.Render("tab"), hintStyle.Render(" category  "),
			keyStyle.Render("↵"), hintStyle.Render(" confirm  "),
			keyStyle.Render("esc"), hintStyle.Render(" back"),
		)
	}

	// ── Bottom-up layout ──────────────────────────────────────────────────
	// The outer renderFirstRun() wraps content in a box with:
	//   Border(2) + Padding(1,3) = 4 extra rows
	// Inside the box, fixed rows are:
	//   stepBar(1) + blank(1) + tab(1) + blank(1) + listBorder(2) +
	//   blank(1) + inputLabel(1) + inputBox(3) + blank(1) + hints(1) = 13
	// Total overhead = 4 (box) + 13 (inner) = 17
	const overheadRows = 17
	listBudget := h - overheadRows
	if listBudget < 3 {
		listBudget = 3
	}
	// Always cap at 12 visible rows so the input + hints stay on screen
	// and scrolling is always available for longer lists.
	if listBudget > 12 {
		listBudget = 12
	}

	var parts []string
	parts = append(parts, stepBar, "")

	// ── Loading ───────────────────────────────────────────────────────────
	if fr.modelsLoading {
		spinner := lipgloss.NewStyle().Foreground(t.Brand).Render("⠋")
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("Fetching all models from " + titleCase(fr.opts.DefaultProvider) + "...")
		parts = append(parts, spinner+" "+desc)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// ── No provider configured ────────────────────────────────────────────
	if fr.opts.DefaultProvider == "" {
		inputBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Width(innerW).
			Render(fr.modelInput.View())
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).
			Render("No provider configured. Enter a model ID or press ↵ to continue.")
		hints := lipgloss.JoinHorizontal(lipgloss.Center,
			keyStyle.Render("↵"), hintStyle.Render(" confirm  "),
			keyStyle.Render("esc"), hintStyle.Render(" back"),
		)
		parts = append(parts, desc, "", inputBox, "", hints)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// ── Full categorized browser ──────────────────────────────────────────
	b := fr.browser
	if b == nil || len(b.categories) == 0 {
		inputBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Padding(0, 1).
			Width(innerW).
			Render(fr.modelInput.View())
		warnLine := lipgloss.NewStyle().Foreground(t.Warning).
			Render("⚠  Could not fetch models. Enter a model ID manually.")
		hints := lipgloss.JoinHorizontal(lipgloss.Center,
			keyStyle.Render("↵"), hintStyle.Render(" confirm  "),
			keyStyle.Render("esc"), hintStyle.Render(" back"),
		)
		parts = append(parts, warnLine, "", inputBox, "", hints)
		return lipgloss.JoinVertical(lipgloss.Left, parts...)
	}

	// ── Category tab bar (compact, scrollable) ────────────────────────────
	// Measure available width for tabs. Active tab always visible; render
	// from catCursor outward, stopping when we run out of width.
	maxTabW := innerW - 4 // leave room for overflow indicator
	var tabParts []string
	usedTabW := 0
	// Always include the active tab first
	for offset := 0; offset < len(b.categories); offset++ {
		i := (b.catCursor + offset) % len(b.categories)
		cat := b.categories[i]
		count := fmt.Sprintf("%d", len(cat.Models))
		label := cat.Icon + " " + cat.Title + " (" + count + ")"
		var tab string
		if i == b.catCursor {
			tab = lipgloss.NewStyle().
				Foreground(t.Brand).Bold(true).
				Border(lipgloss.RoundedBorder()).BorderForeground(t.Brand).
				Padding(0, 1).Render(label)
		} else {
			tab = lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Border(lipgloss.RoundedBorder()).BorderForeground(t.Border).
				Padding(0, 1).Render(label)
		}
		tw := lipgloss.Width(tab)
		if usedTabW+tw > maxTabW && offset > 0 {
			// Show overflow indicator
			tabParts = append(tabParts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("…"))
			break
		}
		tabParts = append(tabParts, tab)
		usedTabW += tw + 1
	}
	tabBar := lipgloss.JoinHorizontal(lipgloss.Top, tabParts...)
	parts = append(parts, tabBar, "")

	// ── Model list with scrollbar ─────────────────────────────────────────
	cat := b.activeCat()
	listW := innerW
	if listW < 20 {
		listW = 20
	}

	if cat != nil && len(cat.Models) > 0 {
		listH := listBudget
		if listH < 3 {
			listH = 3
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

		// Reserve space for scrollbar (3 chars) on wide terminals
		scrollBarW := 0
		if listW > 40 && len(cat.Models) > listH {
			scrollBarW = 3
		}
		modelW := listW - scrollBarW

		// Render model rows
		var rows []string
		for i := b.scrollOffset; i < end; i++ {
			rows = append(rows, fr.renderModelRow(cat.Models[i], i == b.modelCursor, modelW))
		}

		// Build scrollbar
		var scrollBar string
		if scrollBarW > 0 {
			scrollBar = fr.renderScrollbar(len(cat.Models), listH, b.scrollOffset, t)
		}

		// Compose list: rows on the left, scrollbar on the right
		listContent := strings.Join(rows, "\n")
		if scrollBarW > 0 {
			// Pad rows to modelW, then append scrollbar
			listLines := strings.Split(listContent, "\n")
			for i, line := range listLines {
				lineW := lipgloss.Width(line)
				if lineW < modelW {
					listLines[i] = line + strings.Repeat(" ", modelW-lineW)
				}
			}
			scrollLines := strings.Split(scrollBar, "\n")
			// Merge line by line
			merged := make([]string, len(listLines))
			for i := range listLines {
				if i < len(scrollLines) {
					merged[i] = listLines[i] + scrollLines[i]
				} else {
					merged[i] = listLines[i] + strings.Repeat(" ", scrollBarW)
				}
			}
			listContent = strings.Join(merged, "\n")
		}

		listBox := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Border).
			Width(listW).
			Render(listContent)

		// Store list Y offset for mouse handling
		// stepBar(1) + blank(1) + tabBar(1) + blank(1) = 4 rows above list
		fr.modelListY = 4
		fr.modelListH = end - b.scrollOffset
		parts = append(parts, listBox)
	} else {
		fr.modelListY = 0
		fr.modelListH = 0
	}

	// ── Selected model ID input ───────────────────────────────────────────
	inputLabel := lipgloss.NewStyle().Foreground(t.TextSecondary).
		Render("Selected model (edit to override):")
	inputBox := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Width(innerW).
		Render(fr.modelInput.View())
	parts = append(parts, "", inputLabel, inputBox)

	// ── Hints (always visible at bottom) ──────────────────────────────────
	parts = append(parts, "", baseHints())

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderScrollbar renders a vertical scrollbar showing position in the list.
func (fr *FirstRunModel) renderScrollbar(total, visible, offset int, t theme.Theme) string {
	if total <= visible {
		return ""
	}
	barH := visible
	thumbH := 1
	if barH > 2 {
		thumbH = barH * visible / total
		if thumbH < 1 {
			thumbH = 1
		}
	}
	thumbPos := 0
	if total-visible > 0 {
		thumbPos = offset * (barH - thumbH) / (total - visible)
	}

	var lines []string
	for i := 0; i < barH; i++ {
		if i >= thumbPos && i < thumbPos+thumbH {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ▌"))
		} else {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.Border).Render(" ·"))
		}
	}
	return strings.Join(lines, "\n")
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
	maxName := maxW - 20
	if maxName < 8 {
		maxName = 8
	}
	if maxName > maxW-10 {
		maxName = maxW - 10
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

	title := lipgloss.NewStyle().
		Foreground(t.Success).Bold(true).
		Render("✓ Setup complete!")

	subtitle := lipgloss.NewStyle().
		Foreground(t.TextPrimary).Bold(true).
		Render("Here's how to use M31A:")

	instructions := []string{
		"1. Type a task description (e.g., \"fix the failing tests\")",
		"2. M31A will discuss, plan, and execute it",
		"3. Press Esc to go back, ? for help",
	}

	var lines []string
	lines = append(lines, title, "", subtitle, "")
	for _, line := range instructions {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextSecondary).Render("  "+line))
	}
	lines = append(lines, "",
		lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render("Press Enter to continue to M31A"),
	)

	return lipgloss.JoinVertical(lipgloss.Left, lines...)
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
