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
		Render(" QA Gate (Verify) "))
	sb.WriteString("\n")

	// Summary bar
	sb.WriteString(m.renderSummaryBar())
	sb.WriteString("\n")

	// Per-task result panels
	for i, task := range m.tasks {
		sb.WriteString(m.renderResultPanel(task, i == m.selected))
		sb.WriteString("\n")
	}

	// Keys
	sb.WriteString("\n")
	sb.WriteString(lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("H=Self-heal  S=Skip  Esc=Back"))

	return sb.String()
}

// renderSummaryBar renders the summary bar with pass/warn/fail counts.
//
//	─── 3 tasks verified ─── 1 warning ─── 0 failures ───
func (m *VerifyModel) renderSummaryBar() string {
	passCount := 0
	warnCount := 0
	failCount := 0

	for _, task := range m.tasks {
		if result, ok := m.results[task.ID]; ok {
			if result.FilesExist && result.SyntaxOK && result.TestsOK {
				passCount++
			} else if !result.TestsOK && len(result.Errors) > 0 {
				failCount++
			} else {
				warnCount++
			}
		}
	}

	passStyle := lipgloss.NewStyle().Foreground(m.theme.Success)
	warnStyle := lipgloss.NewStyle().Foreground(m.theme.Warning)
	failStyle := lipgloss.NewStyle().Foreground(m.theme.Error)
	sepStyle := lipgloss.NewStyle().Foreground(m.theme.Border)

	parts := []string{
		sepStyle.Render("───"),
		passStyle.Render(fmt.Sprintf("%d tasks verified", passCount)),
		sepStyle.Render("───"),
		warnStyle.Render(fmt.Sprintf("%d warning", warnCount)),
		sepStyle.Render("───"),
		failStyle.Render(fmt.Sprintf("%d failures", failCount)),
		sepStyle.Render("───"),
	}

	return strings.Join(parts, " ")
}

// renderResultPanel renders a single task's verification result in a rounded border card.
func (m *VerifyModel) renderResultPanel(task types.Task, selected bool) string {
	borderStyle := lipgloss.RoundedBorder()
	borderColor := m.theme.Border

	// Determine border color based on result status
	if result, ok := m.results[task.ID]; ok {
		if !result.FilesExist || !result.SyntaxOK || !result.TestsOK {
			if task.Status == types.StatusUnrecoverable {
				borderColor = m.theme.Error
			} else {
				borderColor = m.theme.Warning
			}
		}
	}

	// Build header with status icon
	var headerIcon string
	var headerStyle lipgloss.Style

	switch {
	case task.Status == types.StatusUnrecoverable:
		headerIcon = "[!]"
		headerStyle = lipgloss.NewStyle().Foreground(m.theme.Error).Bold(true)
	case task.Status == types.StatusFailed:
		headerIcon = "⚠"
		headerStyle = lipgloss.NewStyle().Foreground(m.theme.Warning).Bold(true)
	case task.Status == types.StatusDone:
		headerIcon = "✓"
		headerStyle = lipgloss.NewStyle().Foreground(m.theme.Success)
	default:
		headerIcon = "·"
		headerStyle = lipgloss.NewStyle().Foreground(m.theme.TextMuted)
	}

	// Selected indicator
	selectIndicator := ""
	if selected {
		selectIndicator = lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true).Render(" ▼")
	}

	header := fmt.Sprintf("  %s  Task #%d: %s%s",
		headerIcon,
		task.ID,
		task.Description,
		selectIndicator,
	)

	// Build content lines
	var contentParts []string
	contentParts = append(contentParts, headerStyle.Render(header))

	// Check results
	if result, ok := m.results[task.ID]; ok {
		var checks []string
		if result.FilesExist {
			checks = append(checks, m.checkmark("Files exist"))
		} else {
			checks = append(checks, m.cross("Files missing"))
		}
		if result.SyntaxOK {
			checks = append(checks, m.checkmark("Syntax OK"))
		} else {
			checks = append(checks, m.cross("Syntax error"))
		}
		if result.TestsOK {
			checks = append(checks, m.checkmark("Tests pass"))
		} else if len(result.Errors) > 0 {
			checks = append(checks, m.cross("Tests failed"))
		}
		contentParts = append(contentParts, "     "+strings.Join(checks, "  ·  "))

		// Error details for failing tasks
		if len(result.Errors) > 0 {
			for _, err := range result.Errors {
				contentParts = append(contentParts,
					lipgloss.NewStyle().Foreground(m.theme.Error).Render(
						fmt.Sprintf("    ✗ FAIL  %s", err)))
			}
		}

		// Action row for failing tasks
		if !result.FilesExist || !result.SyntaxOK || !result.TestsOK {
			if task.Status == types.StatusUnrecoverable {
				contentParts = append(contentParts,
					lipgloss.NewStyle().Foreground(m.theme.Error).Bold(true).Render("  [UNRECOVERABLE]"))
				contentParts = append(contentParts,
					lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
						"  Run git bisect to find the issue · Check /rollback or fix manually"))
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
				contentParts = append(contentParts,
					lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("  "+healHint))
			}
		}
	}

	content := strings.Join(contentParts, "\n")

	boxWidth := m.width - 2
	if boxWidth < 30 {
		boxWidth = 30
	}

	return lipgloss.NewStyle().
		Border(borderStyle).
		BorderForeground(borderColor).
		Width(boxWidth).
		Padding(0, 1).
		Render(content)
}

func (m *VerifyModel) checkmark(text string) string {
	return lipgloss.NewStyle().
		Foreground(m.theme.Success).
		Render(fmt.Sprintf("✓ %s", text))
}

func (m *VerifyModel) cross(text string) string {
	return lipgloss.NewStyle().
		Foreground(m.theme.Error).
		Render(fmt.Sprintf("✗ %s", text))
}

// renderHealConfirmation renders the self-heal confirmation overlay with double-border.
func (m *VerifyModel) renderHealConfirmation() string {
	doubleBorder := lipgloss.Border{
		Top:         "═",
		Bottom:      "═",
		Left:        "║",
		Right:       "║",
		TopLeft:     "╔",
		TopRight:    "╗",
		BottomLeft:  "╚",
		BottomRight: "╝",
	}

	titleStyle := lipgloss.NewStyle().
		Foreground(m.theme.Warning).
		Bold(true)

	content := lipgloss.JoinVertical(lipgloss.Left,
		"",
		titleStyle.Render(fmt.Sprintf("  Self-Heal: Task #%d", m.confirmHealTask)),
		lipgloss.NewStyle().Foreground(m.theme.Border).Render("  ───────────────────────────────────"),
		"",
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("  M31A will re-analyze the test"),
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render("  failures and attempt a fix."),
		"",
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(
			fmt.Sprintf("  Attempts remaining: %d", 2)),
		"",
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("  [Y / Enter]  Attempt heal"),
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render("  [N / Esc]    Cancel"),
		"",
	)

	modal := lipgloss.NewStyle().
		Background(lipgloss.Color(m.theme.SurfaceElevated)).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Padding(1, 2).
		Border(doubleBorder).
		BorderForeground(lipgloss.Color(m.theme.Warning)).
		Width(40).
		Render(content)

	return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, modal)
}
