package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestPlanModel_New(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Create main.go", Status: types.StatusPending},
		{ID: 2, Description: "Add tests", Dependencies: []int{1}, Status: types.StatusPending},
	}
	th := theme.Dark()

	m := NewPlanModel(tasks, th, "openrouter/model-a", "openrouter", 0.05, "2m", 0, 0)

	if m == nil {
		t.Fatal("expected non-nil PlanModel")
	}
	if len(m.tasks) != 2 {
		t.Errorf("expected 2 tasks, got %d", len(m.tasks))
	}
	if m.modelID != "openrouter/model-a" {
		t.Errorf("expected modelID openrouter/model-a, got %s", m.modelID)
	}
	if m.estCost != 0.05 {
		t.Errorf("expected estCost 0.05, got %f", m.estCost)
	}
}

func TestPlanModel_UpdateWindowSize(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	_, _ = m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	if m.width != 80 {
		t.Errorf("expected width 80, got %d", m.width)
	}
	if m.height != 24 {
		t.Errorf("expected height 24, got %d", m.height)
	}
}

func TestPlanModel_UpdateNavigation(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
		{ID: 3, Description: "Task 3", Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	// Navigate down
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.selected != 1 {
		t.Errorf("expected selected 1 after down, got %d", m.selected)
	}

	// Navigate down again
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	if m.selected != 2 {
		t.Errorf("expected selected 2 after j, got %d", m.selected)
	}

	// Navigate up
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.selected != 1 {
		t.Errorf("expected selected 1 after up, got %d", m.selected)
	}

	// Navigate up with k
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'k'}})
	if m.selected != 0 {
		t.Errorf("expected selected 0 after k, got %d", m.selected)
	}
}

func TestPlanModel_UpdateNavigationBounds(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Only task", Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	// Should not go below 0
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.selected != 0 {
		t.Errorf("selected should stay at 0, got %d", m.selected)
	}

	// Should not go above len-1
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.selected != 0 {
		t.Errorf("selected should stay at 0, got %d", m.selected)
	}
}

func TestPlanModel_UpdateKeyEvents(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	// Test accept key
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	if appMsg == nil || appMsg.Screen != ScreenExecute {
		t.Error("expected ScreenExecute on 'a' key")
	}

	// Test retry key
	m2 := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)
	_, appMsg = m2.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("expected ScreenREPL on 'r' key")
	}

	// Test diff toggle
	m3 := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)
	m3.showDiff = false
	_, _ = m3.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'d'}})
	if !m3.showDiff {
		t.Error("expected showDiff to be toggled on")
	}
	if m3.showGraph {
		t.Error("expected showGraph to be off when diff is on")
	}

	// Test graph toggle
	m4 := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)
	_, _ = m4.Update(tea.KeyMsg{Type: tea.KeyTab})
	if !m4.showGraph {
		t.Error("expected showGraph to be toggled on")
	}
	if m4.showDiff {
		t.Error("expected showDiff to be off when graph is on")
	}
}

func TestPlanModel_UpdateCtrlC(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	// Ctrl+C should not panic and should not emit screen change
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if appMsg != nil {
		t.Error("expected no AppMsg on ctrl+c")
	}
}

func TestPlanModel_ViewNonEmpty(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Create main.go", Status: types.StatusPending, Files: []string{"main.go"}},
		{ID: 2, Description: "Add tests", Dependencies: []int{1}, Status: types.StatusDone, Files: []string{"main_test.go"}},
	}
	m := NewPlanModel(tasks, theme.Dark(), "openrouter/model-a", "openrouter", 0.05, "2m", 0, 0)
	m.width = 80
	m.height = 24

	view := m.View()
	if view == "" {
		t.Fatal("View() should not be empty")
	}
	if !strings.Contains(view, "Plan") {
		t.Error("expected 'Plan' in view")
	}
	if !strings.Contains(view, "Create main.go") {
		t.Error("expected task description in view")
	}
	if !strings.Contains(view, "model-a") {
		t.Error("expected model ID in view")
	}
}

func TestPlanModel_ViewLoading(t *testing.T) {
	m := NewPlanModel(nil, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)

	view := m.View()
	if !strings.Contains(view, "Loading") {
		t.Errorf("expected loading message, got: %s", view)
	}
}

func TestPlanModel_RenderDependencyGraph(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Base task", Status: types.StatusPending},
		{ID: 2, Description: "Dependent task", Dependencies: []int{1}, Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)
	m.width = 80
	m.height = 24
	m.showGraph = true

	view := m.View()
	if !strings.Contains(view, "Dependency Graph") {
		t.Error("expected dependency graph in view")
	}
}

func TestPlanModel_RenderDiff(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Modify file", Action: "Modify", Files: []string{"main.go"}, Status: types.StatusPending},
		{ID: 2, Description: "New file", Action: "Create", Files: []string{"util.go"}, Status: types.StatusPending},
	}
	m := NewPlanModel(tasks, theme.Dark(), "model-a", "openrouter", 0.01, "1m", 0, 0)
	m.width = 80
	m.height = 24
	m.showDiff = true

	view := m.View()
	if !strings.Contains(view, "Predicted File Changes") {
		t.Error("expected diff preview in view")
	}
}
