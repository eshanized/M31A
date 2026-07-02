package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

// handler_navigation.go — navigation and screen routing message handling
// extracted from Update().

// handlePopScreenMsg processes screen pop (back navigation).
func handlePopScreenMsg(m *AppState, msg PopScreenMsg) (tea.Model, tea.Cmd) {
	return m, m.popScreen()
}

// handleKeyActionMsg processes key action dispatch.
func handleKeyActionMsg(m *AppState, msg KeyActionMsg) (tea.Model, tea.Cmd) {
	return m, m.handleKeyAction(msg.Action)
}

// handleLeaderTimeoutMsg processes leader key timeout.
func handleLeaderTimeoutMsg(m *AppState, msg LeaderTimeoutMsg) (tea.Model, tea.Cmd) {
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		m.keyRegistry.DeactivateLeader()
		return m, m.addToastCmd("Leader key timed out", "info", 2*time.Second)
	}
	return m, nil
}

// handleSlashCommandMsg processes slash command execution.
func handleSlashCommandMsg(m *AppState, msg SlashCommandMsg) (tea.Model, tea.Cmd) {
	return m, m.handleSlashCommand(msg.Command, msg.AttachedFiles)
}

// handleHomeSubmitMsg processes home screen goal submission.
func handleHomeSubmitMsg(m *AppState, msg HomeSubmitMsg) (tea.Model, tea.Cmd) {
	m.ensureReplModel()
	if m.replModel != nil {
		m.replModel.textarea.SetValue(msg.Text)
		m.replModel.textarea.Focus()
		m.replModel.updateAutoExpandHeight()
	}
	return m, m.navigateToScreen(ScreenREPL)
}

// handleIntentClassifiedMsg processes intent classification results.
func handleIntentClassifiedMsg(m *AppState, msg IntentClassifiedMsg) (tea.Model, tea.Cmd) {
	return m, m.handleIntentClassified(msg)
}

// handleDiscussAnswerMsg processes discuss answer submissions.
func handleDiscussAnswerMsg(m *AppState, msg DiscussAnswerMsg) (tea.Model, tea.Cmd) {
	return m, m.handleDiscussAnswer(msg)
}

// handleDiscussCompleteMsg processes discuss phase completion.
func handleDiscussCompleteMsg(m *AppState, msg DiscussCompleteMsg) (tea.Model, tea.Cmd) {
	return m, m.handleDiscussComplete()
}
