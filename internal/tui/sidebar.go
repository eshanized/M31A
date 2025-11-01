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
)

// SidebarModel manages the collapsible sidebar panel that shows git status.
type SidebarModel struct {
	git     *git.Git
	theme   theme.Theme
	files   []git.FileStatus
	branch  string
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
		files, err := g.StatusPorcelain()
		if err != nil {
			return SidebarRefreshMsg{Branch: branch}
		}
		sidebarFiles := make([]SidebarFile, 0, len(files))
		for _, f := range files {
			sidebarFiles = append(sidebarFiles, SidebarFile{
				Path:   f.Path,
				Status: f.Status,
			})
		}
		return SidebarRefreshMsg{Files: sidebarFiles, Branch: branch}
	}
}

// Update handles sidebar-specific messages.
func (s *SidebarModel) Update(msg tea.Msg) (*SidebarModel, tea.Cmd) {
	switch msg := msg.(type) {
	case SidebarRefreshMsg:
		s.branch = msg.Branch
		files := make([]git.FileStatus, 0, len(msg.Files))
		for _, f := range msg.Files {
			files = append(files, git.FileStatus{Status: f.Status, Path: f.Path})
		}
		s.files = files
		s.loading = false
	}
	return s, nil
}

// View renders the sidebar.
func (s *SidebarModel) View() string {
	if !s.visible {
		return ""
	}
	t := s.theme
	w := s.width

	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Padding(0, 1).
		Width(w).
		Render("◈ Files")

	divider := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(strings.Repeat("─", w))

	var lines []string
	lines = append(lines, title)
	lines = append(lines, divider)

	// Branch
	if s.branch != "" {
		branchLine := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			PaddingLeft(1).
			Width(w).
			Render("⎇ " + s.branch)
		lines = append(lines, branchLine)
		lines = append(lines, "")
	}

	// Files
	if len(s.files) == 0 {
		noFiles := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(1).
			Width(w).
			Render("no changes")
		lines = append(lines, noFiles)
	} else {
		for _, f := range s.files {
			icon, color := fileStatusIcon(f.Status, t)
			name := TruncateWithEllipsis(f.Path, w-5)
			line := lipgloss.NewStyle().Foreground(color).PaddingLeft(1).
				Render(fmt.Sprintf("%s %s", icon, name))
			lines = append(lines, line)
		}
	}

	content := strings.Join(lines, "\n")
	return lipgloss.NewStyle().
		Width(w).
		Render(content)
}

func fileStatusIcon(status string, t theme.Theme) (string, lipgloss.Color) {
	switch {
	case strings.Contains(status, "M"):
		return "M", t.Warning
	case strings.Contains(status, "A"):
		return "+", t.Success
	case strings.Contains(status, "D"):
		return "-", t.Error
	case strings.Contains(status, "?"):
		return "?", t.TextMuted
	default:
		return "·", t.TextMuted
	}
}
