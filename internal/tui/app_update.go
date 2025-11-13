package tui

import (
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)


// Update implements tea.Model. It is the single dispatch point for all messages.
// CRITICAL: Never mutate AppState from a goroutine. All mutations go here.
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	// ── Window resize ──────────────────────────────────────────────────────────
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		cmds = append(cmds, m.handleWindowResize(msg))

	// ── Keyboard ───────────────────────────────────────────────────────────────
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
			if m.streamCancelFn != nil {
				m.streamCancelFn()
				m.streamCancelFn = nil
			}
			return m, nil
		default:
			cmds = append(cmds, m.routeKeyMsg(msg))
		}

	// ── Screen routing ─────────────────────────────────────────────────────────
	case AppMsg:
		cmds = append(cmds, m.handleAppMsg(msg))

	// ── Key action ────────────────────────────────────────────────────────────
	case KeyActionMsg:
		cmds = append(cmds, m.handleKeyAction(msg.Action))

	// ── Leader timeout ────────────────────────────────────────────────────────
	case LeaderTimeoutMsg:
		if m.keyRegistry != nil {
			m.keyRegistry.DeactivateLeader()
		}

	// ── Slash command ─────────────────────────────────────────────────────────
	case SlashCommandMsg:
		cmds = append(cmds, m.handleSlashCommand(msg.Command))

	// ── Streaming ─────────────────────────────────────────────────────────────
	case StreamMsg:
		if m.replModel != nil {
			cs, _ := m.replModel.handleStreamMsg(msg)
			cmds = append(cmds, cs...)
		}
	case StreamDoneMsg:
		if m.replModel != nil {
			cs, _ := m.replModel.handleStreamDoneMsg(msg)
			cmds = append(cmds, cs...)
		}
	case StreamErrorMsg:
		if m.replModel != nil {
			cs, _ := m.replModel.handleStreamErrorMsg(msg)
			cmds = append(cmds, cs...)
		}
	case TickMsg:
		if m.replModel != nil && (m.replModel.streaming || m.replModel.thinking) {
			replM, cmd := m.replModel.Update(msg)
			if r, ok := replM.(*ReplModel); ok {
				m.replModel = r
			}
			cmds = append(cmds, cmd)
		}

	// ── Health ────────────────────────────────────────────────────────────────
	case HealthCheckTickMsg:
		if m.activeProvider != "" && m.registry != nil {
			if p, err := m.registry.Get(m.activeProvider); err == nil {
				cmds = append(cmds, HealthCheckCmd(p, 10*time.Second))
			}
		}
		cmds = append(cmds, NextHealthTick(types.HealthCheckInterval))

	case HealthCheckResultMsg:
		m.healthStatus = msg.Result
		m.lastHealth = time.Now()

	// ── Cache refresh ─────────────────────────────────────────────────────────
	case RefreshCacheMsg:
		if m.registry != nil {
			provider := msg.ProviderName
			if provider == "" {
				provider = m.activeProvider
			}
			cmds = append(cmds, CacheRefreshCmd(m.registry, provider))
		}
	case CacheRefreshResultMsg:
		if msg.NextCmd != nil {
			cmds = append(cmds, msg.NextCmd)
		}

	// ── Permission modal ──────────────────────────────────────────────────────
	case PermissionRequestMsg:
		m.permRequest = &msg.Request
		m.permCountdown = msg.Request.TimeoutSecs
		timeout := components.DefaultPermissionTimeout
		if msg.Request.TimeoutSecs > 0 {
			timeout = time.Duration(msg.Request.TimeoutSecs) * time.Second
		}
		m.permModal = components.NewPermissionModal(msg.Request, m.themeManager.Current(), timeout)
		m.screen = ScreenPermission

	case PermissionResponseMsg:
		cmds = append(cmds, m.handlePermissionResponse(msg))

	case PermissionTickMsg:
		cmds = append(cmds, m.handlePermissionTick())

	// ── Question modal ────────────────────────────────────────────────────────
	case QuestionRequestMsg:
		m.questionRequest = &msg
		qModel := components.NewQuestionModel(tools.QuestionRequest{
			Question:    msg.Question,
			Header:      msg.Header,
			Options:     msg.Options,
			AllowCustom: true,
		}, m.themeManager.Current(), m.permModalWidth)
		m.questionModel = &qModel
		m.screen = ScreenPermission // reuse permission overlay

	case QuestionResponseMsg:
		cmds = append(cmds, m.handleQuestionResponse(msg))

	// ── Workflow phase result ─────────────────────────────────────────────────
	case PhaseResultMsg:
		cmds = append(cmds, m.handlePhaseResult(msg))

	case PlanReadyMsg:
		cmds = append(cmds, m.handlePlanReady(msg))

	case ExecutePauseMsg:
		if m.executeModel != nil {
			m.executeModel.paused = msg.Paused
		}

	case HealResultMsg:
		if m.verifyModel != nil {
			m.verifyModel.Update(msg)
		}

	// ── Goal submitted ────────────────────────────────────────────────────────
	case GoalSubmittedMsg:
		m.workflowGoal = msg.Goal
		m.screen = ScreenREPL
		cmds = append(cmds, m.runWorkflowFromGoal(msg.Goal))

	// ── Toast ─────────────────────────────────────────────────────────────────
	case ToastMsg:
		m.toasts = append(m.toasts, Toast{
			Text:      msg.Text,
			Type:      msg.Type,
			CreatedAt: time.Now(),
		})
		// Keep only recent toasts (last 5 for overflow buffer)
		if len(m.toasts) > maxVisibleToasts+2 {
			m.toasts = m.toasts[len(m.toasts)-(maxVisibleToasts+2):]
		}
		duration := msg.Duration
		if duration <= 0 {
			duration = 3 * time.Second
		}
		cmds = append(cmds, tea.Tick(duration, func(time.Time) tea.Msg {
			return ToastExpiryMsg{}
		}))

	case ToastExpiryMsg:
		// Remove oldest toast
		if len(m.toasts) > 0 {
			m.toasts = m.toasts[1:]
		}

	// ── Settings saved ────────────────────────────────────────────────────────
	case SettingsSavedMsg:
		// Optionally reload config here
		m.screen = ScreenREPL

	// ── Theme changed ─────────────────────────────────────────────────────────
	case ThemeChangedMsg:
		switch msg.Theme {
		case "dark":
			m.themeManager = theme.NewManager(theme.ModeDark)
		case "light":
			m.themeManager = theme.NewManager(theme.ModeLight)
		case "auto":
			m.themeManager = theme.NewManager(theme.ModeAuto)
		}
		t := m.themeManager.Current()
		if m.replModel != nil {
			m.replModel.SetTheme(t)
		}
		if m.sidebarModel != nil {
			m.sidebarModel.SetTheme(t)
		}
		if m.cmdPalette != nil {
			m.cmdPalette.SetTheme(t)
		}
		if m.settingsModel != nil {
			m.settingsModel.SetTheme(t)
		}

	// ── Fallback event ────────────────────────────────────────────────────────
	case FallbackEventMsg:
		m.activeProvider = msg.To
		slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)

	// ── Model selected ────────────────────────────────────────────────────────
	case ModelSelectedMsg:
		m.activeModel = &msg.Model
		m.activeProvider = msg.Provider
		if m.replModel != nil {
			providerCmd := m.replModel.SetProvider(m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
			cmds = append(cmds, providerCmd)
		}
		m.screen = ScreenREPL

	// ── Sidebar refresh ───────────────────────────────────────────────────────
	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			m.sidebarModel.Update(msg)
		}
		// Propagate git branch to REPL for status bar display
		if m.replModel != nil && msg.Branch != "" {
			m.replModel.sidebarBranch = msg.Branch
		}

	// ── Diff screen ───────────────────────────────────────────────────────────
	case DiffScreenMsg:
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
		}
		m.diffModel.SetDiff(msg.Diff)
		m.diffModel.width = m.width
		m.diffModel.height = m.height
		m.screen = ScreenDiff

	case DiffCloseMsg:
		m.screen = ScreenREPL

	// ── Error ─────────────────────────────────────────────────────────────────
	case ErrorMsg:
		if m.replModel != nil {
			m.replModel.AddMessage(makeAssistantMsg("Error: " + msg.Err.Error()))
		}

	// ── ProviderModelsFetched ─────────────────────────────────────────────────
	case ProviderModelsFetchedMsg:
		if m.replModel != nil {
			m.replModel.handleProviderModelsFetched(msg)
		}

	// ── ThinkingBlockToggle ───────────────────────────────────────────────────
	case ThinkingBlockToggleMsg:
		if m.replModel != nil {
			m.replModel.handleThinkingToggle(msg)
		}

	// ── Model/command palette sub-model forwarding ────────────────────────────
	default:
		switch m.screen {
		case ScreenModelSelector:
			if m.msModel != nil {
				newMs, cmd := m.msModel.Update(msg)
				if nm, ok := newMs.(*ModelSelector); ok {
					m.msModel = nm
				}
				cmds = append(cmds, cmd)
			}
		case ScreenResume:
			if m.resumeModel != nil {
				newResume, cmd := m.resumeModel.Update(msg)
				if nr, ok := newResume.(*ResumeModel); ok {
					m.resumeModel = nr
				}
				cmds = append(cmds, cmd)
			}
		case ScreenDiscuss:
			if m.discussModel != nil {
				newDiscuss, cmd := m.discussModel.Update(msg)
				if nd, ok := newDiscuss.(*DiscussModel); ok {
					m.discussModel = nd
				}
				cmds = append(cmds, cmd)
			}
		}
	}

	return m, tea.Batch(cmds...)
}

