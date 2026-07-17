package repl

import (
	"strings"
)

// updateSlashSuggestions updates slash command autocomplete based on current input.
func (m *ReplModel) updateSlashSuggestions() {
	if m.cmdRegistry == nil {
		return
	}

	current := m.textarea.Value()
	if !strings.HasPrefix(current, "/") {
		m.slashVisible = false
		m.slashSuggestions = nil
		return
	}

	parts := strings.Fields(current)
	partial := ""
	if len(parts) > 0 {
		partial = strings.TrimPrefix(parts[0], "/")
	}

	allCmds := m.cmdRegistry.AllCommands()
	m.slashSuggestions = nil

	if partial == "" {
		m.slashSuggestions = allCmds
	} else {
		q := strings.ToLower(partial)
		for _, cmd := range allCmds {
			name := strings.ToLower(cmd.Name)
			slash := strings.ToLower(strings.TrimPrefix(cmd.Slash, "/"))
			if strings.HasPrefix(slash, q) || strings.HasPrefix(name, q) || strings.Contains(name, q) {
				m.slashSuggestions = append(m.slashSuggestions, cmd)
			}
		}
	}

	if len(m.slashSuggestions) > 0 {
		m.slashVisible = true
		m.slashSelected = 0
		if len(m.slashSuggestions) > 8 {
			m.slashSuggestions = m.slashSuggestions[:8]
		}
	} else {
		m.slashVisible = false
	}
}
