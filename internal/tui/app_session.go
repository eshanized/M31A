package tui

// Session lifecycle, workflow execution, permission, and question handling for AppState.

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// startNewSession creates a new session and switches to the REPL.
func (m *AppState) startNewSession() tea.Cmd {
	if m.sessionManager == nil {
		return nil
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}

	sess, err := m.sessionManager.NewSession(modelID, m.activeProvider)
	if err != nil {
		slog.Error("new session failed", "err", err)
		m.addToast("Failed to create session: "+m31errors.UserMessage(err), "error")
		return nil
	}

	m.sessionID = sess.ID
	m.propagateSessionID(sess.ID)
	m.ensureReplModel()
	m.replModel.ClearMessages()
	m.replModel.SetSessionID(sess.ID)
	m.workflowPhase = types.PhaseIdle
	m.workflowGoal = ""
	// Only switch to REPL if not already on a dedicated landing/startup screen.
	if m.screen != ScreenHome && m.screen != ScreenFirstRun {
		m.screen = ScreenREPL
	}

	return m.syncReplProvider(sess.ID)
}

// runWorkflowFromGoal starts the workflow with adaptive phase routing.
// If m.workflowPhase is already set (e.g., from /resume-task), it resumes from that phase.
func (m *AppState) runWorkflowFromGoal(goal string) tea.Cmd {
	m.workflowGoal = goal
	cmds := []tea.Cmd{m.initWorkflowEngine()}

	// Switch sidebar to todo mode immediately so the user sees phase/task
	// progress instead of the file tree from the moment the workflow starts.
	if m.sidebarModel != nil {
		m.sidebarModel.SetMode(SidebarModeTodo)
	}

	// Classify the goal and set the workflow mode on the engine
	if m.workflowEngine != nil {
		mode := m.resolveWorkflowMode(goal)
		m.workflowEngine.SetWorkflowMode(mode)
		m.workflowMode = mode
		if mode != types.ModeFull {
			m.addToast(fmt.Sprintf("Workflow mode: %s (adaptive — skipping unnecessary phases)", mode), "info")
		}
	}

	// Resume from existing phase if set, otherwise start from Initialize
	startPhase := m.workflowPhase
	if startPhase == types.PhaseIdle || startPhase == "" {
		startPhase = types.PhaseInitialize
	}
	m.workflowPhase = startPhase

	// Seed the sidebar phase pipeline so the phase bar is visible right away.
	if m.sidebarModel != nil {
		m.sidebarModel.SetCurrentPhase(string(startPhase))
	}

	cmds = append(cmds, m.RunPhaseCmd(startPhase))
	return tea.Batch(cmds...)
}

// resolveWorkflowMode determines the appropriate workflow mode.
// If the user has set an explicit mode via config, that takes precedence.
// If a prior intent classification result is available, it uses that.
// Otherwise, the goal is classified with keyword heuristics.
func (m *AppState) resolveWorkflowMode(goal string) types.WorkflowMode {
	// Config override takes highest precedence
	if m.config != nil {
		switch m.config.Features.WorkflowMode {
		case string(types.ModeFull):
			return types.ModeFull
		case string(types.ModeFast):
			return types.ModeFast
		case string(types.ModeDirect):
			return types.ModeDirect
		}
	}

	// Use intent result from the workflow engine if available (LLM-classified)
	if m.workflowEngine != nil {
		if eng, ok := m.workflowEngine.(*workflow.Engine); ok {
			if ir := eng.IntentResult(); ir != nil {
				return types.WorkflowModeForIntent(*ir)
			}
		}
	}

	// Use pending intent if available (from REPL classification)
	if m.pendingIntent != nil {
		return types.WorkflowModeForIntent(*m.pendingIntent)
	}

	// Classify based on goal content when mode is auto or unset
	workDir := "."
	if m.git != nil {
		workDir = m.git.WorkDir()
	}
	complexity := workflow.ClassifyPrompt(goal, workDir)
	mode := workflow.WorkflowModeForComplexity(complexity)

	// Apply quick mode adjustment for simple tasks when enabled
	return adjustModeForQuickMode(mode, goal, m.quickMode)
}

// handlePermissionResponse processes the user's permission decision.
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) tea.Cmd {
	if m.dispatcher == nil {
		return nil
	}
	if m.permRequest == nil {
		return nil
	}
	reqID := m.permRequest.ID
	allowed := msg.Response.Allowed
	remember := msg.Response.Remember
	m.permRequest = nil
	// Return to the previous screen (before permission overlay)
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.screen = prev
	} else {
		m.screen = ScreenREPL
	}
	m.dispatcher.ApprovePermission(reqID, allowed, remember)
	// Update sidebar pending count after approval/denial
	if m.sidebarModel != nil {
		m.sidebarModel.SetPendingPermCount(m.dispatcher.PendingPermCount())
	}
	return permListenerCmd(m.shutdownCtx, m.dispatcher)
}

