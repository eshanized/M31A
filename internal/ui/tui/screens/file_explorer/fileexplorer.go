package file_explorer

import (
	"strings"

	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// FileExplorerModel shows a file tree browser for the project.
type FileExplorerModel struct {
	theme  theme.Theme
	tree   *components.FileTree
	width  int
	height int
}

// NewFileExplorerModel creates a FileExplorerModel.
func NewFileExplorerModel(t theme.Theme, w, h int) *FileExplorerModel {
	root := &components.FileNode{Name: ".", Path: ".", IsDir: true}
	return &FileExplorerModel{
		theme:  t,
		tree:   components.NewFileTree(root, t, w-4, h-6),
		width:  w,
		height: h,
	}
}

// SetRoot sets the file tree root.
func (fe *FileExplorerModel) SetRoot(root *components.FileNode) {
	fe.tree.Root = root
	fe.tree.Expanded[root.Path] = true
}

// SetTheme updates the theme.
func (fe *FileExplorerModel) SetTheme(t theme.Theme) {
	fe.theme = t
	fe.tree.Theme = t
}

// SetDimensions updates dimensions.
func (fe *FileExplorerModel) SetDimensions(w, h int) {
	fe.width = w
	fe.height = h
	fe.tree.Width = w - 4
	fe.tree.Height = h - 6
}

// Init implements tea.Model.
func (fe *FileExplorerModel) Init() tea.Cmd { return nil }

// Update implements tuitypes.Screenable interface.
func (fe *FileExplorerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		fe.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return fe, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "up", "k":
			fe.tree.MoveCursor(-1)
		case "down", "j":
			fe.tree.MoveCursor(1)
		case "enter", " ":
			fe.tree.Toggle()
		}
	}
	return fe, nil
}

// View implements tea.Model.
func (fe *FileExplorerModel) View() string {
	t := fe.theme
	title := components.ScreenTitle{Text: "File Explorer", Theme: t}.Render()
	footer := components.HintBar{
		Hints: []string{"j/k Navigate", "enter Toggle dir", "esc Back"},
		Theme: t,
	}.Render()
	return strings.Join([]string{"", title, "", fe.tree.View(), "", footer}, "\n")
}
