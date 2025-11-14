package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)


// ─── AppState view rendering ──────────────────────────────────────────────────

// View implements tea.Model. It renders the full terminal frame.
// This is the top-level view function; it delegates to per-screen view methods.
func (m *AppState) View() string {
	if m.width == 0 || m.height == 0 {
		return "Loading..."
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

	// ── Responsive layout ─────────────────────────────────────────────────────
	// Width < 40: ultra-compact — show only the active screen content with no
	// chrome (no header, no sidebar, no status bar, no toasts, no transitions).
	if m.width < WidthUltraCompact {
		return m.renderActiveScreen()
	}

	// Toast stack (top-right overlay) — hidden below compact width
	toastOverlay := ""
	if len(m.toasts) > 0 && m.width >= WidthCompact {
		toastOverlay = renderToastStack(m.toasts, t, m.width)
	}

	// Sidebar — auto-hidden below full width (80 cols)
	sidebar := ""
	hasSidebar := m.sidebarModel != nil && m.sidebarModel.IsVisible() && m.width >= WidthFull
	if hasSidebar {
		sidebar = m.sidebarModel.View()
	}

	// Main content
	main := m.renderActiveScreen()

	// Overlay toasts on top-right of main content
	if toastOverlay != "" {
		main = lipgloss.JoinVertical(lipgloss.Left,
			toastOverlay,
			main,
		)
	}

	// Screen transition overlay (dim + screen name)
	if m.transition != nil && m.transition.Active {
		w := m.width
		if hasSidebar {
			w = m.width - m.sidebarModel.GetWidth()
		}
		overlay := m.transition.renderTransitionOverlay(t, w, m.height)
		main = lipgloss.JoinVertical(lipgloss.Left, overlay)
	}

	if hasSidebar {
		return lipgloss.JoinHorizontal(lipgloss.Top, sidebar, main)
	}

	return main
}

// renderActiveScreen delegates to the active screen's view function.
func (m *AppState) renderActiveScreen() string {
	switch m.screen {
	case ScreenREPL:
		return m.renderREPLScreen()
	case ScreenSettings:
		return m.renderSettingsScreen()
	case ScreenModelSelector:
		return m.renderModelSelectorScreen()
	case ScreenPlan:
		return m.renderPlanScreen()
	case ScreenExecute:
		return m.renderExecuteScreen()
	case ScreenVerify:
		return m.renderVerifyScreen()
	case ScreenShip:
		return m.renderShipScreen()
	case ScreenResume:
		return m.renderResumeScreen()
	case ScreenGoalInput:
		return m.renderGoalInputScreen()
	case ScreenFirstRun:
		return m.renderFirstRunScreen()
	case ScreenLedger:
		return m.renderLedgerScreen()
	case ScreenRollback:
		return m.renderRollbackScreen()
	case ScreenMetrics:
		return m.renderMetricsScreen()
	case ScreenDiscuss:
		return m.renderDiscussScreen()
	case ScreenDiff:
		return m.renderDiffScreen()
	default:
		return m.renderREPLScreen()
	}
}

// ─── Per-screen view helpers ──────────────────────────────────────────────────

func (m *AppState) renderREPLScreen() string {
	m.ensureReplModel()
	m.syncReplSize()
	return m.replModel.View()
}

func (m *AppState) renderSettingsScreen() string {
	if m.settingsModel == nil {
		m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath)
		m.settingsModel.width = m.width
		m.settingsModel.height = m.height
	}
	return m.settingsModel.View()
}

func (m *AppState) renderModelSelectorScreen() string {
	if m.msModel == nil {
		return "Loading model selector..."
	}
	return m.msModel.View()
}

func (m *AppState) renderPlanScreen() string {
	if m.planModel == nil {
		return "Loading plan..."
	}
	return m.planModel.View()
}

func (m *AppState) renderExecuteScreen() string {
	if m.executeModel == nil {
		return "Loading execution..."
	}
	return m.executeModel.View()
}

func (m *AppState) renderVerifyScreen() string {
	if m.verifyModel == nil {
		return "Loading verification..."
	}
	return m.verifyModel.View()
}

func (m *AppState) renderShipScreen() string {
	if m.shipModel == nil {
		return "Loading ship summary..."
	}
	return m.shipModel.View()
}

func (m *AppState) renderResumeScreen() string {
	if m.resumeModel == nil {
		return "Loading sessions..."
	}
	return m.resumeModel.View()
}

func (m *AppState) renderGoalInputScreen() string {
	if m.goalInput == nil {
		return "Loading goal input..."
	}
	return m.goalInput.View()
}

func (m *AppState) renderFirstRunScreen() string {
	if m.firstRunModel == nil {
		return "Loading first-run wizard..."
	}
	return m.firstRunModel.View()
}

