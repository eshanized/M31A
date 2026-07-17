package repl

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"github.com/eshanized/M31A/internal/tools"
)

// ─── Mention suggestion management ───────────────────────────────────────────

// cursorPosition returns the absolute character offset of the cursor in the
// textarea value. It computes this from the public Line()/LineInfo() API.
func cursorPosition(m *ReplModel) int {
	val := m.textarea.Value()
	row := m.textarea.Line()
	info := m.textarea.LineInfo()

	lines := strings.Split(val, "\n")
	pos := 0
	for i := 0; i < row && i < len(lines); i++ {
		pos += len(lines[i]) + 1 // +1 for the '\n'
	}
	if row < len(lines) {
		charOffset := info.CharOffset
		if charOffset > len(lines[row]) {
			charOffset = len(lines[row])
		}
		pos += charOffset
	}
	if pos > len(val) {
		pos = len(val)
	}
	return pos
}

// updateMentionSuggestions inspects the current textarea value for an active
// @-mention and updates mentionEntries. The detection is cursor-aware: it finds
// the last '@' at or before the cursor that is not separated by whitespace.
func updateMentionSuggestions(m *ReplModel) {
	val := m.textarea.Value()
	cursorPos := cursorPosition(m)

	// Walk backwards from cursor position to find "@" not separated by whitespace.
	atIdx := -1
	for i := cursorPos - 1; i >= 0; i-- {
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
	m.mentionAtPos = atIdx

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
func completeMention(m *ReplModel) {
	if !m.mentionVisible || len(m.mentionEntries) == 0 {
		return
	}
	entry := m.mentionEntries[m.mentionSelected]
	val := m.textarea.Value()

	// Use the stored '@' position from updateMentionSuggestions.
	atIdx := m.mentionAtPos
	if atIdx < 0 || atIdx >= len(val) || val[atIdx] != '@' {
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
		updateMentionSuggestions(m)
	}
}

// ─── Mention dropdown rendering ───────────────────────────────────────────────

// renderMentionSuggestions renders the @-mention autocomplete dropdown.
func RenderMentionSuggestions(m *ReplModel, width int) string {
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
		metaStyle := lipgloss.NewStyle().Foreground(t.TextMuted)

		if i == m.mentionSelected {
			bg := t.Brand
			nameStyle = nameStyle.Background(bg).Foreground(t.Background)
			pathStyle = pathStyle.Background(bg).Foreground(t.Background)
			atStyle = atStyle.Background(bg).Foreground(t.Background)
			metaStyle = metaStyle.Background(bg).Foreground(t.Background)
		}

		// Render dir prefix (e.g. "internal/tui/") separately in muted colour.
		dir := filepath.Dir(entry.Path)
		dirPart := ""
		if dir != "." {
			dirPart = pathStyle.Render(dir + string(filepath.Separator))
		}

		// File metadata: size or line count
		metaPart := ""
		if !entry.IsDir {
			// PERF-44: Use lazy line count computation
			lineCount := 0
			if m.mentionCompleter != nil {
				lineCount = m.mentionCompleter.GetLineCount(&entry)
			}
			if lineCount > 0 {
				metaPart = metaStyle.Render(fmt.Sprintf("  %dL", lineCount))
			} else if entry.Size > 0 {
				metaPart = metaStyle.Render(fmt.Sprintf("  %s", humanSize(entry.Size)))
			}
		}

		name := nameStyle.Render(icon + entry.DisplayName)
		at := atStyle.Render("@")
		line := "  " + at + dirPart + name + metaPart

		if lipgloss.Width(line) > width-4 {
			line = tuitypes.TruncateWithEllipsis(line, width-4)
		}
		lines = append(lines, line)
	}

	// Footer with keyboard hints
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	footer := hintStyle.Render("  tab select · ↑↓ navigate · esc cancel")

	content := strings.Join(lines, "\n") + "\n" + footer

	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Width(width - 2).
		Render(content)
}

// humanSize formats bytes into a human-readable string.
func humanSize(b int64) string {
	return tools.HumanSize(b)
}
