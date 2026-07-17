package repl

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// repl_search.go — inline viewport search (Ctrl+F).
//
// When active, a search bar appears at the bottom of the viewport.
// Typing highlights matches; Enter navigates to the next match.

// searchMatch records the line number and column offset of a match.
type searchMatch struct {
	line int // 0-indexed line number in viewport content
	col  int // byte offset within the line
}

// searchState holds the state for inline viewport search.
type searchState struct {
	visible   bool
	query     string
	matches   int
	current   int           // 0-indexed current match index
	positions []searchMatch // line positions of matches
}

// toggleSearch toggles the search bar visibility.
func (m *ReplModel) toggleSearch() {
	m.search.visible = !m.search.visible
	if !m.search.visible {
		m.search.query = ""
		m.search.matches = 0
		m.search.current = 0
		m.search.positions = nil
	}
}

// handleSearchKey processes key events when search is active.
func (m *ReplModel) handleSearchKey(msg tea.KeyMsg) tea.Cmd {
	switch msg.String() {
	case "esc":
		m.search.visible = false
		m.search.query = ""
		m.search.matches = 0
		m.search.current = 0
		m.search.positions = nil
		return nil

	case "enter":
		if m.search.matches > 0 {
			m.search.current = (m.search.current + 1) % m.search.matches
			m.scrollToCurrentMatch()
		}
		return nil

	case "shift+tab":
		if m.search.matches > 0 {
			m.search.current = (m.search.current - 1 + m.search.matches) % m.search.matches
			m.scrollToCurrentMatch()
		}
		return nil

	case "ctrl+f":
		m.search.visible = false
		m.search.query = ""
		m.search.matches = 0
		m.search.current = 0
		m.search.positions = nil
		return nil

	case "backspace":
		if len(m.search.query) > 0 {
			m.search.query = m.search.query[:len(m.search.query)-1]
			m.updateSearchMatches()
		}
		return nil

	default:
		if len(msg.Runes) > 0 {
			m.search.query += string(msg.Runes)
			m.updateSearchMatches()
		}
	}
	return nil
}

// updateSearchMatches counts matches and records their line positions.
func (m *ReplModel) updateSearchMatches() {
	if m.search.query == "" {
		m.search.matches = 0
		m.search.current = 0
		m.search.positions = nil
		return
	}
	content := m.viewportContent
	query := strings.ToLower(m.search.query)
	lines := strings.Split(content, "\n")

	var positions []searchMatch
	for lineIdx, line := range lines {
		lower := strings.ToLower(line)
		offset := 0
		for {
			idx := strings.Index(lower[offset:], query)
			if idx < 0 {
				break
			}
			positions = append(positions, searchMatch{line: lineIdx, col: offset + idx})
			offset += idx + 1
		}
	}
	m.search.matches = len(positions)
	m.search.positions = positions
	if m.search.current >= m.search.matches {
		m.search.current = 0
	}
}

// scrollToCurrentMatch scrolls the viewport to the line of the current match.
func (m *ReplModel) scrollToCurrentMatch() {
	if m.search.current < 0 || m.search.current >= len(m.search.positions) {
		return
	}
	line := m.search.positions[m.search.current].line
	m.viewport.SetYOffset(line)
}

// renderSearchBar renders the inline search bar.
func (m *ReplModel) renderSearchBar(width int) string {
	t := m.theme

	queryStyle := lipgloss.NewStyle().Foreground(t.Text)
	if m.search.query == "" {
		queryStyle = lipgloss.NewStyle().Foreground(t.TextMuted)
	}

	label := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Search: ")
	queryText := queryStyle.Render(m.search.query)
	cursor := lipgloss.NewStyle().Foreground(t.Brand).Render("█")

	countText := ""
	if m.search.query != "" {
		countText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			lipgloss.PlaceHorizontal(width-40, lipgloss.Right,
				fmt.Sprintf("%d/%d matches", m.search.current+1, m.search.matches)))
	}

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  esc close, enter next, shift+tab prev")

	content := label + queryText + cursor + countText + hint
	if lipgloss.Width(content) > width {
		content = tuitypes.TruncateWithEllipsis(content, width)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Border).
		Width(width).
		Padding(0, 1).
		Render(content)
}
