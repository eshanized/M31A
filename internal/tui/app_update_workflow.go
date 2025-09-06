package tui

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// handlePlanReady handles the PlanReadyMsg when the plan phase completes with valid tasks.
func (m *AppState) handlePlanReady(msg PlanReadyMsg) (tea.Model, tea.Cmd) {
	if len(msg.Tasks) > 0 {
		if m.planModel == nil {
			t := m.themeManager.Current()
			modelID := ""
			modelName := ""
			if m.activeModel != nil {
				modelID = m.activeModel.ID
				modelName = m.activeModel.Name
			}
			pm := NewPlanModel(msg.Tasks, t, modelID, modelName, m.activeProvider, 0, msg.CostEstimate, m.width, m.height)
			m.planModel = pm
		} else {
			m.planModel.UpdateTasks(msg.Tasks)
			m.planModel.SetDimensions(m.width, m.height)
		}
	}
	m.currentOperation = fmt.Sprintf("Plan ready: %d tasks", len(msg.Tasks))
	return m, workflowMsgDrainer(m, m.phaseGen, m.msgDoneCloser.chan_())
}

// handleTaskStart handles workflow.TaskStartMsg when a task begins execution.
func (m *AppState) handleTaskStart(msg workflow.TaskStartMsg) (tea.Model, tea.Cmd) {
	m.currentOperation = fmt.Sprintf("Running task %d: %s", msg.Task.ID, msg.Task.Action)
	if m.executeModel != nil {
		m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
	}
	return m, workflowMsgDrainer(m, m.phaseGen, m.msgDoneCloser.chan_())
}

// handleTaskUpdate handles workflow.TaskUpdateMsg when a task status changes.
func (m *AppState) handleTaskUpdate(msg workflow.TaskUpdateMsg) (tea.Model, tea.Cmd) {
	m.currentOperation = fmt.Sprintf("Task %d: %s", msg.Task.ID, msg.Status)
	if m.executeModel != nil {
		switch msg.Status {
		case "done":
			m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusDone)
		case "failed":
			m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusFailed)
		}
	}
	return m, workflowMsgDrainer(m, m.phaseGen, m.msgDoneCloser.chan_())
}

// handlePhaseResult handles PhaseResultMsg when a workflow phase completes.
func (m *AppState) handlePhaseResult(msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	if msg.Error != "" {
		m.currentOperation = fmt.Sprintf("Phase %s failed: %s", msg.Phase, msg.Error)
		m.setWorkflowPhase(types.PhaseIdle)
		m.resetDiscussQA()
		m.flushPendingStreamChunks()
		if m.replModel != nil {
			m.replModel.AddMessage(types.Message{
				Role:    "assistant",
				Content: fmt.Sprintf("Workflow error in %s: %s", msg.Phase, msg.Error),
				Segments: []types.MessageSegment{{
					Type: "content", Content: fmt.Sprintf("Workflow error in %s: %s", msg.Phase, msg.Error), Visible: true,
				}},
				CreatedAt: time.Now(),
			})
		}
		return m, nil
	}
	if !msg.Success {
		m.currentOperation = fmt.Sprintf("Phase %s completed unsuccessfully", msg.Phase)
		m.setWorkflowPhase(types.PhaseIdle)
		m.resetDiscussQA()
		m.flushPendingStreamChunks()
		return m, nil
	}
	m.currentOperation = fmt.Sprintf("Phase %s completed", msg.Phase)

	switch msg.Phase {
	case types.PhaseInitialize:
		// TUI coordinates the phase transition (not the engine)
		if m.workflowEngine != nil {
			_ = m.workflowEngine.Transition(context.Background(), types.PhaseInitialize, types.PhaseDiscuss)
		}
		m.setWorkflowPhase(types.PhaseDiscuss)
		m.persistWorkflowState()
		return m, RunPhaseCmd(m, types.PhaseDiscuss, m.workflowGoal)

	case types.PhaseDiscuss:
		return handlePhaseDiscuss(m, msg)

	case types.PhasePlan:
		return handlePhasePlan(m, msg)

	case types.PhaseExecute:
		return handlePhaseExecute(m, msg)

	case types.PhaseVerify:
		return handlePhaseVerify(m, msg)

	case types.PhaseShip:
		return handlePhaseShip(m, msg)

	case types.PhaseIdle:
		m.setWorkflowPhase(types.PhaseIdle)
		m.flushPendingStreamChunks()
		m.currentOperation = ""
		return m, nil

	default:
		return m, nil
	}
}

