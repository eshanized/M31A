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
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseInitialize, types.PhaseDiscuss); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseInitialize, "to", types.PhaseDiscuss, "error", err)
			}
		}
		m.persistWorkflowState()
		return m.RunPhaseCmd(types.PhaseDiscuss)

	case types.PhaseDiscuss:
		if msg.NeedsAnswers {
			m.screen = ScreenDiscuss
			if m.discussModel == nil {
				m.discussModel = NewDiscussModel(
					m.themeManager.Current(),
					m.discussQuestions,
					m.width, m.height,
				)
			}
			m.persistWorkflowState()
			return nil
		}
		m.setWorkflowPhase(types.PhasePlan)
		m.screen = ScreenPlan
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseDiscuss, types.PhasePlan); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseDiscuss, "to", types.PhasePlan, "error", err)
			}
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
		m.persistWorkflowState()
		return m.RunPhaseCmd(types.PhasePlan)

	case types.PhasePlan:
		m.setWorkflowPhase(types.PhaseExecute)
		m.screen = ScreenExecute
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhasePlan, types.PhaseExecute); err != nil {
				slog.Error("phase transition failed", "from", types.PhasePlan, "to", types.PhaseExecute, "error", err)
			}
		}
		if m.executeModel == nil {
			m.executeModel = NewExecuteModel(msg.Tasks, m.themeManager.Current(), m.width, m.height)
		} else {
			m.executeModel.tasks = msg.Tasks
		}
		m.persistWorkflowState()
		return m.RunPhaseCmd(types.PhaseExecute)

	case types.PhaseExecute:
		m.setWorkflowPhase(types.PhaseVerify)
		m.screen = ScreenVerify
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseExecute, types.PhaseVerify); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseExecute, "to", types.PhaseVerify, "error", err)
			}
		}
		m.verifyModel = NewVerifyModel(msg.Tasks, map[int]workflow.VerificationResult{}, m.themeManager.Current(), m.width, m.height)
		m.persistWorkflowState()
		// Refresh sidebar git status after task execution
		if m.sidebarModel != nil {
			return tea.Batch(m.RunPhaseCmd(types.PhaseVerify), m.sidebarModel.refreshCmd())
		}
		return m.RunPhaseCmd(types.PhaseVerify)

	case types.PhaseVerify:
		m.setWorkflowPhase(types.PhaseShip)
		m.screen = ScreenShip
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseVerify, types.PhaseShip); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseVerify, "to", types.PhaseShip, "error", err)
			}
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
		m.persistWorkflowState()
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
		m.planModel.timeEstimate = msg.TimeEstimate
		if msg.CostEstimate != "" {
			m.planModel.costEstimate = msg.CostEstimate
		}
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
