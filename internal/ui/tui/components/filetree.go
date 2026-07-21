package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
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
	node      *FileNode
	depth     int
	isLast    bool   // true if this is the last child at its level
	parentEnd []bool // tracks which ancestors are last children
}

// FileTree renders a tree-view of files with indentation.
type FileTree struct {
	Root      *FileNode
	Cursor    int
	Expanded  map[string]bool
	Theme     theme.Theme
	Width     int
	Height    int
	flatList  []flatNode
	flatDirty bool
}

// NewFileTree creates a FileTree with all directories expanded by default.
func NewFileTree(root *FileNode, t theme.Theme, w, h int) *FileTree {
	ft := &FileTree{
		Root:      root,
		Expanded:  make(map[string]bool),
		Theme:     t,
		Width:     w,
		Height:    h,
		flatDirty: true,
	}
	// Expand all directories by default
	ft.expandAll(root)
	return ft
}

// expandAll recursively marks all directories as expanded.
func (ft *FileTree) expandAll(node *FileNode) {
	if node == nil {
		return
	}
	for _, child := range node.Children {
		if child.IsDir {
			ft.Expanded[child.Path] = true
			ft.expandAll(child)
		}
	}
}

// ExpandAll expands all directories in the tree starting from Root.
func (ft *FileTree) ExpandAll() {
	ft.expandAll(ft.Root)
	ft.flatDirty = true
}

// Toggle expands/collapses a directory.
func (ft *FileTree) Toggle() {
	if ft.Cursor >= 0 && ft.Cursor < len(ft.flatList) {
		flat := ft.flatList[ft.Cursor]
		if flat.node.IsDir {
			ft.Expanded[flat.node.Path] = !ft.Expanded[flat.node.Path]
			ft.flatDirty = true
		}
	}
}

// MoveCursor moves the cursor up/down.
func (ft *FileTree) MoveCursor(delta int) {
	ft.ensureFlat()
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
	ft.ensureFlat()
	result := make([]*FileNode, len(ft.flatList))
	for i, flat := range ft.flatList {
		result[i] = flat.node
	}
	return result
}

func (ft *FileTree) ensureFlat() {
	if !ft.flatDirty {
		return
	}
	ft.flatDirty = false
	ft.flatList = nil
	if ft.Root != nil {
		ft.flattenNode(ft.Root, 0, nil)
	}
}

func (ft *FileTree) flattenNode(node *FileNode, depth int, parentEnd []bool) {
	for i, child := range node.Children {
		isLast := i == len(node.Children)-1
		// Build parentEnd for children: append whether THIS node is last
		var childParentEnd []bool
		if parentEnd != nil {
			childParentEnd = make([]bool, len(parentEnd))
			copy(childParentEnd, parentEnd)
		}
		childParentEnd = append(childParentEnd, isLast)

		ft.flatList = append(ft.flatList, flatNode{
			node:      child,
			depth:     depth,
			isLast:    isLast,
			parentEnd: childParentEnd,
		})
		if child.IsDir && ft.Expanded[child.Path] {
			ft.flattenNode(child, depth+1, childParentEnd)
		}
	}
}

// View renders the file tree.
func (ft *FileTree) View() string {
	t := ft.Theme
	ft.ensureFlat()

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
		lines = append(lines, ft.renderNode(flat, selected))
	}

	return strings.Join(lines, "\n")
}

func (ft *FileTree) renderNode(flat flatNode, selected bool) string {
	t := ft.Theme
	node := flat.node

	// Build tree prefix with proper box-drawing characters.
	// parentEnd tracks whether each ancestor level is the last child.
	// For depth d, parentEnd has d entries (one per ancestor level).
	var indent strings.Builder
	for i := 0; i < flat.depth; i++ {
		if i < len(flat.parentEnd) {
			if flat.parentEnd[i] {
				indent.WriteString("  ")
			} else {
				indent.WriteString("│ ")
			}
		} else {
			indent.WriteString("  ")
		}
	}

	// Add the connector for this node
	if flat.depth > 0 {
		if flat.isLast {
			indent.WriteString("└─")
		} else {
			indent.WriteString("├─")
		}
	}

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

	// Calculate prefix length for width-aware truncation
	prefixLen := len(indent.String())

	// Truncate filename to fit within available width
	maxNameLen := ft.Width - prefixLen - 4 // leave room for icon, space, padding
	if maxNameLen < 8 {
		maxNameLen = 8
	}
	displayName := node.Name
	if len([]rune(displayName)) > maxNameLen {
		// Keep start and end, truncate middle
		keepEnd := 4
		if maxNameLen > keepEnd+3 {
			displayName = string([]rune(displayName)[:maxNameLen-keepEnd-3]) + "…" + string([]rune(displayName)[len([]rune(displayName))-keepEnd:])
		} else {
			displayName = string([]rune(displayName)[:maxNameLen-1]) + "…"
		}
	}

	if node.IsDir {
		nameStyle = nameStyle.Bold(true)
		var prefix string
		if ft.Expanded[node.Path] {
			prefix = "▾"
		} else {
			prefix = "▸"
		}
		return indent.String() + prefix + " " + nameStyle.Render(displayName)
	}

	statusStr := ""
	if icon != "" {
		statusStr = iconStyle.Render(icon) + " "
	}
	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}
	return indent.String() + prefix + statusStr + nameStyle.Render(displayName)
}
