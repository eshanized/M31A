package tui

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func (m *AppState) initWorkflowEngine() {
	if m.registry == nil {
		m.currentOperation = "Workflow engine init failed: no provider registry"
		return
	}
	if m.activeProvider == "" {
		m.currentOperation = "Workflow engine init failed: no active provider"
		return
	}
	if m.sessionManager == nil {
		m.currentOperation = "Workflow engine init failed: no session manager"
		return
	}
	p := m.registry.ActiveProvider()
	if p == nil {
		m.currentOperation = "Workflow engine init failed: active provider is nil"
		return
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}
	if modelID == "" && m.config != nil {
		modelID = m.config.Model.Default
	}
	if modelID == "" {
		m.currentOperation = "Workflow engine init failed: no model selected"
		return
	}

	// Create a session for the workflow
	s, err := m.sessionManager.NewSession(modelID, m.activeProvider)
	if err != nil {
		m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
		return
	}

	// Wire session ID to tool dispatcher (e.g. TodoWrite)
	m.dispatcher.SetSessionID(s.ID)
	m.sessionID = s.ID

	cwd, err := os.Getwd()
	if err != nil {
		cwd = os.TempDir()
	}
	backupDir := filepath.Join(filepath.Dir(m.configPath), "backups")
	sessionBaseDir := filepath.Join(filepath.Dir(m.configPath), "sessions")
	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	est := tokens.NewEstimatorWithOpts(modelID, tokens.EstimatorOpts{
		EMAAlpha: m.config.Model.TokenEMAAlpha,
	})

	eng, err := workflow.NewEngine(s.ID, cwd, backupDir, planningDir,
		p, modelID, m.dispatcher, est, m.sessionManager, m.config)
	if err != nil {
		m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
		return
	}
	eng.SetGit(m.git)
	m.workflowEngine = eng
}

// persistWorkflowState writes the current workflow state (goal, phase,
// pending discuss questions) to session.json. Failures are logged but
// not returned — the workflow continues even if persistence fails (the
// user can still finish the workflow in this session).
//
// Called on every phase transition so closing the app mid-workflow
// preserves progress. After the Ship phase, the persisted state is
// reset to idle so a future /workflow starts fresh.
func (m *AppState) persistWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	if err := m.sessionManager.UpdateWorkflowState(
		m.sessionID, m.workflowGoal, m.currentPhase, m.discussQuestions,
	); err != nil {
		slog.Warn("persistWorkflowState failed", "err", err)
	}
}

// checkResumedWorkflowState reads the persisted workflow state for the
// current session. If a workflow was in progress (phase != idle, phase
// != ship), pre-populates the AppState and shows a toast so the user
// can run /workflow resume to continue. Called once during NewApp
// after initWorkflowEngine has set m.sessionID.
//
// The toast uses the AppState's existing toast fields (toastText,
// toastType, toastExpires) — not a dedicated ToastMsg queue — to match
// the convention used elsewhere in the TUI.
func (m *AppState) checkResumedWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	goal, phase, questions, err := m.sessionManager.LoadWorkflowState(m.sessionID)
	if err != nil {
		// Persistence read failures are non-fatal; the workflow can
		// still start fresh. Log at debug level.
		slog.Debug("LoadWorkflowState failed", "err", err)
		return
	}
	if phase == types.PhaseIdle || phase == types.PhaseShip {
		// No workflow in progress — nothing to resume.
		return
	}
	// Workflow was in progress — pre-populate state for /workflow resume
	m.workflowGoal = goal
	m.currentPhase = phase
	m.discussQuestions = questions
	m.toastText = fmt.Sprintf("Resumable workflow at %s. Use /workflow resume to continue.", phase)
	m.toastType = "info"
	m.toastExpires = time.Now().Add(types.ToastDuration)
}

// resetDiscussQA clears the discuss Q&A state and stops the active timer.
// Called when leaving the discuss phase (finalize, skip, error) so subsequent
// Q&A rounds start from a clean slate.
func (m *AppState) resetDiscussQA() {
	if m.discussAnswerTimeout != nil {
		m.discussAnswerTimeout.Stop()
		m.discussAnswerTimeout = nil
	}
	m.pendingDiscussAnswers = nil
	m.currentDiscussIndex = 0
	m.discussQuestionCount = 0
}