// ─── Routing helpers ──────────────────────────────────────────────────────────

// routeToScreen returns the initialization cmd for the current screen.
func (m *AppState) routeToScreen() tea.Cmd {
	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel == nil {
			fm := NewFirstRunModel(m.themeManager.Current())
			m.firstRunModel = fm
		}
		return m.firstRunModel.Init()
	case ScreenREPL:
		m.ensureReplModel()
		if m.replModel.width == 0 {
			m.replModel.width = m.width
			m.replModel.height = m.height
		}
		return m.replModel.Init()
	default:
		return nil
	}
}

// handleWindowResize resizes all sub-models.
func (m *AppState) handleWindowResize(msg tea.WindowSizeMsg) tea.Cmd {
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
		sw = m.sidebarModel.GetWidth()
	}

	if m.replModel != nil {
		m.replModel.width = msg.Width
		m.replModel.height = msg.Height
		m.replModel.SetSidebarWidth(sw)
		replM, cmd := m.replModel.Update(msg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		return cmd
	}

	if m.planModel != nil {
		m.planModel.SetDimensions(msg.Width, msg.Height)
	}
	if m.executeModel != nil {
		m.executeModel.width = msg.Width
		m.executeModel.height = msg.Height
	}
	if m.settingsModel != nil {
		m.settingsModel.width = msg.Width
		m.settingsModel.height = msg.Height
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetDimensions(msg.Width, msg.Height)
	}
	if m.msModel != nil {
		m.msModel.SetDimensions(msg.Width, msg.Height)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetDimensions(msg.Width, msg.Height)
	}
	if m.diffModel != nil {
		m.diffModel.width = msg.Width
		m.diffModel.height = msg.Height
	}
	return nil
}

