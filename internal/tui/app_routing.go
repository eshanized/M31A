package tui

import tea "github.com/charmbracelet/bubbletea"

// app_routing.go — consolidated screen routing infrastructure.
//
// Stores a map[Screen]func(tea.Msg) tea.Cmd that encapsulates the
// per-screen Update-and-assign pattern. Initialized once, used by
// forwardMsgToScreen(), routeKeyMsg(), and mouse forwarding.

// screenUpdateFunc is a function that updates a screen sub-model and returns
// the resulting command.
type screenUpdateFunc func(tea.Msg) tea.Cmd

// initScreenUpdaters creates the map of screen → update closures.
// Each closure captures a pointer to the model field and checks nil at call time.
// This must be called after all model pointer fields are addressable (i.e., after
// NewApp returns the *AppState).
func (m *AppState) initScreenUpdaters() {
	m.screenUpdaters = make(map[Screen]screenUpdateFunc, 32)

	// Helper for models whose Update returns (tea.Model, tea.Cmd).
	teaUpdate := func(getModel func() **any, setModel func(any), updateFn func(tea.Msg) (tea.Model, tea.Cmd)) screenUpdateFunc {
		return func(msg tea.Msg) tea.Cmd {
			dest := getModel()
			if *dest == nil {
				return nil
			}
			result, cmd := updateFn(msg)
			if result != nil {
				setModel(result)
			}
			return cmd
		}
	}

	// Register tea.Model-returning screens (type-assert pattern)
	registerTeaModel := func(screen Screen, getPtr func() **any, setFn func(any), updateFn func(tea.Msg) (tea.Model, tea.Cmd)) {
		m.screenUpdaters[screen] = teaUpdate(getPtr, setFn, updateFn)
	}

	// Helper for models whose Update returns (*ConcreteType, tea.Cmd).
	ptrUpdate := func(screen Screen, getPtr func() *any, updateFn func(tea.Msg) (any, tea.Cmd)) {
		m.screenUpdaters[screen] = func(msg tea.Msg) tea.Cmd {
			dest := getPtr()
			if *dest == nil {
				return nil
			}
			result, cmd := updateFn(msg)
			if result != nil {
				*dest = result
			}
			return cmd
		}
	}

	_ = registerTeaModel
	_ = ptrUpdate

	// For simplicity and type safety, use direct closures per screen.
	// Each checks nil before calling Update, matching the original behavior.

	m.screenUpdaters[ScreenREPL] = func(msg tea.Msg) tea.Cmd {
		if m.replModel == nil {
			return nil
		}
		newModel, cmd := m.replModel.Update(msg)
		if r, ok := newModel.(*ReplModel); ok {
			m.replModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenPlan] = func(msg tea.Msg) tea.Cmd {
		if m.planModel == nil {
			return nil
		}
		newModel, cmd := m.planModel.Update(msg)
		m.planModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenExecute] = func(msg tea.Msg) tea.Cmd {
		if m.executeModel == nil {
			return nil
		}
		newModel, cmd := m.executeModel.Update(msg)
		m.executeModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenVerify] = func(msg tea.Msg) tea.Cmd {
		if m.verifyModel == nil {
			return nil
		}
		newModel, cmd := m.verifyModel.Update(msg)
		m.verifyModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenRuntimeCheck] = func(msg tea.Msg) tea.Cmd {
		if m.runtimeModel == nil {
			return nil
		}
		newModel, cmd := m.runtimeModel.Update(msg)
		m.runtimeModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenShip] = func(msg tea.Msg) tea.Cmd {
		if m.shipModel == nil {
			return nil
		}
		newModel, cmd := m.shipModel.Update(msg)
		m.shipModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenLedger] = func(msg tea.Msg) tea.Cmd {
		if m.ledgerModel == nil {
			return nil
		}
		newModel, cmd := m.ledgerModel.Update(msg)
		if r, ok := newModel.(*LedgerModel); ok {
			m.ledgerModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenRollback] = func(msg tea.Msg) tea.Cmd {
		if m.rollbackModel == nil {
			return nil
		}
		newModel, cmd := m.rollbackModel.Update(msg)
		if r, ok := newModel.(*RollbackModel); ok {
			m.rollbackModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenConfig] = func(msg tea.Msg) tea.Cmd {
		if m.configModel == nil {
			return nil
		}
		newModel, cmd := m.configModel.Update(msg)
		m.configModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenDiff] = func(msg tea.Msg) tea.Cmd {
		if m.diffModel == nil {
			return nil
		}
		newModel, cmd := m.diffModel.Update(msg)
		if r, ok := newModel.(*DiffModel); ok {
			m.diffModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenHelp] = func(msg tea.Msg) tea.Cmd {
		if m.helpModel == nil {
			m.helpModel = NewHelpModel(m.themeManager.Current())
			m.helpModel.SetKeyRegistry(m.keyRegistry)
			// Register with router
			m.router.Register(ScreenHelp, m.helpModel)
		}
		newModel, cmd := m.helpModel.Update(msg)
		if r, ok := newModel.(*HelpModel); ok {
			m.helpModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenToolDetail] = func(msg tea.Msg) tea.Cmd {
		if m.toolDetailModel == nil {
			return nil
		}
		newModel, cmd := m.toolDetailModel.Update(msg)
		if r, ok := newModel.(*ToolDetailModel); ok {
			m.toolDetailModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenCommandPalette] = func(msg tea.Msg) tea.Cmd {
		if m.commandPaletteScreenModel == nil {
			return nil
		}
		newModel, cmd := m.commandPaletteScreenModel.Update(msg)
		if r, ok := newModel.(*CommandPaletteScreenModel); ok {
			m.commandPaletteScreenModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenFileExplorer] = func(msg tea.Msg) tea.Cmd {
		if m.fileExplorerModel == nil {
			cw, ch := m.contentDimensions()
			m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
			if m.cwd != "" {
				root := buildFileTree(m.cwd, 0, 3)
				if root != nil {
					m.fileExplorerModel.SetRoot(root)
				}
			}
			// Register with router
			m.router.Register(ScreenFileExplorer, m.fileExplorerModel)
		}
		newModel, cmd := m.fileExplorerModel.Update(msg)
		if r, ok := newModel.(*FileExplorerModel); ok {
			m.fileExplorerModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenBisect] = func(msg tea.Msg) tea.Cmd {
		if m.bisectModel == nil {
			return nil
		}
		newModel, cmd := m.bisectModel.Update(msg)
		if r, ok := newModel.(*BisectModel); ok {
			m.bisectModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenDashboard] = func(msg tea.Msg) tea.Cmd {
		if m.dashboardModel == nil {
			return nil
		}
		newModel, cmd := m.dashboardModel.Update(msg)
		if r, ok := newModel.(*DashboardModel); ok {
			m.dashboardModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenNotifications] = func(msg tea.Msg) tea.Cmd {
		if m.notifModel == nil {
			return nil
		}
		newModel, cmd := m.notifModel.Update(msg)
		if r, ok := newModel.(*NotificationModel); ok {
			m.notifModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenMetrics] = func(msg tea.Msg) tea.Cmd {
		if m.metricsModel == nil {
			return nil
		}
		newModel, cmd := m.metricsModel.Update(msg)
		if r, ok := newModel.(*MetricsModel); ok {
			m.metricsModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenResume] = func(msg tea.Msg) tea.Cmd {
		if m.resumeModel == nil {
			return nil
		}
		newModel, cmd := m.resumeModel.Update(msg)
		if r, ok := newModel.(*ResumeModel); ok {
			m.resumeModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenSettings] = func(msg tea.Msg) tea.Cmd {
		if m.settingsModel == nil {
			return nil
		}
		newModel, cmd := m.settingsModel.Update(msg)
		m.settingsModel = newModel
		return cmd
	}
	m.screenUpdaters[ScreenModelSelector] = func(msg tea.Msg) tea.Cmd {
		if m.msModel == nil {
			return nil
		}
		newModel, cmd := m.msModel.Update(msg)
		if r, ok := newModel.(*ModelSelector); ok {
			m.msModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenDiscuss] = func(msg tea.Msg) tea.Cmd {
		if m.discussModel == nil {
			return nil
		}
		newModel, cmd := m.discussModel.Update(msg)
		if r, ok := newModel.(*DiscussModel); ok {
			m.discussModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenGoalInput] = func(msg tea.Msg) tea.Cmd {
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
			// Register with router
			m.router.Register(ScreenGoalInput, m.goalInput)
		}
		newModel, cmd := m.goalInput.Update(msg)
		if r, ok := newModel.(*GoalInputModel); ok {
			m.goalInput = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenFirstRun] = func(msg tea.Msg) tea.Cmd {
		if m.firstRunModel == nil {
			return nil
		}
		newModel, cmd := m.firstRunModel.Update(msg)
		if r, ok := newModel.(*FirstRunModel); ok {
			m.firstRunModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenGhostPicker] = func(msg tea.Msg) tea.Cmd {
		if m.ghostPickerModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
			// Register with router
			m.router.Register(ScreenGhostPicker, m.ghostPickerModel)
		}
		newModel, cmd := m.ghostPickerModel.Update(msg)
		if r, ok := newModel.(*GhostPickerModel); ok {
			m.ghostPickerModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenGhostOutput] = func(msg tea.Msg) tea.Cmd {
		if m.ghostOutputModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
			// Register with router
			m.router.Register(ScreenGhostOutput, m.ghostOutputModel)
		}
		newModel, cmd := m.ghostOutputModel.Update(msg)
		if r, ok := newModel.(*GhostOutputModel); ok {
			m.ghostOutputModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenConfirmQuit] = func(msg tea.Msg) tea.Cmd {
		if m.confirmQuitModel == nil {
			m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), m.width, m.height)
			// Register with router
			m.router.Register(ScreenConfirmQuit, m.confirmQuitModel)
		}
		newModel, cmd := m.confirmQuitModel.Update(msg)
		if r, ok := newModel.(*ConfirmQuitModel); ok {
			m.confirmQuitModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenPhaseModelPicker] = func(msg tea.Msg) tea.Cmd {
		if m.phaseModelPicker == nil {
			cw, ch := m.contentDimensions()
			m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenPhaseModelPicker, m.phaseModelPicker)
		}
		newModel, cmd := m.phaseModelPicker.Update(msg)
		if r, ok := newModel.(*PhaseModelPickerModel); ok {
			m.phaseModelPicker = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenSessionDetail] = func(msg tea.Msg) tea.Cmd {
		if m.sessionDetailModel == nil {
			return nil
		}
		newModel, cmd := m.sessionDetailModel.Update(msg)
		if r, ok := newModel.(*SessionDetailModel); ok {
			m.sessionDetailModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenChatHistory] = func(msg tea.Msg) tea.Cmd {
		if m.chatHistoryModel == nil {
			return nil
		}
		newModel, cmd := m.chatHistoryModel.Update(msg)
		if r, ok := newModel.(*ChatHistoryModel); ok {
			m.chatHistoryModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenHome] = func(msg tea.Msg) tea.Cmd {
		if m.homeModel == nil {
			cw, ch := m.contentDimensions()
			m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
			m.homeModel.SetCommandRegistry(m.cmdRegistry)
			m.homeModel.SetConfig(m.config)
			// Register with router
			m.router.Register(ScreenHome, m.homeModel)
		}
		newModel, cmd := m.homeModel.Update(msg)
		if r, ok := newModel.(*HomeModel); ok {
			m.homeModel = r
		}
		return cmd
	}

	// ScreenDecisions has no sub-model — it renders directly from the workflow engine.
	m.screenUpdaters[ScreenDecisions] = func(msg tea.Msg) tea.Cmd {
		return nil
	}
}

// forwardMsgToScreen forwards a message to the active screen's sub-model
// using the pre-built routing map.
func (m *AppState) forwardMsgToScreen(msg tea.Msg) tea.Cmd {
	if fn, ok := m.screenUpdaters[m.screen]; ok {
		return fn(msg)
	}
	return nil
}

// forwardMouseToScreen forwards a mouse message to the active screen,
// with sidebar click interception for the REPL screen.
func (m *AppState) forwardMouseToScreen(msg tea.MouseMsg) tea.Cmd {
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && m.screen == ScreenREPL {
		sidebarW := m.sidebarModel.GetWidth()
		if sidebarW > 0 && msg.X < sidebarW {
			if c := m.sidebarModel.HandleMouse(msg, 0); c != nil {
				return c
			}
		}
	}
	if fn, ok := m.screenUpdaters[m.screen]; ok {
		return fn(msg)
	}
	return nil
}

// routeKeyToScreen forwards a keyboard message to the active screen,
// with special handling for the permission overlay.
func (m *AppState) routeKeyToScreen(msg tea.KeyMsg) tea.Cmd {
	switch m.screen {
	case ScreenPermission:
		return m.handlePermissionKey(msg)
	default:
		if fn, ok := m.screenUpdaters[m.screen]; ok {
			return fn(msg)
		}
	}
	return nil
}
