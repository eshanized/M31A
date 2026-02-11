package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ─── AppState view rendering ──────────────────────────────────────────────────

// View implements tea.Model. It renders the full terminal frame using the
// unified PageLayout system: 1-line header + content viewport + 1-line footer.
func (m *AppState) View() string {
	if m.width == 0 || m.height == 0 {
		return lipgloss.Place(80, 24, lipgloss.Center, lipgloss.Center,
			lipgloss.NewStyle().Bold(true).Render("M31A"))
	}

	t := m.themeManager.Current()

	// Command palette overlay (rendered on top of everything)
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		return m.cmdPalette.View()
	}

	// Permission/question modal (top priority overlay)
	if m.screen == ScreenPermission {
		return m.renderPermissionModal()
	}

	// Minimum screen guard: refuse to render below 40 cols
	bp := layout.Detect(m.width)
	if bp == layout.UltraNarrow {
		return layout.RenderTooNarrow(m.width, m.height, t)
	}

	// Build unified chrome info
	headerInfo := m.buildHeaderInfo()
	footerInfo := m.buildFooterInfo()

	// Sidebar composition
	m.ensureSidebarModel()
	sidebarVisible := m.sidebarModel != nil && m.sidebarModel.IsVisible()
	hasSidebar := sidebarVisible && layout.ShowSidebar(m.width)
	sidebarOverlay := sidebarVisible && !hasSidebar // narrow terminal, overlay mode
	sidebarStr := ""

	contentWidth := m.width
	if hasSidebar {
		m.sidebarModel.SetHeight(m.height)
		sidebarStr = m.sidebarModel.View()
		contentWidth = m.width - m.sidebarModel.GetWidth()
	}

	// PageChrome defines the dimensions for the unified layout
	chrome := layout.PageChrome{
		Width:  contentWidth,
		Height: m.height,
	}

	// Render active screen content (content-only, no chrome)
	content := m.renderActiveScreen(chrome)

	// Toast overlay (rendered inside content area, top-right)
	if len(m.toasts) > 0 {
		toastOverlay := ""
		if m.width >= WidthCompact {
			toastOverlay = renderToastStack(m.toasts, t, contentWidth)
		} else {
			last := m.toasts[len(m.toasts)-1]
			toastOverlay = renderSingleToast(last, t, 0)
		}
		if toastOverlay != "" {
			content = overlayToastOnContent(content, toastOverlay, contentWidth)
		}
	}

	// Screen transition overlay
	if m.transition != nil && m.transition.Active {
		overlay := m.transition.renderTransitionOverlay(t, contentWidth, chrome.ContentHeight())
		content = overlay
	}

	// Compose the full page
	main := layout.RenderPage(chrome, content, headerInfo, footerInfo, t)

	if hasSidebar {
		return lipgloss.JoinHorizontal(lipgloss.Top, sidebarStr, main)
	}

	// Sidebar overlay mode: render sidebar on top of the page (narrow terminals)
	if sidebarOverlay {
		m.sidebarModel.SetHeight(m.height)
		overlayContent := m.sidebarModel.View()
		overlayW := m.sidebarModel.GetWidth()
		if overlayW > m.width-10 {
			overlayW = m.width - 10
		}
		return layout.RenderOverlay(main, overlayContent, m.width, m.height, overlayW, t)
	}

	return main
}

// buildHeaderInfo constructs the unified header data from AppState.
func (m *AppState) buildHeaderInfo() layout.HeaderInfo {
	info := layout.HeaderInfo{
		Brand: "M31A",
	}

	// Breadcrumb: screen label, or phase breadcrumb, or git branch
	if m.workflowPhase != types.PhaseIdle && m.workflowPhase != "" {
		info.Breadcrumb = string(m.workflowPhase)
	} else {
		info.Breadcrumb = m.screen.Label()
	}

	if m.activeModel != nil {
		info.ModelName = m.activeModel.Name
		if info.ModelName == "" {
			info.ModelName = m.activeModel.ID
		}
	}
	info.Provider = m.activeProvider

	// When per-phase model overrides are active, show a compact badge
	// indicating which models handle Planning vs Coding work.
	if m.planningModelID != "" || m.codingModelID != "" {
		pName := m.planningModelID
		cName := m.codingModelID
		if pName == "" {
			pName = "default"
		}
		if cName == "" {
			cName = "default"
		}
		// Truncate long model IDs to keep the header readable.
		if len(pName) > 20 {
			pName = pName[len(pName)-20:]
		}
		if len(cName) > 20 {
			cName = cName[len(cName)-20:]
		}
		info.ModelName = "P:" + pName + "  C:" + cName
	}

	return info
}


