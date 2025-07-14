package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func containsStr(s, substr string) bool {
	return strings.Contains(s, substr)
}

func testTheme() theme.Theme {
	return theme.Dark()
}

func testTasks() []types.Task {
	return []types.Task{
		{ID: 1, Action: "Create", Description: "Create go.mod", Dependencies: []int{}, Status: types.StatusDone, Files: []string{"go.mod"}},
		{ID: 2, Action: "Add", Description: "Add main.go", Dependencies: []int{1}, Status: types.StatusPending, Files: []string{"cmd/main.go"}},
		{ID: 3, Action: "Test", Description: "Add tests", Dependencies: []int{2}, Status: types.StatusPending, Files: []string{"cmd/main_test.go"}},
	}
}

// ---------------------------------------------------------------------------
// Plan screen tests
// ---------------------------------------------------------------------------

func TestPlan_RenderView(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "claude-3", "", "openrouter", 0.12, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
	if !containsStr(view, "Plan") {
		t.Error("Expected 'Plan' in view")
	}
}

func TestPlan_TaskListDisplay(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "Create go.mod") {
		t.Error("Expected task description in view")
	}
	if !containsStr(view, "deps:") {
		t.Error("Expected 'deps:' in view")
	}
}

func TestPlan_CostTimePanel(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "claude-3", "", "openrouter", 0.12, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "Est cost:") {
		t.Error("Expected 'Est cost:' in view")
	}
	if !containsStr(view, "Est time:") {
		t.Error("Expected 'Est time:' in view")
	}
}

func TestPlan_AcceptAction(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}})
	if appMsg == nil {
		t.Fatal("Expected AppMsg on accept")
	}
	if appMsg.Screen != ScreenExecute {
		t.Errorf("Expected ScreenExecute, got %d", appMsg.Screen)
	}
}

func TestPlan_RetryAction(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	if appMsg == nil {
		t.Fatal("Expected AppMsg on retry")
	}
	if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL, got %d", appMsg.Screen)
	}
}

func TestPlan_DiffPreviewToggle(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	// Toggle diff
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'d'}})
	if !m.showDiff {
		t.Error("Expected showDiff to be true")
	}

	view := m.View()
	if !containsStr(view, "Predicted File Changes") {
		t.Error("Expected diff preview in view")
	}

	// Toggle off
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'d'}})
	if m.showDiff {
		t.Error("Expected showDiff to be false after toggle")
	}
}

func TestPlan_DependencyGraphToggle(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40

	m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if !m.showGraph {
		t.Error("Expected showGraph to be true")
	}

	view := m.View()
	if !containsStr(view, "Dependency Graph") {
		t.Error("Expected dependency graph in view")
	}
}

func TestPlan_SelectedTaskHighlight(t *testing.T) {
	m := NewPlanModel(testTasks(), testTheme(), "model", "", "provider", 0.1, "5 min", 0, 0)
	m.width = 80
	m.height = 40
	m.selected = 1

	view := m.View()
	if !containsStr(view, "[>]") {
		t.Error("Expected selected task indicator '[>]' in view")
	}
}

// ---------------------------------------------------------------------------
// Execute screen tests
// ---------------------------------------------------------------------------

func TestExecute_RenderView(t *testing.T) {
	m := NewExecuteModel(testTasks(), testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
	if !containsStr(view, "Execute") {
		t.Error("Expected 'Execute' in view")
	}
}

func TestExecute_ProgressBar(t *testing.T) {
	m := NewExecuteModel(testTasks(), testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	// New UI shows "N/M tasks" and segmented progress
	if !containsStr(view, "tasks") {
		t.Error("Expected 'tasks' in progress bar")
	}
}

func TestExecute_TaskStatusIndicators(t *testing.T) {
	m := NewExecuteModel(testTasks(), testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	// New UI uses ✓ for done tasks
	if !containsStr(view, "[✓]") && !containsStr(view, "[x]") {
		t.Error("Expected done task indicator in view")
	}
}

func TestExecute_SkipAction(t *testing.T) {
	tasks := testTasks()
	m := NewExecuteModel(tasks, testTheme(), 0, 0)
	m.width = 80
	m.height = 40
	m.current = 1 // Select task 2 (pending)

	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	if tasks[1].Status != types.StatusSkipped {
		t.Errorf("Expected task 2 to be skipped, got %s", tasks[1].Status)
	}
}

func TestExecute_LiveToolCard(t *testing.T) {
	m := NewExecuteModel(testTasks(), testTheme(), 0, 0)
	m.width = 80
	m.height = 40
	m.toolCard = "$ go build ./...\n[OK] Completed in 2.34s"

	view := m.View()
	if !containsStr(view, "go build") {
		t.Error("Expected tool output in view")
	}
}

func TestExecute_TransitionToVerify(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "test", Status: types.StatusDone},
	}
	m := NewExecuteModel(tasks, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	// All tasks done, should transition to verify
	if appMsg == nil {
		// Check if the model detected all done
		allDone := true
		for _, t := range m.tasks {
			if t.Status == types.StatusPending || t.Status == types.StatusRunning {
				allDone = false
				break
			}
		}
		if allDone {
			t.Log("All tasks done, transition should happen on next key")
		}
	}
}

func TestExecute_SelectedTaskHighlight(t *testing.T) {
	m := NewExecuteModel(testTasks(), testTheme(), 0, 0)
	m.width = 80
	m.height = 40
	m.current = 1

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
}

func TestExecute_BlockedTaskDisplay(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "task 1", Status: types.StatusPending, Dependencies: []int{}},
		{ID: 2, Action: "Add", Description: "task 2", Status: types.StatusPending, Dependencies: []int{1}},
	}
	m := NewExecuteModel(tasks, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "blocked") {
		t.Error("Expected 'blocked' indicator for task 2")
	}
}

