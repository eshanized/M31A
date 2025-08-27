package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
)

const sidebarWidth = 42

// sidebarStatusCacheTTL is how long the sidebar caches git status
// to avoid hammering git on every TickMsg (60Hz).
const sidebarStatusCacheTTL = 1 * time.Second

type SidebarRefreshMsg struct {
	Statuses []git.FileStatus
	Err      error
}

type SidebarModel struct {
	theme             theme.Theme
	git               *git.Git
	statuses          []git.FileStatus
	err               string
	loading           bool
	width             int
	height            int
	visible           bool
	lastStatusFetch   time.Time
	gitStatusCache    []git.FileStatus
	gitStatusCacheErr error
	spinner           spinner.Model
}

func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &SidebarModel{
		theme:   t,
		git:     g,
		width:   sidebarWidth,
		loading: true,
		spinner: sp,
	}
}

func (m *SidebarModel) Init() tea.Cmd {
	return tea.Batch(m.refreshCmd(), m.spinner.Tick)
}

func (m *SidebarModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.height = msg.Height
		if m.width == 0 {
			m.width = sidebarWidth
		}
	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd
	case SidebarRefreshMsg:
		m.loading = false
		if msg.Err != nil {
			m.err = msg.Err.Error()
			m.gitStatusCacheErr = msg.Err
		} else {
			m.statuses = msg.Statuses
			m.gitStatusCache = msg.Statuses
			m.gitStatusCacheErr = nil
			m.lastStatusFetch = time.Now()
			m.err = ""
		}
	}
	return m, nil
}

func (m *SidebarModel) View() string {
	if !m.visible {
		return ""
	}

	w := m.width
	if w <= 0 {
		w = sidebarWidth
	}

	var lines []string

	headerStyle := lipgloss.NewStyle().Bold(true).Foreground(m.theme.TextPrimary).Padding(0, 1)
	lines = append(lines, headerStyle.Render("MODIFIED"))

	sep := lipgloss.NewStyle().Foreground(m.theme.Border).Render(strings.Repeat("─", w-2))
	lines = append(lines, sep)

	if m.loading {
		lines = append(lines, "  "+m.spinner.View()+" loading...")
	} else if m.err != "" {
		lines = append(lines, lipgloss.NewStyle().Foreground(m.theme.Error).Padding(0, 1).Render(m.err))
	} else if len(m.statuses) == 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Padding(0, 1).Render("no changes"))
	} else {
		for _, fs := range m.statuses {
			line := m.renderFileStatus(fs, w-2)
			lines = append(lines, line)
		}
	}

	content := lipgloss.JoinVertical(lipgloss.Top, lines...)
	panel := lipgloss.NewStyle().Width(w).Height(m.height).Background(m.theme.Surface).Padding(0, 1).Render(content)

	return panel
}

func (m *SidebarModel) Toggle() {
	m.visible = !m.visible
}

func (m *SidebarModel) SetVisible(v bool) {
	m.visible = v
}

func (m *SidebarModel) IsVisible() bool {
	return m.visible
}

func (m *SidebarModel) SetTheme(t theme.Theme) {
	m.theme = t
}

func (m *SidebarModel) refreshCmd() tea.Cmd {
	// BUG-03 fix: capture cache values in closure for read-only access;
	// cache writes happen in Update() on the main thread only.
	lastFetch := m.lastStatusFetch
	cachedStatuses := m.gitStatusCache
	cachedErr := m.gitStatusCacheErr
	return func() tea.Msg {
		if m.git == nil || !m.git.IsRepo() {
			return SidebarRefreshMsg{Statuses: nil, Err: nil}
		}
		// L-17: Use 1-second cache to avoid hammering git on every TickMsg
		if !lastFetch.IsZero() && time.Since(lastFetch) < sidebarStatusCacheTTL {
			return SidebarRefreshMsg{Statuses: cachedStatuses, Err: cachedErr}
		}
		statuses, err := m.git.StatusPorcelain()
		return SidebarRefreshMsg{Statuses: statuses, Err: err}
	}
}

func (m *SidebarModel) renderFileStatus(fs git.FileStatus, width int) string {
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

	statusStr := lipgloss.NewStyle().Foreground(statusColor).Bold(true).Render(statusChar)

	pathStr := fs.Path
	if fs.Status == "R" && fs.OldPath != "" {
		pathStr = fs.OldPath + " → " + fs.Path
	}
	maxPathLen := width - 4
	pathStr = TruncateWithEllipsis(pathStr, maxPathLen)

	pathStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary)

	line := statusStr + " " + pathStyle.Render(pathStr)

	if fs.Additions > 0 || fs.Deletions > 0 {
		statsWidth := width - lipgloss.Width(line) - 2
		if statsWidth > 0 {
			statsStr := m.renderDiffStats(fs.Additions, fs.Deletions, statsWidth)
			line += " " + statsStr
		}
	}

	return lipgloss.NewStyle().Padding(0, 1).Render(line)
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
