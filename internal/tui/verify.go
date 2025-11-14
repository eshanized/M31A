package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

// VerifyModel displays task verification results and provides self-healing.
type VerifyModel struct {
	tasks     []types.Task
	results   map[int]workflow.VerificationResult
	theme     theme.Theme
	sessionID string
	width     int
	height    int
	viewport  viewport.Model
	healFunc  func(taskID int) tea.Cmd

	// Healing state
	healingTaskID int // -1 if not healing
	healAttempt   int // 0 = not healing, 1+ = current attempt number
	spinner       components.Spinner
}

// NewVerifyModel creates a VerifyModel.
func NewVerifyModel(tasks []types.Task, results map[int]workflow.VerificationResult, t theme.Theme, w, h int) *VerifyModel {
	vm := &VerifyModel{
		tasks:         tasks,
		results:       results,
		theme:         t,
		width:         w,
		height:        h,
		healingTaskID: -1,
		spinner:       components.NewSpinner(),
	}
	vm.initViewport()
	return vm
}

func (vm *VerifyModel) initViewport() {
	h := vm.height - 6
	if h < 5 {
		h = 5
	}
	vm.viewport = viewport.New(vm.width, h)
	vm.viewport.SetContent(vm.renderResults())
}

// UpdateResults replaces the verification results.
func (vm *VerifyModel) UpdateResults(results map[int]workflow.VerificationResult) {
	vm.results = results
	vm.viewport.SetContent(vm.renderResults())
}

// SetHealFunc sets the callback for self-healing a specific task.
func (vm *VerifyModel) SetHealFunc(f func(taskID int) tea.Cmd) {
	vm.healFunc = f
}

// StartHealing marks the model as healing a specific task (shows spinner).
func (vm *VerifyModel) StartHealing(taskID int, attempt int) {
	vm.healingTaskID = taskID
	vm.healAttempt = attempt
	vm.spinner.Reset()
	vm.viewport.SetContent(vm.renderResults())
}

// StopHealing clears the healing state.
func (vm *VerifyModel) StopHealing() {
	vm.healingTaskID = -1
	vm.healAttempt = 0
	vm.viewport.SetContent(vm.renderResults())
}

// TickSpinner advances the spinner animation frame.
func (vm *VerifyModel) TickSpinner() {
	vm.spinner.Next()
	vm.viewport.SetContent(vm.renderResults())
}

// Update handles verify screen key events.
func (vm *VerifyModel) Update(msg tea.Msg) (*VerifyModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "j", "down":
			vm.viewport.LineDown(1)
		case "k", "up":
			vm.viewport.LineUp(1)
		case "enter":
			// Continue → go to ship
			return vm, func() tea.Msg {
				return AppMsg{Screen: ScreenShip}
			}
		case "h":
			// Trigger heal for the first failed task
			if vm.healFunc != nil && vm.healingTaskID < 0 {
				for _, task := range vm.tasks {
					result, ok := vm.results[task.ID]
					if ok && (!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
						vm.StartHealing(task.ID, task.HealsAttempted+1)
						return vm, vm.healFunc(task.ID)
					}
				}
			}
			return vm, nil
		case "s":
			// Skip — go to ship anyway
			return vm, func() tea.Msg {
				return AppMsg{Screen: ScreenShip}
			}
		case "esc", "q":
			return vm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	case HealResultMsg:
		vm.StopHealing()
		if msg.Success {
			for i := range vm.tasks {
				if vm.tasks[i].ID == msg.TaskID {
					vm.tasks[i].Status = types.StatusDone
				}
			}
		}
		vm.viewport.SetContent(vm.renderResults())
	}
	return vm, nil
}

// View renders the verify results screen.
func (vm *VerifyModel) View() string {
	t := vm.theme
	w := vm.width

	passed, failed, total := vm.countResults()

	// Header — color-coded by pass/fail
	var headerClr lipgloss.Color
	var headerIcon string
	if failed > 0 {
		headerClr = t.Error
		headerIcon = "✗"
	} else {
		headerClr = t.Success
		headerIcon = "✓"
	}

	title := lipgloss.NewStyle().Foreground(headerClr).Bold(true).
		Render(fmt.Sprintf("%s Verify · %d/%d passed", headerIcon, passed, total))

	if failed > 0 {
		title += " " + lipgloss.NewStyle().Foreground(t.Error).
			Render(fmt.Sprintf("(%d failed)", failed))
	}

	divider := lipgloss.NewStyle().Foreground(t.TextMuted).Render(strings.Repeat("─", w))

	// Footer (0 padding — last line, no indent)
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("↵ continue  h heal  s skip  q back")

	return lipgloss.JoinVertical(lipgloss.Left,
		title,
		divider,
		vm.viewport.View(),
		divider,
		footer,
	)
}

func (vm *VerifyModel) renderResults() string {
	t := vm.theme
	var lines []string
	for i, task := range vm.tasks {
		result, hasResult := vm.results[task.ID]

		// Determine badge type
		badgeType := components.BadgeNeutral
		badgeText := "pending"
		if hasResult {
			if result.FilesExist && result.SyntaxOK && result.TestsOK {
				badgeType = components.BadgeSuccess
				badgeText = "pass"
			} else {
				badgeType = components.BadgeError
				badgeText = "fail"
			}
		}
		statusBadge := components.SimpleBadge{
			Text:    badgeText,
			Type:    badgeType,
			Compact: true,
			Theme:   t,
		}.Render()

		num := lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
		action := lipgloss.NewStyle().Foreground(t.Text).Render(task.Action)
		line := fmt.Sprintf("  %s %s %s", num, statusBadge, action)
		lines = append(lines, line)

		// Heal spinner for the task being healed
		if vm.healingTaskID >= 0 && task.ID == vm.healingTaskID {
			frame := vm.spinner.Peek()
			healLine := lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(6).
				Render(fmt.Sprintf("%s Heal attempt %d/2...", frame, vm.healAttempt))
			lines = append(lines, healLine)
		}

		// Show detail for failed tasks
		if hasResult && (!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
			var issues []string
			if !result.FilesExist {
				issues = append(issues, "files missing")
			}
			if !result.SyntaxOK {
				issues = append(issues, "syntax errors")
			}
			if !result.TestsOK {
				issues = append(issues, "tests failed")
			}
			// Use Errors field if available
			if len(result.Errors) > 0 {
				issues = result.Errors
			}
			detail := lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(8).
				Render("→ " + strings.Join(issues, ", "))
			lines = append(lines, detail)
		}
	}
	return strings.Join(lines, "\n")
}

func (vm *VerifyModel) countResults() (passed, failed, total int) {
	total = len(vm.tasks)
	for _, task := range vm.tasks {
		result, ok := vm.results[task.ID]
		if !ok {
			continue
		}
		if result.FilesExist && result.SyntaxOK && result.TestsOK {
			passed++
		} else {
			failed++
		}
	}
	return
}
