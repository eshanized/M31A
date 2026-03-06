package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// FileNode represents a node in the file tree.
type FileNode struct {
	Name     string
	Path     string
	IsDir    bool
	Children []*FileNode
	Status   string // git status: M, A, D, ?, ""
	Depth    int    // depth in tree (0 = root)
}

// flatNode is a node with its depth for rendering.
type flatNode struct {
	node  *FileNode
	depth int
}

// FileTree renders a tree-view of files with indentation.
type FileTree struct {
	Root     *FileNode
	Cursor   int
	Expanded map[string]bool
	Theme    theme.Theme
	Width    int
	Height   int
	flatList []flatNode
}

// NewFileTree creates a FileTree.
func NewFileTree(root *FileNode, t theme.Theme, w, h int) *FileTree {
	return &FileTree{
		Root:     root,
		Expanded: make(map[string]bool),
		Theme:    t,
		Width:    w,
		Height:   h,
	}
}

// Toggle expands/collapses a directory.
func (ft *FileTree) Toggle() {
	if ft.Cursor >= 0 && ft.Cursor < len(ft.flatList) {
		flat := ft.flatList[ft.Cursor]
		if flat.node.IsDir {
			ft.Expanded[flat.node.Path] = !ft.Expanded[flat.node.Path]
			ft.flatten()
		}
	}
}

// MoveCursor moves the cursor up/down.
func (ft *FileTree) MoveCursor(delta int) {
	ft.flatten()
	ft.Cursor += delta
	if ft.Cursor < 0 {
		ft.Cursor = 0
	}
	if ft.Cursor >= len(ft.flatList) {
		ft.Cursor = len(ft.flatList) - 1
	}
}

// SelectedNode returns the currently selected node.
func (ft *FileTree) SelectedNode() *FileNode {
	if ft.Cursor >= 0 && ft.Cursor < len(ft.flatList) {
		return ft.flatList[ft.Cursor].node
	}
	return nil
}

// FlatList returns the flattened list of nodes.
func (ft *FileTree) FlatList() []*FileNode {
	ft.flatten()
	result := make([]*FileNode, len(ft.flatList))
	for i, flat := range ft.flatList {
		result[i] = flat.node
	}
	return result
}

func (ft *FileTree) flatten() {
	ft.flatList = nil
	if ft.Root != nil {
		ft.flattenNode(ft.Root, 0)
	}
}

func (ft *FileTree) flattenNode(node *FileNode, depth int) {
	for _, child := range node.Children {
		ft.flatList = append(ft.flatList, flatNode{node: child, depth: depth})
		if child.IsDir && ft.Expanded[child.Path] {
			ft.flattenNode(child, depth+1)
		}
	}
}

// View renders the file tree.
func (ft *FileTree) View() string {
	t := ft.Theme
	ft.flatten()

	if len(ft.flatList) == 0 {
		return lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("No files found.")
	}

	start := 0
	if ft.Height > 0 {
		if ft.Cursor >= ft.Height {
			start = ft.Cursor - ft.Height + 1
		}
		// If cursor is in the first half of the viewport, don't scroll.
		// This keeps the view stable when navigating near the top.
		if ft.Cursor < ft.Height/2 && ft.Height > 4 {
			start = 0
		}
	}
	end := start + ft.Height
	if end > len(ft.flatList) {
		end = len(ft.flatList)
	}
	// Adjust start if end is smaller than expected (near end of list).
	if ft.Height > 0 && end-start < ft.Height && start > 0 {
		start = end - ft.Height
		if start < 0 {
			start = 0
		}
	}

	var lines []string
	for i := start; i < end; i++ {
		flat := ft.flatList[i]
		selected := i == ft.Cursor
		lines = append(lines, ft.renderNode(flat.node, flat.depth, selected))
	}

	return strings.Join(lines, "\n")
}

func (ft *FileTree) renderNode(node *FileNode, depth int, selected bool) string {
	t := ft.Theme

	indent := strings.Repeat("  ", depth)

	icon := ""
	iconStyle := lipgloss.NewStyle()
	switch node.Status {
	case "M":
		icon = "●"
		iconStyle = iconStyle.Foreground(t.Warning)
	case "A":
		icon = "+"
		iconStyle = iconStyle.Foreground(t.Success)
	case "D":
		icon = "✗"
		iconStyle = iconStyle.Foreground(t.Error)
	case "?":
		icon = "?"
		iconStyle = iconStyle.Foreground(t.TextMuted)
	}

	nameStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		nameStyle = nameStyle.Foreground(t.Brand).Bold(true)
	}
	if node.IsDir {
		nameStyle = nameStyle.Bold(true)
		prefix := "  "
		if ft.Expanded[node.Path] {
			prefix = "▾ "
		} else {
			prefix = "▸ "
		}
		return indent + prefix + nameStyle.Render(node.Name+"/")
	}

	statusStr := ""
	if icon != "" {
		statusStr = iconStyle.Render(icon) + " "
	}
	prefix := "    "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("  ▶ ")
	}
	return indent + prefix + statusStr + nameStyle.Render(node.Name)
}
