package tui

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/decision"
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
		// Dimensions not yet known — emit empty frame; bubbletea will repaint
		// on first WindowSizeMsg.
		return ""
	}

	// Visual screen transition: composite old → new frame with slide/fade
	if m.transition != nil && m.transition.Active {
		// Render the new (target) screen frame without mutating m.screen.
		// We pass the target screen to renderFrameForScreen which delegates
		// to the appropriate content renderer without touching m.screen.
		prevFrame := m.transition.PrevFrame
		nextFrame := m.renderFrameForScreen(m.transition.ToScreen)

		progress := m.transition.Progress()
		t := m.themeManager.Current()
		return RenderTransition(prevFrame, nextFrame, progress, m.transition.Type, m.width, m.height, t)
	}

	t := m.themeManager.Current()

	return m.renderFrameWithTheme(t)
}

// renderFrameForScreen renders the full frame as if `targetScreen` were active,
// without mutating m.screen. Used by the transition system to render the target
// screen while preserving Elm architecture purity.
func (m *AppState) renderFrameForScreen(targetScreen Screen) string {
	t := m.themeManager.Current()
	headerInfo := m.buildHeaderInfo()
	footerInfo := m.buildFooterInfo()

	chrome, sidebarStr := m.buildSidebarAndChrome(targetScreen)
	content := m.renderScreenContent(targetScreen, chrome)

	// Toast overlay
	if len(m.toasts) > 0 {
		toastOverlay := ""
		if m.width >= WidthCompact {
			toastOverlay = renderToastStack(m.toasts, t, m.width)
		} else {
			last := m.toasts[len(m.toasts)-1]
			toastOverlay = renderSingleToast(last, t, 0, m.width)
		}
		if toastOverlay != "" {
			content = overlayToastOnContent(content, toastOverlay, chrome.Width)
		}
	}

	main := layout.RenderPage(chrome, content, headerInfo, footerInfo, t, m.themeManager.Cache())
	return m.applySidebar(main, sidebarStr, targetScreen)
}

// buildSidebarAndChrome computes sidebar state and returns the PageChrome dimensions.
// Used by both renderFrameWithTheme and renderFrameForScreen to avoid duplication.
func (m *AppState) buildSidebarAndChrome(screen Screen) (layout.PageChrome, string) {
	m.ensureSidebarModel()
	sidebarVisible := m.sidebarModel != nil && m.sidebarModel.IsVisible()
	hasSidebar := sidebarVisible && layout.ShowSidebar(m.width) && screen != ScreenFirstRun
	sidebarStr := ""

	contentWidth := m.width
	if hasSidebar {
		m.sidebarModel.SetHeight(m.height)
		m.sidebarModel.SetCurrentScreen(screen.Name())
		sidebarStr = m.sidebarModel.View()
		contentWidth = m.width - m.sidebarModel.GetWidth()
	}

	return layout.PageChrome{Width: contentWidth, Height: m.height}, sidebarStr
}

// applySidebar composes the main content with the sidebar (or sidebar overlay).
func (m *AppState) applySidebar(main, sidebarStr string, screen Screen) string {
	if sidebarStr != "" {
		return lipgloss.JoinHorizontal(lipgloss.Top, sidebarStr, main)
	}

	m.ensureSidebarModel()
	sidebarVisible := m.sidebarModel != nil && m.sidebarModel.IsVisible()
	sidebarOverlay := sidebarVisible && !layout.ShowSidebar(m.width) && screen != ScreenFirstRun
	if sidebarOverlay {
		m.sidebarModel.SetHeight(m.height)
		overlayContent := m.sidebarModel.View()
		overlayW := m.sidebarModel.GetWidth()
		if overlayW > m.width-10 {
			overlayW = m.width - 10
		}
		t := m.themeManager.Current()
		return layout.RenderOverlay(main, overlayContent, m.width, m.height, overlayW, t)
	}

	return main
}

