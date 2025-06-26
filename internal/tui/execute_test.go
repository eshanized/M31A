package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestExecuteModel_New(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Build binary", Status: types.StatusRunning},
		{ID: 2, Description: "Run tests", Status: types.StatusPending, Dependencies: []int{1}},
	}
	th := theme.Dark()

	m := NewExecuteModel(tasks, th, 0, 0)

	if m == nil {
		t.Fatal("expected non-nil ExecuteModel")
	}
	if len(m.tasks) != 2 {
		t.Errorf("expected 2 tasks, got %d", len(m.tasks))
	}
	if m.current != 0 {
		t.Errorf("expected current 0, got %d", m.current)
	}
}

func TestExecuteModel_UpdateWindowSize(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusRunning},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)

	_, _ = m.Update(tea.WindowSizeMsg{Width: 100, Height: 30})

	if m.width != 100 {
		t.Errorf("expected width 100, got %d", m.width)
	}
	if m.height != 30 {
		t.Errorf("expected height 30, got %d", m.height)
	}
}

func TestExecuteModel_UpdateNavigation(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusRunning},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
		{ID: 3, Description: "Task 3", Status: types.StatusPending},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)

	// Navigate down
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.current != 1 {
		t.Errorf("expected current 1 after down, got %d", m.current)
	}

	// Navigate down with j
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	if m.current != 2 {
		t.Errorf("expected current 2 after j, got %d", m.current)
	}

	// Navigate up
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.current != 1 {
		t.Errorf("expected current 1 after up, got %d", m.current)
	}

	// Navigate up with k
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'k'}})
	if m.current != 0 {
		t.Errorf("expected current 0 after k, got %d", m.current)
	}
}

func TestExecuteModel_UpdateNavigationBounds(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Only task", Status: types.StatusRunning},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)

	// Should not go below 0
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.current != 0 {
		t.Errorf("current should stay at 0, got %d", m.current)
	}

	// Should not go above len-1
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.current != 0 {
		t.Errorf("current should stay at 0, got %d", m.current)
	}
}

func TestExecuteModel_UpdateSkip(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusRunning},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)
	m.current = 1

	// Skip current task
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	if m.tasks[1].Status != types.StatusSkipped {
		t.Errorf("expected task 2 to be skipped, got %s", m.tasks[1].Status)
	}
}

func TestExecuteModel_UpdateAllDoneTransitions(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
		{ID: 2, Description: "Task 2", Status: types.StatusDone},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)

	// When all tasks are done, pressing any key should transition to Verify
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if appMsg == nil || appMsg.Screen != ScreenVerify {
		t.Error("expected ScreenVerify when all tasks are done")
	}
}

func TestExecuteModel_UpdateCtrlC(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusRunning},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if appMsg != nil {
		t.Error("expected no AppMsg on ctrl+c")
	}
}

func TestExecuteModel_ViewNonEmpty(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Build binary", Status: types.StatusRunning},
		{ID: 2, Description: "Run tests", Status: types.StatusPending, Dependencies: []int{1}},
		{ID: 3, Description: "Deploy", Status: types.StatusDone},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if view == "" {
		t.Fatal("View() should not be empty")
	}
	if !strings.Contains(view, "Execute") {
		t.Error("expected 'Execute' in view")
	}
	if !strings.Contains(view, "Build binary") {
		t.Error("expected task description in view")
	}
	if !strings.Contains(view, "tasks") {
		t.Error("expected progress info in view")
	}
}

func TestExecuteModel_ViewLoading(t *testing.T) {
	m := NewExecuteModel(nil, theme.Dark(), 0, 0)

	view := m.View()
	if !strings.Contains(view, "Loading") {
		t.Errorf("expected loading message, got: %s", view)
	}
}

func TestExecuteModel_ViewWithToolCard(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusRunning},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24
	m.toolCard = "Bash: go build ./..."

	view := m.View()
	if !strings.Contains(view, "Bash: go build ./...") {
		t.Error("expected tool card in view")
	}
}

func TestExecuteModel_ViewSkippedAndFailed(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Done task", Status: types.StatusDone},
		{ID: 2, Description: "Skipped task", Status: types.StatusSkipped},
		{ID: 3, Description: "Failed task", Status: types.StatusFailed},
	}
	m := NewExecuteModel(tasks, theme.Dark(), 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "skipped") {
		t.Error("expected 'skipped' in view")
	}
	if !strings.Contains(view, "failed") {
		t.Error("expected 'failed' in view")
	}
}
