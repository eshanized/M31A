package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/viewport"
	"github.com/charmbracelet/lipgloss"
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
}

// NewVerifyModel creates a VerifyModel.
func NewVerifyModel(tasks []types.Task, results map[int]workflow.VerificationResult, t theme.Theme, w, h int) *VerifyModel {
	vm := &VerifyModel{
		tasks:   tasks,
		results: results,
		theme:   t,
		width:   w,
		height:  h,
	}
	vm.initViewport()
	return vm
}

func (vm *VerifyModel) initViewport() {
	h := vm.height - 7
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
			return vm, func() tea.Msg {
				return AppMsg{Screen: ScreenShip}
			}
		case "esc", "q":
			return vm, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		}
	case HealResultMsg:
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
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render(fmt.Sprintf("✓  Verify — %d/%d passed", passed, total))

	if failed > 0 {
		title += " " + lipgloss.NewStyle().Foreground(t.Error).
			Render(fmt.Sprintf("(%d failed)", failed))
	}

	divider := lipgloss.NewStyle().Foreground(t.TextMuted).Render(strings.Repeat("─", w))
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↵ continue to ship  j/k scroll  q back")

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
		icon := "○"
		color := t.TextMuted
		if hasResult {
			if result.FilesExist && result.SyntaxOK && result.TestsOK {
				icon = "✓"
				color = t.Success
			} else {
				icon = "✗"
				color = t.Error
			}
		}

		num := lipgloss.NewStyle().Foreground(t.TextMuted).Render(fmt.Sprintf("%3d.", i+1))
		statusIcon := lipgloss.NewStyle().Foreground(color).Render(icon)
		action := lipgloss.NewStyle().Foreground(t.Text).Render(task.Action)
		line := fmt.Sprintf("  %s %s %s", num, statusIcon, action)
		lines = append(lines, line)

		// Show verification detail
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
			detail := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(8).
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
