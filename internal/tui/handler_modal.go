package tui

import (
	tea "github.com/charmbracelet/bubbletea"
)

// handler_modal.go — modal overlay message handling extracted from Update().

// handleSessionDetailRequestMsg processes session detail requests: loads
// the session into the detail model.
func handleSessionDetailRequestMsg(m *AppState, msg SessionDetailRequestMsg) (tea.Model, tea.Cmd) {
	if m.sessionDetailModel != nil && msg.Session != nil {
		m.sessionDetailModel.SetSession(msg.Session)
	}
	return m, nil
}

// handleDiffScreenMsg processes diff screen display requests.
func handleDiffScreenMsg(m *AppState, msg DiffScreenMsg) (tea.Model, tea.Cmd) {
	if m.diffModel == nil {
		m.diffModel = NewDiffModel(m.themeManager.Current())
	}
	cw, ch := m.contentDimensions()
	m.diffModel.SetDimensions(cw, ch)
	m.diffModel.SetDiff(msg.Diff)
	m.diffModel.SetTitle(msg.Title)
	if m.sidebarModel != nil {
		m.sidebarModel.Blur()
	}
	m.screen = ScreenDiff
	return m, nil
}

// handleDiffCloseMsg processes diff screen close requests.
func handleDiffCloseMsg(m *AppState, msg DiffCloseMsg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd
	cmds = append(cmds, m.popScreen())
	// Refresh sidebar git status after closing diff
	if m.sidebarModel != nil {
		cmds = append(cmds, m.sidebarModel.refreshCmd())
	}
	return m, tea.Batch(cmds...)
}

// handleToastExpiryMsg processes toast expiry: removes the toast by ID.
func handleToastExpiryMsg(m *AppState, msg ToastExpiryMsg) (tea.Model, tea.Cmd) {
	m.removeToastByID(msg.ToastID)
	return m, nil
}

// handleDismissToastMsg processes explicit toast dismissal.
func handleDismissToastMsg(m *AppState, msg DismissToastMsg) (tea.Model, tea.Cmd) {
	m.removeToastByID(msg.ToastID)
	return m, nil
}