// routeKeyMsg routes key events to the active screen.
func (m *AppState) routeKeyMsg(msg tea.KeyMsg) tea.Cmd {
	// Command palette has priority
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		newPalette, cmd := m.cmdPalette.Update(msg)
		m.cmdPalette = newPalette
		return cmd
	}

	// Global leader key
	if m.keyRegistry != nil {
		handled, cmd := m.keyRegistry.Handle(msg.String(), CtxGlobal)
		if handled {
			return cmd
		}
	}

	switch m.screen {
	case ScreenREPL:
		if m.replModel != nil {
			newRepl, cmd := m.replModel.Update(msg)
			if r, ok := newRepl.(*ReplModel); ok {
				m.replModel = r
			}
			return cmd
		}
	case ScreenPermission:
		return m.handlePermissionKey(msg)
	case ScreenModelSelector:
		if m.msModel != nil {
			newMs, cmd := m.msModel.Update(msg)
			if nm, ok := newMs.(*ModelSelector); ok {
				m.msModel = nm
			}
			return cmd
		}
	case ScreenSettings:
		if m.settingsModel != nil {
			newSettings, cmd := m.settingsModel.Update(msg)
			m.settingsModel = newSettings
			return cmd
		}
	case ScreenResume:
		if m.resumeModel != nil {
			newResume, cmd := m.resumeModel.Update(msg)
			if nr, ok := newResume.(*ResumeModel); ok {
				m.resumeModel = nr
			}
			return cmd
		}
	case ScreenPlan:
		if m.planModel != nil {
			newPlan, cmd := m.planModel.Update(msg)
			m.planModel = newPlan
			return cmd
		}
	case ScreenExecute:
		if m.executeModel != nil {
			newExec, cmd := m.executeModel.Update(msg)
			m.executeModel = newExec
			return cmd
		}
	case ScreenVerify:
		if m.verifyModel != nil {
			newVerify, cmd := m.verifyModel.Update(msg)
			m.verifyModel = newVerify
			return cmd
		}
	case ScreenShip:
		if m.shipModel != nil {
			newShip, cmd := m.shipModel.Update(msg)
			m.shipModel = newShip
			return cmd
		}
	case ScreenDiscuss:
		if m.discussModel != nil {
			newDiscuss, cmd := m.discussModel.Update(msg)
			if nd, ok := newDiscuss.(*DiscussModel); ok {
				m.discussModel = nd
			}
			return cmd
		}
	case ScreenDiff:
		if m.diffModel != nil {
			newDiff, cmd := m.diffModel.Update(msg)
			if nd, ok := newDiff.(*DiffModel); ok {
				m.diffModel = nd
			}
			return cmd
		}
	case ScreenGoalInput:
		if m.goalInput != nil {
			newGoal, cmd := m.goalInput.Update(msg)
			if ng, ok := newGoal.(*GoalInputModel); ok {
				m.goalInput = ng
			}
			return cmd
		}
	case ScreenFirstRun:
		if m.firstRunModel != nil {
			newFR, cmd := m.firstRunModel.Update(msg)
			if nfr, ok := newFR.(*FirstRunModel); ok {
				m.firstRunModel = nfr
			}
			return cmd
		}
	case ScreenLedger:
		if m.ledgerModel != nil {
			newLedger, cmd := m.ledgerModel.Update(msg)
			if nl, ok := newLedger.(*LedgerModel); ok {
				m.ledgerModel = nl
			}
			return cmd
		}
	case ScreenRollback:
		if m.rollbackModel != nil {
			newRB, cmd := m.rollbackModel.Update(msg)
			if nr, ok := newRB.(*RollbackModel); ok {
				m.rollbackModel = nr
			}
			return cmd
		}
	}
	return nil
}

