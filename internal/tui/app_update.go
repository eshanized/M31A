package tui

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
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
				m.lastCtrlCTime = time.Time{}
				m.toasts = append(m.toasts, Toast{
					Text:      "Response cancelled",
					Type:      "warning",
					CreatedAt: time.Now(),
				})
				return m, tea.Tick(3*time.Second, func(time.Time) tea.Msg {
					return ToastExpiryMsg{}
				})
			}
			if !m.lastCtrlCTime.IsZero() && time.Since(m.lastCtrlCTime) < 2*time.Second {
				return m, tea.Quit
			}
			m.lastCtrlCTime = time.Now()
			m.toasts = append(m.toasts, Toast{
				Text:      "Press ctrl+c again to exit",
				Type:      "info",
				CreatedAt: time.Now(),
			})
			return m, tea.Tick(3*time.Second, func(time.Time) tea.Msg {
				return ToastExpiryMsg{}
			})
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
			m.checkAutoDream()
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
		// Forward to execute screen for animated progress bar
		if m.screen == ScreenExecute && m.executeModel != nil {
			execM, cmd := m.executeModel.Update(msg)
			m.executeModel = execM
			cmds = append(cmds, cmd)
		}
		// Forward to verify screen for heal spinner animation
		if m.screen == ScreenVerify && m.verifyModel != nil {
			m.verifyModel.TickSpinner()
		}
		// Screen transition tick
		if m.transition != nil && m.transition.Active {
			if m.transition.TransitionTick() {
				// Transition complete — switch to target screen
				// Sub-model was already created in navigateToScreen → ensureSubModel
				m.screen = m.transition.ToScreen
				m.transition = nil
			} else {
				// Keep ticking
				cmds = append(cmds, StreamTickCmd())
			}
		}

	// ── Health ────────────────────────────────────────────────────────────────
	case HealthCheckTickMsg:
		if m.activeProvider != "" && m.registry != nil {
			if p, err := m.registry.Get(m.activeProvider); err == nil {
				cmds = append(cmds, HealthCheckCmd(m.shutdownCtx, p, 10*time.Second))
			}
		}
		cmds = append(cmds, NextHealthTick(m.shutdownCtx, types.HealthCheckInterval))

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
			cmds = append(cmds, CacheRefreshCmd(m.shutdownCtx, m.registry, provider))
		}
	case CacheRefreshResultMsg:
		if msg.NextCmd != nil {
			cmds = append(cmds, msg.NextCmd)
		}

	// ── Permission modal ──────────────────────────────────────────────────────
	case PermissionRequestMsg:
		m.permRequest = &msg.Request
		m.permCountdown = 0
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
			newVerify, cmd := m.verifyModel.Update(msg)
			m.verifyModel = newVerify
			cmds = append(cmds, cmd)
		}

	case workflow.TaskStartMsg:
		if m.executeModel != nil {
			for i, t := range m.executeModel.tasks {
				if t.ID == msg.Task.ID {
					m.executeModel.SetCurrentTask(i)
					m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
					break
				}
			}
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.TaskUpdateMsg:
		if m.executeModel != nil {
			var status types.TaskStatus
			switch msg.Status {
			case "done":
				status = types.StatusDone
			case "failed":
				status = types.StatusFailed
			default:
				status = types.StatusRunning
			}
			m.executeModel.UpdateTaskStatus(msg.Task.ID, status)
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.ToolStartMsg:
		if m.executeModel != nil {
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("→ %s: %s", msg.ToolName, msg.Description),
			})
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.ToolCompleteMsg:
		if m.executeModel != nil {
			status := "ok"
			if !msg.Success {
				status = "failed"
			}
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("  %s %s (%dms)", status, msg.ToolName, msg.DurationMs),
			})
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.SelfHealStartMsg:
		if m.executeModel != nil {
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("⚠ Self-heal attempt %d/%d for task %d", msg.Attempt, msg.Max, msg.TaskID),
			})
		}
		if m.verifyModel != nil {
			m.verifyModel.StartHealing(msg.TaskID, msg.Attempt)
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.SelfHealCompleteMsg:
		if m.executeModel != nil {
			status := "ok"
			if !msg.Success {
				status = "failed"
			}
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("  Self-heal %s (attempt %d/%d)", status, msg.Attempt, msg.Max),
			})
		}
		if m.verifyModel != nil {
			m.verifyModel.StopHealing()
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.PhaseTransitionStartMsg, workflow.PhaseTransitionCompleteMsg,
		workflow.IntermediateProgressMsg,
		workflow.ThinkingStartMsg, workflow.ThinkingCompleteMsg:
		cmds = append(cmds, m.drainEmitterCmd())

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
		// Rebuild the config viewer content so /config reflects changes instantly.
		// Since SettingsModel mutates m.config in-place (shared pointer), we only
		// need to regenerate the rendered content — no need to pass a new cfg pointer.
		if m.configModel != nil {
			m.configModel.buildContent()
		} else {
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, m.width, m.height)
		}
		m.screen = ScreenREPL

	// ── Theme changed ─────────────────────────────────────────────────────────
	case ThemeChangedMsg:
		m.applyTheme(msg.Theme)

	// ── Settings theme change (from settings screen) ────────────────────────
	case settingsThemeChanged:
		m.applyTheme(msg.ThemeName)

	// ── Fallback event ────────────────────────────────────────────────────────
	case FallbackEventMsg:
		m.activeProvider = msg.To
		slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)
		// Re-validate the active model against the new provider
		if m.registry != nil && m.activeModel != nil {
			if p, err := m.registry.Get(msg.To); err == nil && p != nil {
				if info, err := p.GetModel(m.activeModel.ID); err == nil && info != nil {
					m.activeModel = info
				}
			}
		}

	// ── Model selected ────────────────────────────────────────────────────────
	case ModelSelectedMsg:
		m.activeModel = &msg.Model
		m.activeProvider = msg.Provider
		if m.replModel != nil {
			providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
			cmds = append(cmds, providerCmd)
		}
		m.screen = ScreenREPL

	// ── Sidebar refresh ───────────────────────────────────────────────────────
	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			newSidebar, cmd := m.sidebarModel.Update(msg)
			m.sidebarModel = newSidebar
			cmds = append(cmds, cmd)
		}
		// Propagate git branch to REPL for status bar display
		if m.replModel != nil && msg.Branch != "" {
			m.replModel.sidebarBranch = msg.Branch
		}
		// Propagate changed-file count so the welcome screen project card is accurate
		if m.replModel != nil {
			m.replModel.SetChangedFiles(len(msg.Files))
		}

	// ── Diff screen ───────────────────────────────────────────────────────────
	case DiffScreenMsg:
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
		}
		m.diffModel.SetDiff(msg.Diff)
		m.diffModel.SetTitle(msg.Title)
		m.diffModel.width = m.width
		m.diffModel.height = m.height
		if m.sidebarModel != nil {
			m.sidebarModel.Blur()
		}
		m.screen = ScreenDiff

	case DiffCloseMsg:
		m.screen = ScreenREPL

	// ── Session restore ──────────────────────────────────────────────────────
	case sessionRestoredMsg:
		cmds = append(cmds, m.applySessionRestored(msg))

	case resumeScreenReadyMsg:
		m.sessionList = nil
		if m.resumeModel == nil {
			rm := NewResumeModel(msg.sessions, m.themeManager.Current())
			m.resumeModel = rm
		} else {
			m.resumeModel.Refresh(msg.sessions)
		}
		m.screen = ScreenResume

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
		case ScreenSettings:
			if m.settingsModel != nil {
				newSettings, cmd := m.settingsModel.Update(msg)
				m.settingsModel = newSettings
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
			fm := NewFirstRunModel(m.themeManager.Current(), m.registry)
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

	var cmd tea.Cmd
	if m.replModel != nil {
		m.replModel.width = msg.Width
		m.replModel.height = msg.Height
		m.replModel.SetSidebarWidth(sw)
		replM, replCmd := m.replModel.Update(msg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		cmd = replCmd
	}

	if m.planModel != nil {
		m.planModel.SetDimensions(msg.Width, msg.Height)
	}
	if m.executeModel != nil {
		m.executeModel.width = msg.Width
		m.executeModel.height = msg.Height
	}
	if m.verifyModel != nil {
		m.verifyModel.width = msg.Width
		m.verifyModel.height = msg.Height
	}
	if m.metricsModel != nil {
		m.metricsModel.width = msg.Width
		m.metricsModel.height = msg.Height
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
	return cmd
}

// routeKeyMsg routes key events to the active screen.
func (m *AppState) routeKeyMsg(msg tea.KeyMsg) tea.Cmd {
	// Command palette has priority
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		newPalette, cmd := m.cmdPalette.Update(msg)
		m.cmdPalette = newPalette
		return cmd
	}

	// Confirmation dialog active — intercept y/n/esc
	if m.pendingConfirm != nil {
		switch msg.String() {
		case "y", "Y":
			pending := *m.pendingConfirm
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			pending.ConfirmRequired = false
			return m.processCommandResult(pending)
		case "n", "N", "esc":
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Cancelled."))
			}
			return nil
		}
		return nil
	}

	// Sidebar focus toggle — works from any screen
	if msg.String() == "ctrl+g" && m.sidebarModel != nil && m.sidebarModel.IsVisible() {
		m.sidebarModel.ToggleFocus()
		return nil
	}

	// When sidebar is focused AND we're on the REPL, route keys to sidebar
	if m.screen == ScreenREPL && m.sidebarModel != nil && m.sidebarModel.IsFocused() {
		return m.sidebarModel.HandleKey(msg)
	}

	// Global leader key — only intercept leader ACTIVATION here.
	// When leader is already active, let the key fall through to the
	// screen-specific handler so REPL chords (ctrl+x b, etc.) work.
	if m.keyRegistry != nil && !m.keyRegistry.IsLeaderActive() {
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
	case ScreenConfig:
		if m.configModel != nil {
			newCfg, cmd := m.configModel.Update(msg)
			m.configModel = newCfg
			return cmd
		}
	case ScreenMetrics:
		switch msg.String() {
		case "esc", "q":
			m.screen = ScreenREPL
			return nil
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
			if m.replModel != nil {
				sw := m.sidebarModel.GetWidth()
				m.replModel.SetSidebarWidth(sw)
			}
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
		m.applyTheme(themeName)
		return nil
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

	// Start a brief transition overlay if this is a real screen change.
	// Skip for overlays, first-run, and screens with async init.
	skipTransition := screen == ScreenPermission || screen == ScreenDiff ||
		screen == ScreenFirstRun || screen == ScreenResume ||
		screen == ScreenConfig
	if m.screen != screen && !skipTransition {
		// Eagerly ensure sub-model exists so it's ready when transition completes.
		m.ensureSubModel(screen)
		m.StartTransition(screen, "")
		return StreamTickCmd()
	}

	m.screen = screen
	return m.ensureSubModel(screen)
}

// ensureSubModel creates or resizes the sub-model for the given screen.
func (m *AppState) ensureSubModel(screen Screen) tea.Cmd {
	switch screen {
	case ScreenREPL:
		m.ensureReplModel()
		return nil
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
		}
		m.msModel.SetDimensions(m.width, m.height)
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath)
			m.settingsModel.width = m.width
			m.settingsModel.height = m.height
		}
		return m.settingsModel.Init()
	case ScreenResume:
		// Resume screen loads sessions async; use the existing command.
		return m.openResumeScreen()
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
			m.goalInput.width = m.width
			m.goalInput.height = m.height
		}
		return m.goalInput.Init()
	case ScreenPlan:
		if m.planModel != nil {
			m.planModel.SetDimensions(m.width, m.height)
		}
		return nil
	case ScreenExecute:
		if m.executeModel != nil {
			m.executeModel.width = m.width
			m.executeModel.height = m.height
		}
		return nil
	case ScreenVerify:
		if m.verifyModel != nil {
			m.verifyModel.width = m.width
			m.verifyModel.height = m.height
		}
		return nil
	case ScreenShip:
		if m.shipModel != nil {
			m.shipModel.width = m.width
			m.shipModel.height = m.height
		}
		return nil
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
		if m.sessionManager != nil {
			m.metricsModel.LoadStats(m.sessionManager)
		}
		return nil
	case ScreenConfig:
		if m.configModel == nil {
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, m.width, m.height)
		} else {
			m.configModel.width = m.width
			m.configModel.height = m.height
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
	case "sidebar_wider":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.IncreaseWidth()
			if m.replModel != nil {
				sw := m.sidebarModel.GetWidth()
				m.replModel.SetSidebarWidth(sw)
			}
		}
		return nil
	case "sidebar_narrower":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.DecreaseWidth()
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
		m.applyTheme(themeName)
		return nil
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	}
	return nil
}

// ─── Theme helpers ────────────────────────────────────────────────────────────

// applyTheme switches the theme and propagates it to all sub-models.
func (m *AppState) applyTheme(themeName string) {
	switch themeName {
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
	if m.discussModel != nil {
		m.discussModel.SetTheme(t)
	}
	if m.metricsModel != nil {
		m.metricsModel.SetTheme(t)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetTheme(t)
	}
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
	m.propagateSessionID(sess.ID)
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
		var sessions []session.SessionInfo
		if m.sessionManager != nil {
			infos, err := m.sessionManager.ListSessions()
			if err == nil {
				for i, info := range infos {
					if i >= 20 {
						break
					}
					sessions = append(sessions, info)
				}
			}
		}
		return resumeScreenReadyMsg{sessions: sessions}
	}
}

// resumeScreenReadyMsg carries loaded sessions for the resume screen.
type resumeScreenReadyMsg struct {
	sessions []session.SessionInfo
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
	m.dispatcher.ApprovePermission(reqID, allowed, remember)
	return permListenerCmd(m.shutdownCtx, m.dispatcher)
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
	return questionListenerCmd(m.shutdownCtx, m.dispatcher)
}
