package tui

import (
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// nextPhaseForMode returns the next phase to run based on the current phase
// and the active workflow mode. The second return value is true if a transition
// should happen (false means the phase is terminal).
func nextPhaseForMode(from types.WorkflowPhase, mode types.WorkflowMode) (types.WorkflowPhase, bool) {
	switch from {
	case types.PhaseInitialize:
		if mode == types.ModeDirect {
			return types.PhaseExecute, true
		}
		return types.PhaseDiscuss, true

	case types.PhaseDiscuss:
		if mode == types.ModeFast || mode == types.ModeDirect {
			return types.PhaseExecute, true
		}
		return types.PhasePlan, true

	case types.PhasePlan:
		return types.PhaseExecute, true

	case types.PhaseExecute:
		if mode == types.ModeDirect {
			return types.PhaseShip, true
		}
		return types.PhaseVerify, true

	case types.PhaseVerify:
		return types.PhaseShip, true

	case types.PhaseShip:
		return types.PhaseIdle, false
	}
	return types.PhaseIdle, false
}

// handlePhaseResult processes the result of a workflow phase.
func (m *AppState) handlePhaseResult(msg PhaseResultMsg) tea.Cmd {
	slog.Info("phase result", "phase", msg.Phase, "success", msg.Success, "err", msg.Error, "mode", msg.WorkflowMode)

	if msg.Error != "" {
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg("Workflow error: " + msg.Error))
		}
		m.workflowPhase = types.PhaseIdle
		m.screen = ScreenREPL
		return nil
	}

	// Use the mode from the phase result, falling back to the stored mode
	mode := msg.WorkflowMode
	if mode == "" {
		mode = m.workflowMode
	}

	modelID := ""
	modelName := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
		modelName = m.activeModel.Name
	}

	switch msg.Phase {
	case types.PhaseInitialize:
		next, ok := nextPhaseForMode(types.PhaseInitialize, mode)
		if !ok {
			m.workflowPhase = types.PhaseIdle
			return nil
		}
		m.setWorkflowPhase(next)
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseInitialize, next); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseInitialize, "to", next, "error", err)
			}
		}
		m.persistWorkflowState()
		return m.RunPhaseCmd(next)

	case types.PhaseDiscuss:
		if msg.NeedsAnswers {
			// Read questions from the workflow engine's discuss state
			if m.workflowEngine != nil {
				ds := m.workflowEngine.DiscussState()
				m.discussQuestions = ds.Questions
			}
			m.screen = ScreenDiscuss
			timeoutSecs := 0
			if m.config != nil {
				timeoutSecs = m.config.UI.DiscussTimeout
			}
			m.discussModel = NewDiscussModel(
				m.themeManager.Current(),
				m.discussQuestions,
				m.width, m.height,
			)
			if timeoutSecs > 0 {
				m.discussModel.SetTimeout(timeoutSecs)
			}
			m.persistWorkflowState()
			if timeoutSecs > 0 {
				secs := timeoutSecs
				return tea.Tick(time.Duration(secs)*time.Second, func(time.Time) tea.Msg {
					return DiscussAnswerTimeoutMsg{QuestionIndex: 0}
				})
			}
			return nil
		}
		// No questions — skip to next phase based on mode
		if m.workflowEngine != nil {
			if err := m.workflowEngine.SkipDiscuss(); err != nil {
				slog.Warn("failed to skip discuss phase", "error", err)
			}
		}
		next, ok := nextPhaseForMode(types.PhaseDiscuss, mode)
		if !ok || next == types.PhaseIdle {
			m.setWorkflowPhase(types.PhaseIdle)
			m.screen = ScreenREPL
			return nil
		}
		m.setWorkflowPhase(next)
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseDiscuss, next); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseDiscuss, "to", next, "error", err)
			}
		}
		if next == types.PhaseExecute {
			// In fast/direct mode, skip plan and go straight to execution
			m.screen = ScreenExecute
			tasks := msg.Tasks
			if m.executeModel == nil {
				m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), m.width, m.height)
			} else {
				m.executeModel.tasks = tasks
			}
			m.persistWorkflowState()
			return m.RunPhaseCmd(types.PhaseExecute)
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
		m.screen = ScreenPlan
		m.persistWorkflowState()
		return m.RunPhaseCmd(next)

	case types.PhasePlan:
		// Show plan screen and wait for user approval (do NOT auto-advance to Execute)
		m.screen = ScreenPlan
		if m.planModel == nil {
			m.planModel = NewPlanModel(
				msg.Tasks,
				m.themeManager.Current(),
				modelID, modelName, m.activeProvider,
				0, "",
				m.width, m.height,
			)
		} else {
			m.planModel.UpdateTasks(msg.Tasks)
		}
		// Load and set the rich plan markdown content
		if m.workflowEngine != nil {
			m.planModel.SetPlanContent(m.workflowEngine.PlanContent())
			m.planModel.SetPlanVersion(m.workflowEngine.PlanVersion())
		}
		m.persistWorkflowState()
		return nil

	case types.PhaseExecute:
		// Reset workflow mode back to full after execution for fresh classification next time
		next, ok := nextPhaseForMode(types.PhaseExecute, mode)
		if !ok || next == types.PhaseIdle {
			m.setWorkflowPhase(types.PhaseIdle)
			m.screen = ScreenREPL
			return nil
		}
		m.setWorkflowPhase(next)
		m.screen = ScreenVerify
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseExecute, next); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseExecute, "to", next, "error", err)
			}
		}
		if next == types.PhaseShip {
			// Direct mode: skip verify, go straight to ship
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
		}
		m.verifyModel = NewVerifyModel(msg.Tasks, map[int]workflow.VerificationResult{}, m.themeManager.Current(), m.width, m.height)
		if len(msg.ManualVerificationSteps) > 0 {
			m.verifyModel.SetManualSteps(msg.ManualVerificationSteps)
		}
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
		if m.shipModel != nil && msg.Demonstration != "" {
			m.shipModel.SetDemonstration(msg.Demonstration)
		}
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

// handlePlanApprove handles plan acceptance — transitions from Plan to Execute.
func (m *AppState) handlePlanApprove() tea.Cmd {
	m.setWorkflowPhase(types.PhaseExecute)
	m.screen = ScreenExecute
	if m.workflowEngine != nil {
		if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhasePlan, types.PhaseExecute); err != nil {
			slog.Error("phase transition failed", "from", types.PhasePlan, "to", types.PhaseExecute, "error", err)
		}
	}
	tasks := []types.Task{}
	if m.planModel != nil {
		tasks = m.planModel.tasks
	}
	if m.executeModel == nil {
		m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), m.width, m.height)
	} else {
		m.executeModel.tasks = tasks
	}
	m.persistWorkflowState()
	return m.RunPhaseCmd(types.PhaseExecute)
}

// handlePlanRefine handles plan refinement — re-runs Plan with user feedback.
func (m *AppState) handlePlanRefine(msg PlanRefineMsg) tea.Cmd {
	if m.workflowEngine != nil {
		m.workflowEngine.SetRefinementFeedback(msg.Feedback)
	}
	return m.RunPhaseCmd(types.PhasePlan)
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
