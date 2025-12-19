package tui

import (
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// helpSection represents a section of the help screen.
type helpSection struct {
	title string
	items [][2]string // [key, description]
}

// HelpModel shows keybinding help overlay with scrollable viewport.
type HelpModel struct {
	theme    theme.Theme
	sections []helpSection
	viewport viewport.Model
	width    int
	height   int
}

// NewHelpModel creates a HelpModel with the default M31A keybindings.
func NewHelpModel(t theme.Theme) *HelpModel {
	return &HelpModel{
		theme:    t,
		sections: defaultHelpSections(),
	}
}

// defaultHelpSections returns all keybinding sections.
func defaultHelpSections() []helpSection {
	return []helpSection{
		{
			title: "Global",
			items: [][2]string{
				{"?", "Toggle this help"},
				{"ctrl+c", "Cancel / quit (double to exit)"},
				{"ctrl+p", "Command palette"},
				{"ctrl+b", "Toggle sidebar"},
				{"ctrl+g", "Focus sidebar"},
				{"esc", "Back / close overlay"},
			},
		},
		{
			title: "REPL",
			items: [][2]string{
				{"enter", "Send message"},
				{"ctrl+j", "Insert newline (multi-line)"},
				{"shift+enter", "Insert newline (multi-line)"},
				{"up/down", "Navigate command history"},
				{"tab", "Complete slash/mention suggestion"},
				{"/", "Slash command autocomplete"},
				{"@", "File mention autocomplete"},
				{"ctrl+l", "Scroll to bottom"},
				{"ctrl+u / pgup", "Scroll page up"},
				{"ctrl+d / pgdn", "Scroll page down"},
				{"j / k", "Scroll line down/up (empty input)"},
				{"ctrl+y", "Copy last assistant message"},
				{"ctrl+x", "Leader key prefix"},
			},
		},
		{
			title: "Leader Key (ctrl+x ...)",
			items: [][2]string{
				{"x h", "Toggle help"},
				{"x s", "Toggle sidebar"},
				{"x p", "Command palette"},
				{"x m", "Model selector"},
				{"x r", "Resume session"},
				{"x n", "New workflow"},
			},
		},
		{
			title: "Workflow",
			items: [][2]string{
				{"/new", "Start new workflow"},
				{"/goal", "Set session goal"},
				{"/plan", "Start plan phase"},
				{"/execute", "Start execute phase"},
				{"/verify", "Start verify phase"},
				{"/ship", "Start ship phase"},
				{"/pause", "Pause workflow"},
				{"/resume-task", "Resume workflow"},
				{"/metrics", "Session analytics"},
			},
		},
		{
			title: "Session",
			items: [][2]string{
				{"/sessions", "List recent sessions"},
				{"/resume", "Open session browser"},
				{"/fork", "Fork current session"},
				{"/prev", "Previous session"},
				{"/next", "Next session"},
				{"/save", "Save session"},
				{"/clear", "Clear conversation"},
				{"/history", "Show message count"},
				{"/status", "Show session info"},
			},
		},
		{
			title: "AI & Model",
			items: [][2]string{
				{"/model", "Show or switch model"},
				{"/models", "List all cached models"},
				{"/provider", "Show or switch provider"},
				{"/fallback", "Provider fallback status"},
				{"/optimize", "Suggest cheaper model"},
				{"/compress", "Compress context"},
				{"/memory", "Manage context memory"},
				{"/tokens", "Estimate token count"},
				{"/cost", "Toggle cost display"},
			},
		},
		{
			title: "Git & Diff",
			items: [][2]string{
				{"/diff", "Show git diff"},
				{"/rollback", "Browse commits"},
				{"/bisect", "Git bisect"},
			},
		},
		{
			title: "Settings & System",
			items: [][2]string{
				{"/settings", "Open settings editor"},
				{"/config", "Show or set config"},
				{"/theme", "Switch theme"},
				{"/health", "System health"},
				{"/tools", "List available tools"},
				{"/log", "Recent log entries"},
				{"/key", "API key status"},
				{"/ledger", "Learning ledger"},
				{"/reset", "Reset to first-run"},
				{"/quit", "Exit application"},
			},
		},
	}
}

// SetTheme updates the theme.
func (hm *HelpModel) SetTheme(t theme.Theme) {
	hm.theme = t
}

// SetDimensions updates the help model dimensions.
func (hm *HelpModel) SetDimensions(w, h int) {
	hm.width = w
	hm.height = h
	hm.viewport.Width = w - 4
	hm.viewport.Height = h - 4
	hm.viewport.SetContent(hm.renderContent())
}

// Init implements tea.Model.
func (hm *HelpModel) Init() tea.Cmd { return nil }

// Update implements tea.Model with scrollable viewport.
func (hm *HelpModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		hm.width = msg.Width
		hm.height = msg.Height
		hm.viewport.Width = msg.Width - 4
		hm.viewport.Height = msg.Height - 4
		hm.viewport.SetContent(hm.renderContent())
		return hm, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q", "?":
			return hm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		case "g":
			hm.viewport.GotoTop()
			return hm, nil
		case "G":
			hm.viewport.GotoBottom()
			return hm, nil
		}
	}
	var cmd tea.Cmd
	hm.viewport, cmd = hm.viewport.Update(msg)
	return hm, cmd
}

// renderContent builds the full scrollable help content.
func (hm *HelpModel) renderContent() string {
	t := hm.theme
	w := hm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("M31A Keyboard Shortcuts")
	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))

	var sectionParts []string
	for _, sec := range hm.sections {
		secTitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).
			PaddingLeft(2).PaddingTop(1).Render(sec.title)
		var rows []string
		for _, item := range sec.items {
			key := lipgloss.NewStyle().Foreground(t.Brand).
				Width(18).Render(item[0])
			desc := lipgloss.NewStyle().Foreground(t.Text).Render(item[1])
			rows = append(rows, "    "+key+"  "+desc)
		}
		sectionParts = append(sectionParts, secTitle)
		sectionParts = append(sectionParts, strings.Join(rows, "\n"))
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).PaddingTop(1).
		Render("up/down scroll  g/G top/bottom  esc close")

	return strings.Join(append([]string{title, divider}, append(sectionParts, "", divider, footer)...), "\n")
}

// View implements tea.Model.
func (hm *HelpModel) View() string {
	if hm.viewport.Width == 0 {
		return hm.renderContent()
	}
	return hm.viewport.View()
}
