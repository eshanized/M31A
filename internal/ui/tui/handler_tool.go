package tui

import (
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/types"
)

// handler_tool.go — permission and tool message handling extracted from Update().

// handlePermissionRequestMsg processes a tool permission request, showing the
// permission modal overlay.
func handlePermissionRequestMsg(m *AppState, msg PermissionRequestMsg) (tea.Model, tea.Cmd) {
	m.permRequest = &msg.Request
	m.permCountdown = msg.Request.TimeoutSecs
	timeout := components.DefaultPermissionTimeout
	if msg.Request.TimeoutSecs > 0 {
		timeout = time.Duration(msg.Request.TimeoutSecs) * time.Second
	}
	m.permModal = components.NewPermissionModal(msg.Request, m.themeManager.Current(), timeout)
	m.permModal.SetContext(string(m.workflowPhase), m.workflowGoal)
	m.screen = ScreenPermission
	if m.sidebarModel != nil {
		m.sidebarModel.SetPendingPermCount(msg.Request.QueueDepth)
	}
	return m, nil
}

// handlePermissionResponseMsg processes the user's permission decision.
func handlePermissionResponseMsg(m *AppState, msg PermissionResponseMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePermissionResponse(msg)
}

// handlePermissionTickMsg processes permission countdown ticks.
func handlePermissionTickMsg(m *AppState, msg PermissionTickMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePermissionTick()
}

// handleQuestionRequestMsg processes a question request, showing the question modal.
func handleQuestionRequestMsg(m *AppState, msg QuestionRequestMsg) (tea.Model, tea.Cmd) {
	m.questionRequest = &msg
	qModel := components.NewQuestionModel(types.QuestionRequest{
		Question:    msg.Question,
		Header:      msg.Header,
		Options:     msg.Options,
		AllowCustom: true,
	}, m.themeManager.Current(), m.permModalWidth)
	m.questionModel = &qModel
	m.screen = ScreenPermission // reuse permission overlay
	return m, nil
}

// handleQuestionResponseMsg processes the user's question answer.
func handleQuestionResponseMsg(m *AppState, msg QuestionResponseMsg) (tea.Model, tea.Cmd) {
	return m, m.handleQuestionResponse(msg)
}

// handleToolsQuestionResponse processes a types.QuestionResponse directly.
func handleToolsQuestionResponse(m *AppState, msg types.QuestionResponse) (tea.Model, tea.Cmd) {
	return m, m.handleQuestionResponse(QuestionResponseMsg{Answer: msg.Answer})
}
