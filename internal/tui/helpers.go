package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
)

// makeAssistantMsg creates a standard assistant message with content and segments.
// Eliminates 9+ instances of duplicated Message construction pattern (M-3).
func makeAssistantMsg(content string) types.Message {
	return types.Message{
		Role:    "assistant",
		Content: content,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: content,
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
}

// syncReplProvider synchronizes the REPL model's provider, dispatcher, and command registry.
// Eliminates 12+ instances of the triple-call pattern (H-1).
func (m *AppState) syncReplProvider(sessionID string) tea.Cmd {
	if m.replModel == nil {
		return nil
	}
	m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
	m.replModel.SetDispatcher(m.dispatcher)
	m.replModel.SetCommandRegistry(m.cmdRegistry)
	return nil
}

// listenerCmds returns the permission and question listener commands.
// Eliminates 20+ instances of the dual-listener pattern (H-8).
func (m *AppState) listenerCmds() []tea.Cmd {
	return []tea.Cmd{
		permissionListenerCmd(m.shutdownCtx, m.dispatcher),
		questionListenerCmd(m.shutdownCtx, m.dispatcher),
	}
}

// updateSlashSuggestions updates the slash command suggestions based on current input.
// Eliminates duplicate 43-line blocks in repl.go and repl_keys.go (H-3).
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

	// Extract the partial command (first word after /)
	parts := strings.Fields(current)
	partial := ""
	if len(parts) > 0 {
		partial = strings.TrimPrefix(parts[0], "/")
	}

	// Generate matching commands
	allCmds := m.cmdRegistry.AllCommands()
	m.slashSuggestions = nil
	if partial == "" {
		// Show all commands when just "/" is typed
		m.slashSuggestions = allCmds
	} else {
		// Filter by partial match
		q := strings.ToLower(partial)
		for _, cmd := range allCmds {
			name := strings.ToLower(cmd.Name)
			slash := strings.ToLower(cmd.Slash)
			if strings.HasPrefix(name, q) || strings.HasPrefix(slash, q) || strings.Contains(name, q) {
				m.slashSuggestions = append(m.slashSuggestions, cmd)
			}
		}
	}

	// Show suggestions if we have matches
	if len(m.slashSuggestions) > 0 {
		m.slashVisible = true
		m.slashSelected = 0
		// Limit to 8 suggestions
		if len(m.slashSuggestions) > 8 {
			m.slashSuggestions = m.slashSuggestions[:8]
		}
	} else {
		m.slashVisible = false
	}
}

// ensureReplModel ensures the REPL model is initialized.
// Eliminates 7+ nil-check+assignment blocks (H-6).
func (m *AppState) ensureReplModel() {
	if m.replModel == nil {
		rp := NewReplModel(m.themeManager.Current(), m.version)
		m.replModel = &rp
	}
}

// ensureSidebarModel ensures the sidebar model is initialized.
// Eliminates 2 identical nil-check blocks (H-5).
func (m *AppState) ensureSidebarModel() {
	if m.sidebarModel == nil {
		m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
	}
}

// propagateSessionID propagates the session ID to all workflow sub-models.
// Eliminates 2 identical 4-line blocks (H-4).
func (m *AppState) propagateSessionID(id string) {
	if m.planModel != nil {
		m.planModel.sessionID = id
	}
	if m.executeModel != nil {
		m.executeModel.sessionID = id
	}
	if m.verifyModel != nil {
		m.verifyModel.sessionID = id
	}
	if m.shipModel != nil {
		m.shipModel.sessionID = id
	}
}

// loadAndRestoreSession loads a session and restores it to the REPL and workflow models.
// Eliminates 2 near-identical session loading blocks (H-2).
func (m *AppState) loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd {
	sess, err := m.sessionManager.LoadSession(sessionID)
	if err != nil {
		return func() tea.Msg { return ErrorMsg{Err: fmt.Errorf("failed to load session: %w", err)} }
	}

	m.ensureReplModel()
	providerCmd := m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
	m.replModel.SetDispatcher(m.dispatcher)
	m.replModel.SetSessionID(sessionID)

	if clearExisting {
		m.replModel.ClearMessages()
		m.replModel.SetCommandRegistry(m.cmdRegistry)
	}

	for _, msg := range sess.Messages {
		m.replModel.AddMessage(msg)
	}
	m.propagateSessionID(sessionID)

	return providerCmd
}

// centerScreen centers content in the terminal.
// Eliminates 30+ identical centering calls (H-9).
func centerScreen(content string, w, h int) string {
	return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}

// renderSectionHeader renders a section header with dashed lines filling the width.
// Eliminates 6+ header-bar patterns (M-12).
func renderSectionHeader(title string, width int) string {
	prefix := fmt.Sprintf("── %s ", title)
	remaining := width - lipgloss.Width(prefix)
	if remaining < 0 {
		remaining = 0
	}
	return prefix + strings.Repeat("─", remaining)
}
