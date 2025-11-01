package tui

import (
	"log/slog"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// handlePhaseResult processes the result of a workflow phase.
func (m *AppState) handlePhaseResult(msg PhaseResultMsg) tea.Cmd {
	slog.Info("phase result", "phase", msg.Phase, "success", msg.Success, "err", msg.Error)

	if msg.Error != "" {
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg("Workflow error: " + msg.Error))
		}
		m.workflowPhase = types.PhaseIdle
		m.screen = ScreenREPL
		return nil
	}

	modelID := ""
	modelName := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
		modelName = m.activeModel.Name
	}

	switch msg.Phase {
	case types.PhaseInitialize:
		m.setWorkflowPhase(types.PhaseDiscuss)
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(m.shutdownCtx, types.PhaseInitialize, types.PhaseDiscuss)
		}
		return m.RunPhaseCmd(types.PhaseDiscuss)

	case types.PhaseDiscuss:
		m.setWorkflowPhase(types.PhasePlan)
		m.screen = ScreenPlan
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(m.shutdownCtx, types.PhaseDiscuss, types.PhasePlan)
		}
		if m.planModel == nil {
			m.planModel = NewPlanModel(
				msg.Tasks,
				m.themeManager.Current(),
				modelID, modelName, m.activeProvider,
				0, "",
				m.width, m.height,
			)
		}
		return m.RunPhaseCmd(types.PhasePlan)

	case types.PhasePlan:
		m.setWorkflowPhase(types.PhaseExecute)
		m.screen = ScreenExecute
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(m.shutdownCtx, types.PhasePlan, types.PhaseExecute)
		}
		if m.executeModel == nil {
			m.executeModel = NewExecuteModel(msg.Tasks, m.themeManager.Current(), m.width, m.height)
		} else {
			m.executeModel.tasks = msg.Tasks
		}
		return m.RunPhaseCmd(types.PhaseExecute)

	case types.PhaseExecute:
		m.setWorkflowPhase(types.PhaseVerify)
		m.screen = ScreenVerify
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(m.shutdownCtx, types.PhaseExecute, types.PhaseVerify)
		}
		m.verifyModel = NewVerifyModel(msg.Tasks, map[int]workflow.VerificationResult{}, m.themeManager.Current(), m.width, m.height)
		return m.RunPhaseCmd(types.PhaseVerify)

	case types.PhaseVerify:
		m.setWorkflowPhase(types.PhaseShip)
		m.screen = ScreenShip
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(m.shutdownCtx, types.PhaseVerify, types.PhaseShip)
		}
		summary := ShipSummary{
			SessionID: m.sessionID,
			TaskDone:  countDone(msg.Tasks),
			TaskTotal: len(msg.Tasks),
			Model:     modelName,
			Provider:  m.activeProvider,
		}
		if msg.Usage != nil {
			summary.TotalTokens = msg.Usage.TotalTokens
		}
		summary.TotalCost = msg.Cost
		m.shipModel = NewShipModel(summary, m.themeManager.Current(), m.width, m.height)
		return m.RunPhaseCmd(types.PhaseShip)

	case types.PhaseShip:
		m.setWorkflowPhase(types.PhaseIdle)
		m.persistWorkflowState()
		m.screen = ScreenShip
		return nil
	}

	return nil
}

// handlePlanReady handles PlanReadyMsg when the planning phase emits tasks.
func (m *AppState) handlePlanReady(msg PlanReadyMsg) tea.Cmd {
	if m.planModel != nil {
		m.planModel.UpdateTasks(msg.Tasks)
	}
	return nil
}

// countDone counts done tasks.
func countDone(tasks []types.Task) int {
	n := 0
	for _, t := range tasks {
		if t.Status == types.StatusDone {
			n++
		}
	}
	return n
}
