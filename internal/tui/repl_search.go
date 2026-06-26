package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
)

// repl_search.go — inline viewport search (Ctrl+F).
//
// When active, a search bar appears at the bottom of the viewport.
// Typing filters visible content; Enter/Escape closes the search.

// searchState holds the state for inline viewport search.
type searchState struct {
	visible bool
	query   string
	matches int
	current int // 0-indexed current match index
}

// toggleSearch toggles the search bar visibility.
func (m *ReplModel) toggleSearch() {
	m.search.visible = !m.search.visible
	if !m.search.visible {
		m.search.query = ""
		m.search.matches = 0
		m.search.current = 0
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
		return nil

	case "enter":
		// Navigate to next match
		if m.search.matches > 0 {
			m.search.current = (m.search.current + 1) % m.search.matches
		}
		return nil

	case "ctrl+f":
		// Toggle search off
		m.search.visible = false
		m.search.query = ""
		m.search.matches = 0
		m.search.current = 0
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

// updateSearchMatches counts matches in the viewport content.
func (m *ReplModel) updateSearchMatches() {
	if m.search.query == "" {
		m.search.matches = 0
		m.search.current = 0
		return
	}
	content := m.viewport.View()
	query := strings.ToLower(m.search.query)
	m.search.matches = strings.Count(strings.ToLower(content), query)
	m.search.current = 0
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
				fmt.Sprintf("%d matches", m.search.matches)))
	}

	hint := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  esc to close, enter for next")

	content := label + queryText + cursor + countText + hint
	if lipgloss.Width(content) > width {
		content = TruncateWithEllipsis(content, width)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Border).
		Width(width).
		Padding(0, 1).
		Render(content)
}
