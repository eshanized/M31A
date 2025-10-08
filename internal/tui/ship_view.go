package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
)

func (m *ShipModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading ship..."
	}

	var sb strings.Builder

	remaining := m.width - 4
	title := "Launch Pad"
	subtitle := "Ready to Ship"
	headerInner := fmt.Sprintf("%s ── %s", title, subtitle)
	if len(headerInner) < remaining {
		headerInner += strings.Repeat("─", remaining-len(headerInner))
	}
	headerBorder := "╭─ " + headerInner + "╮"
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(headerBorder))
	sb.WriteString("\n")

	separator := strings.Repeat("─", m.width-2)
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Border).
		Render(separator))
	sb.WriteString("\n\n")

	sessionBadge := components.NewBadge(m.summary.SessionID, components.BadgeMuted, m.theme).Render()
	sb.WriteString(fmt.Sprintf("Session %s  Duration: %s\n\n", sessionBadge, m.summary.Duration))

	sb.WriteString(m.renderMetrics())
	sb.WriteString("\n\n")

	sb.WriteString(m.renderCommits())
	sb.WriteString("\n")

	sb.WriteString(m.renderDiffSummary())
	sb.WriteString("\n")

	sb.WriteString(m.renderShipAction())
	sb.WriteString("\n")

	if m.confirmNewSession {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.Warning).
			Render("Start new session? Current session will be archived. [Y] Confirm  [Esc] Cancel"))
	} else {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Render("[N] New session  [R] REPL  [Esc] Back"))
	}

	return sb.String()
}

func (m *ShipModel) renderMetrics() string {
	metrics := []components.MetricCard{
		{Value: fmt.Sprintf("%d/%d", m.summary.TaskDone, m.summary.TaskTotal), Label: "Tasks", Theme: m.theme},
		{Value: fmt.Sprintf("%d", len(m.summary.Commits)), Label: "Commits", Theme: m.theme},
	}

	if m.summary.TotalTokens > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatMetric(m.summary.TotalTokens),
			Label: "Tokens",
			Theme: m.theme,
		})
	}

	if m.summary.TotalCost > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatCost(m.summary.TotalCost),
			Label: "Cost",
			Theme: m.theme,
		})
	}

	return components.MetricRow(metrics, m.width-4)
}

func (m *ShipModel) renderCommits() string {
	var sb strings.Builder

	titleLen := len(fmt.Sprintf("── Commits (%d) ", len(m.summary.Commits)))
	remaining := m.width - 4
	titleBar := fmt.Sprintf("── Commits (%d) ", len(m.summary.Commits))
	if titleLen < remaining {
		titleBar += strings.Repeat("─", remaining-titleLen)
	}
	sb.WriteString(lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextSecondary).
		Render(titleBar))
	sb.WriteString("\n\n")

	if len(m.summary.Commits) == 0 {
		sb.WriteString(lipgloss.NewStyle().
			Foreground(m.theme.TextMuted).
			Render("  No commits yet"))
		sb.WriteString("\n")
		return sb.String()
	}

	hashStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary)
	msgStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	headStyle := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)

	for i, c := range m.summary.Commits {
		if i == 0 {
			sb.WriteString(fmt.Sprintf("  %s  %s  %s\n",
				headStyle.Render("▶"),
				hashStyle.Render(c.ShortHash),
				msgStyle.Render(c.Message)))
		} else {
			sb.WriteString(fmt.Sprintf("     %s  %s\n",
				hashStyle.Render(c.ShortHash),
				msgStyle.Render(c.Message)))
		}
	}

	return sb.String()
}

func (m *ShipModel) renderDiffSummary() string {
	var sb strings.Builder

	titleBar := "── Diff Summary "
	titleLen := len(titleBar)
	remaining := m.width - 4
	if titleLen < remaining {
		titleBar += strings.Repeat("─", remaining-titleLen)
	}
	sb.WriteString(lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextSecondary).
		Render(titleBar))
	sb.WriteString("\n\n")

	addedStyle := lipgloss.NewStyle().Foreground(m.theme.DiffAdded)
	removedStyle := lipgloss.NewStyle().Foreground(m.theme.DiffRemoved)
	mutedStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)

	totalFiles := m.summary.FilesAdded + m.summary.FilesModified + m.summary.FilesDeleted
	if totalFiles == 0 && m.summary.Insertions == 0 && m.summary.Deletions == 0 {
		sb.WriteString(mutedStyle.Render("  No file changes recorded"))
		sb.WriteString("\n")
		return sb.String()
	}

	if m.summary.FilesAdded > 0 {
		for i := 0; i < m.summary.FilesAdded; i++ {
			sb.WriteString(fmt.Sprintf("  %s %s\n",
				lipgloss.NewStyle().Foreground(m.theme.Success).Render("✦"),
				mutedStyle.Render(fmt.Sprintf("new file %d", i+1))))
		}
	}
	if m.summary.FilesModified > 0 {
		for i := 0; i < m.summary.FilesModified; i++ {
			sb.WriteString(fmt.Sprintf("  %s %s\n",
				lipgloss.NewStyle().Foreground(m.theme.Warning).Render("~"),
				mutedStyle.Render(fmt.Sprintf("modified file %d", i+1))))
		}
	}
	if m.summary.FilesDeleted > 0 {
		for i := 0; i < m.summary.FilesDeleted; i++ {
			sb.WriteString(fmt.Sprintf("  %s %s\n",
				lipgloss.NewStyle().Foreground(m.theme.Error).Render("✗"),
				mutedStyle.Render(fmt.Sprintf("deleted file %d", i+1))))
		}
	}

	totalLine := fmt.Sprintf("Total: %s added  /  %s removed  /  %d files changed",
		addedStyle.Render(fmt.Sprintf("+%d lines", m.summary.Insertions)),
		removedStyle.Render(fmt.Sprintf("-%d lines", m.summary.Deletions)),
		totalFiles)
	sb.WriteString("\n")
	sb.WriteString(totalLine)
	sb.WriteString("\n")

	return sb.String()
}