// handlePermissionTick decrements the permission countdown.
func (m *AppState) handlePermissionTick() tea.Cmd {
	if m.permRequest == nil {
		return nil
	}
	if m.permCountdown > 0 {
		m.permCountdown--
		if m.permModal != nil {
			m.permModal.Tick()
		}
		if m.permCountdown == 0 {
			// Auto-deny on timeout — guard against permRequest already cleared
			if m.permRequest == nil {
				return nil
			}
			return m.handlePermissionResponse(PermissionResponseMsg{
				Response: tools.PermissionResponse{
					RequestID: m.permRequest.ID,
					Allowed:   false,
				},
			})
		}
	}
	return tea.Tick(time.Second, func(time.Time) tea.Msg {
		return PermissionTickMsg{}
	})
}

// handlePermissionKey processes keys in the permission modal.
func (m *AppState) handlePermissionKey(msg tea.KeyMsg) tea.Cmd {
	if m.questionRequest != nil {
		return m.handleQuestionKey(msg)
	}
	if m.permRequest == nil {
		// Return to previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return nil
	}
	switch msg.String() {
	case "y":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
			},
		})
	case "a", "A":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
				Remember:  true,
			},
		})
	case "b", "B":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID:  m.permRequest.ID,
				Allowed:    true,
				ApproveAll: true,
			},
		})
	case "n", "enter", "esc":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   false,
			},
		})
	}
	return nil
}

// handleQuestionKey routes key events to the question model.
func (m *AppState) handleQuestionKey(msg tea.KeyMsg) tea.Cmd {
	if m.questionModel == nil || m.questionRequest == nil {
		// Return to previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return nil
	}
	_, cmd := m.questionModel.Update(msg)
	// QuestionModel emits tools.QuestionResponse via cmd
	return cmd
}

// handleQuestionResponse processes the user's question answer.
func (m *AppState) handleQuestionResponse(msg QuestionResponseMsg) tea.Cmd {
	if m.dispatcher == nil || m.questionRequest == nil {
		return nil
	}
	reqID := m.questionRequest.ID
	m.questionRequest = nil
	// Return to the previous screen (before question overlay)
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.screen = prev
	} else {
		m.screen = ScreenREPL
	}

	m.dispatcher.RespondQuestion(reqID, msg.Answer)
	return questionListenerCmd(m.shutdownCtx, m.dispatcher)
}

// handleDiscussAnswer submits a single discuss answer to the workflow engine.
func (m *AppState) handleDiscussAnswer(msg DiscussAnswerMsg) tea.Cmd {
	if m.workflowEngine == nil {
		return nil
	}
	if err := m.workflowEngine.SubmitDiscussAnswer(msg.Index, msg.Answer); err != nil {
		slog.Warn("failed to submit discuss answer", "index", msg.Index, "error", err)
	}
	return nil
}

// handleDiscussComplete finalizes the discuss phase and transitions to planning.
func (m *AppState) handleDiscussComplete() tea.Cmd {
	if m.workflowEngine == nil {
		return nil
	}
	if err := m.workflowEngine.FinalizeDiscuss(); err != nil {
		slog.Error("failed to finalize discuss", "error", err)
		m.addToast("Failed to save discuss answers", "error")
		return nil
	}

	next, ok := nextPhaseForMode(types.PhaseDiscuss, m.workflowMode)
	if !ok || next == types.PhaseIdle {
		m.setWorkflowPhase(types.PhaseIdle)
		m.screen = ScreenREPL
		return nil
	}

	m.setWorkflowPhase(next)
	if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseDiscuss, next); err != nil {
		slog.Error("phase transition failed", "from", types.PhaseDiscuss, "to", next, "error", err)
		m.addToast("Phase transition failed — session may not resume correctly", "error")
		return nil
	}
	m.persistWorkflowState()

	if next == types.PhaseExecute {
		m.switchScreen(ScreenExecute)
		var tasks []types.Task
		if m.sessionManager != nil {
			var loadErr error
			tasks, loadErr = m.sessionManager.LoadTasks(m.sessionID)
			if loadErr != nil {
				slog.Warn("failed to load tasks for execute screen", "error", loadErr)
			}
		}
		if m.executeModel == nil {
			cw, ch := m.contentDimensions()
			m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), cw, ch)
			m.executeModel.SetWorkflowEngine(m.workflowEngine)
		} else {
			m.executeModel.tasks = tasks
		}
		return m.RunPhaseCmd(types.PhaseExecute)
	}

	m.switchScreen(ScreenPlan)
	return m.RunPhaseCmd(next)
}