// flushPendingStreamChunks delivers any buffered stream chunks to the REPL model.
func (m *AppState) flushPendingStreamChunks() {
	for _, chunk := range m.pendingStreamChunks {
		if m.replModel != nil {
			m.replModel.AppendStreamChunk(chunk)
		}
	}
	m.pendingStreamChunks = nil
}

// handlePhaseDiscuss handles the Discuss phase completion within PhaseResultMsg.
func handlePhaseDiscuss(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	if m.workflowEngine == nil {
		m.currentOperation = "Discuss phase: no engine"
		m.setWorkflowPhase(types.PhaseIdle)
		return m, nil
	}
	engineState := m.workflowEngine.DiscussState()
	questions := engineState.Questions

	if len(questions) == 0 {
		for _, msg2 := range msg.Messages {
			if msg2.Role == "assistant" {
				questions = append(questions, msg2.Content)
			}
		}
	}

	m.discussQuestions = questions
	m.discussQuestionCount = len(questions)

	if !msg.NeedsAnswers || len(questions) == 0 {
		m.setWorkflowPhase(types.PhasePlan)
		m.persistWorkflowState()
		return m, RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)
	}

	m.pendingDiscussAnswers = make(map[int]string)
	m.currentDiscussIndex = 0
	m.setWorkflowPhase(types.PhaseDiscuss)
	m.screen = ScreenREPL
	m.persistWorkflowState()

	return m, m.askNextDiscussQuestion()
}

// handlePhasePlan handles the Plan phase completion within PhaseResultMsg.
func handlePhasePlan(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	if len(msg.Tasks) > 0 {
		if m.planModel == nil {
			t := m.themeManager.Current()
			modelID := ""
			modelName := ""
			if m.activeModel != nil {
				modelID = m.activeModel.ID
				modelName = m.activeModel.Name
			}
			providerName := m.activeProvider
			pm := NewPlanModel(msg.Tasks, t, modelID, modelName, providerName, 0, "", m.width, m.height)
			m.planModel = pm
		} else {
			m.planModel.UpdateTasks(msg.Tasks)
			m.planModel.SetDimensions(m.width, m.height)
		}
	}
	if m.planModel != nil {
		m.planModel.width = m.width
		m.planModel.height = m.height
		m.screen = ScreenPlan
		m.setWorkflowPhase(types.PhasePlan)
		m.persistWorkflowState()
		return m, nil
	}
	m.setWorkflowPhase(types.PhaseExecute)
	m.persistWorkflowState()
	return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)
}

// handlePhaseExecute handles the Execute phase completion within PhaseResultMsg.
func handlePhaseExecute(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	t := m.themeManager.Current()
	m.executeModel = NewExecuteModel(msg.Tasks, t, m.width, m.height)
	m.executeModel.width = m.width
	m.executeModel.height = m.height
	m.executeModel.toolCalls = msg.ToolCalls
	if msg.Usage != nil {
		m.executeModel.totalTokens = msg.Usage.TotalTokens
	}
	m.executeModel.totalCost = msg.Cost
	m.screen = ScreenExecute
	m.setWorkflowPhase(types.PhaseExecute)
	m.persistWorkflowState()
	return m, nil
}

// handlePhaseVerify handles the Verify phase completion within PhaseResultMsg.
func handlePhaseVerify(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	t := m.themeManager.Current()
	results := make(map[int]workflow.VerificationResult)
	for _, task := range msg.Tasks {
		results[task.ID] = workflow.VerificationResult{
			FilesExist: task.Status == types.StatusDone,
			SyntaxOK:   task.Status != types.StatusFailed,
			TestsOK:    task.Status == types.StatusDone,
		}
	}
	if m.verifyModel == nil {
		m.verifyModel = NewVerifyModel(msg.Tasks, results, t, m.width, m.height)
	} else {
		m.verifyModel.UpdateResults(results)
	}
	// Set heal callback to trigger self-healing via the workflow engine
	m.verifyModel.SetHealFunc(func(taskID int) tea.Cmd {
		return func() tea.Msg {
			if m.workflowEngine != nil {
				m.workflowEngine.HealTask(taskID)
			}
			return nil
		}
	})
	m.verifyModel.width = m.width
	m.verifyModel.height = m.height
	m.screen = ScreenVerify
	m.setWorkflowPhase(types.PhaseVerify)
	m.persistWorkflowState()
	return m, nil
}

