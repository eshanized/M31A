package phase_transition


import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// PhaseTransitionModel displays a confirmation screen between workflow phases.
// Shows what was accomplished in the previous phase and asks whether to proceed.
type PhaseTransitionModel struct {
	theme     theme.Theme
	fromPhase types.WorkflowPhase
	toPhase   types.WorkflowPhase
	summary   string
	width     int
	height    int
	cursor    int // 0=proceed, 1=go back, 2=cancel
}

// NewPhaseTransitionModel creates a new phase transition confirmation screen.
func NewPhaseTransitionModel(t theme.Theme, from, to types.WorkflowPhase, summary string, w, h int) *PhaseTransitionModel {
	return &PhaseTransitionModel{
		theme:     t,
		fromPhase: from,
		toPhase:   to,
		summary:   summary,
		width:     w,
		height:    h,
		cursor:    0,
	}
}

func (m *PhaseTransitionModel) Init() tea.Cmd { return nil }

func (m *PhaseTransitionModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "up", "k":
			if m.cursor > 0 {
				m.cursor--
			}
		case "down", "j":
			if m.cursor < 2 {
				m.cursor++
			}
		case "1":
			m.cursor = 0
		case "2":
			m.cursor = 1
		case "3":
			m.cursor = 2
		case "enter", " ":
			switch m.cursor {
			case 0: // Proceed
				return m, func() tea.Msg {
					return PhaseTransitionMsg{
						Approved: true,
						From:     m.fromPhase,
						To:       m.toPhase,
					}
				}
			case 1: // Go back
				return m, func() tea.Msg {
					return PhaseTransitionMsg{
						Approved: false,
						GoBack:   true,
						From:     m.fromPhase,
						To:       m.toPhase,
					}
				}
			case 2: // Cancel
				return m, func() tea.Msg {
					return PhaseTransitionMsg{
						Approved: false,
						Cancel:   true,
						From:     m.fromPhase,
						To:       m.toPhase,
					}
				}
			}
		case "esc", "q", "n":
			return m, func() tea.Msg {
				return PhaseTransitionMsg{
					Approved: false,
					Cancel:   true,
					From:     m.fromPhase,
					To:       m.toPhase,
				}
			}
		case "y", "p":
			return m, func() tea.Msg {
				return PhaseTransitionMsg{
					Approved: true,
					From:     m.fromPhase,
					To:       m.toPhase,
				}
			}
		case "b":
			return m, func() tea.Msg {
				return PhaseTransitionMsg{
					Approved: false,
					GoBack:   true,
					From:     m.fromPhase,
					To:       m.toPhase,
				}
			}
		}
	}
	return m, nil
}

func (m *PhaseTransitionModel) View() string {
	t := m.theme

	var sb strings.Builder

	// Title
	titleStyle := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Padding(0, 1)
	sb.WriteString(titleStyle.Render(fmt.Sprintf("Phase Transition: %s → %s", m.fromPhase, m.toPhase)))
	sb.WriteString("\n\n")

	// Summary of what was accomplished
	if m.summary != "" {
		summaryStyle := lipgloss.NewStyle().
			Foreground(t.TextPrimary).
			Padding(0, 2)
		sb.WriteString(summaryStyle.Render("What was accomplished:"))
		sb.WriteString("\n")
		for _, line := range strings.Split(m.summary, "\n") {
			sb.WriteString(summaryStyle.Render("  " + line))
			sb.WriteString("\n")
		}
		sb.WriteString("\n")
	}

	// Options
	options := []string{
		fmt.Sprintf("Proceed to %s", m.toPhase),
		fmt.Sprintf("Go back to %s", m.fromPhase),
		"Cancel workflow",
	}

	for i, opt := range options {
		cursor := "  "
		if i == m.cursor {
			cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
		}
		style := lipgloss.NewStyle().Foreground(t.TextSecondary)
		if i == m.cursor {
			style = lipgloss.NewStyle().Foreground(t.TextPrimary)
		}
		sb.WriteString(cursor + style.Render(opt))
		sb.WriteString("\n")
	}

	// Footer hints
	sb.WriteString("\n")
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	sb.WriteString(hintStyle.Render("  [y] Proceed  [b] Go back  [n] Cancel  [↑↓] Navigate"))

	return sb.String()
}

// SetDimensions updates the model dimensions.
func (m *PhaseTransitionModel) SetDimensions(w, h int) {
	m.width = w
	m.height = h
}

// SetTheme updates the model's theme.
func (m *PhaseTransitionModel) SetTheme(t theme.Theme) {
	m.theme = t
}

// PhaseTransitionMsg is emitted when the user makes a decision on phase transition.
type PhaseTransitionMsg struct {
	Approved bool
	GoBack   bool
	Cancel   bool
	From     types.WorkflowPhase
	To       types.WorkflowPhase
}

// phaseTransitionSummary generates a human-readable summary of what was accomplished in a phase.
func phaseTransitionSummary(phase types.WorkflowPhase, tasks []types.Task) string {
	switch phase {
	case types.PhaseDiscuss:
		return "Project requirements gathered and clarified."
	case types.PhasePlan:
		return fmt.Sprintf("Plan generated with %d tasks.", len(tasks))
	case types.PhaseExecute:
		done := 0
		for _, t := range tasks {
			if t.Status == types.StatusDone {
				done++
			}
		}
		return fmt.Sprintf("Execution complete: %d/%d tasks done.", done, len(tasks))
	case types.PhaseVerify:
		return "Verification checks passed."
	case types.PhaseRuntime:
		return "Runtime smoke tests completed."
	default:
		return ""
	}
}
