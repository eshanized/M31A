package tui

import (
	"log/slog"
	"path/filepath"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// ─── AppState view rendering ──────────────────────────────────────────────────

// View implements tea.Model. It renders the full terminal frame using the
// unified PageLayout system: 1-line header + content viewport + 1-line footer.
func (m *AppState) View() string {
	// Debug: log View() call
	slog.Debug("AppState.View called", "width", m.width, "height", m.height, "screen", m.screen.Label(), "hasTransition", m.transition != nil && m.transition.Active)

	if m.width == 0 || m.height == 0 {
		// Dimensions not yet known — emit empty frame; bubbletea will repaint
		// on first WindowSizeMsg.
		slog.Debug("View: zero dimensions, returning empty")
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
	case ScreenPhaseTransition:
		return m.renderPhaseTransitionContent(chrome)
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

	// Permission/question modal (top priority overlay).
	// This overlay path returns before renderScreenContent, so the router never
	// controls Permission's View(). The modal is rendered directly on the
	// concrete pointer. This is intentional — Permission is a modal overlay,
	// not a full-screen routed view. It has no router.Register() and never has.
	// Do not "fix" this by removing the early return or adding router registration.
	if m.screen == ScreenPermission {
		modalContent := m.renderPermissionModalContent()
		if result := m.renderDimmedModal(modalContent, t); result != "" {
			return result
		}
		return m.renderPermissionModal()
	}

	// Model Selector as centered dialog overlay on dimmed REPL background.
	// This overlay path returns before renderScreenContent, so the router never
	// controls ModelSelector's View(). The model is rendered directly on the
	// concrete pointer. This is intentional — ModelSelector is an overlay, not
	// a full-screen routed view. Do not "fix" this by removing the early return.
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