// renderScreenContent returns the content for a given screen without touching m.screen.
func (m *AppState) renderScreenContent(screen Screen, chrome layout.PageChrome) string {
	switch screen {
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
	case ScreenRuntimeCheck:
		return m.renderRuntimeContent(chrome)
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
	case ScreenGhostPicker:
		return m.renderGhostPickerContent(chrome)
	case ScreenGhostOutput:
		return m.renderGhostOutputContent(chrome)
	case ScreenConfirmQuit:
		return m.renderConfirmQuitContent(chrome)
	case ScreenChatHistory:
		return m.renderChatHistoryContent(chrome)
	case ScreenCommandPalette:
		return m.renderCommandPaletteContent(chrome)
	case ScreenHome:
		return m.renderHomeContent(chrome)
	case ScreenDecisions:
		return m.renderDecisionsContent(chrome)
	default:
		return m.renderREPLContent(chrome)
	}
}

// renderFrame renders the full frame for the current screen.
func (m *AppState) renderFrame() string {
	t := m.themeManager.Current()
	return m.renderFrameWithTheme(t)
}

// renderDimmedModal renders a modal overlay on top of a dimmed REPL background.
// Returns empty string if the REPL background cannot be rendered.
func (m *AppState) renderDimmedModal(modalContent string, t theme.Theme) string {
	bgFrame := ""
	if m.replModel != nil {
		chrome := layout.PageChrome{Width: m.width, Height: m.height}
		m.ensureReplModel()
		m.syncReplSize(chrome)
		bgFrame = m.replModel.ViewContent(chrome.ContentHeight(), chrome.ContentWidth())
	}
	if bgFrame == "" {
		return ""
	}
	return layout.RenderModalOverlay(bgFrame, modalContent, m.width, m.height, t)
}

// renderFrameWithTheme renders the full frame with the given theme.
func (m *AppState) renderFrameWithTheme(t theme.Theme) string {
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		return m.cmdPalette.View()
	}

	// Permission/question modal (top priority overlay)
	if m.screen == ScreenPermission {
		modalContent := m.renderPermissionModalContent()
		if result := m.renderDimmedModal(modalContent, t); result != "" {
			return result
		}
		return m.renderPermissionModal()
	}

	// Model Selector as centered dialog overlay on dimmed REPL background
	if m.screen == ScreenModelSelector && m.msModel != nil {
		modalW := m.width * 4 / 5
		modalH := m.height * 3 / 4
		if modalW < 40 {
			modalW = 40
		}
		if modalH < 10 {
			modalH = 10
		}
		if modalW > m.width-4 {
			modalW = m.width - 4
		}
		m.msModel.SetDimensions(modalW-4, modalH-4)
		modalContent := m.msModel.View()
		modal := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(t.Brand).
			Background(t.SurfaceElevated).
			Width(modalW - 2).
			MaxHeight(modalH).
			Render(modalContent)
		if result := m.renderDimmedModal(modal, t); result != "" {
			return result
		}
	}

	// Minimum screen guard: refuse to render below 40 cols or below 10 rows
	bp := layout.Detect(m.width)
	if bp == layout.UltraNarrow {
		return layout.RenderTooNarrow(m.width, m.height, t)
	}
	if m.height < 10 {
		return layout.RenderTooNarrow(m.width, m.height, t)
	}

	// Build unified chrome info
	headerInfo := m.buildHeaderInfo()
	footerInfo := m.buildFooterInfo()

	chrome, sidebarStr := m.buildSidebarAndChrome(m.screen)

	// Render active screen content (content-only, no chrome)
	content := m.renderActiveScreen(chrome)

	// Toast overlay (rendered inside content area, top-right)
	if len(m.toasts) > 0 {
		toastOverlay := ""
		if m.width >= WidthCompact {
			toastOverlay = renderToastStack(m.toasts, t, m.width)
		} else {
			last := m.toasts[len(m.toasts)-1]
			toastOverlay = renderSingleToast(last, t, 0, m.width)
		}
		if toastOverlay != "" {
			content = overlayToastOnContent(content, toastOverlay, chrome.Width)
		}
	}

	// Compose the full page
	main := layout.RenderPage(chrome, content, headerInfo, footerInfo, t, m.themeManager.Cache())

	return m.applySidebar(main, sidebarStr, m.screen)
}