// handlePhaseShip handles the Ship phase completion within PhaseResultMsg.
func handlePhaseShip(m *AppState, msg PhaseResultMsg) (tea.Model, tea.Cmd) {
	t := m.themeManager.Current()

	// Auto-backup: snapshot the session directory before resetting
	// workflow state so the user always has a restore point.
	// BUG-02 fix: backupCurrentSession no longer mutates AppState fields directly.
	// Instead, we capture the backup path and emit a ToastMsg via tea.Cmd.
	var backupCmd tea.Cmd
	backupPath := m.backupCurrentSessionAsync()
	if backupPath != "" {
		backupCmd = func() tea.Msg {
			return ToastMsg{
				Text:     fmt.Sprintf("Auto-backup saved: %s", backupPath),
				Duration: 4 * time.Second,
				Type:     "info",
			}
		}
	}

	summary := ShipSummary{
		SessionID: m.workflowEngine.SessionID(),
	}
	if m.activeModel != nil {
		summary.Model = m.activeModel.ID
	}
	summary.Provider = m.activeProvider
	if m.executeModel != nil {
		done, total, failed, skipped := 0, len(msg.Tasks), 0, 0
		for _, task := range msg.Tasks {
			switch task.Status {
			case types.StatusDone:
				done++
			case types.StatusFailed:
				failed++
			case types.StatusSkipped:
				skipped++
			}
		}
		summary.TaskDone = done
		summary.TaskTotal = total
		summary.TaskFailed = failed
		summary.TaskSkipped = skipped
	}
	summary.Commits = msg.Commits
	summary.FilesAdded = msg.DiffStats.FilesAdded
	summary.FilesModified = msg.DiffStats.FilesModified
	summary.FilesDeleted = msg.DiffStats.FilesDeleted
	summary.Insertions = msg.DiffStats.Insertions
	summary.Deletions = msg.DiffStats.Deletions
	summary.Duration = formatDurationMs(msg.DurationMs)
	if msg.Usage != nil {
		summary.TotalTokens = msg.Usage.TotalTokens
	}
	summary.TotalCost = msg.Cost

	m.shipModel = NewShipModel(summary, t, m.width, m.height)
	m.screen = ScreenShip
	m.setWorkflowPhase(types.PhaseIdle)
	m.flushPendingStreamChunks()
	m.persistWorkflowState()
	if m.sessionManager != nil && m.sessionID != "" {
		if err := m.sessionManager.UpdateWorkflowState(
			m.sessionID, "", types.PhaseIdle, nil,
		); err != nil {
			slog.Warn("failed to reset workflow state after ship", "err", err)
		}
	}
	m.workflowGoal = ""
	if backupCmd != nil {
		return m, backupCmd
	}
	return m, nil
}

// handlePermissionRequest handles PermissionRequestMsg by showing the permission modal.
func (m *AppState) handlePermissionRequest(msg PermissionRequestMsg) (tea.Model, tea.Cmd) {
	if m.permissionModalActive {
		m.pendingPermissionRequests = append(m.pendingPermissionRequests, msg)
		return m, permissionListenerCmd(m.dispatcher)
	}
	m.permissionModalActive = true
	m.pendingPermissionRequestID = msg.Request.ID // RC-2: store request ID for response correlation
	m.prevScreen = m.screen
	m.screen = ScreenPermission
	t := m.themeManager.Current()
	timeout := time.Duration(msg.Request.TimeoutSecs) * time.Second
	if timeout <= 0 {
		timeout = time.Duration(types.DefaultPermissionTimeout) * time.Second
	}
	pm := components.NewPermissionModal(msg.Request, t, timeout)
	m.permissionModal = pm
	return m, tea.Batch(
		permissionListenerCmd(m.dispatcher),
		questionListenerCmd(m.dispatcher),
		tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
			return PermissionTickMsg{}
		}),
	)
}

// handlePermissionResponse handles PermissionResponseMsg by approving/denying the permission.
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) (tea.Model, tea.Cmd) {
	m.dispatcher.ApprovePermission(m.pendingPermissionRequestID, msg.Response.Allowed, msg.Response.Remember)
	m.screen = m.prevScreen
	m.permissionModal = nil
	m.permissionModalActive = false
	m.pendingPermissionRequestID = 0
	if len(m.pendingPermissionRequests) > 0 {
		next := m.pendingPermissionRequests[0]
		m.pendingPermissionRequests = m.pendingPermissionRequests[1:]
		return m.handlePermissionRequest(next)
	}
	return m, tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
}

