package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// View implements tea.Model.View.
func (m DiffModel) View() string {
	if m.diff == "" {
		return centerText("No diff to display. Run /diff with arguments.", m.width)
	}
	if len(m.lines) == 0 {
		return "No changes in diff."
	}

	if m.splitView {
		return m.renderSplitView()
	}
	return m.renderUnifiedView()
}

func (m DiffModel) renderSplitView() string {
	leftWidth := m.width / 2
	rightWidth := m.width - leftWidth - 1

	var leftContent strings.Builder
	leftContent.WriteString(lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render(" Files "))
	leftContent.WriteString("\n")

	for _, line := range m.lines {
		if line.Type == DiffHeader && strings.HasPrefix(line.Content, "diff --git") {
			parts := strings.SplitN(line.Content, " b/", 2)
			if len(parts) == 2 {
				leftContent.WriteString(lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("  "+parts[1]))
				leftContent.WriteString("\n")
			}
		}
	}

	var rightContent strings.Builder
	rightContent.WriteString(lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render(" Diff "))
	rightContent.WriteString("\n")

	viewportHeight := m.height - 4
	if viewportHeight < 1 {
		viewportHeight = 1
	}
	end := m.scrollPos + viewportHeight
	if end > len(m.lines) {
		end = len(m.lines)
	}
	visible := m.lines[m.scrollPos:end]

	gutterWidth := 4
	lineNum := m.scrollPos + 1

	for _, line := range visible {
		var lineStyle lipgloss.Style
		var prefix string

		switch line.Type {
		case DiffAdded:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.DiffAdded).Background(m.theme.DiffAddedBg)
			prefix = "+"
		case DiffDeleted:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.DiffRemoved).Background(m.theme.DiffRemovedBg)
			prefix = "-"
		case DiffHunk:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.Thinking).Italic(true)
			prefix = " "
		case DiffHeader:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
			prefix = " "
		case DiffContext:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.Text).Background(m.theme.DiffContextBg)
			prefix = " "
		default:
			lineStyle = lipgloss.NewStyle().Foreground(m.theme.Text)
			prefix = " "
		}

		gutterStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)
		switch line.Type {
		case DiffAdded, DiffDeleted, DiffContext:
			gutter := fmt.Sprintf("%*d ", gutterWidth, lineNum)
			rightContent.WriteString(gutterStyle.Render(gutter))
			rightContent.WriteString(lineStyle.Render(prefix+line.Content))
			lineNum++
		default:
			gutter := strings.Repeat(" ", gutterWidth+1)
			rightContent.WriteString(gutterStyle.Render(gutter))
			rightContent.WriteString(lineStyle.Render(line.Content))
		}
		rightContent.WriteString("\n")
	}

	left := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Width(leftWidth).
		Height(m.height - 2).
		Render(leftContent.String())

	right := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Width(rightWidth).
		Height(m.height - 2).
		Render(rightContent.String())

	helpBar := "↑↓ scroll · V toggle split · Esc back"
	helpStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)

	return lipgloss.JoinHorizontal(lipgloss.Top, left, " ", right) + "\n" + helpStyle.Render(helpBar)
}

func (m DiffModel) renderUnifiedView() string {
	var b strings.Builder
	insertions, deletions, files := m.computeStats()
	titleStr := fmt.Sprintf(" Diff: %s", m.title)
	statsStr := ""
	if files > 0 {
		statsStr = fmt.Sprintf("  %d files changed, +%d insertions, -%d deletions", files, insertions, deletions)
	}
	headerContent := titleStr + statsStr

	headerBorder := "╭─" + headerContent
	remaining := m.width - len(headerBorder) - 1
	if remaining > 0 {
		headerBorder += strings.Repeat("─", remaining)
	}
	headerBorder += "╮"

	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)
	b.WriteString(headerStyle.Render(headerBorder))
	b.WriteString("\n")

	viewportHeight := m.height - 4
	if viewportHeight < 1 {
		viewportHeight = 1
	}
	end := m.scrollPos + viewportHeight
	if end > len(m.lines) {
		end = len(m.lines)
	}
	visible := m.lines[m.scrollPos:end]

	gutterWidth := 4
	lineNum := m.scrollPos + 1

	for _, line := range visible {
		var lineStyle lipgloss.Style
		var prefix string

		switch line.Type {
		case DiffAdded:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.DiffAdded).
				Background(m.theme.DiffAddedBg)
			prefix = "+"
		case DiffDeleted:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.DiffRemoved).
				Background(m.theme.DiffRemovedBg)
			prefix = "-"
		case DiffHunk:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.Thinking).
				Italic(true)
			prefix = " "
		case DiffHeader:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.Brand).
				Bold(true)
			prefix = " "
		case DiffContext:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.Text).
				Background(m.theme.DiffContextBg)
			prefix = " "
		default:
			lineStyle = lipgloss.NewStyle().
				Foreground(m.theme.Text)
			prefix = " "
		}

		gutterStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)
		switch line.Type {
		case DiffAdded, DiffDeleted:
			gutter := fmt.Sprintf("%*d ", gutterWidth, lineNum)
			b.WriteString(gutterStyle.Render(gutter))
			b.WriteString(lineStyle.Render(prefix + line.Content))
			lineNum++
		case DiffContext:
			gutter := fmt.Sprintf("%*d ", gutterWidth, lineNum)
			b.WriteString(gutterStyle.Render(gutter))
			b.WriteString(lineStyle.Render(prefix + line.Content))
			lineNum++
		default:
			gutter := strings.Repeat(" ", gutterWidth+1)
			b.WriteString(gutterStyle.Render(gutter))
			b.WriteString(lineStyle.Render(line.Content))
		}
		b.WriteString("\n")
	}

	helpBar := "↑↓ scroll  ·  V split view  ·  Esc back"
	helpStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	if m.width > 0 {
		helpStyle = helpStyle.Width(m.width)
	}
	b.WriteString(helpStyle.Render(helpBar))

	return b.String()
}

// centerText centers lines of text within the given width.
func centerText(text string, width int) string {
	if width <= 0 {
		return text
	}
	lines := strings.Split(text, "\n")
	for i, line := range lines {
		if len(line) < width {
			pad := (width - len(line)) / 2
			if pad > 0 {
				lines[i] = strings.Repeat(" ", pad) + line
			}
		}
	}
	return strings.Join(lines, "\n")
}
