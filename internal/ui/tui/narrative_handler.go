package tui

import (
	tea "github.com/charmbracelet/bubbletea"
)

// handleNarrativeBubble processes a NarrativeBubbleMsg by updating the
// narrative state and re-rendering the sidebar.
func (m *AppState) handleNarrativeBubble(msg NarrativeBubbleMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.narrativeState == nil {
		return cmds
	}

	// Update the narrative state with the new narrative.
	m.narrativeState.Update(msg.Narrative)

	// The sidebar will automatically re-render with the updated state
	// since we're in the single-threaded Bubble Tea update loop.
	// No explicit sidebar refresh needed - the next View() call will
	// pick up the new state.

	// Continue draining if there are more messages queued.
	cmds = append(cmds, m.drainAdaptiveCmd())

	return cmds
}

// SetNarrativeMode switches the sidebar to narrative mode.
func (m *AppState) SetNarrativeMode() {
	if m.sidebarModel != nil {
		m.sidebarModel.SetMode(SidebarModeNarrative)
	}
}

// GetNarrativeState returns the current narrative state for testing.
func (m *AppState) GetNarrativeState() *NarrativeState {
	return m.narrativeState
}
