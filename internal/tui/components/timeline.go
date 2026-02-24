package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// TimelineEntry represents a single event in a timeline.
type TimelineEntry struct {
	Time   string
	Title  string
	Detail string
	Status string // "done", "active", "pending", "error"
}

// TimelineView renders a vertical timeline of events.
type TimelineView struct {
	Entries []TimelineEntry
	Cursor  int
	Offset  int
	Height  int
	Theme   theme.Theme
	Width   int
}

// View renders the timeline.
func (tv TimelineView) View() string {
	if len(tv.Entries) == 0 {
		return ""
	}

	end := tv.Offset + tv.Height
	if end > len(tv.Entries) {
		end = len(tv.Entries)
	}

	var lines []string
	for i := tv.Offset; i < end; i++ {
		entry := tv.Entries[i]
		selected := i == tv.Cursor
		lines = append(lines, tv.renderEntry(entry, selected, i < end-1))
	}

	return strings.Join(lines, "\n")
}

func (tv TimelineView) renderEntry(entry TimelineEntry, selected, hasNext bool) string {
	t := tv.Theme

	var dotColor lipgloss.Color
	switch entry.Status {
	case "done":
		dotColor = t.Success
	case "active":
		dotColor = t.Brand
	case "error":
		dotColor = t.Error
	default:
		dotColor = t.TextMuted
	}

	dot := lipgloss.NewStyle().Foreground(dotColor).Bold(true).Render("●")
	connector := "│"
	if !hasNext {
		connector = " "
	}

	timeStr := lipgloss.NewStyle().Foreground(t.TextMuted).Width(8).Render(entry.Time)
	titleStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		titleStyle = titleStyle.Foreground(t.Brand).Bold(true)
	}
	title := titleStyle.Render(entry.Title)

	line1 := "  " + dot + " " + timeStr + "  " + title

	detail := ""
	if entry.Detail != "" {
		detail = "  " + lipgloss.NewStyle().Foreground(connectorColor(t)).Render(connector) +
			"         " + lipgloss.NewStyle().Foreground(t.TextSecondary).Render(entry.Detail)
	}

	if hasNext {
		connectorLine := "  " + lipgloss.NewStyle().Foreground(t.Border).Render(connector)
		if detail != "" {
			return line1 + "\n" + detail + "\n" + connectorLine
		}
		return line1 + "\n" + connectorLine
	}
	if detail != "" {
		return line1 + "\n" + detail
	}
	return line1
}

func connectorColor(t theme.Theme) lipgloss.Color {
	return t.Border
}
