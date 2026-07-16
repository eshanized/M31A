package file_explorer


import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"os"
	"path/filepath"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
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

// buildFileTree recursively builds a file tree from the given directory.
// maxDepth controls how deep to recurse (0 = just the root).
func buildFileTree(rootPath string, depth, maxDepth int) *components.FileNode {
	info, err := os.Stat(rootPath)
	if err != nil {
		return nil
	}

	node := &components.FileNode{
		Name:  filepath.Base(rootPath),
		Path:  rootPath,
		IsDir: info.IsDir(),
		Depth: depth,
	}

	if !info.IsDir() || depth >= maxDepth {
		return node
	}

	entries, err := os.ReadDir(rootPath)
	if err != nil {
		return node
	}

	// Separate dirs and files, sort alphabetically
	var dirs, files []os.DirEntry
	for _, e := range entries {
		name := e.Name()
		// Skip hidden files and common non-project directories
		if strings.HasPrefix(name, ".") || name == "vendor" || name == "node_modules" {
			continue
		}
		if e.IsDir() {
			dirs = append(dirs, e)
		} else {
			files = append(files, e)
		}
	}

	// Add directories first, then files
	for _, d := range dirs {
		child := buildFileTree(filepath.Join(rootPath, d.Name()), depth+1, maxDepth)
		if child != nil {
			node.Children = append(node.Children, child)
		}
	}
	for _, f := range files {
		node.Children = append(node.Children, &components.FileNode{
			Name:  f.Name(),
			Path:  filepath.Join(rootPath, f.Name()),
			IsDir: false,
			Depth: depth + 1,
		})
	}

	return node
}
