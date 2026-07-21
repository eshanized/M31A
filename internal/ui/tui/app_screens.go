package tui

import (
	"fmt"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/layout"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

// ─── Per-screen content renderers ─────────────────────────────────────────────
// Each renderer returns ONLY the content area. No header, footer, or chrome.

func (m *AppState) renderREPLContent(chrome layout.PageChrome) string {
	m.ensureReplModel()

	// Subagent panel: reserve its rendered height from the REPL budget.
	var panel string
	panelHeight := 0
	if m.subagentsVisible && m.subagentsModel != nil && !m.subagentsModel.IsEmpty() {
		m.subagentsModel.SetSize(chrome.ContentWidth(), maxInt(4, chrome.ContentHeight()/3))
		m.subagentsModel.SetTheme(m.themeManager.Current())
		panel = m.subagentsModel.View()
		// Count the rendered rows (lines) so the REPL knows how much to shrink.
		for _, r := range panel {
			if r == '\n' {
				panelHeight++
			}
		}
		if panelHeight > chrome.ContentHeight()-4 {
			panelHeight = chrome.ContentHeight() - 4
		}
	}
	replH := chrome.ContentHeight() - panelHeight
	if replH < 4 {
		replH = 4
	}
	replChrome := layout.PageChrome{Width: chrome.ContentWidth(), Height: replH + layout.ChromeHeight}
	m.syncReplSize(replChrome)
	replContent := m.replModel.ViewContent(replH, chrome.ContentWidth())
	if panel == "" {
		return replContent
	}
	return panel + "\n" + replContent
}

func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func (m *AppState) renderSettingsContent(chrome layout.PageChrome) string {
	// Ensure Settings is registered with router
	if m.settingsModel == nil {
		m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
	}
	m.settingsModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	if m.router != nil {
		m.router.Register(ScreenSettings, m.settingsModel)
		return m.router.View()
	}
	return m.settingsModel.View()
}

func (m *AppState) renderModelSelectorContent(chrome layout.PageChrome) string {
	// ModelSelector is rendered as a centered dialog overlay in renderFrameWithTheme()
	// (lines 229-253), which returns before this function is called. This code path
	// is unreachable in normal operation — the overlay intercepts rendering first.
	// Kept as a fallback for completeness; does not register with the router because
	// View() is never delegated through it for this screen.
	if m.msModel == nil {
		m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
	}
	m.msModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.msModel.View()
}

func (m *AppState) renderPlanContent(chrome layout.PageChrome) string {
	// Ensure Plan is registered with router
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
	m.planModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderExecuteContent(chrome layout.PageChrome) string {
	// Ensure Execute is registered with router
	if m.executeModel == nil {
		cw, ch := m.contentDimensions()
		m.executeModel = NewExecuteModel([]types.Task{}, m.themeManager.Current(), cw, ch)
		m.executeModel.SetWorkflowEngine(m.workflowEngine)
		m.router.Register(ScreenExecute, m.executeModel)
	}
	m.executeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderVerifyContent(chrome layout.PageChrome) string {
	// Ensure Verify is registered with router
	if m.verifyModel == nil {
		cw, ch := m.contentDimensions()
		m.verifyModel = NewVerifyModel([]types.Task{}, map[int]workflow.VerificationResult{}, m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenVerify, m.verifyModel)
	}
	m.verifyModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderShipContent(chrome layout.PageChrome) string {
	// Ensure Ship is registered with router
	if m.shipModel == nil {
		m.shipModel = NewShipModel(ShipSummary{}, m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenShip, m.shipModel)
	}
	m.shipModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderResumeContent(chrome layout.PageChrome) string {
	// Ensure Resume is registered with router
	if m.resumeModel == nil {
		m.resumeModel = NewResumeModel(nil, m.themeManager.Current())
		m.router.Register(ScreenResume, m.resumeModel)
	}
	m.resumeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderGoalInputContent(chrome layout.PageChrome) string {
	// Ensure GoalInput is registered with router
	if m.goalInput == nil {
		m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
		m.router.Register(ScreenGoalInput, m.goalInput)
	}
	m.goalInput.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderFirstRunContent(chrome layout.PageChrome) string {
	if m.firstRunModel == nil {
		return renderLoading("Loading first-run wizard…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.firstRunModel.SetContentWidth(chrome.ContentWidth())
	if m.router != nil {
		m.router.Register(ScreenFirstRun, m.firstRunModel)
		if m.router.ActiveID() != ScreenFirstRun {
			m.router.SwitchTo(ScreenFirstRun)
		}
		return m.router.View()
	}
	return m.firstRunModel.View()
}

func (m *AppState) renderLedgerContent(chrome layout.PageChrome) string {
	// Ensure Ledger is registered with router
	if m.ledgerModel == nil {
		m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
		m.ledgerModel.LoadEntries()
		m.router.Register(ScreenLedger, m.ledgerModel)
	}
	m.ledgerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderRollbackContent(chrome layout.PageChrome) string {
	// Ensure Rollback is registered with router
	if m.rollbackModel == nil {
		cw, ch := m.contentDimensions()
		m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
		m.rollbackModel.LoadCommits()
		m.router.Register(ScreenRollback, m.rollbackModel)
	}
	m.rollbackModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderMetricsContent(chrome layout.PageChrome) string {
	// Ensure Metrics is registered with router
	if m.metricsModel == nil {
		m.metricsModel = NewMetricsModel(m.themeManager.Current())
		m.router.Register(ScreenMetrics, m.metricsModel)
	}
	m.metricsModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderDiscussContent(chrome layout.PageChrome) string {
	// Ensure Discuss is registered with router
	if m.discussModel == nil {
		cw, ch := m.contentDimensions()
		m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
		if m.config != nil && m.config.UI.DiscussTimeout > 0 {
			m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
		}
		m.router.Register(ScreenDiscuss, m.discussModel)
	}
	m.discussModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderConfigContent(chrome layout.PageChrome) string {
	// Ensure Config is registered with router
	if m.configModel == nil {
		cw, ch := m.contentDimensions()
		m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
	}
	m.configModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	m.configModel.SetTheme(m.themeManager.Current())
	m.configModel.cfg = m.config
	if m.router != nil {
		m.router.Register(ScreenConfig, m.configModel)
		return m.router.View()
	}
	return m.configModel.View()
}

func (m *AppState) renderDiffContent(chrome layout.PageChrome) string {
	// Ensure Diff is registered with router
	if m.diffModel == nil {
		m.diffModel = NewDiffModel(m.themeManager.Current())
		m.router.Register(ScreenDiff, m.diffModel)
	}
	m.diffModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderHelpContent(chrome layout.PageChrome) string {
	// Ensure Help is registered with router
	if m.helpModel == nil {
		m.helpModel = NewHelpModel(m.themeManager.Current())
		m.helpModel.SetKeyRegistry(m.keyRegistry)
		m.router.Register(ScreenHelp, m.helpModel)
	}
	m.helpModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderBisectContent(chrome layout.PageChrome) string {
	// Ensure Bisect is registered with router
	if m.bisectModel == nil {
		cw, ch := m.contentDimensions()
		m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenBisect, m.bisectModel)
	}
	m.bisectModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderNotificationsContent(chrome layout.PageChrome) string {
	if m.notifModel == nil {
		return renderEmptyState("Notifications", "No notifications yet — they'll appear here as you use M31A", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.notifModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	if m.router != nil {
		m.router.Register(ScreenNotifications, m.notifModel)
		return m.router.View()
	}
	return m.notifModel.View()
}

func (m *AppState) renderDashboardContent(chrome layout.PageChrome) string {
	// Ensure Dashboard is registered with router
	if m.dashboardModel == nil {
		cw, ch := m.contentDimensions()
		m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenDashboard, m.dashboardModel)
	}
	m.dashboardModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	if m.workflowEngine != nil {
		m.dashboardModel.SetWorkflowState(m.workflowPhase, m.workflowGoal, "", m.activeProvider)
	}
	return m.router.View()
}

func (m *AppState) renderSessionDetailContent(chrome layout.PageChrome) string {
	// Ensure SessionDetail is registered with router
	if m.sessionDetailModel == nil {
		m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenSessionDetail, m.sessionDetailModel)
	}
	m.sessionDetailModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderFileExplorerContent(chrome layout.PageChrome) string {
	// Ensure FileExplorer is registered with router
	if m.fileExplorerModel == nil {
		m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		if m.cwd != "" {
			root := buildFileTree(m.cwd, 0, 3)
			if root != nil {
				m.fileExplorerModel.SetRoot(root)
			}
		}
		m.router.Register(ScreenFileExplorer, m.fileExplorerModel)
	}
	m.fileExplorerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderToolDetailContent(chrome layout.PageChrome) string {
	// Ensure ToolDetail is registered with router
	if m.toolDetailModel == nil {
		cw, ch := m.contentDimensions()
		m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenToolDetail, m.toolDetailModel)
	}
	m.toolDetailModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderPhaseModelPickerContent(chrome layout.PageChrome) string {
	// Ensure PhaseModelPicker is registered with router
	if m.phaseModelPicker == nil {
		if m.registry == nil || len(m.registry.ListAll()) == 0 {
			return renderEmptyState("Model picker", "No providers configured", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenPhaseModelPicker, m.phaseModelPicker)
	}
	m.phaseModelPicker.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderGhostPickerContent(chrome layout.PageChrome) string {
	// Ensure GhostPicker is registered with router
	if m.ghostPickerModel == nil {
		m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenGhostPicker, m.ghostPickerModel)
	}
	m.ghostPickerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderGhostOutputContent(chrome layout.PageChrome) string {
	// Ensure GhostOutput is registered with router
	if m.ghostOutputModel == nil {
		m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenGhostOutput, m.ghostOutputModel)
	}
	m.ghostOutputModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderConfirmQuitContent(chrome layout.PageChrome) string {
	// Ensure ConfirmQuit is registered with router
	if m.confirmQuitModel == nil {
		m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenConfirmQuit, m.confirmQuitModel)
	}
	m.confirmQuitModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
	// Ensure ChatHistory is registered with router
	if m.chatHistoryModel == nil {
		m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenChatHistory, m.chatHistoryModel)
	}
	m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderCommandPaletteContent(chrome layout.PageChrome) string {
	// Ensure CommandPaletteScreen is registered with router
	if m.commandPaletteScreenModel == nil {
		m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		m.router.Register(ScreenCommandPalette, m.commandPaletteScreenModel)
	}
	m.commandPaletteScreenModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

func (m *AppState) renderHomeContent(chrome layout.PageChrome) string {
	// Ensure Home is registered with router
	if m.homeModel == nil {
		m.homeModel = NewHomeModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight(), m.version)
		m.homeModel.SetCommandRegistry(m.cmdRegistry)
		m.homeModel.SetConfig(m.config)
		m.router.Register(ScreenHome, m.homeModel)
	}
	m.homeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}

// clampPermissionModalWidth computes the clamped modal width from the
// configured permModalWidth and the current terminal width. Extracted to
// avoid duplicating the clamping logic in both render paths.
func (m *AppState) clampPermissionModalWidth() int {
	w := m.permModalWidth
	if w < MinModalWidth {
		w = MinModalWidth
	}
	if w > m.width-4 {
		w = m.width - 4
	}
	if w < 20 {
		w = 20
	}
	return w
}

// renderPermissionModalContent returns the modal card without centering,
// for use with RenderModalOverlay.
func (m *AppState) renderPermissionModalContent() string {
	if m.questionRequest != nil {
		return m.renderQuestionModal()
	}
	if m.permRequest == nil {
		return ""
	}

	modalWidth := m.clampPermissionModalWidth()

	if m.permModal != nil {
		return m.permModal.Render(modalWidth, 0)
	}

	t := m.themeManager.Current()
	return RenderPermissionModal(m.permRequest, m.permCountdown, modalWidth, m.width, m.height, t, m.permCountdown < 5, string(m.workflowPhase), m.workflowGoal)
}

// renderPermissionModal renders the permission or question overlay.
func (m *AppState) renderPermissionModal() string {
	if m.questionRequest != nil {
		return m.renderQuestionModal()
	}
	if m.permRequest == nil {
		// Use ViewContent (not View) to avoid duplicating the footer status bar.
		chrome := layout.PageChrome{Width: m.width, Height: m.height}
		m.ensureReplModel()
		m.syncReplSize(chrome)
		return m.replModel.ViewContent(chrome.ContentHeight(), chrome.ContentWidth())
	}

	modalWidth := m.clampPermissionModalWidth()

	if m.permModal != nil {
		return m.permModal.Render(m.width, m.height)
	}

	return RenderPermissionModal(m.permRequest, m.permCountdown, modalWidth, m.width, m.height, m.themeManager.Current(), m.permCountdown < 5, string(m.workflowPhase), m.workflowGoal)
}

// ─── REPL sync helpers ────────────────────────────────────────────────────────

// ensureReplModel creates the REPL model if not yet initialized.
func (m *AppState) ensureReplModel() {
	if m.replModel != nil {
		return
	}
	rm := NewReplModel(m.themeManager.Current(), m.version)
	m.replModel = &rm
	m.replModel.SetCommandRegistry(m.cmdRegistry)
	m.replModel.SetKeyRegistry(m.keyRegistry)
	m.replModel.SetFrecentHistory(m.frecentHistory)
	if m.cwd != "" {
		m.replModel.SetCwd(m.cwd)
	}
}

// syncReplSize ensures the REPL model dimensions match the content area.
func (m *AppState) syncReplSize(chrome layout.PageChrome) {
	if m.replModel == nil {
		return
	}
	cw := chrome.ContentWidth()
	ch := chrome.ContentHeight()
	if m.replModel.width != cw || m.replModel.height != ch {
		m.replModel.width = cw
		m.replModel.height = ch
		// cw already accounts for the sidebar; pass 0 to avoid double-subtracting.
		m.replModel.SetSidebarWidth(0)
	}
}

// syncReplProvider updates the REPL provider/model reference and returns
// a tea.Cmd that fetches models.
func (m *AppState) syncReplProvider(sessionID string) tea.Cmd {
	if m.replModel == nil {
		return nil
	}
	return m.replModel.SetProvider(m.shutdownCtx, m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
}

// updateSidebarUsage pushes token usage data from the REPL to the sidebar.
func (m *AppState) updateSidebarUsage() {
	if m.sidebarModel == nil || m.replModel == nil {
		return
	}
	var tokens, ctxLen int
	var cost float64
	var showCost bool
	var modelName string
	if m.replModel.lastUsage != nil {
		tokens = m.replModel.lastUsage.TotalTokens
	}
	if m.replModel.activeModel != nil {
		ctxLen = int(m.replModel.activeModel.ContextLength)
		modelName = m.replModel.activeModel.Name
		if modelName == "" {
			modelName = m.replModel.activeModel.ID
		}
	}
	cost = m.replModel.lastCost
	showCost = m.replModel.cfg != nil && m.replModel.cfg.UI.ShowCostEstimate
	m.sidebarModel.SetTokenUsage(tokens, ctxLen, cost, showCost, modelName)
	// Update new sidebar metrics
	m.sidebarModel.UpdateTokenBurn()
	m.sidebarModel.UpdateContextPressure()
	m.sidebarModel.UpdateCostAccumulator()
	m.sidebarModel.UpdateExecutionMetrics()
}

// renderQuestionModal renders the AskUserQuestion overlay using components.QuestionModel.
func (m *AppState) renderQuestionModal() string {
	q := m.questionRequest
	if q == nil {
		return ""
	}

	// Scale modal width: 2/3 of terminal, between 30 and 80 cols
	width := m.width * 2 / 3
	if width < 30 {
		width = 30
	}
	if width > 80 {
		width = 80
	}
	if width > m.width-4 {
		width = m.width - 4
	}

	if m.questionModel != nil {
		m.questionModel.SetWidth(width)
		content := m.questionModel.View()
		if q.TimeoutSecs > 0 {
			timeoutLine := lipgloss.NewStyle().
				Foreground(m.themeManager.Current().TextMuted).
				Italic(true).
				Render(fmt.Sprintf("Timeout: %ds", q.TimeoutSecs))
			content = content + "\n" + timeoutLine
		}
		modal := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.themeManager.Current().Brand).
			Background(m.themeManager.Current().SurfaceElevated).
			Padding(1, 2).
			Width(width).
			Render(content)
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, modal)
	}

	t := m.themeManager.Current()
	bodyContent := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.Text).Render(q.Question),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("Type your answer and press ↵"),
	)
	card := components.Card{
		Title:   q.Header,
		Content: bodyContent,
		Width:   width,
		Border:  lipgloss.RoundedBorder(),
		Style:   components.CardBrand,
		Theme:   t,
	}.Render()
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, card)
}

// RenderPermissionModal renders a full-screen permission modal.
func RenderPermissionModal(req *tools.PermissionRequest, countdown, width, termW, termH int, t theme.Theme, urgent bool, phase, goal string) string {
	if req == nil {
		return ""
	}

	// Generate plain English description
	desc := components.GenerateDescription(*req, phase, goal)

	// ── Title: "M31A wants to [action]" ──────────────────────────────────
	actionText := desc.Action
	if actionText == "" {
		actionText = "use a tool"
	}
	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(t.Primary)
	titleLine := titleStyle.Render(fmt.Sprintf("M31A wants to %s", actionText))

	// ── Consequence explanation ───────────────────────────────────────────
	var consequenceLine string
	if desc.Consequence != "" {
		consequenceLine = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(desc.Consequence)
	}

	// ── Target (if different from action) ─────────────────────────────────
	var targetLine string
	if desc.Target != "" {
		targetLine = lipgloss.NewStyle().Foreground(t.TextMuted).Render("Target: " + desc.Target)
	}

	// ── Command box (secondary, below divider) ───────────────────────────
	cmdContentW := width - 10
	if cmdContentW < 8 {
		cmdContentW = 8
	}
	highlighted := components.HighlightCommand(req.Command, t)
	highlighted = components.TruncateWithEllipsis(highlighted, cmdContentW)
	cmdBox := lipgloss.NewStyle().
		Background(t.SurfaceElevated).
		Foreground(t.TextPrimary).
		Padding(1).
		Width(width - 6).
		Render(highlighted)

	// ── Keybindings ───────────────────────────────────────────────────────
	keyStyle := lipgloss.NewStyle().Bold(true).Foreground(t.Primary)
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	keys := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.JoinHorizontal(lipgloss.Top,
			keyStyle.Render("[Y]"),
			hintStyle.Render(" Allow once      "),
			keyStyle.Render("[A]"),
			hintStyle.Render(" Always allow"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			keyStyle.Render("[B]"),
			hintStyle.Render(" Approve all     "),
			keyStyle.Render("[N]"),
			hintStyle.Render(" Deny"),
		),
		lipgloss.JoinHorizontal(lipgloss.Top,
			keyStyle.Render("[Esc]"),
			hintStyle.Render(" Deny (safe default)"),
		),
	)

	// ── Countdown (only when <5 seconds remaining) ────────────────────────
	var countdownLine string
	if countdown <= 0 {
		countdownLine = lipgloss.NewStyle().Foreground(t.Error).Render("Auto-deny: tool will be rejected")
	} else if countdown <= 5 {
		countdownLine = lipgloss.NewStyle().Foreground(t.Warning).Render(fmt.Sprintf("Auto-deny in %ds", countdown))
	}
	// No countdown shown when >5 seconds (cleaner UI)

	// ── Rule context ──────────────────────────────────────────────────────
	var ruleLine string
	if req.RuleTool != "" || req.RulePattern != "" {
		ruleLine = lipgloss.NewStyle().Foreground(t.TextMuted).Render(
			fmt.Sprintf("Matched rule: tool=%q pattern=%q action=%q",
				req.RuleTool, req.RulePattern, req.RuleAction),
		)
	}

	// ── Assemble ──────────────────────────────────────────────────────────
	bodyLines := []string{
		titleLine,
	}
	if consequenceLine != "" {
		bodyLines = append(bodyLines, consequenceLine)
	}
	if targetLine != "" {
		bodyLines = append(bodyLines, targetLine)
	}
	bodyLines = append(bodyLines,
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("Command"),
		cmdBox,
	)
	if ruleLine != "" {
		bodyLines = append(bodyLines, "", ruleLine)
	}
	bodyLines = append(bodyLines,
		"",
		keys,
	)
	if countdownLine != "" {
		bodyLines = append(bodyLines, "", countdownLine)
	}

	bodyContent := lipgloss.JoinVertical(lipgloss.Left, bodyLines...)

	// Risk communicated via border color accent
	borderStyle := lipgloss.RoundedBorder()
	borderColor := t.BorderSubtle
	switch req.RiskLevel {
	case types.RiskDangerous:
		borderColor = t.Warning
	case types.RiskDestructive:
		borderColor = t.Error
	}

	card := lipgloss.NewStyle().
		Border(borderStyle).
		BorderForeground(borderColor).
		Width(width).
		Padding(1).
		Render(bodyContent)

	if urgent {
		card = lipgloss.NewStyle().
			Border(borderStyle).
			BorderForeground(t.Error).
			Width(width).
			Padding(1).
			Render(bodyContent)
		return lipgloss.Place(termW, termH, lipgloss.Center, lipgloss.Center, card)
	}

	return lipgloss.Place(termW, termH, lipgloss.Center, lipgloss.Center, card)
}

func (m *AppState) renderDecisionsContent(chrome layout.PageChrome) string {
	// Ensure DecisionScreen is registered with router
	if m.decisionScreen == nil {
		cw, ch := m.contentDimensions()
		m.decisionScreen = NewDecisionScreen(m.themeManager.Current(), cw, ch)
	}
	m.decisionScreen.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	m.decisionScreen.SetDecisions(m.cachedDecisions)
	if m.router != nil {
		m.router.Register(ScreenDecisions, m.decisionScreen)
		return m.router.View()
	}
	return m.decisionScreen.View()
}

// renderPhaseTransitionContent renders the phase transition confirmation screen.
func (m *AppState) renderPhaseTransitionContent(chrome layout.PageChrome) string {
	if m.phaseTransitionModel == nil {
		return ""
	}
	m.phaseTransitionModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.phaseTransitionModel.View()
}
