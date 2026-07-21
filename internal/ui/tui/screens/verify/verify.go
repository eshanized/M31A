package verify

import (
	"fmt"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// VerifyModel displays task verification results and provides self-healing.
type VerifyModel struct {
	tasks       []types.Task
	results     map[int]workflow.VerificationResult
	theme       theme.Theme
	sessionID   string
	width       int
	height      int
	viewport    viewport.Model
	manualSteps []string
	healFunc    func(taskID int) tea.Cmd

	// Healing state
	healingTaskID int // -1 if not healing
	healAttempt   int // 0 = not healing, 1+ = current attempt number
	healCursor    int // index into failed tasks list for heal selection
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

// Init implements tuitypes.Screenable.
func (vm *VerifyModel) Init() tea.Cmd { return nil }

// SetDimensions implements tuitypes.Screenable.
func (vm *VerifyModel) SetDimensions(w, h int) {
	vm.width = w
	vm.height = h
	vm.initViewport()
}

// SetTheme implements tuitypes.Screenable.
func (vm *VerifyModel) SetTheme(t theme.Theme) {
	vm.theme = t
	vm.viewport.SetContent(vm.renderResults())
}

func (vm *VerifyModel) initViewport() {
	// verifyViewChrome: progressLine(1) + separator(1) + hints(1) + margins(3)
	const verifyViewChrome = 6
	h := vm.height - verifyViewChrome
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

// SetManualSteps sets manual verification steps from the plan for display.
func (vm *VerifyModel) SetManualSteps(steps []string) {
	vm.manualSteps = steps
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
func (vm *VerifyModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			failedCount := vm.countFailedTasks()
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				if vm.healCursor > 0 {
					vm.healCursor--
					vm.syncViewportToCursor()
				}
			case tea.MouseButtonWheelDown:
				if failedCount > 0 && vm.healCursor < failedCount-1 {
					vm.healCursor++
					vm.syncViewportToCursor()
				}
			}
		}
	case tea.KeyMsg:
		switch msg.String() {
		case "j", "down":
			failedCount := vm.countFailedTasks()
			if failedCount > 0 && vm.healCursor < failedCount-1 {
				vm.healCursor++
				vm.syncViewportToCursor()
			}
		case "k", "up":
			if vm.healCursor > 0 {
				vm.healCursor--
				vm.syncViewportToCursor()
			}
		case "enter":
			// Continue → go to ship
			return vm, func() tea.Msg {
				return tuitypes.AppMsg{Screen: tuitypes.ScreenShip}
			}
		case "h":
			// Trigger heal for the task at healCursor
			if vm.healFunc != nil && vm.healingTaskID < 0 {
				failedTasks := vm.getFailedTasks()
				if vm.healCursor >= 0 && vm.healCursor < len(failedTasks) {
					task := failedTasks[vm.healCursor]
					vm.StartHealing(task.ID, task.HealsAttempted+1)
					return vm, vm.healFunc(task.ID)
				}
			}
			return vm, nil
		case "s":
			// Skip — go to ship anyway
			return vm, func() tea.Msg {
				return tuitypes.AppMsg{Screen: tuitypes.ScreenShip}
			}
		case "esc", "q":
			return vm, func() tea.Msg {
				return tuitypes.PopScreenMsg{}
			}
		}
	case tuitypes.HealResultMsg:
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

// View renders the verify results screen content.
// Header, footer, and chrome are handled by the unified PageLayout.
func (vm *VerifyModel) View() string {
	t := vm.theme

	// Summary header
	passed, failed, pending := 0, 0, 0
	for _, task := range vm.tasks {
		if r, ok := vm.results[task.ID]; ok {
			if r.FilesExist && r.SyntaxOK && r.TestsOK {
				passed++
			} else {
				failed++
			}
		} else {
			pending++
		}
	}
	total := len(vm.tasks)

	var summaryLine string
	if failed > 0 {
		summaryLine = lipgloss.NewStyle().Foreground(t.Error).Bold(true).
			Render(fmt.Sprintf("✗ %d/%d failed", failed, total))
	} else if passed == total && total > 0 {
		summaryLine = lipgloss.NewStyle().Foreground(t.Success).Bold(true).
			Render(fmt.Sprintf("✓ All %d tasks passed", total))
	} else {
		summaryLine = lipgloss.NewStyle().Foreground(t.TextSecondary).
			Render(fmt.Sprintf("%d/%d verified", passed, total))
	}
	if pending > 0 {
		summaryLine += " " + lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(fmt.Sprintf("(%d pending)", pending))
	}

	sep := lipgloss.NewStyle().Foreground(t.BorderSubtle).
		Render(strings.Repeat("─", vm.width))

	result := vm.viewport.View()
	hints := lipgloss.NewStyle().Foreground(t.TextMuted).
		Render("j/k: scroll  h: heal  Enter/s: ship  Esc: back")
	return lipgloss.JoinVertical(lipgloss.Left, summaryLine, sep, result, hints)
}

func (vm *VerifyModel) renderResults() string {
	t := vm.theme
	var lines []string
	failedIdx := -1
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
				failedIdx++
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

		// Show cursor indicator on selected failed task
		cursor := "  "
		if hasResult && failedIdx >= 0 && failedIdx == vm.healCursor &&
			(!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
			cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
		}

		line := fmt.Sprintf("%s%s %s %s", cursor, num, statusBadge, action)
		lines = append(lines, line)

		// Heal spinner for the task being healed
		if vm.healingTaskID >= 0 && task.ID == vm.healingTaskID {
			frame := vm.spinner.Peek()
			healLine := lipgloss.NewStyle().Foreground(t.Warning).PaddingLeft(6).
				Render(fmt.Sprintf("%s Heal attempt %d/2…", frame, vm.healAttempt))
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

	// Manual verification steps from the plan
	if len(vm.manualSteps) > 0 {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("Manual Verification Steps:"))
		for _, step := range vm.manualSteps {
			lines = append(lines, lipgloss.NewStyle().Foreground(t.TextSecondary).PaddingLeft(2).Render("• "+step))
		}
	}

	return strings.Join(lines, "\n")
}

// countFailedTasks returns the number of tasks with failing verification results.
func (vm *VerifyModel) countFailedTasks() int {
	count := 0
	for _, task := range vm.tasks {
		result, ok := vm.results[task.ID]
		if ok && (!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
			count++
		}
	}
	return count
}

// syncViewportToCursor scrolls the viewport so the task at healCursor is visible.
func (vm *VerifyModel) syncViewportToCursor() {
	failedIdx := -1
	for i, task := range vm.tasks {
		result, hasResult := vm.results[task.ID]
		if hasResult && (!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
			failedIdx++
			if failedIdx == vm.healCursor {
				vm.viewport.GotoTop()
				vm.viewport.LineDown(i)
				break
			}
		}
	}
	vm.viewport.SetContent(vm.renderResults())
}

// getFailedTasks returns the tasks with failing verification results.
func (vm *VerifyModel) getFailedTasks() []types.Task {
	var failed []types.Task
	for _, task := range vm.tasks {
		result, ok := vm.results[task.ID]
		if ok && (!result.FilesExist || !result.SyntaxOK || !result.TestsOK) {
			failed = append(failed, task)
		}
	}
	return failed
}
