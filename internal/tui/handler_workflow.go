package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// handler_workflow.go — workflow phase and event handling extracted from Update().

// isSimpleTask checks if a task description matches quick-mode heuristics.
// Simple tasks are single-file changes, bug fixes, documentation, or config changes.
func isSimpleTask(goal string) bool {
	lower := strings.ToLower(goal)
	simplePatterns := []string{
		"fix", "bug", "error", "failing",
		"typo", "rename", "move", "delete",
		"doc", "readme", "comment",
		"config", "env", "setting",
		"test", "lint", "fmt",
	}
	for _, p := range simplePatterns {
		if strings.Contains(lower, p) {
			return true
		}
	}
	return false
}

// adjustModeForQuickMode returns the appropriate workflow mode when quick mode
// is enabled. For simple tasks, it returns ModeDirect (skips Discuss, Plan, Verify).
// For complex tasks, it returns the original mode unchanged.
func adjustModeForQuickMode(original types.WorkflowMode, goal string, quickMode bool) types.WorkflowMode {
	if !quickMode {
		return original
	}
	if isSimpleTask(goal) {
		return types.ModeDirect
	}
	return original
}

// handlePhaseResultMsg processes the result of a completed workflow phase.
func handlePhaseResultMsg(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePhaseResult(msg)
}

// handlePlanReadyMsg processes a plan-ready event from the workflow engine.
func handlePlanReadyMsg(m *AppState, msg PlanReadyMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePlanReady(msg)
}

// handlePlanApproveMsg processes plan approval and transitions to execute.
func handlePlanApproveMsg(m *AppState, msg PlanApproveMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePlanApprove()
}

// handlePlanRefineMsg processes plan refinement feedback.
func handlePlanRefineMsg(m *AppState, msg PlanRefineMsg) (tea.Model, tea.Cmd) {
	return m, m.handlePlanRefine(msg)
}

// handleDemonstrationReadyMsg processes a demonstration-ready event for the ship screen.
func handleDemonstrationReadyMsg(m *AppState, msg workflow.DemonstrationReadyMsg) (tea.Model, tea.Cmd) {
	if m.shipModel != nil {
		m.shipModel.SetDemonstration(msg.Content)
	}
	return m, nil
}

// handleExecutePauseMsg processes execute pause/resume events.
func handleExecutePauseMsg(m *AppState, msg ExecutePauseMsg) (tea.Model, tea.Cmd) {
	if m.executeModel != nil {
		m.executeModel.paused = msg.Paused
	}
	return m, nil
}

// handleHealResultMsg processes a self-heal result and forwards it to the verify model.
func handleHealResultMsg(m *AppState, msg HealResultMsg) (tea.Model, tea.Cmd) {
	if m.verifyModel != nil {
		newVerify, cmd := m.verifyModel.Update(msg)
		m.verifyModel = newVerify
		return m, cmd
	}
	return m, nil
}

// handleTaskStartWorkflowMsg processes workflow.TaskStartMsg.
func handleTaskStartWorkflowMsg(m *AppState, msg workflow.TaskStartMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowTaskStart(msg)...)
}

// handleTaskUpdateWorkflowMsg processes workflow.TaskUpdateMsg.
func handleTaskUpdateWorkflowMsg(m *AppState, msg workflow.TaskUpdateMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowTaskUpdate(msg)...)
}

// handleToolStartWorkflowMsg processes workflow.ToolStartMsg.
func handleToolStartWorkflowMsg(m *AppState, msg workflow.ToolStartMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowToolStart(msg)...)
}

// handleToolCompleteWorkflowMsg processes workflow.ToolCompleteMsg.
func handleToolCompleteWorkflowMsg(m *AppState, msg workflow.ToolCompleteMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowToolComplete(msg)...)
}

// handleSelfHealStartWorkflowMsg processes workflow.SelfHealStartMsg.
func handleSelfHealStartWorkflowMsg(m *AppState, msg workflow.SelfHealStartMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowSelfHealStart(msg)...)
}

// handleSelfHealCompleteWorkflowMsg processes workflow.SelfHealCompleteMsg.
func handleSelfHealCompleteWorkflowMsg(m *AppState, msg workflow.SelfHealCompleteMsg) (tea.Model, tea.Cmd) {
	return m, tea.Batch(m.handleWorkflowSelfHealComplete(msg)...)
}
