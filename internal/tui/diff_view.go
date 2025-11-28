package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ─── Diff View Rendering ─────────────────────────────────────────────────────

func renderDiffView(dm *DiffModel) string {
	t := dm.theme
	w := dm.width
	if w < 20 {
		w = 80
	}

	var parts []string

	// ── Header ──────────────────────────────────────────────────────────────────
	headerIcon := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("  DIFF")
	headerSep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" · ")

	titleText := "Git Diff"
	if dm.title != "" {
		titleText = dm.title
	}
	headerTitle := lipgloss.NewStyle().Foreground(t.Text).Bold(true).Render(titleText)

	headerLine := headerIcon + headerSep + headerTitle
	parts = append(parts, headerLine)

	// ── File path bar ───────────────────────────────────────────────────────────
	if dm.filePath != "" {
		filePath := lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).Render(dm.filePath)
		parts = append(parts, filePath)
	}

	// ── Stats row ───────────────────────────────────────────────────────────────
	statsLine := renderDiffStats(dm, t, w)
	parts = append(parts, statsLine)

	// ── Separator ───────────────────────────────────────────────────────────────
	sep := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))
	parts = append(parts, sep)

	// ── Diff body ───────────────────────────────────────────────────────────────
	if dm.diff == "" {
		emptyMsg := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(2).
			PaddingTop(2).
			Render("No diff to display.")
		parts = append(parts, emptyMsg)
	} else {
		parts = append(parts, dm.viewport.View())
	}

	// ── Bottom separator ────────────────────────────────────────────────────────
	parts = append(parts, sep)

	// ── Legend + scroll info ────────────────────────────────────────────────────
	legend := renderDiffLegend(dm, t)
	parts = append(parts, legend)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderDiffStats renders the stats row with additions/deletions count and visual bar.
func renderDiffStats(dm *DiffModel, t theme.Theme, _ int) string {
	total := dm.additions + dm.deletions
	if total == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No changes")
	}

	addBadge := lipgloss.NewStyle().
		Background(t.DiffAdded).Foreground(t.Background).
		Padding(0, 1).Bold(true).
		Render(fmt.Sprintf("+%d", dm.additions))

	delBadge := lipgloss.NewStyle().
		Background(t.DiffRemoved).Foreground(t.Background).
		Padding(0, 1).Bold(true).
		Render(fmt.Sprintf("-%d", dm.deletions))

	// Visual ratio bar (max 20 chars)
	const barWidth = 20
	addBlocks := 0
	delBlocks := 0
	if total > 0 {
		addBlocks = (dm.additions * barWidth) / total
		delBlocks = barWidth - addBlocks
	}
	addBar := lipgloss.NewStyle().Foreground(t.DiffAdded).Render(strings.Repeat("█", addBlocks))
	delBar := lipgloss.NewStyle().Foreground(t.DiffRemoved).Render(strings.Repeat("█", delBlocks))
	ratioBar := addBar + delBar

	return fmt.Sprintf("  %s  %s  %s  %d changes", addBadge, delBadge, ratioBar, total)
}

// renderDiffLegend renders the bottom legend with navigation hints and scroll position.
func renderDiffLegend(dm *DiffModel, t theme.Theme) string {
	added := lipgloss.NewStyle().
		Background(t.DiffAddedBg).
		Foreground(t.DiffAdded).
		Render("+ added")
	removed := lipgloss.NewStyle().
		Background(t.DiffRemovedBg).
		Foreground(t.DiffRemoved).
		Render("- removed")

	// Scroll position
	scrollPct := 0
	if dm.viewport.TotalLineCount() > 0 {
		scrollPct = int(float64(dm.viewport.YOffset+dm.viewport.Height) /
			float64(dm.viewport.TotalLineCount()) * 100)
		if scrollPct > 100 {
			scrollPct = 100
		}
	}
	scrollLabel := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render(fmt.Sprintf("%d%%", scrollPct))

	hints := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↑↓/jk scroll  pgup/pgdown page  esc/q close")

	return "  " + added + "  " + removed + "    " + hints + "    " + scrollLabel
}

// colorizeDiff applies syntax coloring to a unified diff string.
func colorizeDiff(diff string, t theme.Theme) string {
	lines := strings.Split(diff, "\n")
	const lineNumWidth = 4
	var (
		out           []string
		lineNum       int
		lineNumStyle  = lipgloss.NewStyle().Foreground(t.TextMuted).Width(lineNumWidth).Align(lipgloss.Right)
		fileHeaderSty = lipgloss.NewStyle().Foreground(t.TextMuted)
		hunkSty       = lipgloss.NewStyle().Foreground(t.Thinking).Bold(true)
		contextSty    = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)
		addSty        = lipgloss.NewStyle().Background(t.DiffAddedBg).Foreground(t.DiffAdded)
		delSty        = lipgloss.NewStyle().Background(t.DiffRemovedBg).Foreground(t.DiffRemoved)
	)

	for _, line := range lines {
		switch {
		case strings.HasPrefix(line, "@@"):
			out = append(out, "    "+hunkSty.Render(line))
			lineNum = 0

		case strings.HasPrefix(line, "--- ") || strings.HasPrefix(line, "+++ "):
			out = append(out, "    "+fileHeaderSty.Render(line))

		case strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++"):
			lineNum++
			ln := lineNumStyle.Render(fmt.Sprintf("%d", lineNum))
			out = append(out, ln+"│"+addSty.Render(line))

		case strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---"):
			ln := lineNumStyle.Render("")
			out = append(out, ln+"│"+delSty.Render(line))

		case strings.HasPrefix(line, "\\ "):
			out = append(out, "    "+contextSty.Render(line))

		default:
			lineNum++
			ln := lineNumStyle.Render(fmt.Sprintf("%d", lineNum))
			out = append(out, ln+"│"+contextSty.Render(line))
		}
	}
	return strings.Join(out, "\n")
}