// handleAppMsg handles AppMsg screen transitions.
func (m *AppState) handleAppMsg(msg AppMsg) tea.Cmd {
	if msg.ModelSelected != nil {
		m.activeModel = &msg.ModelSelected.Model
		m.activeProvider = msg.ModelSelected.Provider
		m.screen = ScreenREPL
		return nil
	}

	if msg.Screen != 0 || msg.Action != "" {
		return m.routeAppMsgAction(msg)
	}

	if msg.SessionID != "" {
		return m.loadAndRestoreSession(msg.SessionID, true)
	}

	return nil
}

// routeAppMsgAction handles action-based screen routing.
func (m *AppState) routeAppMsgAction(msg AppMsg) tea.Cmd {
	switch msg.Action {
	case "new_session":
		return m.startNewSession()
	case "session_list":
		return m.openResumeScreen()
	case "open_settings":
		return m.openSettingsScreen()
	case "open_palette":
		if m.cmdPalette == nil {
			m.cmdPalette = NewCommandPalette(m.cmdRegistry, m.themeManager.Current())
			m.cmdPalette.SetDimensions(m.width, m.height)
		}
		m.cmdPalette.Open()
		return nil
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
		}
		return nil
	case "toggle_theme":
		newMode := m.themeManager.Cycle()
		themeName := "dark"
		switch newMode {
		case theme.ModeLight:
			themeName = "light"
		case theme.ModeAuto:
			themeName = "auto"
		}
		return func() tea.Msg {
			return ThemeChangedMsg{Theme: themeName}
		}
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	}

	// Screen-based routing
	return m.navigateToScreen(msg.Screen)
}

// navigateToScreen transitions to the given screen.
func (m *AppState) navigateToScreen(screen Screen) tea.Cmd {
	m.prevScreen = m.screen
	m.screen = screen
	switch screen {
	case ScreenREPL:
		m.ensureReplModel()
		return nil
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
			m.msModel.SetDimensions(m.width, m.height)
		}
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath)
			m.settingsModel.width = m.width
			m.settingsModel.height = m.height
		}
		return nil
	case ScreenResume:
		return m.openResumeScreen()
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
			m.goalInput.width = m.width
			m.goalInput.height = m.height
		}
		return m.goalInput.Init()
	case ScreenLedger:
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			m.ledgerModel.width = m.width
			m.ledgerModel.height = m.height
		}
		return nil
	case ScreenRollback:
		if m.rollbackModel == nil {
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, m.width, m.height)
		}
		return nil
	case ScreenMetrics:
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
			m.metricsModel.width = m.width
			m.metricsModel.height = m.height
		}
		return nil
	default:
		return nil
	}
}

