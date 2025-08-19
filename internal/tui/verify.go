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
	theme           theme.Theme
	tasks           []types.Task
	results         map[int]workflow.VerificationResult
	selected        int
	width           int
	height          int
	spinner         spinner.Model
	confirmHeal     bool // awaiting self-heal confirmation
	confirmHealTask int  // task ID being confirmed for heal
	sessionID       string
	healFunc        func(taskID int) tea.Cmd // callback to trigger self-healing
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

// UpdateResults replaces the verification results map.
func (m *VerifyModel) UpdateResults(results map[int]workflow.VerificationResult) {
	m.results = results
}

// SetHealFunc sets the callback invoked when self-heal is confirmed.
func (m *VerifyModel) SetHealFunc(fn func(taskID int) tea.Cmd) {
	m.healFunc = fn
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
		// Handle self-heal confirmation
		if m.confirmHeal {
			switch msg.String() {
			case "y", "Y", "enter":
				// Confirm self-heal: reset task to pending, then trigger healing
				for i := range m.tasks {
					if m.tasks[i].ID == m.confirmHealTask && m.tasks[i].Status == types.StatusFailed {
						m.tasks[i].Status = types.StatusPending
						break
					}
				}
				m.confirmHeal = false
				if m.healFunc != nil {
					return []tea.Cmd{m.healFunc(m.confirmHealTask)}, nil
				}
				return nil, nil
			case "n", "N", "esc":
				m.confirmHeal = false
				return nil, nil
			}
			return nil, nil
		}

		switch msg.String() {
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		case "up", "k":
			if m.selected > 0 {
				m.selected--
			}
		case "down", "j":
			if m.selected < len(m.tasks)-1 {
				m.selected++
			}
		case "h", "H":
			// Self-heal: show confirmation before resetting
			if m.selected >= 0 && m.selected < len(m.tasks) && m.tasks[m.selected].Status == types.StatusFailed {
				m.confirmHeal = true
				m.confirmHealTask = m.tasks[m.selected].ID
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

	// Show self-heal confirmation dialog
	if m.confirmHeal {
		return m.renderHealConfirmation()
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

			// Show error details for failed checks
			if len(result.Errors) > 0 {
				for _, err := range result.Errors {
					sb.WriteString(lipgloss.NewStyle().
						Foreground(m.theme.Error).
						Render(fmt.Sprintf("    %s\n", err)))
				}
			}

		if !result.FilesExist || !result.SyntaxOK || !result.TestsOK {
			if task.Status == types.StatusUnrecoverable {
				sb.WriteString(lipgloss.NewStyle().
					Foreground(m.theme.Error).
					Bold(true).
					Render("  [UNRECOVERABLE]"))
				sb.WriteString("\n")
				sb.WriteString(lipgloss.NewStyle().
					Foreground(m.theme.TextSecondary).
					Render("  Run git bisect to find the issue · Check /rollback or fix manually"))
				sb.WriteString("\n")
			} else {
				healHint := "[H] Self-heal"
				if task.HealsAttempted > 0 {
					remaining := 2 - task.HealsAttempted
					if remaining > 0 {
						healHint = fmt.Sprintf("[H] Self-heal (%d attempt%s remaining)", remaining, map[bool]string{true: "", false: "s"}[remaining == 1])
					} else {
						healHint = "[H] Self-heal (exhausted)"
					}
				}
				sb.WriteString(lipgloss.NewStyle().
					Foreground(m.theme.TextSecondary).
					Render("  " + healHint))
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
		Render("H=Self-heal  S=Skip  Esc=Back"))

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

func (m *VerifyModel) renderHealConfirmation() string {
	prompt := fmt.Sprintf("Self-heal task %d?\n\n(Y/N) — Press Y to attempt heal, N to cancel", m.confirmHealTask)

	confirmStyle := lipgloss.NewStyle().
		Background(lipgloss.Color(m.theme.SurfaceElevated)).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Padding(1, 2).
		Border(lipgloss.DoubleBorder()).
		BorderForeground(lipgloss.Color(m.theme.Warning))

	rendered := confirmStyle.Render(prompt)
	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, rendered)
}
