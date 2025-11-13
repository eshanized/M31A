package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
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

	loading bool
	err     string
}

// NewSidebarModel creates a new SidebarModel.
func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	return &SidebarModel{
		git:     g,
		theme:   t,
		visible: true,
		width:   sidebarDefaultWidth,
	}
}

// Toggle shows/hides the sidebar.
func (s *SidebarModel) Toggle() {
	s.visible = !s.visible
}

// IsVisible returns true if the sidebar is shown.
func (s *SidebarModel) IsVisible() bool {
	return s.visible
}

// GetWidth returns the sidebar display width (0 if hidden).
func (s *SidebarModel) GetWidth() int {
	if !s.visible {
		return 0
	}
	return s.width
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
		s.loading = false
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
	contentW := w - 1 // reserve 1 column for the right border

	// ── Header row ────────────────────────────────────────────────────────────
	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Padding(0, 1).
		Width(contentW).
		Render("ℹ M31A")

	divider := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(strings.Repeat("─", contentW))

	var lines []string
	lines = append(lines, title)
	lines = append(lines, divider)

	// ── Git section ────────────────────────────────────────────────────────────
	if s.branch != "" {
		secLabel := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Bold(true).
			PaddingLeft(1).
			Render("GIT")
		lines = append(lines, secLabel)

		// Branch row with ⎇ icon
		branchLine := lipgloss.NewStyle().
			Foreground(t.TextSecondary).
			PaddingLeft(1).
			Width(contentW).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)

		// Remote tracking (e.g. "origin/main") in muted
		if s.remote != "" {
			remoteLine := lipgloss.NewStyle().
				Foreground(t.TextMuted).
				PaddingLeft(3).
				Width(contentW).
				Render(s.remote)
			lines = append(lines, remoteLine)
		}

		// Summary counts (modified/added/deleted)
		modCount, addCount, delCount, untracked := countFileStatuses(s.files)
		if modCount+addCount+delCount+untracked > 0 {
			countLine := lipgloss.JoinHorizontal(lipgloss.Top,
				lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(1).Render(fmt.Sprintf("● %d", modCount)),
				lipgloss.NewStyle().Foreground(t.Success).Render(fmt.Sprintf(" +%d", addCount)),
				lipgloss.NewStyle().Foreground(t.Error).Render(fmt.Sprintf(" -%d", delCount)),
			)
			if untracked > 0 {
				countLine += lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf(" ?%d", untracked))
			}
			lines = append(lines, countLine)
		}
		lines = append(lines, "")
	}

	// ── Files section ──────────────────────────────────────────────────────────
	filesLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(true).
		PaddingLeft(1).
		Render("FILES")
	lines = append(lines, filesLabel)

	if len(s.files) == 0 {
		noFiles := lipgloss.NewStyle().
			Foreground(t.Success).
			Italic(true).
			PaddingLeft(2).
			Width(contentW).
			Render("✓ working tree clean")
		lines = append(lines, noFiles)
	} else {
		// Group files by status
		groups := groupFilesByStatus(s.files)
		groupOrder := []struct {
			key   string
			icon  string
			color lipgloss.Color
		}{
			{"modified", "●", t.Warning},
			{"added", "+", t.Success},
			{"deleted", "−", t.Error},
			{"renamed", "→", t.TextSecondary},
			{"untracked", "?", t.TextMuted},
			{"other", "·", t.TextMuted},
		}
		for _, g := range groupOrder {
			paths, ok := groups[g.key]
			if !ok || len(paths) == 0 {
				continue
			}
			// Group header
			header := lipgloss.NewStyle().
				Foreground(g.color).
				PaddingLeft(1).
				Render(g.icon + " " + g.key)
			lines = append(lines, header)

			// Files in this group
			for _, p := range paths {
				name := TruncateWithEllipsis(p, contentW-6)
				line := lipgloss.NewStyle().
					Foreground(t.TextSecondary).
					PaddingLeft(4).
					Render(name)
				lines = append(lines, line)
			}
		}
	}

	content := strings.Join(lines, "\n")

	// Render the sidebar panel at the correct width
	panel := lipgloss.NewStyle().
		Width(contentW).
		Render(content)

	// Build a vertical right border string matching the content height
	lineCount := len(lines)
	rightBorderStr := strings.Repeat("│\n", lineCount)
	rightBorderStr = strings.TrimSuffix(rightBorderStr, "\n")
	rightBorder := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(rightBorderStr)

	return lipgloss.JoinHorizontal(lipgloss.Top, panel, rightBorder)
}

// groupFilesByStatus groups file paths by their git status category.
func groupFilesByStatus(files []git.FileStatus) map[string][]string {
	groups := make(map[string][]string)
	for _, f := range files {
		group := statusGroup(f.Status)
		groups[group] = append(groups[group], f.Path)
	}
	return groups
}

func statusGroup(status string) string {
	switch {
	case strings.Contains(status, "M"):
		return "modified"
	case strings.Contains(status, "A"):
		return "added"
	case strings.Contains(status, "D"):
		return "deleted"
	case strings.Contains(status, "R"):
		return "renamed"
	case strings.Contains(status, "?"):
		return "untracked"
	default:
		return "other"
	}
}

func fileStatusIcon(status string, t theme.Theme) (string, lipgloss.Color) {
	switch {
	case strings.Contains(status, "M"):
		return "●", t.Warning
	case strings.Contains(status, "A"):
		return "+", t.Success
	case strings.Contains(status, "D"):
		return "−", t.Error
	case strings.Contains(status, "R"):
		return "→", t.TextSecondary
	case strings.Contains(status, "?"):
		return "?", t.TextMuted
	default:
		return "·", t.TextMuted
	}
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
