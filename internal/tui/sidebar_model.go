package tui

import (
	"context"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

const (
	sidebarDefaultWidth = 30
	sidebarMinWidth     = 20
	sidebarMaxWidth     = 50
)

// SidebarModel manages the collapsible sidebar panel that shows git status.
type SidebarModel struct {
	git     *git.Git
	theme   theme.Theme
	files   []git.FileStatus
	branch  string
	remote  string
	visible bool
	width   int
	height  int // total terminal height passed from AppState

	// File tree for navigation
	focused bool
	tree    *components.FileTree

	// Optional fields for enhanced display
	version   string
	sessionID string

	// Token usage
	totalTokens int
	contextLen  int
	cost        float64
	showCost    bool
	modelName   string

	loading bool

	// Periodic refresh
	shutdownCtx context.Context
}

// NewSidebarModel creates a new SidebarModel.
func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	root := &components.FileNode{Name: ".", Path: ".", IsDir: true}
	return &SidebarModel{
		git:     g,
		theme:   t,
		visible: true,
		width:   sidebarDefaultWidth,
		tree:    components.NewFileTree(root, t, sidebarDefaultWidth-4, 10),
	}
}

// SetVersion sets the version string displayed in the header.
func (s *SidebarModel) SetVersion(v string) {
	s.version = v
}

// SetSessionID sets the active session ID for display.
func (s *SidebarModel) SetSessionID(id string) {
	s.sessionID = id
}

// SetTokenUsage updates the token usage display in the sidebar.
func (s *SidebarModel) SetTokenUsage(totalTokens, contextLen int, cost float64, showCost bool, modelName string) {
	s.totalTokens = totalTokens
	s.contextLen = contextLen
	s.cost = cost
	s.showCost = showCost
	s.modelName = modelName
}

// SetShutdownContext sets the context for the periodic refresh ticker.
func (s *SidebarModel) SetShutdownContext(ctx context.Context) {
	s.shutdownCtx = ctx
}

// Toggle shows/hides the sidebar.
func (s *SidebarModel) Toggle() {
	s.visible = !s.visible
}

// IsVisible returns true if the sidebar is shown.
func (s *SidebarModel) IsVisible() bool {
	return s.visible
}

// IsFocused returns true if the sidebar has keyboard focus.
func (s *SidebarModel) IsFocused() bool {
	return s.focused
}

// Focus gives keyboard focus to the sidebar.
func (s *SidebarModel) Focus() {
	s.focused = true
}

// Blur removes keyboard focus from the sidebar.
func (s *SidebarModel) Blur() {
	s.focused = false
}

// ToggleFocus toggles sidebar keyboard focus.
func (s *SidebarModel) ToggleFocus() {
	s.focused = !s.focused
	if s.focused && s.tree != nil && s.tree.Cursor < 0 {
		s.tree.Cursor = 0
	}
}

// GetWidth returns the sidebar display width (0 if hidden).
func (s *SidebarModel) GetWidth() int {
	if !s.visible {
		return 0
	}
	return s.width
}

// SelectedFile returns the currently selected file, or nil if none.
func (s *SidebarModel) SelectedFile() *git.FileStatus {
	if s.tree == nil {
		return nil
	}
	node := s.tree.SelectedNode()
	if node == nil || node.IsDir {
		return nil
	}
	// Find the git status for this file
	for _, f := range s.files {
		if f.Path == node.Path {
			return &f
		}
	}
	return nil
}

// IncreaseWidth grows the sidebar width by 2, up to the max.
func (s *SidebarModel) IncreaseWidth() {
	if s.width+2 <= sidebarMaxWidth {
		s.width += 2
	}
}

// DecreaseWidth shrinks the sidebar width by 2, down to the min.
func (s *SidebarModel) DecreaseWidth() {
	if s.width-2 >= sidebarMinWidth {
		s.width -= 2
	}
}

// SetWidth sets the sidebar width, clamped to min/max.
func (s *SidebarModel) SetWidth(w int) {
	if w < sidebarMinWidth {
		w = sidebarMinWidth
	}
	if w > sidebarMaxWidth {
		w = sidebarMaxWidth
	}
	s.width = w
}

// SetTheme updates the sidebar theme.
func (s *SidebarModel) SetTheme(t theme.Theme) {
	s.theme = t
}

