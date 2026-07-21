package tui

// Navigation and screen management methods for AppState.

import (
	"log/slog"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

// routeToScreen returns the initialization cmd for the current screen.
// This switch is intentionally kept separate from ensureSubModel():
// routeToScreen() is called on FIRST navigation to a screen (from navigateToScreen)
// and creates the sub-model + calls Init(). ensureSubModel() is called on
// TRANSITIONS and popScreen() and handles resize + conditional re-init.
// They share structure but differ in lifecycle semantics.
func (m *AppState) routeToScreen() tea.Cmd {
	slog.Debug("routeToScreen called", "screen", m.screen.Label(), "prevScreen", m.prevScreen.Label())
	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel == nil {
			fm := NewFirstRunModel(m.themeManager.Current(), m.registry, m.config, m.version, m.shutdownCtx)
			cw, ch := m.contentDimensions()
			fm.SetDimensions(cw, ch)
			m.firstRunModel = fm
			if m.router != nil {
				m.router.Register(ScreenFirstRun, m.firstRunModel)
			}
		}
		return m.firstRunModel.Init()
	case ScreenREPL:
		m.ensureReplModel()
		if m.replModel.width == 0 {
			cw, ch := m.contentDimensions()
			m.replModel.SetDimensions(cw, ch)
		}
		if m.router != nil {
			m.router.Register(ScreenREPL, m.replModel)
		}
		return m.replModel.Init()
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
			if m.router != nil {
				m.router.Register(ScreenModelSelector, m.msModel)
			}
		}
		cw, ch := m.contentDimensions()
		m.msModel.SetDimensions(cw, ch)
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
			if m.router != nil {
				m.router.Register(ScreenSettings, m.settingsModel)
			}
		}
		cw, ch := m.contentDimensions()
		m.settingsModel.SetDimensions(cw, ch)
		return m.settingsModel.Init()
	case ScreenResume:
		return m.openResumeScreen()
	case ScreenPlan:
		if m.planModel != nil {
			cw, ch := m.contentDimensions()
			m.planModel.SetDimensions(cw, ch)
		}
		if m.router != nil && m.planModel != nil {
			m.router.Register(ScreenPlan, m.planModel)
		}
		return nil
	case ScreenExecute:
		if m.executeModel != nil {
			cw, ch := m.contentDimensions()
			m.executeModel.SetDimensions(cw, ch)
		}
		if m.router != nil && m.executeModel != nil {
			m.router.Register(ScreenExecute, m.executeModel)
		}
		return nil
	case ScreenVerify:
		if m.verifyModel != nil {
			cw, ch := m.contentDimensions()
			m.verifyModel.SetDimensions(cw, ch)
		}
		if m.router != nil && m.verifyModel != nil {
			m.router.Register(ScreenVerify, m.verifyModel)
		}
		return nil
	case ScreenRuntimeCheck:
		if m.runtimeModel != nil {
			cw, ch := m.contentDimensions()
			m.runtimeModel.SetDimensions(cw, ch)
			if m.router != nil {
				m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
			}
			return func() tea.Msg { return RuntimeTickMsg{} }
		}
		return nil
	case ScreenShip:
		if m.shipModel != nil {
			cw, ch := m.contentDimensions()
			m.shipModel.SetDimensions(cw, ch)
		}
		if m.router != nil && m.shipModel != nil {
			m.router.Register(ScreenShip, m.shipModel)
		}
		return nil
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
		}
		cw, ch := m.contentDimensions()
		m.goalInput.SetDimensions(cw, ch)
		return m.goalInput.Init()
	case ScreenLedger:
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			cw, ch := m.contentDimensions()
			m.ledgerModel.SetDimensions(cw, ch)
			m.ledgerModel.LoadEntries()
		}
		return nil
	case ScreenRollback:
		if m.rollbackModel == nil {
			cw, ch := m.contentDimensions()
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
			if m.router != nil {
				m.router.Register(ScreenRollback, m.rollbackModel)
			}
			m.rollbackModel.LoadCommits()
		}
		return nil
	case ScreenMetrics:
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.metricsModel.SetDimensions(cw, ch)
		if m.sessionManager != nil {
			return m.metricsModel.LoadStatsCmd(m.sessionManager)
		}
		return nil
	case ScreenDiscuss:
		if m.discussModel == nil {
			cw, ch := m.contentDimensions()
			m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
			if m.router != nil {
				m.router.Register(ScreenDiscuss, m.discussModel)
			}
			if m.config != nil && m.config.UI.DiscussTimeout > 0 {
				m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
			}
		} else {
			cw, ch := m.contentDimensions()
			m.discussModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenConfig:
		if m.configModel == nil {
			cw, ch := m.contentDimensions()
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
			if m.router != nil {
				m.router.Register(ScreenConfig, m.configModel)
			}
		}
		return nil
	case ScreenHelp:
		if m.helpModel == nil {
			m.helpModel = NewHelpModel(m.themeManager.Current())
			if m.router != nil {
				m.router.Register(ScreenHelp, m.helpModel)
			}
			m.helpModel.SetKeyRegistry(m.keyRegistry)
		}
		cw, ch := m.contentDimensions()
		m.helpModel.SetDimensions(cw, ch)
		return m.helpModel.Init()
	case ScreenBisect:
		if m.bisectModel == nil {
			cw, ch := m.contentDimensions()
			m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenBisect, m.bisectModel)
			}
		}
		return nil
	case ScreenNotifications:
		if m.notifModel == nil {
			cw, ch := m.contentDimensions()
			m.notifModel = NewNotificationModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenNotifications, m.notifModel)
			}
		}
		return nil
	case ScreenDecisions:
		if m.decisionScreen == nil {
			if m.themeManager != nil {
				cw, ch := m.contentDimensions()
				m.decisionScreen = NewDecisionScreen(m.themeManager.Current(), cw, ch)
				if m.router != nil {
					m.router.Register(ScreenDecisions, m.decisionScreen)
				}
			}
		}
		return nil
	case ScreenDashboard:
		if m.dashboardModel == nil {
			cw, ch := m.contentDimensions()
			m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenDashboard, m.dashboardModel)
			}
		}
		if m.workflowEngine != nil {
			m.dashboardModel.SetWorkflowState(m.workflowPhase, m.workflowGoal, "", m.activeProvider)
		}
		return nil
	case ScreenSessionDetail:
		if m.sessionDetailModel == nil {
			cw, ch := m.contentDimensions()
			m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenSessionDetail, m.sessionDetailModel)
			}
		}
		return nil
	case ScreenFileExplorer:
		if m.fileExplorerModel == nil {
			cw, ch := m.contentDimensions()
			m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenFileExplorer, m.fileExplorerModel)
			}
			if m.cwd != "" {
				root := buildFileTree(m.cwd, 0, 3)
				if root != nil {
					m.fileExplorerModel.SetRoot(root)
				}
			}
		}
		return nil
	case ScreenToolDetail:
		if m.toolDetailModel == nil {
			cw, ch := m.contentDimensions()
			m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenToolDetail, m.toolDetailModel)
			}
		}
		return nil
	case ScreenPhaseModelPicker:
		if m.phaseModelPicker == nil {
			cw, ch := m.contentDimensions()
			m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenPhaseModelPicker, m.phaseModelPicker)
			}
		}
		return nil
	case ScreenGhostPicker:
		if m.ghostPickerModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenGhostPicker, m.ghostPickerModel)
			}
		}
		return nil
	case ScreenGhostOutput:
		if m.ghostOutputModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenGhostOutput, m.ghostOutputModel)
			}
		}
		return nil
	case ScreenConfirmQuit:
		if m.confirmQuitModel == nil {
			cw, ch := m.contentDimensions()
			m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenConfirmQuit, m.confirmQuitModel)
			}
		}
		return nil
	case ScreenDiff:
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
			if m.router != nil {
				m.router.Register(ScreenDiff, m.diffModel)
			}
		}
		cw, ch := m.contentDimensions()
		m.diffModel.SetDimensions(cw, ch)
		return nil
	case ScreenCommandPalette:
		if m.commandPaletteScreenModel == nil {
			cw, ch := m.contentDimensions()
			m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), cw, ch)
			if m.router != nil {
				m.router.Register(ScreenCommandPalette, m.commandPaletteScreenModel)
			}
		}
		return m.commandPaletteScreenModel.Init()
	case ScreenHome:
		if m.homeModel == nil {
			cw, ch := m.contentDimensions()
			m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
			m.homeModel.SetCommandRegistry(m.cmdRegistry)
			m.homeModel.SetConfig(m.config)
			if m.router != nil {
				m.router.Register(ScreenHome, m.homeModel)
			}
		}
		return m.homeModel.Init()
	default:
		return nil
	}
}

