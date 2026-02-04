package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// FileExplorerModel shows a file tree browser for the project.
type FileExplorerModel struct {
	theme theme.Theme
	tree  *components.FileTree
	width  int
	height int
}

// NewFileExplorerModel creates a FileExplorerModel.
func NewFileExplorerModel(t theme.Theme, w, h int) *FileExplorerModel {
	root := &components.FileNode{Name: ".", Path: ".", IsDir: true}
	return &FileExplorerModel{
		theme: t,
		tree:  components.NewFileTree(root, t, w-4, h-6),
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

// Update implements tea.Model.
func (fe *FileExplorerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		fe.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return fe, func() tea.Msg { return PopScreenMsg{} }
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
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render("File Explorer")
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[j/k] Navigate   [enter] Toggle dir   [esc] Back")
	return strings.Join([]string{"", title, "", fe.tree.View(), "", footer}, "\n")
}
