package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func TestVerifyModel_New(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Create main.go", Status: types.StatusDone},
		{ID: 2, Description: "Add tests", Status: types.StatusDone},
	}
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
		2: {TaskID: 2, FilesExist: true, SyntaxOK: true, TestsOK: false, Errors: []string{"test failed"}},
	}
	th := theme.Dark()

	m := NewVerifyModel(tasks, results, th)

	if m == nil {
		t.Fatal("expected non-nil VerifyModel")
	}
	if len(m.tasks) != 2 {
		t.Errorf("expected 2 tasks, got %d", len(m.tasks))
	}
	if len(m.results) != 2 {
		t.Errorf("expected 2 results, got %d", len(m.results))
	}
}

func TestVerifyModel_UpdateWindowSize(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())

	_, _ = m.Update(tea.WindowSizeMsg{Width: 90, Height: 30})

	if m.width != 90 {
		t.Errorf("expected width 90, got %d", m.width)
	}
	if m.height != 30 {
		t.Errorf("expected height 30, got %d", m.height)
	}
}

func TestVerifyModel_UpdateNavigation(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
		{ID: 3, Description: "Task 3", Status: types.StatusDone},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())

	// Navigate down
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.selected != 1 {
		t.Errorf("expected selected 1 after down, got %d", m.selected)
	}

	// Navigate down with j
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

func TestVerifyModel_UpdateNavigationBounds(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Only task", Status: types.StatusDone},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())

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

func TestVerifyModel_UpdateSkip(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
		{ID: 2, Description: "Task 2", Status: types.StatusPending},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())
	m.selected = 1

	// Skip selected task
	_, _ = m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	if m.tasks[1].Status != types.StatusSkipped {
		t.Errorf("expected task 2 to be skipped, got %s", m.tasks[1].Status)
	}
}

func TestVerifyModel_UpdateAllDoneTransitions(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
		{ID: 2, Description: "Task 2", Status: types.StatusDone},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())

	// When all tasks are done, pressing any key should transition to Ship
	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if appMsg == nil || appMsg.Screen != ScreenShip {
		t.Error("expected ScreenShip when all tasks are done")
	}
}

func TestVerifyModel_UpdateCtrlC(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark())

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if appMsg != nil {
		t.Error("expected no AppMsg on ctrl+c")
	}
}

func TestVerifyModel_ViewNonEmpty(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Create main.go", Status: types.StatusDone},
		{ID: 2, Description: "Add tests", Status: types.StatusPending},
	}
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
		2: {TaskID: 2, FilesExist: false, SyntaxOK: false, TestsOK: false, Errors: []string{"not run"}},
	}
	m := NewVerifyModel(tasks, results, theme.Dark())
	m.width = 80
	m.height = 24

	view := m.View()
	if view == "" {
		t.Fatal("View() should not be empty")
	}
	if !strings.Contains(view, "Verify") {
		t.Error("expected 'Verify' in view")
	}
	if !strings.Contains(view, "Create main.go") {
		t.Error("expected task description in view")
	}
}

func TestVerifyModel_ViewLoading(t *testing.T) {
	m := NewVerifyModel(nil, nil, theme.Dark())

	view := m.View()
	if !strings.Contains(view, "Loading") {
		t.Errorf("expected loading message, got: %s", view)
	}
}

func TestVerifyModel_ViewWithResults(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusDone},
	}
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
	}
	m := NewVerifyModel(tasks, results, theme.Dark())
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "Files exist") {
		t.Error("expected 'Files exist' in view")
	}
	if !strings.Contains(view, "Syntax OK") {
		t.Error("expected 'Syntax OK' in view")
	}
	if !strings.Contains(view, "Tests pass") {
		t.Error("expected 'Tests pass' in view")
	}
}

func TestVerifyModel_ViewWithFailures(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusPending},
	}
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: false, SyntaxOK: false, TestsOK: false, Errors: []string{"error"}},
	}
	m := NewVerifyModel(tasks, results, theme.Dark())
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "Files missing") {
		t.Error("expected 'Files missing' in view")
	}
	if !strings.Contains(view, "Syntax error") {
		t.Error("expected 'Syntax error' in view")
	}
	if !strings.Contains(view, "Self-heal") {
		t.Error("expected 'Self-heal' option in view")
	}
}

func TestVerifyModel_ViewUnrecoverable(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "Task 1", Status: types.StatusUnrecoverable},
	}
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: false, SyntaxOK: false, TestsOK: false, Errors: []string{"fatal error"}},
	}
	m := NewVerifyModel(tasks, results, theme.Dark())
	m.width = 80
	m.height = 24

	view := m.View()
	if !strings.Contains(view, "UNRECOVERABLE") {
		t.Error("expected 'UNRECOVERABLE' in view for unrecoverable task")
	}
}
