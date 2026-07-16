package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/pkg/types"
)

// routeKeyMsg routes key events to the active screen.
func (m *AppState) routeKeyMsg(msg tea.KeyMsg) tea.Cmd {
	// Command palette has priority
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		newPalette, cmd := m.cmdPalette.Update(msg)
		m.cmdPalette = newPalette
		return cmd
	}

	// Confirmation dialog active -- intercept y/n/esc
	if m.pendingConfirm != nil {
		switch msg.String() {
		case "y", "Y":
			pending := *m.pendingConfirm
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			pending.ConfirmRequired = false
			return m.processCommandResult(pending)
		case "n", "N", "esc":
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Cancelled."))
			}
			return nil
		}
		return nil
	}

	// Intent classification confirmation active -- intercept y/n
	if m.pendingIntent != nil {
		switch msg.String() {
		case "y", "Y":
			intent := m.pendingIntent
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			mode := types.WorkflowModeForIntent(*intent)
			m.workflowMode = mode
			if m.workflowEngine != nil {
				m.workflowEngine.SetWorkflowMode(mode)
			}
			return m.runWorkflowFromGoal(input)
		case "n", "N", "esc":
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Proceeding in chat mode."))
			}
			p := m.registry.ActiveProvider()
			if p != nil && m.agentMode {
				return m.startAgentLoop(p, input)
			}
			if p != nil {
				return m.sendPlainTextChat(p, input)
			}
			return nil
		}
		return nil
	}

	// Sidebar focus toggle -- works from any screen
	if msg.String() == "ctrl+g" && m.sidebarModel != nil && m.sidebarModel.IsVisible() {
		m.sidebarModel.ToggleFocus()
		return nil
	}

	// Leader key must never be swallowed by sidebar or screen handlers.
	// Activate leader mode or complete a chord before routing to sidebar.
	if m.keyRegistry != nil {
		if !m.keyRegistry.IsLeaderActive() {
			// First press: check if this is the leader key itself
			if msg.String() == m.keyRegistry.LeaderKey() {
				handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
				if handled {
					return cmd
				}
			}
		} else {
			// Leader active: complete chord regardless of focused screen.
			// Use CtxREPL so Handle searches CtxREPL then CtxGlobal,
			// catching all registered chords.
			handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
			if handled {
				return cmd
			}
		}
	}

	// When sidebar is focused, route keys to sidebar regardless of active screen.
	// ctrl+g can toggle sidebar focus from any screen, so keys must reach it.
	if m.sidebarModel != nil && m.sidebarModel.IsFocused() {
		return m.sidebarModel.HandleKey(msg)
	}

	return m.routeKeyToScreen(msg)
}