// navigateToScreen transitions to the given screen.
func (m *AppState) navigateToScreen(screen Screen) tea.Cmd {
	m.prevScreen = m.screen

	// Push current screen to back-stack for esc-to-go-back navigation.
	// Skip: same screen, permission overlay, diff overlay, and REPL (always fallback).
	if m.screen != screen && m.screen != ScreenPermission && screen != ScreenPermission &&
		m.screen != ScreenREPL {
		// Avoid duplicate consecutive entries
		if len(m.screenStack) == 0 || m.screenStack[len(m.screenStack)-1] != m.screen {
			m.screenStack = append(m.screenStack, m.screen)
		}
		if m.screenCap > 0 && len(m.screenStack) > m.screenCap {
			m.screenStack = m.screenStack[len(m.screenStack)-m.screenCap:]
		}
	}

	// Start a brief transition overlay if this is a real screen change.
	// Skip for overlays and first-run.
	skipTransition := screen == ScreenPermission || screen == ScreenDiff ||
		screen == ScreenFirstRun || screen == ScreenModelSelector
	if m.screen != screen && !skipTransition {
		// Eagerly ensure sub-model exists so it's ready when transition completes.
		initCmd := m.ensureSubModel(screen)
		m.StartTransition(screen, "")
		m.switchScreen(screen)
		return tea.Batch(StreamTickCmd(), initCmd)
	}

	m.switchScreen(screen)
	return m.ensureSubModel(screen)
}

