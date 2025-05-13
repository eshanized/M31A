package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

const sidebarWidth = 42

// SidebarRefreshMsg is emitted when git status has been fetched.
type SidebarRefreshMsg struct {
	Statuses []git.FileStatus
	Err      error
}

// SidebarModel renders a panel showing git-modified files.
type SidebarModel struct {
	theme    theme.Theme
	git      *git.Git
	statuses []git.FileStatus
	loading  bool
	err      string
	width    int
	height   int
	visible  bool
}

func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	return &SidebarModel{
		theme:   t,
		git:     g,
		width:   sidebarWidth,
		loading: true,
	}
}

// Init starts fetching git status.
func (m *SidebarModel) Init() tea.Cmd {
	return m.refreshCmd()
}

// Update handles messages for the sidebar.
func (m *SidebarModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.height = msg.Height
		if m.width == 0 {
			m.width = sidebarWidth
		}
	case SidebarRefreshMsg:
		m.loading = false
		if msg.Err != nil {
			m.err = msg.Err.Error()
		} else {
			m.statuses = msg.Statuses
			m.err = ""
		}
	}
	return m, nil
}

// View renders the sidebar panel.
func (m *SidebarModel) View() string {
	if !m.visible {
		return ""
	}

	w := m.width
	if w <= 0 {
		w = sidebarWidth
	}

	var lines []string

	// Header
	headerStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextPrimary).
		Padding(0, 1)
	lines = append(lines, headerStyle.Render("MODIFIED"))

	// Separator
	sep := lipgloss.NewStyle().
		Foreground(m.theme.Border).
		Render(strings.Repeat("─", w-2))
	lines = append(lines, sep)

	if m.loading {
		lines = append(lines, "  loading...")
	} else if m.err != "" {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.Error).
			Padding(0, 1).
			Render(m.err))
	} else if len(m.statuses) == 0 {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Padding(0, 1).
			Render("no changes"))
	} else {
		for _, fs := range m.statuses {
			line := m.renderFileStatus(fs, w-2)
			lines = append(lines, line)
		}
	}

	// Pad to fill height
	content := lipgloss.JoinVertical(lipgloss.Top, lines...)
	panel := lipgloss.NewStyle().
		Width(w).
		Height(m.height).
		Background(m.theme.Surface).
		Padding(0, 1).
		Render(content)

	return panel
}

// Toggle flips the visibility state.
func (m *SidebarModel) Toggle() {
	m.visible = !m.visible
}

// SetVisible sets the visibility explicitly.
func (m *SidebarModel) SetVisible(v bool) {
	m.visible = v
}

// IsVisible returns current visibility.
func (m *SidebarModel) IsVisible() bool {
	return m.visible
}

// SetTheme updates the sidebar theme and triggers a re-render.
func (m *SidebarModel) SetTheme(t theme.Theme) {
	m.theme = t
}

// refreshCmd returns a tea.Cmd that fetches git status asynchronously.
func (m *SidebarModel) refreshCmd() tea.Cmd {
	return func() tea.Msg {
		if m.git == nil || !m.git.IsRepo() {
			return SidebarRefreshMsg{Statuses: nil, Err: nil}
		}
		statuses, err := m.git.StatusPorcelain()
		return SidebarRefreshMsg{Statuses: statuses, Err: err}
	}
}

func (m *SidebarModel) renderFileStatus(fs git.FileStatus, width int) string {
	// Status indicator
	statusChar := fs.Status
	statusColor := m.theme.TextSecondary
	switch statusChar {
	case "M":
		statusColor = m.theme.Warning
	case "A":
		statusColor = m.theme.Success
	case "D":
		statusColor = m.theme.Error
	case "R":
		statusColor = m.theme.Thinking
	case "?":
		statusColor = m.theme.TextSecondary
	}

	statusStr := lipgloss.NewStyle().
		Foreground(statusColor).
		Bold(true).
		Render(statusChar)

	// Path (truncated to fit)
	pathStr := fs.Path
	if fs.Status == "R" && fs.OldPath != "" {
		pathStr = fs.OldPath + " → " + fs.Path
	}
	maxPathLen := width - 4 // leave room for status char and padding
	if lipgloss.Width(pathStr) > maxPathLen {
		runes := []rune(pathStr)
		if len(runes) > maxPathLen-3 {
			pathStr = string(runes[:maxPathLen-3]) + "..."
		}
	}

	pathStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextPrimary)

	line := statusStr + " " + pathStyle.Render(pathStr)

	// Diff stats if available
	if fs.Additions > 0 || fs.Deletions > 0 {
		statsWidth := width - lipgloss.Width(line) - 2
		if statsWidth > 0 {
			statsStr := m.renderDiffStats(fs.Additions, fs.Deletions, statsWidth)
			line += " " + statsStr
		}
	}

	return lipgloss.NewStyle().
		Padding(0, 1).
		Render(line)
}

func (m *SidebarModel) renderDiffStats(add, del int, maxWidth int) string {
	parts := []string{}
	if add > 0 {
		parts = append(parts, fmt.Sprintf("+%d", add))
	}
	if del > 0 {
		parts = append(parts, fmt.Sprintf("-%d", del))
	}
	if len(parts) == 0 {
		return ""
	}
	result := strings.Join(parts, " ")
	if len(result) > maxWidth {
		return ""
	}

	addStyle := lipgloss.NewStyle().Foreground(m.theme.Success)
	delStyle := lipgloss.NewStyle().Foreground(m.theme.Error)

	var out []string
	if add > 0 {
		out = append(out, addStyle.Render(fmt.Sprintf("+%d", add)))
	}
	if del > 0 {
		out = append(out, delStyle.Render(fmt.Sprintf("-%d", del)))
	}
	return strings.Join(out, " ")
}