// buildFooterInfo constructs the unified footer data from AppState.
func (m *AppState) buildFooterInfo() layout.FooterInfo {
	info := layout.FooterInfo{}

	// Working directory
	if m.replModel != nil && m.replModel.cwd != "" {
		info.Cwd = pathBase(m.replModel.cwd)
	}

	// Git branch
	if m.sidebarModel != nil && m.sidebarModel.branch != "" {
		info.GitBranch = m.sidebarModel.branch
	}

	// Operation status
	if m.replModel != nil {
		if m.replModel.thinking {
			var dur int64
			if !m.replModel.thinkingStartAt.IsZero() {
				dur = time.Since(m.replModel.thinkingStartAt).Milliseconds()
			}
			if dur > 0 {
				info.Operation = "thinking " + formatDurationMs(dur)
			} else {
				info.Operation = "thinking..."
			}
		} else if m.replModel.streaming {
			info.Operation = "responding..."
		}
		info.SpinnerFrame = m.replModel.spinner.Peek()
	}

	if m.workflowPhase != types.PhaseIdle && m.workflowPhase != "" {
		info.Operation = string(m.workflowPhase)
	}

	// Leader key
	if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
		info.LeaderActive = true
	}

	// Keyboard hints
	info.KeyboardHints = []string{"ctrl+p cmds", "ctrl+b sidebar", "ctrl+x leader"}
	if m.replModel != nil && (m.replModel.streaming || m.replModel.thinking) {
		info.KeyboardHints = append([]string{"ctrl+c cancel"}, info.KeyboardHints...)
	}

	// Cost
	if m.replModel != nil && m.replModel.lastUsage != nil &&
		m.replModel.cfg != nil && m.replModel.cfg.UI.ShowCostEstimate {
		info.TokenCount = m.replModel.lastUsage.TotalTokens
		info.Cost = m.replModel.lastCost
		info.ShowCost = true
	}

	return info
}

// overlayToastOnContent places toast notifications on the top-right of content.
func overlayToastOnContent(content, toast string, width int) string {
	contentLines := strings.Split(content, "\n")
	toastLines := strings.Split(toast, "\n")

	for i, tl := range toastLines {
		if i >= len(contentLines) {
			break
		}
		toastW := lipgloss.Width(tl)
		contentW := lipgloss.Width(contentLines[i])
		if contentW > width-toastW-1 {
			// Overlay toast on right side of content line
			base := contentLines[i]
			// Truncate base to make room for toast
			baseTruncated := truncateToVisibleWidth(base, width-toastW-1)
			padding := width - lipgloss.Width(baseTruncated) - toastW
			if padding < 0 {
				padding = 0
			}
			contentLines[i] = baseTruncated + strings.Repeat(" ", padding) + tl
		}
	}

	return strings.Join(contentLines, "\n")
}

// truncateToVisibleWidth truncates a styled string to max visible cells.
func truncateToVisibleWidth(s string, maxW int) string {
	w := lipgloss.Width(s)
	if w <= maxW {
		return s
	}
	// Simple truncation — for styled text we use the layout package helper
	return s // TODO: use layout.TruncateToWidth when exported
}

// renderActiveScreen delegates to the active screen's content renderer.
// Each screen returns ONLY its content area — no header, footer, or chrome.
func (m *AppState) renderActiveScreen(chrome layout.PageChrome) string {
	switch m.screen {
	case ScreenREPL:
		return m.renderREPLContent(chrome)
	case ScreenSettings:
		return m.renderSettingsContent(chrome)
	case ScreenModelSelector:
		return m.renderModelSelectorContent(chrome)
	case ScreenPlan:
		return m.renderPlanContent(chrome)
	case ScreenExecute:
		return m.renderExecuteContent(chrome)
	case ScreenVerify:
		return m.renderVerifyContent(chrome)
	case ScreenShip:
		return m.renderShipContent(chrome)
	case ScreenResume:
		return m.renderResumeContent(chrome)
	case ScreenGoalInput:
		return m.renderGoalInputContent(chrome)
	case ScreenFirstRun:
		return m.renderFirstRunContent(chrome)
	case ScreenLedger:
		return m.renderLedgerContent(chrome)
	case ScreenRollback:
		return m.renderRollbackContent(chrome)
	case ScreenMetrics:
		return m.renderMetricsContent(chrome)
	case ScreenDiscuss:
		return m.renderDiscussContent(chrome)
	case ScreenConfig:
		return m.renderConfigContent(chrome)
	case ScreenDiff:
		return m.renderDiffContent(chrome)
	case ScreenHelp:
		return m.renderHelpContent(chrome)
	case ScreenBisect:
		return m.renderBisectContent(chrome)
	case ScreenThemePicker:
		return m.renderThemePickerContent(chrome)
	case ScreenNotifications:
		return m.renderNotificationsContent(chrome)
	case ScreenDashboard:
		return m.renderDashboardContent(chrome)
	case ScreenSessionDetail:
		return m.renderSessionDetailContent(chrome)
	case ScreenFileExplorer:
		return m.renderFileExplorerContent(chrome)
	case ScreenToolDetail:
		return m.renderToolDetailContent(chrome)
	case ScreenPhaseModelPicker:
		return m.renderPhaseModelPickerContent(chrome)
	default:
		return m.renderREPLContent(chrome)
	}
}

