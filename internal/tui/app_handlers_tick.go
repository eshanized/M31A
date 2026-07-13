package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

// app_handlers_tick.go — TickMsg and transition handling extracted from Update().

// handleTickMsg processes TickMsg: forwards to the active screen sub-model for
// animation, and advances screen transitions.
func (m *AppState) handleTickMsg(msg TickMsg) []tea.Cmd {
	var cmds []tea.Cmd

	// Forward to REPL for streaming/thinking animation
	if m.replModel != nil && (m.replModel.streaming || m.replModel.thinking) {
		replM, cmd := m.replModel.Update(msg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		cmds = append(cmds, cmd)
	}

	// Forward to active screen for spinner/animation updates
	cmds = append(cmds, m.forwardTickToScreen(msg)...)

	// Screen transition tick
	if m.transition != nil && m.transition.Active {
		if m.transition.TransitionTick() {
			m.screen = m.transition.ToScreen
			m.transition = nil
		} else {
			cmds = append(cmds, tea.Tick(transitionTickInterval, func(t time.Time) tea.Msg {
				return TickMsg{Time: t}
			}))
		}
	}

	return cmds
}

// forwardTickToScreen dispatches TickMsg to the active screen's sub-model
// for spinner and animation updates. Each screen has its own tick behavior.
func (m *AppState) forwardTickToScreen(msg TickMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.screen == ScreenExecute && m.executeModel != nil {
		execM, cmd := m.executeModel.Update(msg)
		if r, ok := execM.(*ExecuteModel); ok {
			m.executeModel = r
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenPhaseModelPicker && m.phaseModelPicker != nil {
		newPMP, cmd := m.phaseModelPicker.Update(msg)
		if r, ok := newPMP.(*PhaseModelPickerModel); ok {
			m.phaseModelPicker = r
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenModelSelector && m.msModel != nil {
		newMS, cmd := m.msModel.Update(msg)
		if r, ok := newMS.(*ModelSelector); ok {
			m.msModel = r
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenVerify && m.verifyModel != nil {
		m.verifyModel.TickSpinner()
	}
	if m.screen == ScreenRuntimeCheck && m.runtimeModel != nil {
		newRuntime, cmd := m.runtimeModel.Update(msg)
		if r, ok := newRuntime.(*RuntimeModel); ok {
			m.runtimeModel = r
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenPlan && m.planModel != nil {
		newPlan, cmd := m.planModel.Update(msg)
		if np, ok := newPlan.(*PlanModel); ok {
			m.planModel = np
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenShip && m.shipModel != nil {
		newShip, cmd := m.shipModel.Update(msg)
		if r, ok := newShip.(*ShipModel); ok {
			m.shipModel = r
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenDashboard && m.dashboardModel != nil {
		newDash, cmd := m.dashboardModel.Update(msg)
		if nd, ok := newDash.(*DashboardModel); ok {
			m.dashboardModel = nd
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenChatHistory && m.chatHistoryModel != nil {
		newCH, cmd := m.chatHistoryModel.Update(msg)
		if nch, ok := newCH.(*ChatHistoryModel); ok {
			m.chatHistoryModel = nch
		}
		cmds = append(cmds, cmd)
	}
	if m.screen == ScreenGhostOutput && m.ghostOutputModel != nil {
		newGO, cmd := m.ghostOutputModel.Update(msg)
		if ngo, ok := newGO.(*GhostOutputModel); ok {
			m.ghostOutputModel = ngo
		}
		cmds = append(cmds, cmd)
	}

	return cmds
}
