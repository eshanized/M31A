package tui

import (
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

// handler_sidebar.go — sidebar and goal-related message handling extracted from Update().

// handleGoalSubmittedMsg processes goal submission: sets workflow goal,
// creates the phase model picker, and navigates to the picker screen.
func handleGoalSubmittedMsg(m *AppState, msg GoalSubmittedMsg) (tea.Model, tea.Cmd) {
	m.workflowGoal = msg.Goal
	cw, ch := m.contentDimensions()
	picker := NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
	m.phaseModelPicker = picker
	m.screen = ScreenPhaseModelPicker
	return m, picker.Init()
}

// handleSidebarRevertMsg reverts the sidebar from todo mode back to file tree.
func handleSidebarRevertMsg(m *AppState, msg SidebarRevertMsg) (tea.Model, tea.Cmd) {
	if m.sidebarModel != nil {
		m.sidebarModel.RevertToFiles()
	}
	return m, nil
}

// handleSidebarRefreshTickMsg processes periodic sidebar refresh ticks.
func handleSidebarRefreshTickMsg(m *AppState, msg SidebarRefreshTickMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleSidebarRefreshTick(msg)...)
}

// handleSidebarRefreshMsg processes sidebar refresh events with file data.
func handleSidebarRefreshMsg(m *AppState, msg SidebarRefreshMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleSidebarRefresh(msg)...)
}

// handleSidebarTodoUpdateMsg processes sidebar todo list updates from the LLM.
func handleSidebarTodoUpdateMsg(m *AppState, msg SidebarTodoUpdateMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleSidebarTodoUpdate(msg)...)
}

// handleGhostWriteRequestMsg processes ghost write requests: shows a toast
// and navigates to the ghost output screen.
func handleGhostWriteRequestMsg(m *AppState, msg GhostWriteRequestMsg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd
	if len(msg.Files) > 0 {
		cmds = append(cmds, m.addToastCmd(
			fmt.Sprintf("Ghost write started for %d file(s)...", len(msg.Files)),
			"info", 3*time.Second))
		// Navigate to ghost output screen
		cmds = append(cmds, m.navigateToScreen(ScreenGhostOutput))
	}
	return m, tea.Batch(cmds...)
}

// handleGhostWriteResultMsg processes ghost write results and sets them on the output model.
func handleGhostWriteResultMsg(m *AppState, msg GhostWriteResultMsg) (tea.Model, tea.Cmd) {
	if m.ghostOutputModel != nil && msg.Result != nil {
		m.ghostOutputModel.SetResult(msg.Result)
	}
	return m, nil
}

// handleOptimizedMsg processes arbitrage optimization results.
func handleOptimizedMsg(m *AppState, msg OptimizedMsg) (tea.Model, tea.Cmd) {
	if len(msg.Recommendations) > 0 {
		rec := msg.Recommendations[0]
		return m, m.addToastCmd(
			fmt.Sprintf("Optimization: recommended %s (saving $%.4f)",
				rec.RecommendedModel.ModelID, rec.Savings),
			"info", 5*time.Second)
	}
	return m, nil
}