// askNextDiscussQuestion emits a QuestionRequestMsg for the current
// question and starts a 5-minute timeout. Returns a tea.Cmd that
// produces both the question and the timeout (use tea.Batch).
func (m *AppState) askNextDiscussQuestion() tea.Cmd {
	if m.currentDiscussIndex >= len(m.discussQuestions) {
		// All questions answered — finalize and advance
		return m.finalizeDiscussAndAdvance()
	}
	q := m.discussQuestions[m.currentDiscussIndex]
	header := fmt.Sprintf("Discuss Q%d/%d", m.currentDiscussIndex+1, m.discussQuestionCount)

	// Stop any existing timeout
	if m.discussAnswerTimeout != nil {
		m.discussAnswerTimeout.Stop()
	}
	// Start timeout (configurable via UIConfig.DiscussTimeout, default 5 minutes)
	timeoutSecs := 300
	if m.config != nil && m.config.UI.DiscussTimeout > 0 {
		timeoutSecs = m.config.UI.DiscussTimeout
	}
	m.discussAnswerTimeout = time.NewTimer(time.Duration(timeoutSecs) * time.Second)

	// C-3 fix: add nil guard on dispatcher to prevent panic
	var responseCh chan tools.QuestionResponse
	if m.dispatcher != nil {
		responseCh = m.dispatcher.QuestionResponseCh()
	}

	return tea.Batch(
		func() tea.Msg {
			return QuestionRequestMsg{
				Question:    q,
				Header:      header,
				Options:     []string{},
				AllowCustom: true,
				ResponseCh:  responseCh,
			}
		},
		// C-3 fix: use a goroutine that checks if the timer channel is ready
		// instead of blocking on <-m.discussAnswerTimeout.C which leaks if
		// the phase changes before the timer fires.
		func() tea.Msg {
			<-m.discussAnswerTimeout.C
			return DiscussAnswerTimeoutMsg{QuestionIndex: m.currentDiscussIndex}
		},
	)
}

// finalizeDiscussAndAdvance calls engine.FinalizeDiscuss, then advances to Plan.
func (m *AppState) finalizeDiscussAndAdvance() tea.Cmd {
	if m.workflowEngine != nil {
		if err := m.workflowEngine.FinalizeDiscuss(); err != nil {
			slog.Warn("FinalizeDiscuss failed", "err", err)
			m.setWorkflowPhase(types.PhaseIdle)
			return nil
		}
	}
	m.resetDiscussQA()
	m.setWorkflowPhase(types.PhasePlan)
	return RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)
}

// skipDiscussAndAdvance calls engine.SkipDiscuss, then advances to Plan.
func (m *AppState) skipDiscussAndAdvance() tea.Cmd {
	if m.workflowEngine != nil {
		if err := m.workflowEngine.SkipDiscuss(); err != nil {
			slog.Warn("SkipDiscuss failed", "err", err)
		}
	}
	return m.finalizeDiscussAndAdvance()
}

func (m *AppState) handleKeyAction(msg KeyActionMsg) (*AppState, tea.Cmd) {
	switch msg.Action {
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			m.sidebarManuallyHidden = !m.sidebarModel.IsVisible()
			if m.sidebarModel.IsVisible() {
				if m.replModel != nil {
					m.replModel.SetSidebarWidth(sidebarWidth)
				}
				return m, m.sidebarModel.refreshCmd()
			}
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
		return m, nil
	case "open_settings":
		m.screen = ScreenSettings
		return m, nil
	case "new_session":
		m.screen = ScreenFirstRun
		m.replModel = nil
		return m, nil
	case "session_list":
		if m.resumeModel != nil {
			m.resumeModel.Refresh()
		}
		m.screen = ScreenResume
		return m, nil
	case "cycle_model":
		if m.registry != nil {
			m.prevScreen = m.screen
			m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
			m.screen = ScreenModelSelector
			return m, m.modelSelector.Init()
		}
	case "cycle_model_forward":
		if m.sessionManager != nil {
			cmd := m.cycleRecentModel(+1)
			return m, cmd
		}
	case "cycle_model_backward":
		if m.sessionManager != nil {
			cmd := m.cycleRecentModel(-1)
			return m, cmd
		}
	case "toggle_theme":
		if m.themeManager != nil {
			m.themeManager.Cycle()
			m.headerCacheValid = false // H-10: invalidate header cache on theme change
			t := m.themeManager.Current()
			if m.replModel != nil {
				m.replModel.SetTheme(t)
			}
			if m.sidebarModel != nil {
				m.sidebarModel.SetTheme(t)
			}
			if m.settingsModel != nil {
				m.settingsModel.SetTheme(t)
			}
		}
	}
	return m, nil
}
