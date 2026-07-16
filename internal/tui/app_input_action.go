package tui

import (
	"log/slog"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

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