// buildHeaderInfo constructs the unified header data from AppState.
func (m *AppState) buildHeaderInfo() layout.HeaderInfo {
	info := layout.HeaderInfo{
		Brand: "M31A",
	}

	// Build breadcrumb from screenStack for navigation path display.
	t := m.themeManager.Current()
	bc := components.Breadcrumb{Theme: t, Width: m.width}
	breadcrumbItems := components.BuildFromStack(m.screenStack, m.screen)
	var breadcrumbParts []string
	for _, item := range breadcrumbItems {
		breadcrumbParts = append(breadcrumbParts, item.Label)
	}
	bc.Parts = breadcrumbParts
	breadcrumbStr := bc.View()

	// Fallback to workflow phase or screen label if breadcrumb is empty
	if breadcrumbStr == "" {
		if m.workflowPhase != types.PhaseIdle && m.workflowPhase != "" {
			info.Breadcrumb = string(m.workflowPhase)
		} else {
			info.Breadcrumb = m.screen.Label()
		}
	} else {
		info.Breadcrumb = breadcrumbStr
	}

	if m.activeModel != nil {
		info.ModelName = m.activeModel.Name
		if info.ModelName == "" {
			info.ModelName = m.activeModel.ID
		}
	}
	info.Provider = m.activeProvider

	// Context usage: last response's tokens vs. model's context window.
	if m.replModel != nil && m.replModel.lastUsage != nil {
		info.CtxUsed = m.replModel.lastUsage.TotalTokens
		if m.replModel.activeModel != nil && m.replModel.activeModel.ContextLength > 0 {
			info.CtxTotal = int(m.replModel.activeModel.ContextLength)
		}
	}

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
		info.Cwd = filepath.Base(m.replModel.cwd)
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
				info.Operation = "thinking…"
			}
		} else if m.replModel.streaming {
			info.Operation = "streaming…"
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
	info.KeyboardHints = []string{"ctrl+p cmds", "ctrl+b sidebar"}
	switch m.screen {
	case ScreenSettings:
		info.KeyboardHints = []string{"s save global", "L save local", "q back"}
	case ScreenPlan:
		info.KeyboardHints = []string{"enter review", "r refine", "esc back"}
	case ScreenExecute:
		info.KeyboardHints = []string{"j/k scroll", "p pause", "ctrl+c cancel", "esc back"}
	case ScreenVerify:
		info.KeyboardHints = []string{"h heal", "enter/s ship", "esc back"}
	case ScreenRuntimeCheck:
		info.KeyboardHints = []string{"j/k scroll", "enter continue", "esc back"}
	case ScreenShip:
		info.KeyboardHints = []string{"enter confirm", "esc back"}
	case ScreenModelSelector:
		info.KeyboardHints = []string{"tab cycle", "enter select", "esc back"}
	case ScreenResume:
		info.KeyboardHints = []string{"enter restore", "esc back"}
	case ScreenDiscuss:
		info.KeyboardHints = []string{"enter submit", "esc skip", "Ctrl+S skip all"}
	case ScreenHelp:
		info.KeyboardHints = []string{"g top", "G bottom", "esc/q back"}
	case ScreenLedger:
		info.KeyboardHints = []string{"j/k scroll", "esc back"}
	case ScreenRollback:
		info.KeyboardHints = []string{"j/k scroll", "enter restore", "esc back"}
	case ScreenMetrics:
		info.KeyboardHints = []string{"j/k scroll", "esc back"}
	case ScreenConfig:
		info.KeyboardHints = []string{"j/k scroll", "esc back"}
	case ScreenBisect:
		info.KeyboardHints = []string{"g good", "b bad", "s skip", "esc back"}
	case ScreenNotifications:
		info.KeyboardHints = []string{"j/k scroll", "esc back"}
	case ScreenDashboard:
		info.KeyboardHints = []string{"enter phase", "esc back"}
	case ScreenPermission:
		info.KeyboardHints = []string{"y allow", "n deny", "a always"}
	case ScreenPhaseModelPicker:
		info.KeyboardHints = []string{"tab cycle", "enter select", "esc back"}
	case ScreenGhostPicker:
		info.KeyboardHints = []string{"j/k navigate", "space toggle", "enter write", "esc back"}
	case ScreenGhostOutput:
		info.KeyboardHints = []string{"j/k navigate", "esc back"}
	case ScreenConfirmQuit:
		info.KeyboardHints = []string{"y quit", "n stay"}
	case ScreenChatHistory:
		info.KeyboardHints = []string{"j/k navigate", "enter continue", "g/G top/bottom", "esc back"}
	case ScreenCommandPalette:
		info.KeyboardHints = []string{"j/k navigate", "enter execute", "type to filter", "esc close"}
	case ScreenHome:
		info.KeyboardHints = []string{"enter submit", "ctrl+p cmds", "ctrl+m models"}
	}
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
	return TruncateWithEllipsis(s, maxW)
}