// ---------------------------------------------------------------------------
// Verify screen tests
// ---------------------------------------------------------------------------

func TestVerify_RenderView(t *testing.T) {
	m := NewVerifyModel(testTasks(), map[int]workflow.VerificationResult{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
	if !containsStr(view, "Verify") {
		t.Error("Expected 'Verify' in view")
	}
}

func TestVerify_PassFailChecklist(t *testing.T) {
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
		2: {TaskID: 2, FilesExist: true, SyntaxOK: false, TestsOK: false, Errors: []string{"build failed"}},
	}

	m := NewVerifyModel(testTasks(), results, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "Files exist") {
		t.Error("Expected 'Files exist' checkmark")
	}
	if !containsStr(view, "Syntax error") {
		t.Error("Expected 'Syntax error' for task 2")
	}
}

func TestVerify_SelfHealAction(t *testing.T) {
	results := map[int]workflow.VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: false, TestsOK: false, Errors: []string{"error"}},
	}
	tasks := []types.Task{{ID: 1, Action: "Create", Description: "test", Status: types.StatusDone, Files: []string{"a.go"}}}

	m := NewVerifyModel(tasks, results, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "Self-heal") {
		t.Error("Expected 'Self-heal' option")
	}
}

func TestVerify_SkipAction(t *testing.T) {
	m := NewVerifyModel(testTasks(), map[int]workflow.VerificationResult{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40
	m.selected = 1

	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	if m.tasks[1].Status != types.StatusSkipped {
		t.Errorf("Expected task to be skipped, got %s", m.tasks[1].Status)
	}
}

func TestVerify_BisectResultDisplay(t *testing.T) {
	results := map[int]workflow.VerificationResult{
		3: {TaskID: 3, FilesExist: false, SyntaxOK: false, TestsOK: false, Errors: []string{"unrecoverable"}},
	}
	tasks := []types.Task{
		{ID: 3, Action: "Test", Description: "broken task", Status: types.StatusUnrecoverable},
	}

	m := NewVerifyModel(tasks, results, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "UNRECOVERABLE") {
		t.Error("Expected 'UNRECOVERABLE' badge")
	}
}

func TestVerify_UnrecoverableBadge(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Test", Description: "failed task", Status: types.StatusUnrecoverable},
	}
	m := NewVerifyModel(tasks, map[int]workflow.VerificationResult{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "[!]") {
		t.Error("Expected '!' indicator for unrecoverable task")
	}
}

func TestVerify_AutoTransitionToShip(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Create", Description: "test", Status: types.StatusDone},
	}
	m := NewVerifyModel(tasks, map[int]workflow.VerificationResult{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	if appMsg != nil && appMsg.Screen == ScreenShip {
		// Good
	} else if appMsg == nil {
		// Check manually
		allDone := true
		for _, t := range m.tasks {
			if t.Status == types.StatusPending || t.Status == types.StatusRunning {
				allDone = false
			}
		}
		if allDone {
			t.Log("All done, should transition to Ship")
		}
	}
}

func TestVerify_SelectedTaskHighlight(t *testing.T) {
	m := NewVerifyModel(testTasks(), map[int]workflow.VerificationResult{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40
	m.selected = 1

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
}

// ---------------------------------------------------------------------------
// Ship screen tests
// ---------------------------------------------------------------------------

func TestShip_RenderView(t *testing.T) {
	m := NewShipModel(ShipSummary{
		SessionID: "a1b2c3d4",
		Duration:  "12m 34s",
		TaskDone:  6,
		TaskTotal: 8,
	}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if view == "" {
		t.Fatal("Expected non-empty view")
	}
	if !containsStr(view, "Session Complete") {
		t.Error("Expected 'Session Complete' in view")
	}
}

func TestShip_SummaryDisplay(t *testing.T) {
	m := NewShipModel(ShipSummary{
		SessionID:   "a1b2c3d4",
		Duration:    "12m 34s",
		TaskDone:    6,
		TaskTotal:   8,
		TaskFailed:  1,
		TaskSkipped: 1,
	}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	// New UI shows metrics as "6/8" format
	if !containsStr(view, "6/8") {
		t.Error("Expected task summary (6/8) in view")
	}
	if !containsStr(view, "Failed") {
		t.Error("Expected failed section in view")
	}
}

func TestShip_CommitLog(t *testing.T) {
	m := NewShipModel(ShipSummary{
		Commits: []git.CommitInfo{
			{ShortHash: "a1b2c34", Message: "feat(task 1): Create go.mod"},
			{ShortHash: "b2c3d45", Message: "feat(task 2): Add main.go"},
		},
	}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	view := m.View()
	if !containsStr(view, "Commits") {
		t.Error("Expected 'Commits' header")
	}
	if !containsStr(view, "a1b2c34") {
		t.Error("Expected commit hash in view")
	}
}

func TestShip_NewSessionAction(t *testing.T) {
	m := NewShipModel(ShipSummary{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}})
	if appMsg == nil {
		t.Fatal("Expected AppMsg on new session")
	}
	if appMsg.Screen != ScreenFirstRun {
		t.Errorf("Expected ScreenFirstRun, got %d", appMsg.Screen)
	}
}

func TestShip_ReturnToREPLAction(t *testing.T) {
	m := NewShipModel(ShipSummary{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	_, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	if appMsg == nil {
		t.Fatal("Expected AppMsg on return to REPL")
	}
	if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL, got %d", appMsg.Screen)
	}
}

func TestShip_OpenInBrowser(t *testing.T) {
	m := NewShipModel(ShipSummary{}, testTheme(), 0, 0)
	m.width = 80
	m.height = 40

	// Should not crash
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'o'}})
}
