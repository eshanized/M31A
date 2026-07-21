package tui

import (
	"fmt"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
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
		// ModeAuto follows full workflow path (no phase skipping)
		return types.PhaseDiscuss, true

	case types.PhaseDiscuss:
		if mode == types.ModeFast || mode == types.ModeDirect {
			return types.PhaseExecute, true
		}
		// ModeAuto follows full workflow path (no phase skipping)
		return types.PhasePlan, true

	case types.PhasePlan:
		return types.PhaseExecute, true

	case types.PhaseExecute:
		if mode == types.ModeDirect {
			return types.PhaseShip, true
		}
		// ModeAuto follows full workflow path (no phase skipping)
		return types.PhaseVerify, true

	case types.PhaseVerify:
		if mode == types.ModeDirect {
			return types.PhaseShip, true
		}
		// ModeAuto follows full workflow path (no phase skipping)
		return types.PhaseRuntime, true

	case types.PhaseRuntime:
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
			m.replModel.AddMessage(MakeAssistantMsg("Workflow error: " + msg.Error))
		}
		m.workflowPhase = types.PhaseIdle
		m.screen = ScreenREPL
		return nil
	}

	// Inline phase completion feedback in the REPL
	if m.replModel != nil {
		summary := fmt.Sprintf("**Phase: %s** — complete ✓", msg.Phase)
		if len(msg.Tasks) > 0 {
			done := 0
			for _, t := range msg.Tasks {
				if t.Status == types.StatusDone {
					done++
				}
			}
			summary += fmt.Sprintf(" (%d/%d tasks)", done, len(msg.Tasks))
		}
		m.replModel.AddMessage(MakeAssistantMsg(summary))
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
			m.switchScreen(ScreenDiscuss)
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
		// No questions — show transition confirmation before proceeding
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
		// Show transition confirmation for Discuss->Plan
		summary := phaseTransitionSummary(types.PhaseDiscuss, msg.Tasks)
		cw, ch := m.contentDimensions()
		m.phaseTransitionModel = NewPhaseTransitionModel(m.themeManager.Current(), types.PhaseDiscuss, next, summary, cw, ch)
		m.switchScreen(ScreenPhaseTransition)
		m.persistWorkflowState()
		return nil

	case types.PhasePlan:
		// Show plan screen and wait for user approval (do NOT auto-advance to Execute)
		m.switchScreen(ScreenPlan)
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
		// Populate sidebar with plan tasks so the user sees the task list immediately
		if m.sidebarModel != nil && len(msg.Tasks) > 0 {
			m.sidebarModel.SetMode(SidebarModeTodo)
			m.sidebarModel.SetCurrentPhase(string(types.PhasePlan))
			m.sidebarModel.InitTaskProgress(len(msg.Tasks))
			for _, task := range msg.Tasks {
				m.sidebarModel.AddTodoItem(SidebarTodoItem{
					Content:  task.Description,
					Status:   "pending",
					Priority: "medium",
					Source:   "task",
					TaskID:   task.ID,
				})
			}
		}
		// Show plan summary in the REPL for inline visibility
		if m.replModel != nil && len(msg.Tasks) > 0 {
			var sb strings.Builder
			fmt.Fprintf(&sb, "**Plan: %d tasks generated**\n\n", len(msg.Tasks))
			for i, task := range msg.Tasks {
				if i >= 15 {
					fmt.Fprintf(&sb, "… and %d more tasks\n", len(msg.Tasks)-15)
					break
				}
				fmt.Fprintf(&sb, "  %d. [%s] %s\n", task.ID, task.Action, task.Description)
			}
			sb.WriteString("\nPress `r` to refine or approve to execute.")
			m.replModel.AddMessage(MakeAssistantMsg(sb.String()))
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
		// Show transition confirmation for Execute->Verify
		summary := phaseTransitionSummary(types.PhaseExecute, msg.Tasks)
		cw, ch := m.contentDimensions()
		m.phaseTransitionModel = NewPhaseTransitionModel(m.themeManager.Current(), types.PhaseExecute, next, summary, cw, ch)
		m.switchScreen(ScreenPhaseTransition)
		m.persistWorkflowState()
		return nil

	case types.PhaseVerify:
		// Show transition confirmation for Verify->Runtime
		summary := phaseTransitionSummary(types.PhaseVerify, msg.Tasks)
		cw, ch := m.contentDimensions()
		m.phaseTransitionModel = NewPhaseTransitionModel(m.themeManager.Current(), types.PhaseVerify, types.PhaseRuntime, summary, cw, ch)
		m.switchScreen(ScreenPhaseTransition)
		m.persistWorkflowState()
		return nil

	case types.PhaseRuntime:
		// Update runtime model with results
		if m.runtimeModel != nil && msg.RuntimeSummary != nil {
			m.runtimeModel.SetSummary(*msg.RuntimeSummary)
		} else if m.runtimeModel != nil {
			m.runtimeModel.SetSummary(workflow.RuntimeSummary{
				TotalPassed: 0,
				TotalFailed: 0,
				TotalTests:  0,
				Errors:      []string{"runtime verification skipped — no dev server could be started"},
			})
		}
		// Show runtime screen and wait for user to continue
		m.switchScreen(ScreenRuntimeCheck)
		m.persistWorkflowState()
		return nil

	case types.PhaseShip:
		m.setWorkflowPhase(types.PhaseIdle)
		m.persistWorkflowState()
		m.switchScreen(ScreenShip)
		if m.shipModel != nil && msg.Demonstration != "" {
			m.shipModel.SetDemonstration(msg.Demonstration)
		}
		// Auto-revert sidebar from todo mode back to file tree
		if m.sidebarModel != nil && m.sidebarModel.GetMode() == SidebarModeTodo {
			m.sidebarModel.RevertToFiles()
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
	m.switchScreen(ScreenExecute)
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
		cw, ch := m.contentDimensions()
		m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), cw, ch)
		m.executeModel.SetWorkflowEngine(m.workflowEngine)
		m.router.Register(ScreenExecute, m.executeModel)
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

// handlePhaseTransitionDecision processes the user's decision on phase transition.
func (m *AppState) handlePhaseTransitionDecision(msg PhaseTransitionMsg) (tea.Model, tea.Cmd) {
	if msg.Approved {
		// Proceed to next phase
		m.setWorkflowPhase(msg.To)
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, msg.From, msg.To); err != nil {
				slog.Error("phase transition failed", "from", msg.From, "to", msg.To, "error", err)
			}
		}
		// Handle specific transitions
		switch msg.To {
		case types.PhasePlan:
			if m.planModel == nil {
				cw, ch := m.contentDimensions()
				m.planModel = NewPlanModel(
					[]types.Task{},
					m.themeManager.Current(),
					"", "", m.activeProvider,
					0, "",
					cw, ch,
				)
			}
			m.switchScreen(ScreenPlan)
			m.persistWorkflowState()
			return nil, m.RunPhaseCmd(types.PhasePlan)
		case types.PhaseExecute:
			tasks := []types.Task{}
			if m.planModel != nil {
				tasks = m.planModel.tasks
			}
			if m.executeModel == nil {
				cw, ch := m.contentDimensions()
				m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), cw, ch)
				m.executeModel.SetWorkflowEngine(m.workflowEngine)
				m.router.Register(ScreenExecute, m.executeModel)
			} else {
				m.executeModel.tasks = tasks
			}
			m.switchScreen(ScreenExecute)
			m.persistWorkflowState()
			return nil, m.RunPhaseCmd(types.PhaseExecute)
		case types.PhaseVerify:
			cw, ch := m.contentDimensions()
			m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenVerify, m.verifyModel)
			m.switchScreen(ScreenVerify)
			m.persistWorkflowState()
			return nil, m.RunPhaseCmd(types.PhaseVerify)
		case types.PhaseRuntime:
			if m.runtimeModel == nil {
				cw, ch := m.contentDimensions()
				m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
				m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
			}
			m.switchScreen(ScreenRuntimeCheck)
			m.persistWorkflowState()
			return nil, m.RunPhaseCmd(types.PhaseRuntime)
		}
		return nil, m.RunPhaseCmd(msg.To)
	}

	if msg.GoBack {
		// Go back to previous phase
		m.setWorkflowPhase(msg.From)
		switch msg.From {
		case types.PhaseDiscuss:
			m.switchScreen(ScreenDiscuss)
		case types.PhasePlan:
			m.switchScreen(ScreenPlan)
		case types.PhaseExecute:
			m.switchScreen(ScreenExecute)
		case types.PhaseVerify:
			m.switchScreen(ScreenVerify)
		default:
			m.switchScreen(ScreenREPL)
		}
		m.persistWorkflowState()
		return nil, nil
	}

	// Cancel — return to REPL
	m.setWorkflowPhase(types.PhaseIdle)
	m.switchScreen(ScreenREPL)
	m.persistWorkflowState()
	return nil, nil
}