// ─── Per-screen content renderers ─────────────────────────────────────────────
// Each renderer returns ONLY the content area. No header, footer, or chrome.

func (m *AppState) renderREPLContent(chrome layout.PageChrome) string {
	m.ensureReplModel()
	m.syncReplSize(chrome)
	return m.replModel.ViewContent(chrome.ContentHeight(), chrome.ContentWidth())
}

func (m *AppState) renderSettingsContent(chrome layout.PageChrome) string {
	if m.settingsModel == nil {
		m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.shutdownCtx)
	}
	m.settingsModel.width = chrome.ContentWidth()
	m.settingsModel.height = chrome.ContentHeight()
	return m.settingsModel.View()
}

func (m *AppState) renderModelSelectorContent(chrome layout.PageChrome) string {
	if m.msModel == nil {
		return renderLoading("Loading model selector...", m.themeManager.Current())
	}
	return m.msModel.View()
}

func (m *AppState) renderPlanContent(chrome layout.PageChrome) string {
	if m.planModel == nil {
		return renderLoading("Loading plan...", m.themeManager.Current())
	}
	return m.planModel.View()
}

func (m *AppState) renderExecuteContent(chrome layout.PageChrome) string {
	if m.executeModel == nil {
		return renderLoading("Loading execution...", m.themeManager.Current())
	}
	return m.executeModel.View()
}

func (m *AppState) renderVerifyContent(chrome layout.PageChrome) string {
	if m.verifyModel == nil {
		return renderLoading("Loading verification...", m.themeManager.Current())
	}
	return m.verifyModel.View()
}

func (m *AppState) renderShipContent(chrome layout.PageChrome) string {
	if m.shipModel == nil {
		return renderLoading("Loading ship summary...", m.themeManager.Current())
	}
	return m.shipModel.View()
}

func (m *AppState) renderResumeContent(chrome layout.PageChrome) string {
	if m.resumeModel == nil {
		return renderLoading("Loading sessions...", m.themeManager.Current())
	}
	return m.resumeModel.View()
}

func (m *AppState) renderGoalInputContent(chrome layout.PageChrome) string {
	if m.goalInput == nil {
		return renderLoading("Loading goal input...", m.themeManager.Current())
	}
	return m.goalInput.View()
}

func (m *AppState) renderFirstRunContent(chrome layout.PageChrome) string {
	if m.firstRunModel == nil {
		return renderLoading("Loading first-run wizard...", m.themeManager.Current())
	}
	m.firstRunModel.SetContentWidth(chrome.ContentWidth())
	return m.firstRunModel.View()
}

func (m *AppState) renderLedgerContent(chrome layout.PageChrome) string {
	if m.ledgerModel == nil {
		return renderLoading("Loading ledger...", m.themeManager.Current())
	}
	return m.ledgerModel.View()
}

func (m *AppState) renderRollbackContent(chrome layout.PageChrome) string {
	if m.rollbackModel == nil {
		return renderLoading("Loading rollback browser...", m.themeManager.Current())
	}
	return m.rollbackModel.View()
}

func (m *AppState) renderMetricsContent(chrome layout.PageChrome) string {
	if m.metricsModel == nil {
		return renderLoading("Loading metrics...", m.themeManager.Current())
	}
	return m.metricsModel.View()
}

func (m *AppState) renderDiscussContent(chrome layout.PageChrome) string {
	if m.discussModel == nil {
		return renderLoading("Loading discuss...", m.themeManager.Current())
	}
	return m.discussModel.View()
}

func (m *AppState) renderConfigContent(chrome layout.PageChrome) string {
	if m.configModel == nil {
		m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, chrome.ContentWidth(), chrome.ContentHeight())
	} else {
		m.configModel.buildContent()
	}
	return m.configModel.View()
}

func (m *AppState) renderDiffContent(chrome layout.PageChrome) string {
	if m.diffModel == nil {
		return "Loading diff..."
	}
	return m.diffModel.View()
}

func (m *AppState) renderHelpContent(chrome layout.PageChrome) string {
	if m.helpModel == nil {
		return renderLoading("Loading help...", m.themeManager.Current())
	}
	return m.helpModel.View()
}

func (m *AppState) renderBisectContent(chrome layout.PageChrome) string {
	if m.bisectModel == nil {
		return renderLoading("Loading bisect...", m.themeManager.Current())
	}
	return m.bisectModel.View()
}