// handlePermissionTick handles PermissionTickMsg for the permission modal timeout.
func (m *AppState) handlePermissionTick() (tea.Model, tea.Cmd) {
	if m.screen == ScreenPermission && m.permissionModal != nil {
		m.permissionModal.Tick()
		if m.permissionModal.Remaining() <= 0 {
			resp := m.permissionModal.Deny()
			m.dispatcher.ApprovePermission(m.pendingPermissionRequestID, resp.Allowed, resp.Remember)
			m.screen = m.prevScreen
			m.permissionModal = nil
			m.permissionModalActive = false
			m.pendingPermissionRequestID = 0
			if len(m.pendingPermissionRequests) > 0 {
				next := m.pendingPermissionRequests[0]
				m.pendingPermissionRequests = m.pendingPermissionRequests[1:]
				return m.handlePermissionRequest(next)
			}
			return m, tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		}
		return m, tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
			return PermissionTickMsg{}
		})
	}
	return m, nil
}

// handleQuestionRequest handles QuestionRequestMsg by showing the question in REPL.
func (m *AppState) handleQuestionRequest(msg QuestionRequestMsg) (tea.Model, tea.Cmd) {
	if m.replModel != nil {
		m.replModel.ShowQuestion(msg)
	}
	return m, questionListenerCmd(m.dispatcher)
}

// handleQuestionResponse handles QuestionResponseMsg from the user.
func (m *AppState) handleQuestionResponse(msg QuestionResponseMsg) (tea.Model, tea.Cmd) {
	if m.pendingDiscussAnswers != nil && m.workflowEngine != nil {
		idx := m.currentDiscussIndex
		if err := m.workflowEngine.SubmitDiscussAnswer(idx, msg.Answer); err != nil {
			slog.Warn("SubmitDiscussAnswer failed", "idx", idx, "err", err)
		} else {
			m.pendingDiscussAnswers[idx] = msg.Answer
		}
		m.currentDiscussIndex++
		if m.currentDiscussIndex >= m.discussQuestionCount {
			return m, m.finalizeDiscussAndAdvance()
		}
		return m, m.askNextDiscussQuestion()
	}
	if m.dispatcher == nil {
		return m, nil
	}
	dresp := tools.QuestionResponse{Answer: msg.Answer}
	select {
	case m.dispatcher.QuestionResponseCh() <- dresp:
	default:
	}
	return m, questionListenerCmd(m.dispatcher)
}

// handleDiscussAnswerTimeout handles DiscussAnswerTimeoutMsg for the 5-minute timeout.
func (m *AppState) handleDiscussAnswerTimeout() (tea.Model, tea.Cmd) {
	if m.pendingDiscussAnswers == nil {
		return m, nil
	}
	m.currentOperation = fmt.Sprintf("Discuss timeout on Q%d", m.currentDiscussIndex+1)
	return m, m.skipDiscussAndAdvance()
}

// handleAppMsg handles AppMsg for workflow phase routing.
func (m *AppState) handleAppMsg(msg AppMsg) (tea.Model, tea.Cmd) {
	switch msg.Screen {
	case ScreenExecute:
		if m.workflowEngine == nil {
			return m, nil
		}
		m.setWorkflowPhase(types.PhaseExecute)
		return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)
	case ScreenVerify:
		if m.workflowEngine == nil {
			return m, nil
		}
		m.setWorkflowPhase(types.PhaseVerify)
		return m, RunPhaseCmd(m, types.PhaseVerify, m.workflowGoal)
	case ScreenShip:
		if m.workflowEngine == nil {
			return m, nil
		}
		m.setWorkflowPhase(types.PhaseShip)
		return m, RunPhaseCmd(m, types.PhaseShip, m.workflowGoal)
	}

	if msg.ModelSelected != nil {
		if m.registry != nil {
			_ = m.registry.SetActive(msg.ModelSelected.Provider)
		}
		m.activeProvider = msg.ModelSelected.Provider
		m.activeModel = &msg.ModelSelected.Model
		// Sync model to workflow engine
		if m.workflowEngine != nil && m.registry != nil {
			if p := m.registry.ActiveProvider(); p != nil {
				m.workflowEngine.SetModel(m.activeModel.ID, p)
			}
		}
		// Sync model to REPL
		if m.replModel != nil {
			m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
		}
		m.headerCacheValid = false
		m.screen = m.prevScreen
		return m, nil
	}

	var providerCmd tea.Cmd
	if msg.Screen == ScreenREPL && m.replModel == nil {
		rp := NewReplModel(m.themeManager.Current(), m.version)
		m.replModel = &rp
		m.initialized = true
		if m.sidebarModel == nil {
			m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
		}
		sessionID := ""
		if m.sessionManager != nil && m.activeModel != nil && m.activeProvider != "" {
			s, err := m.sessionManager.NewSession(m.activeModel.ID, m.activeProvider)
			if err == nil {
				sessionID = s.ID
				m.sessionID = sessionID
				m.dispatcher.SetSessionID(s.ID)
			}
		}
		providerCmd = m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
		m.replModel.SetDispatcher(m.dispatcher)
		m.replModel.SetCommandRegistry(m.cmdRegistry)
		if m.width > 0 && m.height > 0 {
			m.replModel.Update(tea.WindowSizeMsg{Width: m.width, Height: m.height})
		}
	}
	if msg.Screen == ScreenModelSelector {
		m.prevScreen = m.screen
		m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
		m.screen = ScreenModelSelector
		return m, m.modelSelector.Init()
	}
	m.screen = msg.Screen
	return m, providerCmd
}