// SetHeight sets the total available height for the sidebar so it can
// constrain the file list and enable scrolling.
func (s *SidebarModel) SetHeight(h int) {
	s.height = h
	if s.tree != nil {
		treeHeight := h - sidebarFixedOverhead
		if treeHeight < 3 {
			treeHeight = 3
		}
		s.tree.Height = treeHeight
	}
}

// HandleKey processes a key event when the sidebar is focused.
// Returns a command to execute (e.g., show diff) or nil.
func (s *SidebarModel) HandleKey(msg tea.KeyMsg) tea.Cmd {
	if s.tree == nil {
		return nil
	}
	switch msg.String() {
	case "up", "k":
		s.tree.MoveCursor(-1)
	case "down", "j":
		s.tree.MoveCursor(1)
	case "home", "g":
		s.tree.Cursor = 0
	case "end", "G":
		if list := s.tree.FlatList(); len(list) > 0 {
			s.tree.Cursor = len(list) - 1
		}
	case "enter", " ":
		node := s.tree.SelectedNode()
		if node != nil && !node.IsDir {
			return s.showFileDiff(node)
		}
		s.tree.Toggle()
	case "esc":
		s.focused = false
	}
	return nil
}

// showFileDiff returns a command that fetches the diff for the given file node.
func (s *SidebarModel) showFileDiff(node *components.FileNode) tea.Cmd {
	g := s.git
	filePath := node.Path
	fileStatus := node.Status
	return func() tea.Msg {
		if g == nil || !g.IsRepo() {
			return nil
		}
		diff, err := g.DiffFile(filePath, fileStatus)
		if err != nil || strings.TrimSpace(diff) == "" {
			return nil
		}
		return DiffScreenMsg{
			Diff:  diff,
			Title: "diff — " + filePath,
		}
	}
}

// sidebarFixedOverhead is the number of non-file-list lines in the sidebar.
// Layout: header(1) + sep(1) + git(≈3) + usage(≈5) + files-label(1) + session(≈2) = ~13 lines.
const sidebarFixedOverhead = 13

// refreshCmd returns a tea.Cmd that loads git status asynchronously.
func (s *SidebarModel) refreshCmd() tea.Cmd {
	g := s.git
	return func() tea.Msg {
		if g == nil || !g.IsRepo() {
			return SidebarRefreshMsg{}
		}
		branch, _ := g.CurrentBranch()
		remote, _ := g.RemoteTracking()
		files, err := g.StatusPorcelain()
		if err != nil {
			return SidebarRefreshMsg{Branch: branch, Remote: remote}
		}
		sidebarFiles := make([]SidebarFile, 0, len(files))
		for _, f := range files {
			sidebarFiles = append(sidebarFiles, SidebarFile{
				Path:   f.Path,
				Status: f.Status,
			})
		}
		return SidebarRefreshMsg{Files: sidebarFiles, Branch: branch, Remote: remote}
	}
}

// Update handles sidebar-specific messages.
func (s *SidebarModel) Update(msg tea.Msg) (*SidebarModel, tea.Cmd) {
	switch msg := msg.(type) {
	case SidebarRefreshMsg:
		s.branch = msg.Branch
		s.remote = msg.Remote
		files := make([]git.FileStatus, 0, len(msg.Files))
		for _, f := range msg.Files {
			files = append(files, git.FileStatus{Status: f.Status, Path: f.Path})
		}
		s.files = files
		// Rebuild the tree
		root := buildSidebarTree(files)
		s.tree.Root = root
		s.tree.ExpandAll()
		s.loading = false
		return s, nil

	case SidebarRefreshTickMsg:
		if s.visible && s.shutdownCtx != nil {
			select {
			case <-s.shutdownCtx.Done():
				return s, nil
			default:
			}
			return s, tea.Batch(s.refreshCmd(), NextSidebarRefreshTick(s.shutdownCtx, SidebarRefreshInterval))
		}
		return s, nil
	}

	if s.visible && s.branch == "" && !s.loading {
		s.loading = true
		return s, s.refreshCmd()
	}

	if s.visible && s.shutdownCtx != nil && !s.loading {
		return s, NextSidebarRefreshTick(s.shutdownCtx, SidebarRefreshInterval)
	}

	return s, nil
}