// popScreen navigates back to the previous screen in the back-stack, or to REPL if empty.
func (m *AppState) popScreen() tea.Cmd {
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.switchScreen(prev)
		return m.ensureSubModel(prev)
	}
	m.switchScreen(ScreenREPL)
	return nil
}

// contentDimensions returns the content-area width and height, accounting for
// the unified chrome (header+footer) and the sidebar when visible.
func (m *AppState) contentDimensions() (w, h int) {
	w = m.width
	h = m.height - layout.ChromeHeight
	if h < 1 {
		h = 1
	}
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
		sidebarW := m.sidebarModel.GetWidth()
		if sidebarW > 0 && sidebarW < w {
			w -= sidebarW
		}
	}
	if w < 1 {
		w = 1
	}
	slog.Debug("contentDimensions", "terminalW", m.width, "terminalH", m.height, "contentW", w, "contentH", h, "sidebarVisible", m.sidebarModel != nil && m.sidebarModel.IsVisible(), "showSidebar", layout.ShowSidebar(m.width))
	return w, h
}

// ensureSubModel creates or resizes the sub-model for the given screen.
func (m *AppState) ensureSubModel(screen Screen) tea.Cmd {
	cw, ch := m.contentDimensions()
	switch screen {
	case ScreenREPL:
		m.ensureReplModel()
		m.replModel.SetDimensions(cw, ch)
		if m.router != nil {
			m.router.Register(ScreenREPL, m.replModel)
		}
		return nil
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
		}
		m.msModel.SetDimensions(cw, ch)
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
			if m.router != nil {
				m.router.Register(ScreenSettings, m.settingsModel)
			}
		}
		m.settingsModel.SetDimensions(cw, ch)
		return m.settingsModel.Init()
	case ScreenResume:
		// Resume screen loads sessions async; use the existing command.
		return m.openResumeScreen()
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
			if m.router != nil {
				m.router.Register(ScreenGoalInput, m.goalInput)
			}
		}
		m.goalInput.SetDimensions(cw, ch)
		return m.goalInput.Init()
	case ScreenPlan:
		if m.planModel != nil {
			m.planModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenExecute:
		if m.executeModel == nil {
			m.executeModel = NewExecuteModel([]types.Task{}, m.themeManager.Current(), cw, ch)
			m.executeModel.SetWorkflowEngine(m.workflowEngine)
		}
		m.executeModel.SetDimensions(cw, ch)
		if m.router != nil {
			m.router.Register(ScreenExecute, m.executeModel)
		}
		return nil
	case ScreenVerify:
		if m.verifyModel == nil {
			m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
		}
		m.verifyModel.SetDimensions(cw, ch)
		if m.router != nil {
			m.router.Register(ScreenVerify, m.verifyModel)
		}
		return nil
	case ScreenRuntimeCheck:
		if m.runtimeModel == nil {
			m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
		}
		m.runtimeModel.SetDimensions(cw, ch)
		if m.router != nil {
			m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
		}
		return nil
	case ScreenShip:
		if m.shipModel == nil {
			m.shipModel = NewShipModel(ShipSummary{}, m.themeManager.Current(), cw, ch)
		}
		m.shipModel.SetDimensions(cw, ch)
		if m.router != nil {
			m.router.Register(ScreenShip, m.shipModel)
		}
		return nil
	case ScreenLedger:
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			if m.router != nil {
				m.router.Register(ScreenLedger, m.ledgerModel)
			}
			m.ledgerModel.SetDimensions(cw, ch)
			m.ledgerModel.LoadEntries()
		}
		return nil
	case ScreenRollback:
		if m.rollbackModel == nil {
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
			if m.router != nil {
				m.router.Register(ScreenRollback, m.rollbackModel)
			}
			m.rollbackModel.LoadCommits()
		}
		return nil
	case ScreenMetrics:
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
			if m.router != nil {
				m.router.Register(ScreenMetrics, m.metricsModel)
			}
		}
		m.metricsModel.SetDimensions(cw, ch)
		if m.sessionManager != nil {
			return m.metricsModel.LoadStatsCmd(m.sessionManager)
		}
		return nil
	case ScreenConfig:
		if m.configModel == nil {
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
			if m.router != nil {
				m.router.Register(ScreenConfig, m.configModel)
			}
		} else {
			// Sync live config pointer so edits made in settings are visible
			m.configModel.cfg = m.config
			m.configModel.cfgPath = m.configPath
			m.configModel.SetDimensions(cw, ch)
			m.configModel.theme = m.themeManager.Current()
			m.configModel.buildSections()
			m.configModel.updateViewportContent()
		}
		return nil
	case ScreenHelp:
		if m.helpModel == nil {
			m.helpModel = NewHelpModel(m.themeManager.Current())
			m.helpModel.SetKeyRegistry(m.keyRegistry)
		}
		m.helpModel.SetDimensions(cw, ch)
		return m.helpModel.Init()
	case ScreenDiscuss:
		if m.discussModel == nil {
			m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
			// Activate discuss timeout from config
			if m.config != nil && m.config.UI.DiscussTimeout > 0 {
				m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
			}
		} else {
			m.discussModel.SetDimensions(cw, ch)
		}
		return m.discussModel.Init()
	case ScreenBisect:
		if m.bisectModel == nil {
			m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
		} else {
			m.bisectModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenNotifications:
		if m.notifModel == nil {
			m.notifModel = NewNotificationModel(m.themeManager.Current(), cw, ch)
		} else {
			m.notifModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenDashboard:
		if m.dashboardModel == nil {
			m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
		} else {
			m.dashboardModel.SetDimensions(cw, ch)
		}
		if m.workflowEngine != nil {
			m.dashboardModel.SetWorkflowState(m.workflowPhase, m.workflowGoal, "", m.activeProvider)
		}
		return nil
	case ScreenSessionDetail:
		if m.sessionDetailModel == nil {
			m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
		} else {
			m.sessionDetailModel.SetDimensions(cw, ch)
		}
		// Load session data if we have a session ID from AppMsg
		if m.sessionDetailModel.sess == nil && m.sessionManager != nil {
			// Try to find the session from the resume model's current selection
			if m.resumeModel != nil && len(m.resumeModel.sessions) > 0 {
				idx := m.resumeModel.cursor
				if idx >= 0 && idx < len(m.resumeModel.sessions) {
					sid := m.resumeModel.sessions[idx].ID
					if sess, err := m.sessionManager.LoadSession(sid); err == nil && sess != nil {
						m.sessionDetailModel.SetSession(sess)
					}
				}
			}
		}
		return nil
	case ScreenFileExplorer:
		if m.fileExplorerModel == nil {
			m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
			// Build real file tree from working directory
			if m.cwd != "" {
				root := buildFileTree(m.cwd, 0, 3)
				if root != nil {
					m.fileExplorerModel.SetRoot(root)
				}
			}
		} else {
			m.fileExplorerModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenToolDetail:
		if m.toolDetailModel == nil {
			m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
		} else {
			m.toolDetailModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenFirstRun:
		fm := NewFirstRunModel(m.themeManager.Current(), m.registry, m.config, m.version, m.shutdownCtx)
		fm.SetDimensions(cw, ch)
		m.firstRunModel = fm
		return fm.Init()
	case ScreenGhostPicker:
		if m.ghostPickerModel == nil {
			m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
		} else {
			m.ghostPickerModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenGhostOutput:
		if m.ghostOutputModel == nil {
			m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
		} else {
			m.ghostOutputModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenConfirmQuit:
		if m.confirmQuitModel == nil {
			m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), cw, ch)
		} else {
			m.confirmQuitModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenDecisions:
		if m.decisionScreen == nil {
			if m.themeManager != nil {
				m.decisionScreen = NewDecisionScreen(m.themeManager.Current(), cw, ch)
			}
		} else {
			m.decisionScreen.SetDimensions(cw, ch)
		}
		return nil
	case ScreenPhaseModelPicker:
		if m.phaseModelPicker == nil {
			m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
		} else {
			m.phaseModelPicker.SetDimensions(cw, ch)
		}
		return nil
	case ScreenChatHistory:
		if m.chatHistoryModel == nil {
			m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), cw, ch)
		} else {
			m.chatHistoryModel.SetDimensions(cw, ch)
		}
		// Load messages from current REPL session
		if m.replModel != nil {
			m.chatHistoryModel.SetMessages(m.replModel.Messages())
		}
		return nil
	case ScreenCommandPalette:
		if m.commandPaletteScreenModel == nil {
			m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), cw, ch)
		} else {
			m.commandPaletteScreenModel.SetDimensions(cw, ch)
		}
		return m.commandPaletteScreenModel.Init()
	case ScreenHome:
		if m.homeModel == nil {
			m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
			m.homeModel.SetCommandRegistry(m.cmdRegistry)
			m.homeModel.SetConfig(m.config)
		} else {
			m.homeModel.SetDimensions(cw, ch)
		}
		return m.homeModel.Init()
	default:
		return nil
	}
}

// openResumeScreen loads session list and navigates to the resume screen.
func (m *AppState) openResumeScreen() tea.Cmd {
	return func() tea.Msg {
		var sessions []session.SessionInfo
		total := 0
		if m.sessionManager != nil {
			infos, err := m.sessionManager.ListSessions()
			if err == nil {
				total = len(infos)
				for i, info := range infos {
					if i >= 20 {
						break
					}
					sessions = append(sessions, info)
				}
			}
		}
		return resumeScreenReadyMsg{sessions: sessions, total: total}
	}
}

// resumeScreenReadyMsg carries loaded sessions for the resume screen.
type resumeScreenReadyMsg struct {
	sessions []session.SessionInfo
	total    int
}

// openSettingsScreen transitions to the settings screen.
func (m *AppState) openSettingsScreen() tea.Cmd {
	return m.navigateToScreen(ScreenSettings)
}