// renderActiveScreen delegates to the active screen's content renderer.
// Each screen returns ONLY its content area — no header, footer, or chrome.
func (m *AppState) renderActiveScreen(chrome layout.PageChrome) string {
	return m.renderScreenContent(m.screen, chrome)
}

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
	if m.settingsModel == nil {
		m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
	}
	m.settingsModel.width = chrome.ContentWidth()
	m.settingsModel.height = chrome.ContentHeight()
	return m.settingsModel.View()
}

func (m *AppState) renderModelSelectorContent(chrome layout.PageChrome) string {
	if m.msModel == nil {
		return renderLoading("Loading model selector…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.msModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.msModel.View()
}

func (m *AppState) renderPlanContent(chrome layout.PageChrome) string {
	if m.planModel == nil {
		if m.workflowPhase == types.PhasePlan {
			return renderLoading("Generating plan…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
		}
		return renderEmptyState("No plan available", "Run /plan or start a workflow with /new", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.planModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.planModel.View()
}

func (m *AppState) renderExecuteContent(chrome layout.PageChrome) string {
	if m.executeModel == nil {
		return renderEmptyState("No tasks to execute", "Run /new to start a workflow", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.executeModel.width = chrome.ContentWidth()
	m.executeModel.height = chrome.ContentHeight()
	return m.executeModel.View()
}

func (m *AppState) renderVerifyContent(chrome layout.PageChrome) string {
	if m.verifyModel == nil {
		return renderEmptyState("No verification results", "Run /verify after executing tasks", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.verifyModel.width = chrome.ContentWidth()
	m.verifyModel.height = chrome.ContentHeight()
	return m.verifyModel.View()
}

func (m *AppState) renderShipContent(chrome layout.PageChrome) string {
	if m.shipModel == nil {
		return renderEmptyState("Nothing to ship", "Complete the workflow phases first — run /new to start", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.shipModel.width = chrome.ContentWidth()
	m.shipModel.height = chrome.ContentHeight()
	return m.shipModel.View()
}

func (m *AppState) renderResumeContent(chrome layout.PageChrome) string {
	if m.resumeModel == nil {
		return renderLoading("Loading sessions…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.resumeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.resumeModel.View()
}

func (m *AppState) renderGoalInputContent(chrome layout.PageChrome) string {
	if m.goalInput == nil {
		return renderEmptyState("Goal input", "Type a goal below and press enter to start a workflow", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.goalInput.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.goalInput.View()
}

func (m *AppState) renderFirstRunContent(chrome layout.PageChrome) string {
	if m.firstRunModel == nil {
		return renderLoading("Loading first-run wizard…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.firstRunModel.SetContentWidth(chrome.ContentWidth())
	return m.firstRunModel.View()
}

func (m *AppState) renderLedgerContent(chrome layout.PageChrome) string {
	if m.ledgerModel == nil {
		return renderEmptyState("Learning ledger", "No learning entries yet — complete a workflow to populate the ledger", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.ledgerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.ledgerModel.View()
}

func (m *AppState) renderRollbackContent(chrome layout.PageChrome) string {
	if m.rollbackModel == nil {
		return renderEmptyState("Rollback browser", "No commit history loaded — ensure git is initialized", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.rollbackModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.rollbackModel.View()
}

func (m *AppState) renderMetricsContent(chrome layout.PageChrome) string {
	if m.metricsModel == nil {
		return renderEmptyState("Session metrics", "No metrics available — complete some tasks to see analytics", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.metricsModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.metricsModel.View()
}

func (m *AppState) renderDiscussContent(chrome layout.PageChrome) string {
	if m.discussModel == nil {
		return renderEmptyState("Discussion", "No discussion questions — start a workflow with /new", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.discussModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.discussModel.View()
}

func (m *AppState) renderConfigContent(chrome layout.PageChrome) string {
	if m.configModel == nil {
		m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, chrome.ContentWidth(), chrome.ContentHeight(), m.keychain)
	} else {
		m.configModel.cfg = m.config
		m.configModel.width = chrome.ContentWidth()
		m.configModel.height = chrome.ContentHeight()
		m.configModel.theme = m.themeManager.Current()
	}
	return m.configModel.View()
}

func (m *AppState) renderDiffContent(chrome layout.PageChrome) string {
	if m.diffModel == nil {
		return renderEmptyState("Diff viewer", "No diff to display — run /diff or use the workflow", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.diffModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.diffModel.View()
}

func (m *AppState) renderHelpContent(chrome layout.PageChrome) string {
	if m.helpModel == nil {
		return renderEmptyState("Help", "Loading keyboard shortcuts…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.helpModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.helpModel.View()
}

func (m *AppState) renderBisectContent(chrome layout.PageChrome) string {
	if m.bisectModel == nil {
		return renderEmptyState("Git bisect", "No bisect session active — run /bisect to start", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.bisectModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.bisectModel.View()
}

func (m *AppState) renderNotificationsContent(chrome layout.PageChrome) string {
	if m.notifModel == nil {
		return renderEmptyState("Notifications", "No notifications yet — they'll appear here as you use M31A", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.notifModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.notifModel.View()
}

func (m *AppState) renderDashboardContent(chrome layout.PageChrome) string {
	if m.dashboardModel == nil {
		return renderEmptyState("Dashboard", "No workflow active — type a goal or run /new to start", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.dashboardModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.dashboardModel.View()
}

func (m *AppState) renderSessionDetailContent(chrome layout.PageChrome) string {
	if m.sessionDetailModel == nil {
		return renderEmptyState("Session detail", "No session selected — use /resume to browse sessions", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.sessionDetailModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.sessionDetailModel.View()
}

func (m *AppState) renderFileExplorerContent(chrome layout.PageChrome) string {
	if m.fileExplorerModel == nil {
		return renderEmptyState("File explorer", "No files to display — ensure the working directory is set", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.fileExplorerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.fileExplorerModel.View()
}

func (m *AppState) renderToolDetailContent(chrome layout.PageChrome) string {
	if m.toolDetailModel == nil {
		return renderEmptyState("Tool output", "No tool output selected — click a tool card in the REPL to inspect it", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.toolDetailModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.toolDetailModel.View()
}

func (m *AppState) renderPhaseModelPickerContent(chrome layout.PageChrome) string {
	if m.phaseModelPicker == nil {
		return renderEmptyState("Model picker", "Loading models…", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.phaseModelPicker.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.phaseModelPicker.View()
}

func (m *AppState) renderGhostPickerContent(chrome layout.PageChrome) string {
	if m.ghostPickerModel == nil {
		return renderEmptyState("Ghost mode", "No ghost files available — run a workflow to generate ghost outputs", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.ghostPickerModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.ghostPickerModel.View()
}

func (m *AppState) renderGhostOutputContent(chrome layout.PageChrome) string {
	if m.ghostOutputModel == nil {
		return renderEmptyState("Ghost output", "No ghost output yet — select a ghost file to see results", chrome.ContentWidth(), chrome.ContentHeight(), m.themeManager.Current())
	}
	m.ghostOutputModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.ghostOutputModel.View()
}

func (m *AppState) renderConfirmQuitContent(chrome layout.PageChrome) string {
	if m.confirmQuitModel == nil {
		m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
	}
	m.confirmQuitModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.confirmQuitModel.View()
}

func (m *AppState) renderChatHistoryContent(chrome layout.PageChrome) string {
	if m.chatHistoryModel == nil {
		m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
		// Load messages from current REPL session on first creation
		if m.replModel != nil {
			m.chatHistoryModel.SetMessages(m.replModel.Messages())
		}
	}
	m.chatHistoryModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.chatHistoryModel.View()
}

func (m *AppState) renderCommandPaletteContent(chrome layout.PageChrome) string {
	if m.commandPaletteScreenModel == nil {
		m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight())
	}
	m.commandPaletteScreenModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.commandPaletteScreenModel.View()
}

func (m *AppState) renderHomeContent(chrome layout.PageChrome) string {
	if m.homeModel == nil {
		m.homeModel = NewHomeModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight(), m.version)
		m.homeModel.SetCommandRegistry(m.cmdRegistry)
		m.homeModel.SetConfig(m.config)
	}
	m.homeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.homeModel.renderHome()
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

// ─── Render helpers ───────────────────────────────────────────────────────────

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

// ─── Utility ──────────────────────────────────────────────────────────────────

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
	width := chrome.ContentWidth()
	height := chrome.ContentHeight()
	theme := m.themeManager.Current()

	if m.workflowEngine == nil {
		return renderEmptyState("Decisions", "No workflow engine available", width, height, theme)
	}

	decisions := decision.RedactSlice(m.workflowEngine.SnapshotDecisions())
	if len(decisions) == 0 {
		return renderEmptyState("Decisions", "No decisions recorded yet — start a workflow with /new", width, height, theme)
	}

	// Build table
	var b strings.Builder
	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(theme.Primary).
		MarginBottom(1)

	headerStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(theme.TextPrimary).
		BorderBottom(true).
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(theme.BorderSubtle)

	rowStyle := lipgloss.NewStyle().
		Foreground(theme.TextPrimary)

	catStyle := map[decision.Category]lipgloss.Style{
		decision.CategoryTool:     lipgloss.NewStyle().Foreground(lipgloss.Color("6")),
		decision.CategoryPlan:     lipgloss.NewStyle().Foreground(lipgloss.Color("5")),
		decision.CategoryIntent:   lipgloss.NewStyle().Foreground(lipgloss.Color("4")),
		decision.CategoryRetry:    lipgloss.NewStyle().Foreground(lipgloss.Color("3")),
		decision.CategoryStrategy: lipgloss.NewStyle().Foreground(lipgloss.Color("2")),
		decision.CategoryModel:    lipgloss.NewStyle().Foreground(lipgloss.Color("133")),
	}

	b.WriteString(titleStyle.Render("Decision Log"))
	b.WriteString("\n\n")

	// Header row
	b.WriteString(headerStyle.Render(fmt.Sprintf("%-20s %-8s %s", "Time", "Category", "Decision")))
	b.WriteString("\n")

	// Data rows
	for _, d := range decisions {
		ts := d.Timestamp.Format("15:04:05")
		cat := string(d.Category)
		if s, ok := catStyle[d.Category]; ok {
			cat = s.Render(cat)
		}
		decision := d.Decision
		if len(decision) > width-40 {
			decision = decision[:width-43] + "..."
		}
		b.WriteString(rowStyle.Render(fmt.Sprintf("%-20s %-8s %s", ts, cat, decision)))
		b.WriteString("\n")
	}

	// Summary
	totalCost := decision.CostSummary(decisions)
	summaryStyle := lipgloss.NewStyle().
		Foreground(theme.TextMuted).
		MarginTop(1)
	b.WriteString(summaryStyle.Render(fmt.Sprintf(
		"%d decisions | %d tokens | %.1fs total duration",
		len(decisions), totalCost.Tokens, totalCost.Duration,
	)))

	return lipgloss.NewStyle().
		Width(width).
		Height(height).
		Render(b.String())
}