func (m *AppState) renderThemePickerContent(chrome layout.PageChrome) string {
	if m.themePickerModel == nil {
		return renderLoading("Loading themes...", m.themeManager.Current())
	}
	return m.themePickerModel.View()
}

func (m *AppState) renderNotificationsContent(chrome layout.PageChrome) string {
	if m.notifModel == nil {
		return renderLoading("Loading notifications...", m.themeManager.Current())
	}
	return m.notifModel.View()
}

func (m *AppState) renderDashboardContent(chrome layout.PageChrome) string {
	if m.dashboardModel == nil {
		return renderLoading("Loading dashboard...", m.themeManager.Current())
	}
	return m.dashboardModel.View()
}

func (m *AppState) renderSessionDetailContent(chrome layout.PageChrome) string {
	if m.sessionDetailModel == nil {
		return renderLoading("Loading session...", m.themeManager.Current())
	}
	return m.sessionDetailModel.View()
}

func (m *AppState) renderFileExplorerContent(chrome layout.PageChrome) string {
	if m.fileExplorerModel == nil {
		return renderLoading("Loading files...", m.themeManager.Current())
	}
	return m.fileExplorerModel.View()
}

func (m *AppState) renderToolDetailContent(chrome layout.PageChrome) string {
	if m.toolDetailModel == nil {
		return renderLoading("Loading tool output...", m.themeManager.Current())
	}
	return m.toolDetailModel.View()
}

func (m *AppState) renderPhaseModelPickerContent(chrome layout.PageChrome) string {
	if m.phaseModelPicker == nil {
		return renderLoading("Loading model picker...", m.themeManager.Current())
	}
	return m.phaseModelPicker.View()
}

// renderPermissionModal renders the permission or question overlay.
func (m *AppState) renderPermissionModal() string {
	if m.questionRequest != nil {
		return m.renderQuestionModal()
	}
	if m.permRequest == nil {
		return m.renderREPLContent(layout.PageChrome{Width: m.width, Height: m.height})
	}

	if m.permModal != nil {
		return m.permModal.Render(m.width, m.height)
	}

	return RenderPermissionModal(m.permRequest, m.permCountdown, m.permModalWidth, m.themeManager.Current(), m.permCountdown < 5)
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
}

// syncReplSize ensures the REPL model dimensions match the content area.
func (m *AppState) syncReplSize(chrome layout.PageChrome) {
	if m.replModel == nil {
		return
	}
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
		sw = m.sidebarModel.GetWidth()
	}
	cw := chrome.ContentWidth()
	ch := chrome.ContentHeight()
	if m.replModel.width != cw || m.replModel.height != ch {
		m.replModel.width = cw
		m.replModel.height = ch
		m.replModel.SetSidebarWidth(sw)
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

// ─── Render helpers ───────────────────────────────────────────────────────────

// renderQuestionModal renders the AskUserQuestion overlay using components.QuestionModel.
func (m *AppState) renderQuestionModal() string {
	q := m.questionRequest
	if q == nil {
		return ""
	}
	width := m.permModalWidth
	if width < 40 {
		width = 60
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

// ─── Utility ──────────────────────────────────────────────────────────────────

// RenderPermissionModal renders a full-screen permission modal.
func RenderPermissionModal(req *tools.PermissionRequest, countdown, width int, t theme.Theme, urgent bool) string {
	if req == nil {
		return ""
	}

	riskStyle := lipgloss.NewStyle().Foreground(t.Warning)
	if req.RiskLevel == types.RiskDestructive {
		riskStyle = lipgloss.NewStyle().Foreground(t.Error)
	}

	bodyContent := lipgloss.JoinVertical(lipgloss.Left,
		"  Tool:  "+req.ToolName,
		"  Command:",
		lipgloss.NewStyle().PaddingLeft(4).MaxWidth(width-4).Render(req.Command),
		"  Risk:  "+riskStyle.Render(string(req.RiskLevel)),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  y/↵ allow   n/esc deny   a allow always"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  Timeout: "+formatSI(countdown)+"s"),
	)

	borderStyle := lipgloss.RoundedBorder()

	card := components.Card{
		Title:   "Permission Required",
		Content: bodyContent,
		Width:   width,
		Border:  borderStyle,
		Style:   components.CardBrand,
		Theme:   t,
	}.Render()

	if urgent {
		card = lipgloss.NewStyle().
			Border(borderStyle).
			BorderForeground(t.Error).
			Width(width).
			Render(bodyContent)
		card = lipgloss.Place(0, 0, lipgloss.Center, lipgloss.Center, card)
		return card
	}

	return lipgloss.Place(0, 0, lipgloss.Center, lipgloss.Center, card)
}
