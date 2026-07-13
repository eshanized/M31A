package tui

// Input routing, key action handling, and theme application for AppState.

import (
	"log/slog"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// handleWindowResize resizes all sub-models using content dimensions
// (terminal minus unified chrome and sidebar).
func (m *AppState) handleWindowResize(msg tea.WindowSizeMsg) tea.Cmd {
	// Compute content dimensions: terminal minus unified chrome (header+footer)
	contentW := msg.Width
	contentH := msg.Height - layout.ChromeHeight
	if contentH < 1 {
		contentH = 1
	}

	// Subtract sidebar width when visible at full breakpoint
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(msg.Width) {
		sw = m.sidebarModel.GetWidth()
		contentW -= sw
	}
	if contentW < 1 {
		contentW = 1
	}

	// Build a content-sized WindowSizeMsg for sub-models
	contentMsg := tea.WindowSizeMsg{Width: contentW, Height: contentH}

	var cmd tea.Cmd
	if m.replModel != nil {
		m.replModel.width = contentW
		m.replModel.height = contentH
		// contentW already accounts for the sidebar; pass 0 to avoid double-subtracting.
		m.replModel.SetSidebarWidth(0)
		replM, replCmd := m.replModel.Update(contentMsg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		cmd = replCmd
	}

	if m.planModel != nil {
		m.planModel.SetDimensions(contentW, contentH)
	}
	if m.executeModel != nil {
		m.executeModel.width = contentW
		m.executeModel.height = contentH
	}
	if m.verifyModel != nil {
		m.verifyModel.width = contentW
		m.verifyModel.height = contentH
	}
	if m.metricsModel != nil {
		m.metricsModel.width = contentW
		m.metricsModel.height = contentH
	}
	if m.settingsModel != nil {
		m.settingsModel.width = contentW
		m.settingsModel.height = contentH
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetDimensions(contentW, contentH)
	}
	if m.msModel != nil {
		m.msModel.SetDimensions(contentW, contentH)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetDimensions(contentW, contentH)
	}
	if m.diffModel != nil {
		m.diffModel.SetDimensions(contentW, contentH)
	}
	if m.goalInput != nil {
		m.goalInput.SetDimensions(contentW, contentH)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetDimensions(contentW, contentH)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetDimensions(contentW, contentH)
	}
	if m.shipModel != nil {
		m.shipModel.width = contentW
		m.shipModel.height = contentH
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetDimensions(contentW, contentH)
	}
	if m.configModel != nil {
		m.configModel.width = contentW
		m.configModel.height = contentH
	}
	if m.discussModel != nil {
		m.discussModel.SetDimensions(contentW, contentH)
	}
	if m.helpModel != nil {
		m.helpModel.SetDimensions(contentW, contentH)
	}
	if m.bisectModel != nil {
		m.bisectModel.SetDimensions(contentW, contentH)
	}
	if m.notifModel != nil {
		m.notifModel.SetDimensions(contentW, contentH)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetDimensions(contentW, contentH)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetDimensions(contentW, contentH)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetDimensions(contentW, contentH)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetDimensions(contentW, contentH)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetDimensions(contentW, contentH)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetDimensions(contentW, contentH)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetDimensions(contentW, contentH)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetDimensions(contentW, contentH)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetDimensions(contentW, contentH)
	}
	if m.homeModel != nil {
		m.homeModel.SetDimensions(contentW, contentH)
	}

	// UX-38: Notify when sidebar auto-hides due to narrow terminal
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && msg.Width < WidthFull {
		if !m.sidebarAutoHideNotified {
			m.addToast("Sidebar hidden — resize wider or press ctrl+b to toggle", "info")
			m.sidebarAutoHideNotified = true
		}
	} else if msg.Width >= WidthFull {
		m.sidebarAutoHideNotified = false
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

	// Intent classification confirmation active — intercept y/n
	if m.pendingIntent != nil {
		switch msg.String() {
		case "y", "Y":
			intent := m.pendingIntent
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			mode := types.WorkflowModeForIntent(*intent)
			m.workflowMode = mode
			if m.workflowEngine != nil {
				m.workflowEngine.SetWorkflowMode(mode)
			}
			return m.runWorkflowFromGoal(input)
		case "n", "N", "esc":
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Proceeding in chat mode."))
			}
			p := m.registry.ActiveProvider()
			if p != nil && m.agentMode {
				return m.startAgentLoop(p, input)
			}
			if p != nil {
				return m.sendPlainTextChat(p, input)
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

	// Leader key must never be swallowed by sidebar or screen handlers.
	// Activate leader mode or complete a chord before routing to sidebar.
	if m.keyRegistry != nil {
		if !m.keyRegistry.IsLeaderActive() {
			// First press: check if this is the leader key itself
			if msg.String() == m.keyRegistry.LeaderKey() {
				handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
				if handled {
					return cmd
				}
			}
		} else {
			// Leader active: complete chord regardless of focused screen.
			// Use CtxREPL so Handle searches CtxREPL then CtxGlobal,
			// catching all registered chords.
			handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
			if handled {
				return cmd
			}
		}
	}

	// When sidebar is focused, route keys to sidebar regardless of active screen.
	// ctrl+g can toggle sidebar focus from any screen, so keys must reach it.
	if m.sidebarModel != nil && m.sidebarModel.IsFocused() {
		return m.sidebarModel.HandleKey(msg)
	}

	return m.routeKeyToScreen(msg)
}

// handleKeyAction processes KeyActionMsg strings.
func (m *AppState) handleKeyAction(action string) tea.Cmd {
	switch action {
	case "open_home":
		return m.navigateToScreen(ScreenHome)
	case "open_settings":
		return m.navigateToScreen(ScreenSettings)
	case "open_help":
		return m.navigateToScreen(ScreenHelp)
	case "open_ledger":
		return m.navigateToScreen(ScreenLedger)
	case "open_rollback":
		return m.navigateToScreen(ScreenRollback)
	case "open_dashboard":
		return m.navigateToScreen(ScreenDashboard)
	case "open_notifications":
		return m.navigateToScreen(ScreenNotifications)
	case "open_files":
		return m.navigateToScreen(ScreenFileExplorer)
	case "open_config":
		return m.navigateToScreen(ScreenConfig)
	case "open_session_detail":
		return m.navigateToScreen(ScreenSessionDetail)
	case "open_tool_detail":
		return m.navigateToScreen(ScreenToolDetail)
	case "toggle_subagents":
		if m.subagentsModel != nil && !m.subagentsModel.IsEmpty() {
			m.subagentsVisible = !m.subagentsVisible
		}
		return nil
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			if m.replModel != nil {
				// REPL width is already content-area; pass 0 to avoid double-subtracting.
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "sidebar_wider":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.IncreaseWidth()
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "sidebar_narrower":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.DecreaseWidth()
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "new_session":
		return m.startNewSession()
	case "session_list":
		return m.openResumeScreen()
	case "open_palette":
		return m.navigateToScreen(ScreenCommandPalette)
	case "cycle_model", "cycle_model_forward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cycle_model_backward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	case "view_ship_diff":
		if m.git != nil {
			if diff, err := m.git.Diff("", ""); err == nil && diff != "" {
				return func() tea.Msg {
					return DiffScreenMsg{Diff: diff, Title: "Ship Diff"}
				}
			}
		}
		return nil
	case "runtime_continue":
		m.setWorkflowPhase(types.PhaseShip)
		m.switchScreen(ScreenShip)
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseRuntime, types.PhaseShip); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseRuntime, "to", types.PhaseShip, "error", err)
			}
		}
		tasks, _ := m.sessionManager.LoadTasks(m.sessionID)
		modelName := ""
		if m.activeModel != nil {
			modelName = m.activeModel.Name
		}
		summary := ShipSummary{
			SessionID: m.sessionID,
			TaskDone:  countDone(tasks),
			TaskTotal: len(tasks),
			Model:     modelName,
			Provider:  m.activeProvider,
		}
		cw, ch := m.contentDimensions()
		m.shipModel = NewShipModel(summary, m.themeManager.Current(), cw, ch)
		m.persistWorkflowState()
		return m.RunPhaseCmd(types.PhaseShip)
	}
	return nil
}

// applyTheme switches the theme and propagates it to all sub-models.
// M31A ships with a single dark theme. Light/auto themes are not supported.
func (m *AppState) applyTheme(themeName string) {
	if themeName != "dark" {
		// Light/auto themes are not supported; always use dark.
		themeName = "dark"
	}
	m.themeManager = theme.NewManager(theme.ModeDark)

	// Persist theme selection to config so it survives restarts.
	if m.config != nil {
		m.config.UI.Theme = themeName
		if m.configPath != "" {
			if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
				slog.Warn("failed to save theme to config", "error", err)
			}
		}
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
	if m.diffModel != nil {
		m.diffModel.SetTheme(t)
	}
	if m.goalInput != nil {
		m.goalInput.SetTheme(t)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetTheme(t)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetTheme(t)
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetTheme(t)
	}
	if m.msModel != nil {
		m.msModel.SetTheme(t)
	}
	if m.helpModel != nil {
		m.helpModel.SetTheme(t)
	}
	if m.configModel != nil {
		m.configModel.theme = t
	}
	if m.bisectModel != nil {
		m.bisectModel.SetTheme(t)
	}
	if m.notifModel != nil {
		m.notifModel.SetTheme(t)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetTheme(t)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetTheme(t)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetTheme(t)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetTheme(t)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetTheme(t)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetTheme(t)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetTheme(t)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetTheme(t)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetTheme(t)
	}
}
