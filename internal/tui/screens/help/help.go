package help

import (
	"fmt"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// helpSection represents a section of the help screen.
type helpSection struct {
	title string
	items [][2]string // [key, description]
}

// HelpModel shows keybinding help overlay with scrollable viewport.
type HelpModel struct {
	theme       theme.Theme
	sections    []helpSection
	keyRegistry *tuitypes.KeyRegistry
	viewport    viewport.Model
	width       int
	height      int
}

// NewHelpModel creates a HelpModel with the default M31A keybindings.
func NewHelpModel(t theme.Theme) *HelpModel {
	return &HelpModel{
		theme:    t,
		sections: defaultHelpSections(),
	}
}

// SetKeyRegistry sets the key registry for dynamic help generation.
func (hm *HelpModel) SetKeyRegistry(kr *tuitypes.KeyRegistry) {
	hm.keyRegistry = kr
	hm.rebuildSections()
}

// rebuildSections regenerates help sections from the keybinding registry
// and hardcoded slash-command sections. This ensures the help screen always
// matches the actual keybindings registered in the application.
func (hm *HelpModel) rebuildSections() {
	if hm.keyRegistry == nil {
		return
	}

	kr := hm.keyRegistry
	leaderPrefix := kr.LeaderKey() + " "

	// ── Global section (non-leader bindings from CtxGlobal) ──────────
	var globalItems [][2]string
	globalItems = append(globalItems, [2]string{"?", "Toggle this help"})
	globalItems = append(globalItems, [2]string{"ctrl+c", "Cancel / quit (double to exit)"})
	globalItems = append(globalItems, [2]string{"ctrl+p", "Command palette"})
	for _, b := range kr.GetContextSpecificBindings(tuitypes.CtxGlobal) {
		if strings.HasPrefix(b.Key, leaderPrefix) {
			continue // leader chords go in their own section
		}
		globalItems = append(globalItems, [2]string{b.Key, b.Description})
	}

	// ── REPL section (non-leader bindings from CtxREPL) ─────────────
	var replItems [][2]string
	replItems = append(replItems, [2]string{"enter", "Send message"})
	replItems = append(replItems, [2]string{"ctrl+j", "Insert newline (multi-line)"})
	replItems = append(replItems, [2]string{"shift+enter", "Insert newline (multi-line)"})
	replItems = append(replItems, [2]string{"up/down", "Navigate command history"})
	replItems = append(replItems, [2]string{"tab", "Complete slash/mention suggestion"})
	replItems = append(replItems, [2]string{"/", "Slash command autocomplete"})
	replItems = append(replItems, [2]string{"@", "File mention autocomplete"})
	replItems = append(replItems, [2]string{"ctrl+l / end", "Scroll to bottom"})
	replItems = append(replItems, [2]string{"ctrl+u / pgup", "Scroll page up"})
	replItems = append(replItems, [2]string{"ctrl+d / pgdn", "Scroll page down"})
	replItems = append(replItems, [2]string{"j / k", "Scroll line down/up (empty input)"})
	for _, b := range kr.GetContextSpecificBindings(tuitypes.CtxREPL) {
		if strings.HasPrefix(b.Key, leaderPrefix) {
			continue
		}
		// Skip keys already listed above
		duplicate := false
		for _, existing := range replItems {
			if existing[0] == b.Key {
				duplicate = true
				break
			}
		}
		if !duplicate {
			replItems = append(replItems, [2]string{b.Key, b.Description})
		}
	}

	// ── Leader Key section (all chord bindings) ─────────────────────
	seen := make(map[string]bool)
	var leaderItems [][2]string
	for _, b := range kr.GetContextSpecificBindings(tuitypes.CtxGlobal) {
		if strings.HasPrefix(b.Key, leaderPrefix) {
			shortKey := strings.TrimPrefix(b.Key, leaderPrefix)
			if !seen[shortKey] {
				seen[shortKey] = true
				leaderItems = append(leaderItems, [2]string{"x " + shortKey, b.Description})
			}
		}
	}
	for _, b := range kr.GetContextSpecificBindings(tuitypes.CtxREPL) {
		if strings.HasPrefix(b.Key, leaderPrefix) {
			shortKey := strings.TrimPrefix(b.Key, leaderPrefix)
			if !seen[shortKey] {
				seen[shortKey] = true
				leaderItems = append(leaderItems, [2]string{"x " + shortKey, b.Description})
			}
		}
	}

	// ── Workflow section (slash commands) ────────────────────────────
	workflowItems := [][2]string{
		{"/new", "Start new workflow"},
		{"/goal", "Set session goal"},
		{"/plan", "Start plan phase"},
		{"/execute", "Start execute phase"},
		{"/verify", "Start verify phase"},
		{"/ship", "Start ship phase"},
		{"/pause", "Pause workflow"},
		{"/resume-task", "Resume workflow"},
		{"/metrics", "Session analytics"},
	}

	// ── Session section ──────────────────────────────────────────────
	sessionItems := [][2]string{
		{"/chat", "Start new chat session"},
		{"/sessions", "List recent sessions"},
		{"/resume", "Open session browser"},
		{"/save", "Save session"},
		{"/clear", "Clear conversation"},
		{"/search", "Search messages"},
		{"/flush", "Clear screen, reset view"},
		{"/history", "Chat history browser"},
		{"/status", "Show session info"},
	}

	// ── AI & Model section ───────────────────────────────────────────
	aiItems := [][2]string{
		{"/model", "Show or switch model"},
		{"/provider", "Show or switch provider"},
		{"/fallback", "Provider fallback status"},
		{"/optimize", "Suggest cheaper model"},
		{"/compress", "Compress context"},
		{"/memory", "Manage context memory"},
		{"/tokens", "Estimate token count"},
		{"/cost", "Toggle cost display"},
	}

	// ── Git & Diff section ───────────────────────────────────────────
	gitItems := [][2]string{
		{"/diff", "Show git diff"},
		{"/rollback", "Browse commits"},
		{"/bisect", "Git bisect"},
	}

	// ── Settings & System section ────────────────────────────────────
	systemItems := [][2]string{
		{"/settings", "Open settings editor"},
		{"/config", "Show or set config"},
		{"/health", "System health"},
		{"/tools", "List available tools"},
		{"/log", "Recent log entries"},
		{"/key", "API key status"},
		{"/keychain", "API key status (alias)"},
		{"/dream", "Context memory (alias)"},
		{"/ledger", "Learning ledger"},
		{"/about", "Version & system info"},
		{"/reset", "Reset to first-run"},
		{"/quit", "Exit application"},
		{"/exit", "Exit (alias)"},
	}

	hm.sections = []helpSection{
		{title: "Global", items: globalItems},
		{title: "REPL", items: replItems},
		{title: fmt.Sprintf("Leader Key (%s ...)", kr.LeaderKey()), items: leaderItems},
		{title: "Workflow", items: workflowItems},
		{title: "Session", items: sessionItems},
		{title: "AI & Model", items: aiItems},
		{title: "Git & Diff", items: gitItems},
		{title: "Settings & System", items: systemItems},
	}
}

// defaultHelpSections returns all keybinding sections (fallback when no registry).
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
				{"ctrl+l / end", "Scroll to bottom"},
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
				{"x h", "Help screen"},
				{"x s", "Settings"},
				{"x b", "Toggle sidebar"},
				{"x m", "Model selector"},
				{"x r", "Resume session"},
				{"x n", "New workflow"},
				{"x l", "Learning ledger"},
				{"x k", "Rollback browser"},
				{"x d", "Workflow dashboard"},
				{"x f", "File explorer"},
				{"x !", "Notifications"},
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
				{"/chat", "Start new chat session"},
				{"/sessions", "List recent sessions"},
				{"/resume", "Open session browser"},
				{"/save", "Save session"},
				{"/clear", "Clear conversation"},
				{"/search", "Search messages"},
				{"/flush", "Clear screen, reset view"},
				{"/history", "Chat history browser"},
				{"/status", "Show session info"},
			},
		},
		{
			title: "AI & Model",
			items: [][2]string{
				{"/model", "Show or switch model"},
				{"/model", "Open model selector"},
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
				{"/health", "System health"},
				{"/tools", "List available tools"},
				{"/log", "Recent log entries"},
				{"/key", "API key status"},
				{"/keychain", "API key status (alias)"},
				{"/dream", "Context memory (alias)"},
				{"/ledger", "Learning ledger"},
				{"/about", "Version & system info"},
				{"/reset", "Reset to first-run"},
				{"/quit", "Exit application"},
				{"/exit", "Exit (alias)"},
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
	hm.viewport.Width = max(10, w-4)
	hm.viewport.Height = max(4, h-4)
	hm.viewport.SetContent(hm.renderContent())
}

// Init implements tea.Model.
func (hm *HelpModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable interface.
func (hm *HelpModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		hm.width = msg.Width
		hm.height = msg.Height
		hm.viewport.Width = max(10, msg.Width-4)
		hm.viewport.Height = max(4, msg.Height-4)
		hm.viewport.SetContent(hm.renderContent())
		return hm, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q", "?":
			return hm, func() tea.Msg {
				return tuitypes.PopScreenMsg{}
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
	if w <= 0 {
		w = 80 // only when uninitialized
	}

	// Key column: ideal 18, scaled down on narrow terminals
	keyColW := 18
	if w < 60 {
		keyColW = max(10, w/4)
	}

	title := components.ScreenTitle{Text: "M31A Keyboard Shortcuts", Theme: t}.Render()
	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w-2))

	var sectionParts []string
	for _, sec := range hm.sections {
		secTitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).
			PaddingLeft(2).PaddingTop(1).Render(sec.title)
		var rows []string
		for _, item := range sec.items {
			key := lipgloss.NewStyle().Foreground(t.Brand).
				Width(keyColW).Render(item[0])
			desc := lipgloss.NewStyle().Foreground(t.Text).Render(item[1])
			rows = append(rows, "    "+key+"  "+desc)
		}
		sectionParts = append(sectionParts, secTitle)
		sectionParts = append(sectionParts, strings.Join(rows, "\n"))
	}

	footer := components.HintBar{
		Hints: []string{"up/down scroll", "g/G top/bottom", "esc close"},
		Theme: t,
	}.Render()

	return strings.Join(append([]string{title, divider}, append(sectionParts, "", divider, footer)...), "\n")
}

// View implements tea.Model.
func (hm *HelpModel) View() string {
	if hm.viewport.Width == 0 {
		return hm.renderContent()
	}
	return hm.viewport.View()
}