// View renders the sidebar with file tree, git info, and token usage.
func (s *SidebarModel) View() string {
	if !s.visible {
		return ""
	}
	t := s.theme
	w := s.width
	contentW := w - 1

	var lines []string

	// ── Header: brand + version ───────────────────────────────────────────────
	version := s.version
	if version == "" {
		version = "dev"
	}
	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(1).
		Render("M31A")
	versionBadge := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(" " + version)
	lines = append(lines, title+versionBadge)

	// ── Thin separator ────────────────────────────────────────────────────────
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", contentW)))

	// ── Git branch + status ───────────────────────────────────────────────────
	if s.branch != "" {
		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)

		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			var pills []string
			if modCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Foreground(t.Warning).
					Render(fmt.Sprintf("●%d", modCount)))
			}
			if addCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Foreground(t.Success).
					Render(fmt.Sprintf("+%d", addCount)))
			}
			if delCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Foreground(t.Error).
					Render(fmt.Sprintf("-%d", delCount)))
			}
			if untracked > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Foreground(t.TextMuted).
					Render(fmt.Sprintf("?%d", untracked)))
			}
			countLine := lipgloss.NewStyle().PaddingLeft(1).Render(strings.Join(pills, " "))
			lines = append(lines, countLine)
		}
	}

	// ── Token usage ───────────────────────────────────────────────────────────
	if s.totalTokens > 0 {
		lines = append(lines, "")
		usageLabel := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Bold(true).
			PaddingLeft(1).
			Render("USAGE")
		lines = append(lines, usageLabel)

		// Context meter
		if s.contextLen > 0 {
			pct := float64(s.totalTokens) / float64(s.contextLen)
			if pct > 1 {
				pct = 1
			}
			const barSegments = 8
			filled := int(pct * barSegments)
			var ctxColor lipgloss.Color
			switch {
			case pct >= 0.9:
				ctxColor = t.Error
			case pct >= 0.7:
				ctxColor = t.Warning
			default:
				ctxColor = t.TextMuted
			}
			bar := "["
			bar += strings.Repeat("█", filled)
			bar += strings.Repeat("░", barSegments-filled)
			bar += "]"
			pctStr := fmt.Sprintf("%d%%", int(pct*100))
			meterLine := lipgloss.NewStyle().
				Foreground(ctxColor).
				PaddingLeft(1).
				Render(bar + " " + pctStr)
			lines = append(lines, meterLine)
		}

		// Token count
		tokStr := formatTokenCountSidebar(s.totalTokens)
		tokLine := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(1).
			Render(tokStr)
		lines = append(lines, tokLine)

		// Cost (if enabled)
		if s.showCost && s.cost > 0 {
			var costStr string
			if s.cost < 0.01 {
				costStr = "<$0.01"
			} else {
				costStr = fmt.Sprintf("$%.2f", s.cost)
			}
			costLine := lipgloss.NewStyle().
				Foreground(t.Warning).
				PaddingLeft(1).
				Render(costStr)
			lines = append(lines, costLine)
		}

		// Model name
		if s.modelName != "" {
			modelDisplay := s.modelName
			if len(modelDisplay) > contentW-2 {
				modelDisplay = modelDisplay[:contentW-5] + "..."
			}
			modelLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(1).
				Render(modelDisplay)
			lines = append(lines, modelLine)
		}
	}

	// ── File tree ──────────────────────────────────────────────────────────────
	lines = append(lines, "")
	filesLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("FILES")
	if s.focused {
		filesLabel += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ↑↓")
	}
	lines = append(lines, filesLabel)

	if len(s.files) == 0 {
		noFiles := lipgloss.NewStyle().
			Foreground(t.Success).
			PaddingLeft(2).
			Width(contentW).
			Render("✓ clean")
		lines = append(lines, noFiles)
	} else {
		if s.tree != nil {
			s.tree.Width = contentW
			treeView := s.tree.View()
			lines = append(lines, strings.Split(treeView, "\n")...)
		}
	}

	// ── Session (compact) ─────────────────────────────────────────────────────
	if s.sessionID != "" {
		lines = append(lines, "")
		sessDisplay := s.sessionID
		if len(sessDisplay) > contentW-2 {
			sessDisplay = sessDisplay[:contentW-5] + "..."
		}
		sessLine := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(1).
			Render(sessDisplay)
		lines = append(lines, sessLine)
	}

	// Pad each line to exactly contentW characters for consistent border alignment.
	var paddedLines []string
	for _, line := range lines {
		lineW := lipgloss.Width(line)
		if lineW < contentW {
			line += strings.Repeat(" ", contentW-lineW)
		}
		paddedLines = append(paddedLines, line)
	}
	panel := lipgloss.NewStyle().Render(strings.Join(paddedLines, "\n"))

	// Build right border
	lineCount := len(lines)
	borderColor := t.TextMuted
	if s.focused {
		borderColor = t.Brand
	}
	var rightBorderParts []string
	for i := 0; i < lineCount; i++ {
		rightBorderParts = append(rightBorderParts,
			lipgloss.NewStyle().Foreground(borderColor).Render("│"))
	}
	rightBorderStr := strings.Join(rightBorderParts, "\n")
	rightBorder := lipgloss.NewStyle().Render(rightBorderStr)

	return lipgloss.JoinHorizontal(lipgloss.Top, panel, rightBorder)
}

