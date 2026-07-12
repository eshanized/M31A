package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/decision"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DecisionScreen is a lightweight Screenable that renders the decision log.
// It reads decisions from a cached slice set by the AppState via SetDecisions.
type DecisionScreen struct {
	decisions []decision.DecisionReceipt
	theme     theme.Theme
	width     int
	height    int
}

// NewDecisionScreen creates a DecisionScreen.
func NewDecisionScreen(t theme.Theme, w, h int) *DecisionScreen {
	return &DecisionScreen{
		theme:  t,
		width:  w,
		height: h,
	}
}

// SetDecisions updates the decisions to display.
func (ds *DecisionScreen) SetDecisions(decisions []decision.DecisionReceipt) {
	ds.decisions = decisions
}

// SetDimensions updates the screen dimensions.
func (ds *DecisionScreen) SetDimensions(w, h int) {
	ds.width = w
	ds.height = h
}

// SetTheme updates the theme.
func (ds *DecisionScreen) SetTheme(t theme.Theme) {
	ds.theme = t
}

// Init implements Screenable.
func (ds *DecisionScreen) Init() tea.Cmd { return nil }

// Update implements Screenable.
func (ds *DecisionScreen) Update(msg tea.Msg) (Screenable, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		ds.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return ds, func() tea.Msg { return PopScreenMsg{} }
		}
	}
	return ds, nil
}

// View implements Screenable. Renders the decision log table.
func (ds *DecisionScreen) View() string {
	width := ds.width
	height := ds.height
	t := ds.theme

	decisions := decision.RedactSlice(ds.decisions)
	if len(decisions) == 0 {
		return renderEmptyState("Decisions", "No decisions recorded yet — start a workflow with /new", width, height, t)
	}

	var b strings.Builder
	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(t.Primary).
		MarginBottom(1)

	headerStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(t.TextPrimary).
		BorderBottom(true).
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(t.BorderSubtle)

	rowStyle := lipgloss.NewStyle().
		Foreground(t.TextPrimary)

	catStyle := map[decision.Category]lipgloss.Style{
		decision.CategoryTool:     lipgloss.NewStyle().Foreground(lipgloss.Color("6")),
		decision.CategoryPlan:     lipgloss.NewStyle().Foreground(lipgloss.Color("5")),
		decision.CategoryIntent:   lipgloss.NewStyle().Foreground(lipgloss.Color("4")),
		decision.CategoryRetry:    lipgloss.NewStyle().Foreground(lipgloss.Color("3")),
		decision.CategoryStrategy: lipgloss.NewStyle().Foreground(lipgloss.Color("2")),
		decision.CategoryModel:    lipgloss.NewStyle().Foreground(lipgloss.Color("133")),
	}

	b.WriteString(titleStyle.Render("Decision Log"))
	b.WriteString("\n\n")

	b.WriteString(headerStyle.Render(fmt.Sprintf("%-20s %-8s %s", "Time", "Category", "Decision")))
	b.WriteString("\n")

	for _, d := range decisions {
		ts := d.Timestamp.Format("15:04:05")
		cat := string(d.Category)
		if s, ok := catStyle[d.Category]; ok {
			cat = s.Render(cat)
		}
		dec := d.Decision
		if len(dec) > width-40 {
			dec = dec[:width-43] + "..."
		}
		b.WriteString(rowStyle.Render(fmt.Sprintf("%-20s %-8s %s", ts, cat, dec)))
		b.WriteString("\n")
	}

	totalCost := decision.CostSummary(decisions)
	summaryStyle := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		MarginTop(1)
	b.WriteString(summaryStyle.Render(fmt.Sprintf(
		"%d decisions | %d tokens | %.1fs total duration",
		len(decisions), totalCost.Tokens, totalCost.Duration,
	)))

	return lipgloss.NewStyle().
		Width(width).
		Height(height).
		Render(b.String())
}
