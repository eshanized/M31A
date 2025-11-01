package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// helpSection represents a section of the help screen.
type helpSection struct {
	title string
	items [][2]string // [key, description]
}

// HelpModel shows keybinding help overlay.
type HelpModel struct {
	theme    theme.Theme
	sections []helpSection
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

// defaultHelpSections returns the default keybinding sections.
func defaultHelpSections() []helpSection {
	return []helpSection{
		{
			title: "Navigation",
			items: [][2]string{
				{"?", "Toggle this help"},
				{"esc", "Back / close overlay"},
				{"ctrl+c", "Quit"},
			},
		},
		{
			title: "REPL",
			items: [][2]string{
				{"↵", "Send message"},
				{"ctrl+↵", "Submit goal"},
				{"↑/↓", "Scroll history"},
				{"/", "Command palette"},
			},
		},
		{
			title: "Workflow",
			items: [][2]string{
				{"/discuss", "Start discuss phase"},
				{"/plan", "Start plan phase"},
				{"/execute", "Start execute phase"},
				{"/verify", "Verify phase"},
				{"/ship", "Ship phase"},
			},
		},
		{
			title: "Commands",
			items: [][2]string{
				{"/model", "Open model selector"},
				{"/resume", "Resume a session"},
				{"/rollback", "Rollback to commit"},
				{"/ledger", "View learning ledger"},
				{"/metrics", "View session metrics"},
				{"/diff", "Show git diff"},
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
}

// Init implements tea.Model.
func (hm *HelpModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (hm *HelpModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		hm.width = msg.Width
		hm.height = msg.Height
		return hm, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q", "?":
			return hm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	}
	return hm, nil
}

// View implements tea.Model.
func (hm *HelpModel) View() string {
	t := hm.theme
	w := hm.width
	if w < 30 {
		w = 80
	}

	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(1).
		Render("  M31A Keyboard Shortcuts")
	divider := lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", w))

	var sectionParts []string
	for _, sec := range hm.sections {
		secTitle := lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true).
			PaddingLeft(2).PaddingTop(1).Render(sec.title)
		var rows []string
		for _, item := range sec.items {
			key := lipgloss.NewStyle().Foreground(t.Brand).
				Width(14).Render(item[0])
			desc := lipgloss.NewStyle().Foreground(t.Text).Render(item[1])
			rows = append(rows, "    "+key+"  "+desc)
		}
		sectionParts = append(sectionParts, secTitle)
		sectionParts = append(sectionParts, strings.Join(rows, "\n"))
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("esc / q / ? close")

	return lipgloss.JoinVertical(lipgloss.Left,
		append([]string{title, divider}, append(sectionParts, "", divider, footer)...)...)
}
