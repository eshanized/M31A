package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// VerifyModel displays verification results for each task.
type VerifyModel struct {
	theme    theme.Theme
	tasks    []types.Task
	results  map[int]workflow.VerificationResult
	selected int
	width    int
	height   int
	spinner  spinner.Model
}

// NewVerifyModel creates a Verify screen model. width/height are
// required non-zero dimensions so the screen renders immediately
// on creation without waiting for a separate WindowSizeMsg (D-03 fix).
func NewVerifyModel(tasks []types.Task, results map[int]workflow.VerificationResult, t theme.Theme, width, height int) *VerifyModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	return &VerifyModel{
		theme:   t,
		tasks:   tasks,
		results: results,
		width:   width,
		height:  height,
		spinner: sp,
	}
}

func (m *VerifyModel) Init() tea.Cmd {
	return m.spinner.Tick
}

func (m *VerifyModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
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
		case "up", "k":
			if m.selected > 0 {
				m.selected--
			}
		case "down", "j":
			if m.selected < len(m.tasks)-1 {
				m.selected++
			}
		case "h", "H":
			// Self-heal selected task: reset failed task to pending for re-execution
			// H-13: bounds check covers both single-task and multi-task selections.
			if m.selected >= 0 && m.selected < len(m.tasks) && m.tasks[m.selected].Status == types.StatusFailed {
				m.tasks[m.selected].Status = types.StatusPending
			}
		case "s", "S":
			// Skip selected task
			// H-13: bounds check covers both single-task and multi-task selections.
			if m.selected >= 0 && m.selected < len(m.tasks) {
				m.tasks[m.selected].Status = types.StatusSkipped
			}
		}

		// Auto-transition to Ship if all done
		allDone := true
		for _, t := range m.tasks {
			if t.Status == types.StatusPending || t.Status == types.StatusRunning {
				allDone = false
				break
			}
		}
		if allDone {
			return nil, &AppMsg{Screen: ScreenShip}
		}
	}
	return nil, nil
}

func (m *VerifyModel) View() string {
	if m.width == 0 {
		return m.spinner.View() + " Loading verify..."
	}

	var sb strings.Builder

	// Header
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render(" Verify "))
	sb.WriteString("\n\n")

	// Task verification results
	for i, task := range m.tasks {
		prefix := "[ ]"
		if task.Status == types.StatusDone {
			prefix = "[x]"
		} else if task.Status == types.StatusUnrecoverable {
			prefix = "[!]"
		}

		if i == m.selected {
			prefix = lipgloss.NewStyle().
				Foreground(m.theme.Brand).
				Render(prefix)
		}

		sb.WriteString(fmt.Sprintf("%s %d. %s\n", prefix, task.ID, task.Description))

		// Show verification result
		if result, ok := m.results[task.ID]; ok {
			if result.FilesExist {
				sb.WriteString(m.checkmark("Files exist"))
			} else {
				sb.WriteString(m.cross("Files missing"))
			}
			if result.SyntaxOK {
				sb.WriteString(m.checkmark("Syntax OK"))
			} else {
				sb.WriteString(m.cross("Syntax error"))
			}
			if result.TestsOK {
				sb.WriteString(m.checkmark("Tests pass"))
			} else if len(result.Errors) > 0 {
				sb.WriteString(m.cross("Tests failed"))
			}

			if !result.FilesExist || !result.SyntaxOK || !result.TestsOK {
				if task.Status == types.StatusUnrecoverable {
					sb.WriteString(lipgloss.NewStyle().
						Foreground(m.theme.Error).
						Bold(true).
						Render("  [UNRECOVERABLE]"))
					sb.WriteString("\n")
				} else {
					sb.WriteString(lipgloss.NewStyle().
						Foreground(m.theme.TextSecondary).
						Render("  [H] Self-heal"))
					sb.WriteString("\n")
				}
			}
		}

		sb.WriteString("\n")
	}

	// Keys
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("H=Self-heal  S=Skip"))

	return sb.String()
}

func (m *VerifyModel) checkmark(text string) string {
	return lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Render(fmt.Sprintf("    ✓ %s\n", text))
}

func (m *VerifyModel) cross(text string) string {
	return lipgloss.NewStyle().
		Foreground(m.theme.Error).
		Render(fmt.Sprintf("    ✗ %s\n", text))
}