// handleKeyAction processes KeyActionMsg strings.
func (m *AppState) handleKeyAction(action string) tea.Cmd {
	switch action {
	case "open_settings":
		return m.navigateToScreen(ScreenSettings)
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			if m.replModel != nil {
				sw := m.sidebarModel.GetWidth()
				m.replModel.SetSidebarWidth(sw)
			}
		}
		return nil
	case "new_session":
		return m.startNewSession()
	case "session_list":
		return m.openResumeScreen()
	case "open_palette":
		if m.cmdPalette == nil {
			m.cmdPalette = NewCommandPalette(m.cmdRegistry, m.themeManager.Current())
			m.cmdPalette.SetDimensions(m.width, m.height)
		}
		m.cmdPalette.Open()
		return nil
	case "cycle_model", "cycle_model_forward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cycle_model_backward":
		return m.navigateToScreen(ScreenModelSelector)
	case "toggle_theme":
		newMode := m.themeManager.Cycle()
		themeName := "dark"
		switch newMode {
		case theme.ModeLight:
			themeName = "light"
		case theme.ModeAuto:
			themeName = "auto"
		}
		return func() tea.Msg {
			return ThemeChangedMsg{Theme: themeName}
		}
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	}
	return nil
}

// ─── Session helpers ──────────────────────────────────────────────────────────

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
		return nil
	}

	m.sessionID = sess.ID
	m.ensureReplModel()
	m.replModel.ClearMessages()
	m.replModel.SetSessionID(sess.ID)
	m.workflowPhase = types.PhaseIdle
	m.workflowGoal = ""
	m.screen = ScreenREPL

	return m.syncReplProvider(sess.ID)
}

// openResumeScreen loads session list and navigates to the resume screen.
func (m *AppState) openResumeScreen() tea.Cmd {
	return func() tea.Msg {
		var sessions []*session.Session
		if m.sessionManager != nil {
			infos, err := m.sessionManager.ListSessions()
			if err == nil {
				for i, info := range infos {
					if i >= 20 {
						break
					}
					sess, err := m.sessionManager.LoadSession(info.ID)
					if err == nil {
						sessions = append(sessions, sess)
					}
				}
			}
		}
		return resumeScreenReadyMsg{sessions: sessions}
	}
}

// resumeScreenReadyMsg carries loaded sessions for the resume screen.
type resumeScreenReadyMsg struct {
	sessions []*session.Session
}

// openSettingsScreen transitions to the settings screen.
func (m *AppState) openSettingsScreen() tea.Cmd {
	return m.navigateToScreen(ScreenSettings)
}

// runWorkflowFromGoal starts the discuss → plan → execute workflow.
func (m *AppState) runWorkflowFromGoal(goal string) tea.Cmd {
	m.workflowGoal = goal
	cmds := []tea.Cmd{m.initWorkflowEngine()}
	m.workflowPhase = types.PhaseInitialize
	cmds = append(cmds, m.RunPhaseCmd(types.PhaseInitialize))
	return tea.Batch(cmds...)
}

// ─── Permission helpers ───────────────────────────────────────────────────────

// handlePermissionResponse processes the user's permission decision.
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) tea.Cmd {
	if m.dispatcher == nil {
		return nil
	}
	reqID := m.permRequest.ID
	allowed := msg.Response.Allowed
	remember := msg.Response.Remember
	m.permRequest = nil
	m.screen = ScreenREPL
	if m.dispatcher != nil {
		go m.dispatcher.ApprovePermission(reqID, allowed, remember)
	}
	return nil
}

// handlePermissionTick decrements the permission countdown.
func (m *AppState) handlePermissionTick() tea.Cmd {
	if m.permCountdown > 0 {
		m.permCountdown--
		if m.permModal != nil {
			m.permModal.Tick()
		}
		if m.permCountdown == 0 {
			// Auto-deny on timeout
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
		m.screen = ScreenREPL
		return nil
	}
	switch msg.String() {
	case "y", "enter":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
			},
		})
	case "a":
		// Allow always (remember)
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
				Remember:  true,
			},
		})
	case "n", "esc":
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
		m.screen = ScreenREPL
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
	respCh := m.questionRequest.ResponseCh
	m.questionRequest = nil
	m.screen = ScreenREPL

	if respCh != nil {
		go func() {
			respCh <- tools.QuestionResponse{Answer: msg.Answer}
		}()
	}
	return nil
}