// handleFallbackEvent handles FallbackEventMsg for provider auto-fallback.
func (m *AppState) handleFallbackEvent(msg FallbackEventMsg) (tea.Model, tea.Cmd) {
	m.activeProvider = msg.To
	m.fallbackNotification = &FallbackNotification{
		Event:     msg,
		Dismissed: false,
		ShownAt:   time.Now(),
	}
	return m, nil
}

// handleSettingsSaved handles SettingsSavedMsg when settings are saved.
func (m *AppState) handleSettingsSaved() (tea.Model, tea.Cmd) {
	m.currentOperation = "Settings saved"
	if m.config != nil && m.settingsModel != nil {
		m.settingsModel.SetConfig(m.config)
	}

	var settingsCmd tea.Cmd
	if m.config != nil && m.registry != nil {
		cfgProvider := m.config.Provider.Default
		if cfgProvider != "" && m.activeProvider != cfgProvider {
			if err := m.registry.SetActive(cfgProvider); err == nil {
				m.activeProvider = cfgProvider
				if m.replModel != nil {
					settingsCmd = m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.sessionID, m.config)
					m.replModel.SetDispatcher(m.dispatcher)
				}
				if m.workflowEngine != nil && m.activeModel != nil {
					if p := m.registry.ActiveProvider(); p != nil {
						m.workflowEngine.SetModel(m.activeModel.ID, p)
					}
				}
			}
		}

		cfgModel := m.config.Model.Default
		if cfgModel != "" && (m.activeModel == nil || m.activeModel.ID != cfgModel) {
			if p := m.registry.ActiveProvider(); p != nil {
				ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
				if models, err := p.FetchModels(ctx); err == nil {
					for _, mi := range models {
						if mi.ID == cfgModel {
							m.activeModel = &mi
							m.headerCacheValid = false
							if m.replModel != nil {
								settingsCmd = m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.sessionID, m.config)
								m.replModel.SetDispatcher(m.dispatcher)
							}
							if m.workflowEngine != nil {
								m.workflowEngine.SetModel(mi.ID, p)
							}
							break
						}
					}
				}
				cancel()
			}
		}
	}

	if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
		m.lastActivity = time.Now()
	}
	return m, tea.Batch(
		NextHealthTick(types.HealthCheckInterval),
		NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
		settingsCmd,
	)
}

// handleThemeChanged handles ThemeChangedMsg for dark/light theme switching.
func (m *AppState) handleThemeChanged(msg ThemeChangedMsg) (tea.Model, tea.Cmd) {
	switch msg.Theme {
	case "dark":
		m.themeManager = theme.NewManager(theme.ModeDark)
	case "light":
		m.themeManager = theme.NewManager(theme.ModeLight)
	case "auto":
		// Detect terminal background luminance and select appropriate mode
		if lipgloss.HasDarkBackground() {
			m.themeManager = theme.NewManager(theme.ModeDark)
		} else {
			m.themeManager = theme.NewManager(theme.ModeLight)
		}
	}
	t := m.themeManager.Current()
	if m.replModel != nil {
		m.replModel.SetTheme(t)
	}
	m.modelSelector.SetTheme(t)
	if m.sidebarModel != nil {
		m.sidebarModel.SetTheme(t)
	}
	if m.settingsModel != nil {
		m.settingsModel.SetTheme(t)
	}
	if m.planModel != nil {
		m.planModel.theme = t
	}
	if m.executeModel != nil {
		m.executeModel.theme = t
	}
	if m.verifyModel != nil {
		m.verifyModel.theme = t
	}
	if m.shipModel != nil {
		m.shipModel.theme = t
	}
	return m, nil
}