func (m *AppState) renderLedgerScreen() string {
	if m.ledgerModel == nil {
		return "Loading ledger..."
	}
	return m.ledgerModel.View()
}

func (m *AppState) renderRollbackScreen() string {
	if m.rollbackModel == nil {
		return "Loading rollback browser..."
	}
	return m.rollbackModel.View()
}

func (m *AppState) renderMetricsScreen() string {
	if m.metricsModel == nil {
		return "Loading metrics..."
	}
	return m.metricsModel.View()
}

func (m *AppState) renderDiscussScreen() string {
	if m.discussModel == nil {
		return "Loading discuss..."
	}
	return m.discussModel.View()
}

func (m *AppState) renderDiffScreen() string {
	if m.diffModel == nil {
		return "Loading diff..."
	}
	return m.diffModel.View()
}

// renderPermissionModal renders the permission or question overlay.
func (m *AppState) renderPermissionModal() string {
	if m.questionRequest != nil {
		return m.renderQuestionModal()
	}
	if m.permRequest == nil {
		m.screen = ScreenREPL
		return m.renderREPLScreen()
	}

	// Use the rich components.PermissionModal if initialized
	if m.permModal != nil {
		return m.permModal.Render(m.width, m.height)
	}

	// Fallback to legacy renderer if modal was not yet initialized
	return RenderPermissionModal(m.permRequest, m.permCountdown, m.permModalWidth, m.themeManager.Current())
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

	if m.registry != nil {
		cmd := m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.sessionID, m.config)
		_ = cmd // will be run on next Init call
	}
}

// syncReplSize ensures the REPL model dimensions match the terminal.
func (m *AppState) syncReplSize() {
	if m.replModel == nil {
		return
	}
	// Sidebar width is 0 when auto-hidden by responsive layout (< 80 cols)
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && m.width >= WidthFull {
		sw = m.sidebarModel.GetWidth()
	}
	if m.replModel.width != m.width || m.replModel.height != m.height {
		m.replModel.width = m.width
		m.replModel.height = m.height
		m.replModel.SetSidebarWidth(sw)
	}
}

// syncReplProvider updates the REPL provider/model reference and returns
// a tea.Cmd that fetches models.
func (m *AppState) syncReplProvider(sessionID string) tea.Cmd {
	if m.replModel == nil {
		return nil
	}
	return m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
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

	// Use rich QuestionModel if initialized
	if m.questionModel != nil {
		m.questionModel.SetWidth(width)
		content := m.questionModel.View()
		modal := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.themeManager.Current().Brand).
			Background(m.themeManager.Current().SurfaceElevated).
			Padding(1, 2).
			Width(width).
			Render(content)
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, modal)
	}

	// Fallback: simple inline rendering
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

// ─── Header rendering (top chrome) ───────────────────────────────────────────

// renderHeader renders the top header bar used in non-REPL screens.
func (m *AppState) renderHeader(title string) string {
	t := m.themeManager.Current()
	modelID := ""
	modelName := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
		modelName = m.activeModel.Name
	}

	// Get git branch (non-fatal if not a git repo)
	gitBranch := ""
	if m.git != nil {
		if b, err := m.git.CurrentBranch(); err == nil {
			gitBranch = b
		}
	}

	return RenderHeader(
		t,
		m.activeProvider,
		modelID,
		modelName,
		m.workflowPhase,
		gitBranch,
		0, 0,
		m.width,
	)
}

// ─── Utility ──────────────────────────────────────────────────────────────────

// errorf creates a simple error.
func errorf(msg string) error {
	return &simpleError{msg: msg}
}

type simpleError struct {
	msg string
}

func (e *simpleError) Error() string { return e.msg }

// RenderPermissionModal renders a full-screen permission modal.
func RenderPermissionModal(req *tools.PermissionRequest, countdown, width int, t theme.Theme) string {
	if req == nil {
		return ""
	}

	riskStyle := lipgloss.NewStyle().Foreground(t.Warning)
	if req.RiskLevel == types.RiskDestructive {
		riskStyle = lipgloss.NewStyle().Foreground(t.Error)
	}

	bodyContent := lipgloss.JoinVertical(lipgloss.Left,
		"  Tool:  "+req.ToolName,
		"  Command:  "+TruncateWithEllipsis(req.Command, width-12),
		"  Risk:  "+riskStyle.Render(string(req.RiskLevel)),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  y/↵ allow   n/esc deny"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("  Timeout: "+formatSI(countdown)+"s"),
	)

	card := components.Card{
		Title:   "Permission Required",
		Content: bodyContent,
		Width:   width,
		Border:  lipgloss.RoundedBorder(),
		Style:   components.CardBrand,
		Theme:   t,
	}.Render()

	return lipgloss.Place(0, 0, lipgloss.Center, lipgloss.Center, card)
}
