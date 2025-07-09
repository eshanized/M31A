package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ShipSummary holds session completion data.
type ShipSummary struct {
	TaskDone      int
	TaskTotal     int
	TaskFailed    int
	TaskSkipped   int
	Commits       []git.CommitInfo
	Duration      string
	SessionID     string
	FilesAdded    int
	FilesModified int
	FilesDeleted  int
	Insertions    int
	Deletions     int
	TotalTokens   int
	TotalCost     float64
}

// ShipModel displays the session completion summary.
type ShipModel struct {
	theme   theme.Theme
	summary ShipSummary
	width   int
	height  int
	spinner spinner.Model
}

// NewShipModel creates a Ship screen model. width/height are required
// non-zero dimensions so the screen renders immediately on creation
// without waiting for a separate WindowSizeMsg (D-03 fix).
func NewShipModel(summary ShipSummary, t theme.Theme, width, height int) *ShipModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &ShipModel{
		theme:   t,
		summary: summary,
		width:   width,
		height:  height,
		spinner: sp,
	}
}

func (m *ShipModel) Init() tea.Cmd {
	return m.spinner.Tick
}

func (m *ShipModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "n", "N":
			// New session
			return nil, &AppMsg{Screen: ScreenFirstRun}
		case "r", "R":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}
	return nil, nil
}

func (m *ShipModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading ship..."
	}

	var sb strings.Builder

	// Celebration header
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Bold(true).
		Render(" ✓ Session Complete!"))
	sb.WriteString("\n\n")

	// Session info with badges
	sessionBadge := components.NewBadge(m.summary.SessionID, components.BadgeMuted).Render()
	sb.WriteString(fmt.Sprintf("Session %s  Duration: %s\n\n", sessionBadge, m.summary.Duration))

	// Metrics dashboard
	sb.WriteString(m.renderMetrics())
	sb.WriteString("\n\n")

	// Task summary with segmented bar
	sb.WriteString(m.renderTaskSummary())
	sb.WriteString("\n\n")

	// File changes summary
	if m.summary.FilesAdded > 0 || m.summary.FilesModified > 0 || m.summary.FilesDeleted > 0 {
		sb.WriteString(m.renderFileChanges())
		sb.WriteString("\n\n")
	}

	// Commits
	if len(m.summary.Commits) > 0 {
		sb.WriteString(lipgloss.NewStyle().
			Bold(true).
			Foreground(m.theme.Brand).
			Render("Commits"))
		sb.WriteString("\n")

		for _, c := range m.summary.Commits {
			hashStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
			sb.WriteString(fmt.Sprintf("  %s %s\n", hashStyle.Render(c.ShortHash), c.Message))
		}
		sb.WriteString("\n")
	}

	// Next actions
	sb.WriteString(m.renderNextActions())

	// Keys
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[N] New session  [R] REPL  [Esc] Back"))

	return sb.String()
}

// renderMetrics renders the session metrics dashboard.
func (m *ShipModel) renderMetrics() string {
	metrics := []components.MetricCard{
		{Value: fmt.Sprintf("%d/%d", m.summary.TaskDone, m.summary.TaskTotal), Label: "Tasks"},
		{Value: fmt.Sprintf("%d", len(m.summary.Commits)), Label: "Commits"},
	}

	if m.summary.TotalTokens > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatMetric(m.summary.TotalTokens),
			Label: "Tokens",
		})
	}

	if m.summary.TotalCost > 0 {
		metrics = append(metrics, components.MetricCard{
			Value: components.FormatCost(m.summary.TotalCost),
			Label: "Cost",
		})
	}

	return components.MetricRow(metrics, m.width-4)
}

// renderTaskSummary renders the task summary with a segmented bar.
func (m *ShipModel) renderTaskSummary() string {
	t := theme.Default()

	// Segmented bar
	segBar := components.SegmentedBar{
		Segments: []components.Segment{
			{Count: m.summary.TaskDone, Color: t.Success, Label: "done"},
			{Count: m.summary.TaskFailed, Color: t.Error, Label: "failed"},
			{Count: m.summary.TaskSkipped, Color: t.Warning, Label: "skipped"},
		},
		Width: m.width - 4,
	}

	// Stats row
	stats := components.StatGroup{
		Stats: []components.StatRow{
			{Label: "Done", Value: fmt.Sprintf("%d", m.summary.TaskDone), Icon: "✓"},
			{Label: "Failed", Value: fmt.Sprintf("%d", m.summary.TaskFailed), Icon: "✗"},
			{Label: "Skipped", Value: fmt.Sprintf("%d", m.summary.TaskSkipped), Icon: "-"},
		},
		Width: m.width - 4,
	}

	return segBar.Render() + "\n" + stats.Render()
}

// renderFileChanges renders the file changes summary.
func (m *ShipModel) renderFileChanges() string {
	t := theme.Default()

	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Render("File Changes")

	stats := []components.StatRow{
		{Label: "Added", Value: fmt.Sprintf("%d", m.summary.FilesAdded), Icon: "+"},
		{Label: "Modified", Value: fmt.Sprintf("%d", m.summary.FilesModified), Icon: "~"},
		{Label: "Deleted", Value: fmt.Sprintf("%d", m.summary.FilesDeleted), Icon: "-"},
	}

	// Color-code the stats
	for i := range stats {
		switch i {
		case 0:
			stats[i].Label = lipgloss.NewStyle().Foreground(t.Success).Render(stats[i].Label)
		case 1:
			stats[i].Label = lipgloss.NewStyle().Foreground(t.Thinking).Render(stats[i].Label)
		case 2:
			stats[i].Label = lipgloss.NewStyle().Foreground(t.Error).Render(stats[i].Label)
		}
	}

	parts := make([]string, len(stats))
	for i, s := range stats {
		parts[i] = s.Render()
	}

	// Diff stat bar
	diffStat := components.SegmentedBar{
		Segments: []components.Segment{
			{Count: m.summary.Insertions, Color: t.Success},
			{Count: m.summary.Deletions, Color: t.Error},
		},
		Width: m.width - 8,
	}

	return header + "\n" + strings.Join(parts, "  ") + "\n" + diffStat.Render()
}

// renderNextActions renders suggested next actions.
func (m *ShipModel) renderNextActions() string {
	header := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.TextSecondary).
		Render("Suggested next actions")

	actions := []string{
		"Run tests to verify changes",
		"Review changes with /diff",
		"Commit any remaining changes",
	}

	actionStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1)

	actionLines := make([]string, len(actions))
	for i, a := range actions {
		dot := lipgloss.NewStyle().Foreground(m.theme.Brand).Render("•")
		actionLines[i] = actionStyle.Render(dot + " " + a)
	}

	return header + "\n" + strings.Join(actionLines, "\n")
}
