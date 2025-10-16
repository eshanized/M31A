package tui

import (
	"context"
	"log/slog"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// ─── Bubble Tea Model interface ───────────────────────────────────────────────

// Init implements tea.Model. It starts the health ticker and permission listener.
func (m *AppState) Init() tea.Cmd {
	cmds := []tea.Cmd{
		m.routeToScreen(),
		NextHealthTick(types.HealthCheckInterval),
	}
	if m.dispatcher != nil {
		cmds = append(cmds, permListenerCmd(m.shutdownCtx, m.dispatcher))
		cmds = append(cmds, questionListenerCmd(m.shutdownCtx, m.dispatcher))
	}
	return tea.Batch(cmds...)
}

// ─── Shutdown ─────────────────────────────────────────────────────────────────

// Shutdown cleanly tears down all background goroutines.
func (m *AppState) Shutdown() {
	if m.workflowCancel != nil {
		m.workflowCancel()
	}
	if m.shutdownCancel != nil {
		m.shutdownCancel()
	}
	if m.streamCancelFn != nil {
		m.streamCancelFn()
	}
}

// ─── Workflow ─────────────────────────────────────────────────────────────────

// setWorkflowPhase transitions to a new workflow phase, updating state.
func (m *AppState) setWorkflowPhase(phase types.WorkflowPhase) {
	m.workflowPhase = phase
	if m.replModel != nil {
		m.replModel.lastStatus = "Phase: " + string(phase)
	}
}

// RunPhaseCmd runs a workflow phase in a goroutine and returns a tea.Cmd.
func (m *AppState) RunPhaseCmd(phase types.WorkflowPhase) tea.Cmd {
	if m.workflowEngine == nil {
		return func() tea.Msg {
			return PhaseResultMsg{
				Phase:   phase,
				Success: false,
				Error:   "workflow engine not initialized",
			}
		}
	}

	ctx, cancel := context.WithCancel(m.shutdownCtx)
	m.workflowCancel = cancel

	engine := m.workflowEngine
	goal := m.workflowGoal

	return func() tea.Msg {
		result, err := engine.RunPhase(ctx, phase, goal)
		if err != nil {
			return PhaseResultMsg{
				Phase:   phase,
				Success: false,
				Error:   err.Error(),
			}
		}
		if result == nil {
			return PhaseResultMsg{Phase: phase, Success: true}
		}
		return PhaseResultMsg{
			Phase:    phase,
			Tasks:    result.Tasks,
			Messages: result.Messages,
			Success:  result.Error == "",
			Error:    result.Error,
			Usage:    result.Usage,
			Cost:     result.Cost,
		}
	}
}

// ─── Permission/Question listeners ───────────────────────────────────────────

// permListenerCmd reads one permission request from the dispatcher and emits it.
func permListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case req := <-d.RequestCh():
			return PermissionRequestMsg{Request: req}
		case <-ctx.Done():
			return nil
		}
	}
}

// questionListenerCmd reads one question request from the dispatcher and emits it.
func questionListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		select {
		case req := <-d.QuestionRequestCh():
			return QuestionRequestMsg{
				Question:   req.Question,
				Header:     req.Header,
				Options:    req.Options,
				ResponseCh: d.QuestionResponseCh(),
			}
		case <-ctx.Done():
			return nil
		}
	}
}

// ─── Workflow engine initialization ──────────────────────────────────────────

// initWorkflowEngine creates a new workflow.Engine for the current session.
func (m *AppState) initWorkflowEngine() tea.Cmd {
	if m.workflowEngine != nil {
		return nil
	}
	if m.registry == nil || m.activeProvider == "" {
		slog.Warn("cannot init workflow engine: no provider")
		return nil
	}

	p := m.registry.ActiveProvider()
	if p == nil {
		return nil
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}
	if m.config != nil && modelID == "" {
		modelID = m.config.Model.Default
	}

	sessDir := m.sessionManager.BaseDir()
	workDir := "."
	if m.git != nil {
		workDir = m.git.WorkDir()
	}

	planningDir := sessDir + "/" + m.sessionID + "/planning"

	tokenEst := tokens.NewEstimator(modelID)

	engine, err := workflow.NewEngine(
		m.sessionID,
		workDir,
		sessDir+"/backups",
		planningDir,
		p,
		modelID,
		m.dispatcher,
		tokenEst,
		m.sessionManager,
		m.config,
	)
	if err != nil {
		slog.Error("workflow engine init failed", "err", err)
		return nil
	}

	if m.git != nil {
		engine.SetGit(m.git)
	}

	m.workflowEngine = engine
	return nil
}

// persistWorkflowState saves the current workflow state to disk.
func (m *AppState) persistWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	_ = m.sessionManager.UpdateWorkflowState(
		m.sessionID,
		m.workflowGoal,
		m.workflowPhase,
		m.discussQuestions,
	)
}