func (m *ShipModel) renderShipAction() string {
	var sb strings.Builder

	titleBar := "── Ship Action "
	titleLen := len(titleBar)
	remaining := m.width - 4
	if titleLen < remaining {
		titleBar += strings.Repeat("─", remaining-titleLen)
	}
	sb.WriteString(lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextSecondary).
		Render(titleBar))
	sb.WriteString("\n\n")

	cardWidth := m.width - 8
	if cardWidth < 40 {
		cardWidth = 40
	}

	cardStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Padding(0, 1)

	keyStyle := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
	descStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary)

	var cardLines []string
	cardLines = append(cardLines, fmt.Sprintf("%s %s",
		keyStyle.Render("[S]"),
		descStyle.Render("Ship: push branch + open PR")))
	cardLines = append(cardLines, fmt.Sprintf("%s %s",
		keyStyle.Render("[C]"),
		descStyle.Render("Copy: copy commit hashes to clipboard")))
	cardLines = append(cardLines, fmt.Sprintf("%s %s",
		keyStyle.Render("[R]"),
		descStyle.Render("Rollback: revert commits via /rollback")))
	cardLines = append(cardLines, fmt.Sprintf("%s %s",
		keyStyle.Render("[Esc]"),
		descStyle.Render("Return to REPL without shipping")))

	cardContent := strings.Join(cardLines, "\n")
	rendered := cardStyle.Width(cardWidth).Render(cardContent)
	sb.WriteString(rendered)
	sb.WriteString("\n")

	return sb.String()
}

func (m *ShipModel) renderFileChanges() string {
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Render("File Changes")

	stats := []components.StatRow{
		{Label: "Added", Value: fmt.Sprintf("%d", m.summary.FilesAdded), Icon: "+"},
		{Label: "Modified", Value: fmt.Sprintf("%d", m.summary.FilesModified), Icon: "~"},
		{Label: "Deleted", Value: fmt.Sprintf("%d", m.summary.FilesDeleted), Icon: "-"},
	}

	for i := range stats {
		switch i {
		case 0:
			stats[i].Label = lipgloss.NewStyle().Foreground(m.theme.Success).Render(stats[i].Label)
		case 1:
			stats[i].Label = lipgloss.NewStyle().Foreground(m.theme.Thinking).Render(stats[i].Label)
		case 2:
			stats[i].Label = lipgloss.NewStyle().Foreground(m.theme.Error).Render(stats[i].Label)
		}
		stats[i].Theme = m.theme
	}

	parts := make([]string, len(stats))
	for i, s := range stats {
		parts[i] = s.Render()
	}

	diffStat := components.SegmentedBar{
		Segments: []components.Segment{
			{Count: m.summary.Insertions, Color: m.theme.Success},
			{Count: m.summary.Deletions, Color: m.theme.Error},
		},
		Width: m.width - 8,
		Theme: m.theme,
	}

	return header + "\n" + strings.Join(parts, "  ") + "\n" + diffStat.Render()
}

func (m *ShipModel) renderNextActions() string {
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextSecondary).
		Render("Suggested next actions")

	var actions []string
	if m.summary.TaskFailed > 0 {
		actions = append(actions, "Review failed tasks with /verify")
		actions = append(actions, "Check /rollback to revert problematic commits")
	}
	actions = append(actions, "Review changes with /diff")
	if len(m.summary.Commits) > 0 {
		actions = append(actions, "Push commits to remote")
	}
	actions = append(actions, "Start a new session with [N]")

	actionStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1)

	actionLines := make([]string, len(actions))
	for i, a := range actions {
		dot := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("•")
		actionLines[i] = actionStyle.Render(dot + " " + a)
	}

	return header + "\n" + strings.Join(actionLines, "\n")
}
