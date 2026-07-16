package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/session"
)

// ─── AppState session helpers ─────────────────────────────────────────────────

// loadAndRestoreSession loads a session from disk and restores it to the REPL.
func (m *AppState) loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd {
	return func() tea.Msg {
		if m.sessionManager == nil {
			return ErrorMsg{Err: fmt.Errorf("session manager not initialized")}
		}
		sess, err := m.sessionManager.LoadSession(sessionID)
		if err != nil {
			return ErrorMsg{Err: fmt.Errorf("failed to load session: %w", err)}
		}
		return sessionRestoredMsg{sess: sess, clearExisting: clearExisting}
	}
}

// sessionRestoredMsg carries a restored session from loadAndRestoreSession.
type sessionRestoredMsg struct {
	sess          *session.Session
	clearExisting bool
}

// ensureSidebarModel creates the sidebar model if not yet initialized.
func (m *AppState) ensureSidebarModel() {
	if m.sidebarModel == nil {
		m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
		m.sidebarModel.SetHeight(m.height)
		m.sidebarModel.SetMaxToolCalls(5)
	}
}

// propagateSessionID propagates the session ID to all workflow sub-models.
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
	if m.sidebarModel != nil {
		m.sidebarModel.SetSessionID(id)
	}
	if m.workflowEngine != nil {
		m.workflowEngine.SetSessionID(id)
	}
	if m.dispatcher != nil {
		m.dispatcher.SetSessionID(id)
	}
}

// applySessionRestored applies a sessionRestoredMsg to the REPL.
func (m *AppState) applySessionRestored(msg sessionRestoredMsg) tea.Cmd {
	sess := msg.sess
	m.ensureReplModel()
	providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
	m.replModel.SetDispatcher(m.dispatcher)
	m.replModel.SetSessionID(sess.ID)
	m.sessionID = sess.ID

	if msg.clearExisting {
		m.replModel.ClearMessages()
		m.replModel.SetCommandRegistry(m.cmdRegistry)
	}

	for _, message := range sess.Messages {
		m.replModel.AddMessage(message)
	}
	m.propagateSessionID(sess.ID)
	m.screen = ScreenREPL

	return providerCmd
}

// ─── ReplModel helpers ────────────────────────────────────────────────────────

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