// countFileStatuses returns counts of modified, added, deleted, and untracked files.
func countFileStatuses(files []git.FileStatus) (mod, add, del, untracked int) {
	for _, f := range files {
		switch {
		case strings.Contains(f.Status, "M"):
			mod++
		case strings.Contains(f.Status, "A"):
			add++
		case strings.Contains(f.Status, "D"):
			del++
		case strings.Contains(f.Status, "?"):
			untracked++
		}
	}
	return
}

// formatTokenCountSidebar formats token count for sidebar display.
func formatTokenCountSidebar(n int) string {
	if n >= 1000 {
		return fmt.Sprintf("%.1fK tokens", float64(n)/1000)
	}
	return fmt.Sprintf("%d tokens", n)
}

// HandleMouse processes mouse events for the sidebar.
// Returns a tea.Cmd if a file was clicked (to show diff), or nil.
func (s *SidebarModel) HandleMouse(msg tea.MouseMsg, sidebarX int) tea.Cmd {
	if !s.visible || !s.focused {
		return nil
	}

	if msg.Action != tea.MouseActionPress || msg.Button != tea.MouseButtonLeft {
		return nil
	}

	// Check if click is within the sidebar's X range
	if msg.X < sidebarX || msg.X >= sidebarX+s.width {
		return nil
	}

	// Convert absolute Y to sidebar-relative Y (account for header)
	// The sidebar starts at Y=0 in the terminal. The file tree starts after
	// the header sections. We need to figure out which tree item was clicked.
	if s.tree == nil {
		return nil
	}

	// Count header lines to find where the file tree starts
	headerLines := s.countHeaderLines()
	relativeY := msg.Y - headerLines
	if relativeY < 0 {
		return nil
	}

	// Map click to tree cursor
	flatList := s.tree.FlatList()
	if relativeY >= len(flatList) {
		return nil
	}

	s.tree.Cursor = relativeY
	node := s.tree.SelectedNode()
	if node != nil && !node.IsDir {
		return s.showFileDiff(node)
	}
	if node != nil && node.IsDir {
		s.tree.Toggle()
	}

	return nil
}

// countHeaderLines returns the number of lines before the file tree section.
func (s *SidebarModel) countHeaderLines() int {
	n := 1 // header (M31A + version)
	n++    // separator
	if s.branch != "" {
		n++ // branch line
		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			n++ // status pills
		}
	}
	if s.totalTokens > 0 {
		n += 4 // USAGE label + meter + tokens + cost/model (approximate)
	}
	n++ // blank line before FILES
	n++ // FILES label
	return n
}

// buildSidebarTree converts a flat list of git-tracked files into a tree structure.
func buildSidebarTree(files []git.FileStatus) *components.FileNode {
	root := &components.FileNode{
		Name:  ".",
		Path:  ".",
		IsDir: true,
	}

	// Per-level index for O(1) child lookup instead of O(C) linear scan (TU-6 fix).
	childIndex := map[*components.FileNode]map[string]*components.FileNode{
		root: {},
	}

	for _, f := range files {
		parts := strings.Split(f.Path, "/")
		current := root

		for i, part := range parts {
			isLast := i == len(parts)-1
			nodePath := strings.Join(parts[:i+1], "/")

			index := childIndex[current]
			child, found := index[part]

			if !found {
				child = &components.FileNode{
					Name:  part,
					Path:  nodePath,
					IsDir: !isLast,
				}
				current.Children = append(current.Children, child)
				index[part] = child
				childIndex[child] = map[string]*components.FileNode{}
			}

			if isLast {
				child.Status = f.Status
			}

			current = child
		}
	}

	return root
}
