package tui

import (
	"path/filepath"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// ─── Mention suggestion management ───────────────────────────────────────────

// updateMentionSuggestions inspects the current textarea value for an active
// @-mention (the last unspaced "@" in the text) and updates mentionEntries.
func (m *ReplModel) updateMentionSuggestions() {
	val := m.textarea.Value()

	// Walk backwards from end to find "@" not separated by whitespace.
	atIdx := -1
	for i := len(val) - 1; i >= 0; i-- {
		ch := val[i]
		if ch == '@' {
			atIdx = i
			break
		}
		if ch == ' ' || ch == '\t' || ch == '\n' {
			// Hit whitespace before finding '@' — no active mention.
			break
		}
	}

	if atIdx < 0 {
		m.mentionVisible = false
		m.mentionEntries = nil
		return
	}

	query := val[atIdx+1:]
	// If query contains whitespace the mention is already completed or cancelled.
	if strings.ContainsAny(query, " \t\n") {
		m.mentionVisible = false
		m.mentionEntries = nil
		return
	}

	m.mentionQuery = query
	m.mentionStartCol = atIdx

	// Create or invalidate the completer if the cwd has changed.
	if m.mentionCompleter == nil || m.mentionCompleter.cwd != m.cwd {
		m.mentionCompleter = NewMentionCompleter(m.cwd)
	}

	entries := m.mentionCompleter.Filter(query)
	if len(entries) == 0 {
		m.mentionVisible = false
		m.mentionEntries = nil
		return
	}

	m.mentionEntries = entries
	m.mentionVisible = true
	if m.mentionSelected >= len(m.mentionEntries) {
		m.mentionSelected = 0
	}
}

// completeMention replaces the active @query in the textarea with the selected path.
func (m *ReplModel) completeMention() {
	if !m.mentionVisible || len(m.mentionEntries) == 0 {
		return
	}
	entry := m.mentionEntries[m.mentionSelected]
	val := m.textarea.Value()

	// Locate the same "@" position we found during update.
	atIdx := -1
	for i := len(val) - 1; i >= 0; i-- {
		ch := val[i]
		if ch == '@' {
			atIdx = i
			break
		}
		if ch == ' ' || ch == '\t' || ch == '\n' {
			break
		}
	}
	if atIdx < 0 {
		m.mentionVisible = false
		return
	}

	path := entry.Path
	if entry.IsDir {
		// Trailing separator keeps the mention "open" so user can drill in.
		path += string(filepath.Separator)
	} else {
		// Append a space so the user can continue typing after the mention.
		path += " "
	}

	newVal := val[:atIdx+1] + path
	m.textarea.SetValue(newVal)
	m.textarea.CursorEnd()

	if !entry.IsDir {
		// File mention complete — dismiss dropdown.
		m.mentionVisible = false
		m.mentionEntries = nil
		m.mentionQuery = ""
	} else {
		// Directory selected — re-filter so user can keep narrowing.
		m.updateMentionSuggestions()
	}
}

// ─── Mention dropdown rendering ───────────────────────────────────────────────

// renderMentionSuggestions renders the @-mention autocomplete dropdown.
func (m *ReplModel) renderMentionSuggestions(width int) string {
	t := m.theme
	var lines []string

	for i, entry := range m.mentionEntries {
		// Simple ASCII icons so we don't depend on emoji terminal support.
		icon := "  "
		if entry.IsDir {
			icon = "▸ "
		}

		pathStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
		nameStyle := lipgloss.NewStyle().Foreground(t.Text)
		atStyle := lipgloss.NewStyle().Foreground(t.Brand).Bold(true)

		if i == m.mentionSelected {
			bg := t.Brand
			nameStyle = nameStyle.Background(bg).Foreground(t.Background)
			pathStyle = pathStyle.Background(bg).Foreground(t.Background)
			atStyle = atStyle.Background(bg).Foreground(t.Background)
		}

		// Render dir prefix (e.g. "internal/tui/") separately in muted colour.
		dir := filepath.Dir(entry.Path)
		dirPart := ""
		if dir != "." {
			dirPart = pathStyle.Render(dir + string(filepath.Separator))
		}

		name := nameStyle.Render(icon + entry.DisplayName)
		at := atStyle.Render("@")
		line := "  " + at + dirPart + name

		if lipgloss.Width(line) > width-4 {
			line = TruncateWithEllipsis(line, width-4)
		}
		lines = append(lines, line)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Width(width - 2).
		Render(strings.Join(lines, "\n"))
}
