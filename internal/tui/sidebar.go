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
	sidebarDefaultWidth = 28
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
		s.tree.Cursor = len(s.tree.FlatList()) - 1
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
// Fixed chrome: header(2) + sep(1) + git-section(≈5) +
// files-label(2) + session-section(3) + shortcuts-section(6) = ~19 lines.
const sidebarFixedOverhead = 20

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
		s.tree.Expanded["."] = true
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

// View renders the sidebar with a thin right border.
func (s *SidebarModel) View() string {
	if !s.visible {
		return ""
	}
	t := s.theme
	w := s.width
	contentW := w - 1

	var lines []string

	// ── Header: M31A + version badge ──────────────────────────────────────────
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
		PaddingLeft(1).
		Render(version)
	headerRow := lipgloss.JoinHorizontal(lipgloss.Left, title, versionBadge)
	lines = append(lines, headerRow)

	// ── Gradient separator ────────────────────────────────────────────────────
	sepChars := []string{"▓", "▒", "░"}
	var sep strings.Builder
	for i := 0; i < len(sepChars) && i < contentW; i++ {
		sep.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Render(sepChars[i]))
	}
	remaining := contentW - len(sepChars)
	if remaining > 0 {
		sep.WriteString(lipgloss.NewStyle().Foreground(t.Border).Render(strings.Repeat("─", remaining)))
	}
	lines = append(lines, sep.String())

	// ── Git section ────────────────────────────────────────────────────────────
	if s.branch != "" {
		secLabel := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Bold(true).
			PaddingLeft(1).
			Render("GIT")
		lines = append(lines, "", secLabel)

		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)

		if s.remote != "" {
			remoteLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3).
				Width(contentW).
				Render(s.remote)
			lines = append(lines, remoteLine)
		}

		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			var pills []string
			if modCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Warning).Foreground(t.Background).
					Padding(0, 1).Bold(true).
					Render(fmt.Sprintf("●%d", modCount)))
			}
			if addCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Success).Foreground(t.Background).
					Padding(0, 1).Bold(true).
					Render(fmt.Sprintf("+%d", addCount)))
			}
			if delCount > 0 {
				pills = append(pills, lipgloss.NewStyle().
					Background(t.Error).Foreground(t.Background).
					Padding(0, 1).Bold(true).
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

	// ── Files section ──────────────────────────────────────────────────────────
	lines = append(lines, "")
	filesLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("FILES")
	if s.focused {
		filesLabel += lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ↑↓ enter")
	}
	lines = append(lines, filesLabel)

	if len(s.files) == 0 {
		noFiles := lipgloss.NewStyle().
			Foreground(t.Success).
			PaddingLeft(2).
			Width(contentW).
			Render("✓ working tree clean")
		lines = append(lines, noFiles)
	} else {
		// Update tree dimensions and render
		if s.tree != nil {
			s.tree.Width = contentW
			treeView := s.tree.View()
			for _, line := range strings.Split(treeView, "\n") {
				// Truncate each line to fit the sidebar width
				if len([]rune(line)) > contentW {
					line = string([]rune(line)[:contentW-1]) + "…"
				}
				lines = append(lines, line)
			}
		}
	}

	// ── Session section ────────────────────────────────────────────────────────
	lines = append(lines, "")
	sessionLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("SESSION")
	lines = append(lines, sessionLabel)

	if s.sessionID != "" {
		sessDisplay := s.sessionID
		if len(sessDisplay) > contentW-4 {
			sessDisplay = sessDisplay[:contentW-7] + "..."
		}
		sessLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(2).
			Render("⊙ " + sessDisplay)
		lines = append(lines, sessLine)
	} else {
		noSession := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(2).
			Render("Start typing to begin a session")
		lines = append(lines, noSession)
	}

	// ── Keyboard shortcuts (bottom, muted) ─────────────────────────────────────
	lines = append(lines, "")
	shortcutDivider := lipgloss.NewStyle().
		Foreground(t.Border).
		Render(strings.Repeat("─", contentW))
	lines = append(lines, shortcutDivider)

	shortcuts := []struct {
		key  string
		desc string
	}{
		{"ctrl+p", "commands"},
		{"ctrl+b", "sidebar"},
		{"ctrl+g", "focus"},
		{"ctrl+x", "leader"},
	}
	for _, sc := range shortcuts {
		k := lipgloss.NewStyle().Foreground(t.TextMuted).Render(sc.key)
		d := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).Render(" " + sc.desc)
		line := lipgloss.NewStyle().PaddingLeft(1).Render(k + d)
		lines = append(lines, line)
	}

	content := strings.Join(lines, "\n")

	panel := lipgloss.NewStyle().
		Width(contentW).
		Render(content)

	// Build right border: brand accent when focused, muted otherwise
	lineCount := len(lines)
	brandSegment := 3
	borderColor := t.TextMuted
	if s.focused {
		borderColor = t.Brand
	}
	var rightBorderParts []string
	for i := 0; i < lineCount; i++ {
		if i < brandSegment {
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(t.Brand).Render("│"))
		} else {
			rightBorderParts = append(rightBorderParts,
				lipgloss.NewStyle().Foreground(borderColor).Render("│"))
		}
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

// buildSidebarTree converts a flat list of git-tracked files into a tree structure.
func buildSidebarTree(files []git.FileStatus) *components.FileNode {
	root := &components.FileNode{
		Name:  ".",
		Path:  ".",
		IsDir: true,
	}

	for _, f := range files {
		parts := strings.Split(f.Path, "/")
		current := root

		for i, part := range parts {
			isLast := i == len(parts)-1
			nodePath := strings.Join(parts[:i+1], "/")

			var child *components.FileNode
			for _, c := range current.Children {
				if c.Name == part {
					child = c
					break
				}
			}

			if child == nil {
				child = &components.FileNode{
					Name:  part,
					Path:  nodePath,
					IsDir: !isLast,
				}
				current.Children = append(current.Children, child)
			}

			if isLast {
				child.Status = f.Status
			}

			current = child
		}
	}

	return root
}
