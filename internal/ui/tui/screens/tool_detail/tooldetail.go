package tool_detail

import (
	"strings"

	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// ToolDetailModel shows expanded tool output with full content and scrolling.
type ToolDetailModel struct {
	theme    theme.Theme
	title    string
	content  string
	viewport viewport.Model
	width    int
	height   int
}

// NewToolDetailModel creates a ToolDetailModel.
func NewToolDetailModel(t theme.Theme, w, h int) *ToolDetailModel {
	vp := viewport.New(w-4, h-8)
	return &ToolDetailModel{
		theme:    t,
		viewport: vp,
		width:    w,
		height:   h,
	}
}

// SetContent sets the tool output content.
func (td *ToolDetailModel) SetContent(title, content string) {
	td.title = title
	td.content = content
	td.viewport.SetContent(content)
}

// SetTheme updates the theme.
func (td *ToolDetailModel) SetTheme(t theme.Theme) {
	td.theme = t
}

// SetDimensions updates dimensions.
func (td *ToolDetailModel) SetDimensions(w, h int) {
	td.width = w
	td.height = h
	td.viewport = viewport.New(w-4, h-8)
	if td.content != "" {
		td.viewport.SetContent(td.content)
	}
}

// Init implements tea.Model.
func (td *ToolDetailModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable.
func (td *ToolDetailModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		td.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return td, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "g":
			td.viewport.GotoTop()
			return td, nil
		case "G":
			td.viewport.GotoBottom()
			return td, nil
		}
	}
	var cmd tea.Cmd
	td.viewport, cmd = td.viewport.Update(msg)
	return td, cmd
}

// View implements tea.Model.
func (td *ToolDetailModel) View() string {
	t := td.theme

	header := components.ScreenTitle{Text: td.title, Theme: t}.Render()

	footer := components.HintBar{
		Hints: []string{"g/G Top/Bottom", "esc Back"},
		Theme: t,
	}.Render()

	return strings.Join([]string{"", header, "", td.viewport.View(), "", footer}, "\n")
}
