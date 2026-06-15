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
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	var parts []string

	// ── File path bar ───────────────────────────────────────────────────────────
	if dm.filePath != "" {
		filePath := lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).Render(dm.filePath)
		parts = append(parts, filePath)
	}

	// ── Stats row ───────────────────────────────────────────────────────────────
	statsLine := renderDiffStats(dm, t, w)
	parts = append(parts, statsLine)

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

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderDiffStats renders the stats row with additions/deletions count and visual bar.
func renderDiffStats(dm *DiffModel, t theme.Theme, w int) string {
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

	// Visual ratio bar: scale to ~1/6 of terminal width, min 10, max 30
	barWidth := w / 6
	if barWidth < 10 {
		barWidth = 10
	}
	if barWidth > 30 {
		barWidth = 30
	}
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
