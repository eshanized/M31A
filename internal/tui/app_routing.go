package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

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
			cw, ch := m.contentDimensions()
			m.planModel = NewPlanModel(
				[]types.Task{},
				m.themeManager.Current(),
				"", "", "",
				0, "",
				cw, ch,
			)
			m.router.Register(ScreenPlan, m.planModel)
		}
		newModel, cmd := m.planModel.Update(msg)
		if r, ok := newModel.(*PlanModel); ok {
			m.planModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenExecute] = func(msg tea.Msg) tea.Cmd {
		if m.executeModel == nil {
			cw, ch := m.contentDimensions()
			m.executeModel = NewExecuteModel([]types.Task{}, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenExecute, m.executeModel)
		}
		newModel, cmd := m.executeModel.Update(msg)
		if r, ok := newModel.(*ExecuteModel); ok {
			m.executeModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenVerify] = func(msg tea.Msg) tea.Cmd {
		if m.verifyModel == nil {
			cw, ch := m.contentDimensions()
			m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenVerify, m.verifyModel)
		}
		newModel, cmd := m.verifyModel.Update(msg)
		if r, ok := newModel.(*VerifyModel); ok {
			m.verifyModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenRuntimeCheck] = func(msg tea.Msg) tea.Cmd {
		if m.runtimeModel == nil {
			cw, ch := m.contentDimensions()
			m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
		}
		newModel, cmd := m.runtimeModel.Update(msg)
		if r, ok := newModel.(*RuntimeModel); ok {
			m.runtimeModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenShip] = func(msg tea.Msg) tea.Cmd {
		if m.shipModel == nil {
			cw, ch := m.contentDimensions()
			m.shipModel = NewShipModel(ShipSummary{}, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenShip, m.shipModel)
		}
		newModel, cmd := m.shipModel.Update(msg)
		if r, ok := newModel.(*ShipModel); ok {
			m.shipModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenLedger] = func(msg tea.Msg) tea.Cmd {
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			m.ledgerModel.LoadEntries()
			m.router.Register(ScreenLedger, m.ledgerModel)
		}
		newModel, cmd := m.ledgerModel.Update(msg)
		if r, ok := newModel.(*LedgerModel); ok {
			m.ledgerModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenRollback] = func(msg tea.Msg) tea.Cmd {
		if m.rollbackModel == nil {
			cw, ch := m.contentDimensions()
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
			m.rollbackModel.LoadCommits()
			m.router.Register(ScreenRollback, m.rollbackModel)
		}
		newModel, cmd := m.rollbackModel.Update(msg)
		if r, ok := newModel.(*RollbackModel); ok {
			m.rollbackModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenConfig] = func(msg tea.Msg) tea.Cmd {
		if m.configModel == nil {
			cw, ch := m.contentDimensions()
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
			m.router.Register(ScreenConfig, m.configModel)
		}
		newModel, cmd := m.configModel.Update(msg)
		if r, ok := newModel.(*ConfigModel); ok {
			m.configModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenDiff] = func(msg tea.Msg) tea.Cmd {
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
			m.router.Register(ScreenDiff, m.diffModel)
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
			cw, ch := m.contentDimensions()
			m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenToolDetail, m.toolDetailModel)
		}
		newModel, cmd := m.toolDetailModel.Update(msg)
		if r, ok := newModel.(*ToolDetailModel); ok {
			m.toolDetailModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenCommandPalette] = func(msg tea.Msg) tea.Cmd {
		if m.commandPaletteScreenModel == nil {
			cw, ch := m.contentDimensions()
			m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenCommandPalette, m.commandPaletteScreenModel)
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
			cw, ch := m.contentDimensions()
			m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenBisect, m.bisectModel)
		}
		newModel, cmd := m.bisectModel.Update(msg)
		if r, ok := newModel.(*BisectModel); ok {
			m.bisectModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenDashboard] = func(msg tea.Msg) tea.Cmd {
		if m.dashboardModel == nil {
			cw, ch := m.contentDimensions()
			m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenDashboard, m.dashboardModel)
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
		// Ensure Notifications is registered with router on first message.
		if m.router != nil {
			m.router.Register(ScreenNotifications, m.notifModel)
		}
		newModel, cmd := m.notifModel.Update(msg)
		if r, ok := newModel.(*NotificationModel); ok {
			m.notifModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenMetrics] = func(msg tea.Msg) tea.Cmd {
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
			m.router.Register(ScreenMetrics, m.metricsModel)
		}
		newModel, cmd := m.metricsModel.Update(msg)
		if r, ok := newModel.(*MetricsModel); ok {
			m.metricsModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenResume] = func(msg tea.Msg) tea.Cmd {
		if m.resumeModel == nil {
			m.resumeModel = NewResumeModel(nil, m.themeManager.Current())
			m.router.Register(ScreenResume, m.resumeModel)
		}
		newModel, cmd := m.resumeModel.Update(msg)
		if r, ok := newModel.(*ResumeModel); ok {
			m.resumeModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenSettings] = func(msg tea.Msg) tea.Cmd {
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
			m.router.Register(ScreenSettings, m.settingsModel)
		}
		newModel, cmd := m.settingsModel.Update(msg)
		if r, ok := newModel.(*SettingsModel); ok {
			m.settingsModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenModelSelector] = func(msg tea.Msg) tea.Cmd {
		// ModelSelector is an overlay — View() is handled by renderFrameWithTheme()
		// directly on the concrete pointer; router registration is unnecessary.
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
		}
		newModel, cmd := m.msModel.Update(msg)
		if r, ok := newModel.(*ModelSelector); ok {
			m.msModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenDiscuss] = func(msg tea.Msg) tea.Cmd {
		if m.discussModel == nil {
			cw, ch := m.contentDimensions()
			m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
			if m.config != nil && m.config.UI.DiscussTimeout > 0 {
				m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
			}
			m.router.Register(ScreenDiscuss, m.discussModel)
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
		// Register with router on first message (model created by routeToScreen/ensureSubModel).
		if m.router != nil {
			m.router.Register(ScreenFirstRun, m.firstRunModel)
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
			cw, ch := m.contentDimensions()
			m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenSessionDetail, m.sessionDetailModel)
		}
		newModel, cmd := m.sessionDetailModel.Update(msg)
		if r, ok := newModel.(*SessionDetailModel); ok {
			m.sessionDetailModel = r
		}
		return cmd
	}
	m.screenUpdaters[ScreenChatHistory] = func(msg tea.Msg) tea.Cmd {
		if m.chatHistoryModel == nil {
			cw, ch := m.contentDimensions()
			m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenChatHistory, m.chatHistoryModel)
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

	// ScreenDecisions uses a lightweight DecisionScreen wrapper for router registration.
	m.screenUpdaters[ScreenDecisions] = func(msg tea.Msg) tea.Cmd {
		if m.decisionScreen == nil {
			cw, ch := m.contentDimensions()
			m.decisionScreen = NewDecisionScreen(m.themeManager.Current(), cw, ch)
			m.router.Register(ScreenDecisions, m.decisionScreen)
		}
		newModel, cmd := m.decisionScreen.Update(msg)
		if r, ok := newModel.(*DecisionScreen); ok {
			m.decisionScreen = r
		}
		return cmd
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
