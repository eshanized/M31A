package session

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// ---------------------------------------------------------------------------
// PROJECT.md tests
// ---------------------------------------------------------------------------

func TestPlanning_SaveAndLoadProject(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	original := &types.ProjectState{
		Goal:        "Build a TUI coding agent",
		ProjectType: "Go CLI",
		Framework:   "Bubble Tea",
		Answers: map[string]string{
			"What language?": "Go",
			"What UI lib?":   "Bubble Tea",
		},
	}

	if saveErr := mgr.SaveProject(s.ID, original); saveErr != nil {
		t.Fatalf("SaveProject failed: %v", saveErr)
	}

	loaded, err := mgr.LoadProject(s.ID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if loaded == nil {
		t.Fatal("Expected non-nil project")
	}

	if loaded.Goal != original.Goal {
		t.Errorf("Goal: expected %q, got %q", original.Goal, loaded.Goal)
	}
	if loaded.ProjectType != original.ProjectType {
		t.Errorf("ProjectType: expected %q, got %q", original.ProjectType, loaded.ProjectType)
	}
	if loaded.Framework != original.Framework {
		t.Errorf("Framework: expected %q, got %q", original.Framework, loaded.Framework)
	}
	if len(loaded.Answers) != len(original.Answers) {
		t.Errorf("Answers count: expected %d, got %d", len(original.Answers), len(loaded.Answers))
	}
	for q, expectedA := range original.Answers {
		if loaded.Answers[q] != expectedA {
			t.Errorf("Answer for %q: expected %q, got %q", q, expectedA, loaded.Answers[q])
		}
	}
}

func TestPlanning_LoadProjectMissing(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	project, err := mgr.LoadProject("nonexistent")
	if err != nil {
		t.Fatalf("LoadProject on missing file should not error, got: %v", err)
	}
	if project != nil {
		t.Error("Expected nil project for missing file")
	}
}

func TestPlanning_ToleratesExtraWhitespace(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Manually write PROJECT.md with extra whitespace and blank lines
	content := `# Project

**Goal:**    Build a TUI agent with extra   spaces
**Type:**    Go   CLI

**Framework:**   Bubble Tea

## Questions

- **Q:**   What language?   → **A:**   Go
- **Q:** What UI lib? → **A:**   Bubble Tea
`
	projectPath := filepath.Join(mgr.planningDirPath(), "PROJECT.md")
	if writeErr := os.WriteFile(projectPath, []byte(content), 0644); writeErr != nil {
		t.Fatalf("Failed to write PROJECT.md: %v", writeErr)
	}

	project, err := mgr.LoadProject(s.ID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project == nil {
		t.Fatal("Expected non-nil project")
	}

	if project.Goal != "Build a TUI agent with extra   spaces" {
		t.Errorf("Goal: expected %q, got %q", "Build a TUI agent with extra   spaces", project.Goal)
	}
	if project.ProjectType != "Go   CLI" {
		t.Errorf("ProjectType: expected %q, got %q", "Go   CLI", project.ProjectType)
	}
	if project.Framework != "Bubble Tea" {
		t.Errorf("Framework: expected %q, got %q", "Bubble Tea", project.Framework)
	}
	if len(project.Answers) != 2 {
		t.Fatalf("Expected 2 answers, got %d", len(project.Answers))
	}
	if project.Answers["What language?"] != "Go" {
		t.Errorf("Answer 'What language?': expected %q, got %q", "Go", project.Answers["What language?"])
	}
	if project.Answers["What UI lib?"] != "Bubble Tea" {
		t.Errorf("Answer 'What UI lib?': expected %q, got %q", "Bubble Tea", project.Answers["What UI lib?"])
	}
}

func TestPlanning_ToleratesMissingSections(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// PROJECT.md without Questions section
	content := `# Project

**Goal:** Simple project
**Type:** Test
**Framework:** None
`
	projectPath := filepath.Join(mgr.planningDirPath(), "PROJECT.md")
	if writeErr := os.WriteFile(projectPath, []byte(content), 0644); writeErr != nil {
		t.Fatalf("Failed to write PROJECT.md: %v", writeErr)
	}

	project, err := mgr.LoadProject(s.ID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project == nil {
		t.Fatal("Expected non-nil project")
	}
	if project.Goal != "Simple project" {
		t.Errorf("Goal: expected %q, got %q", "Simple project", project.Goal)
	}
	if len(project.Answers) != 0 {
		t.Errorf("Expected 0 answers, got %d", len(project.Answers))
	}
}

// ---------------------------------------------------------------------------
// TASKS.md tests
// ---------------------------------------------------------------------------

func TestPlanning_SaveAndLoadTasks(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	original := []types.Task{
		{
			ID:           1,
			Action:       "Create",
			Description:  "Build main.go entry point",
			Dependencies: []int{},
			Files:        []string{"cmd/main.go"},
			Status:       types.StatusDone,
		},
		{
			ID:           2,
			Action:       "Add",
			Description:  "Add auth middleware",
			Dependencies: []int{1},
			Files:        []string{"internal/auth/middleware.go", "internal/auth/types.go"},
			Status:       types.StatusPending,
		},
		{
			ID:           3,
			Action:       "Test",
			Description:  "Write unit tests for auth",
			Dependencies: []int{1, 2},
			Files:        []string{"internal/auth/auth_test.go"},
			Status:       types.StatusRunning,
		},
	}

	if saveErr := mgr.SaveTasks(s.ID, original); saveErr != nil {
		t.Fatalf("SaveTasks failed: %v", saveErr)
	}

	loaded, err := mgr.LoadTasks(s.ID)
	if err != nil {
		t.Fatalf("LoadTasks failed: %v", err)
	}
	if len(loaded) != 3 {
		t.Fatalf("Expected 3 tasks, got %d", len(loaded))
	}

	// Verify all fields round-trip correctly
	for i, task := range loaded {
		if task.ID != original[i].ID {
			t.Errorf("Task %d ID: expected %d, got %d", i, original[i].ID, task.ID)
		}
		if task.Action != original[i].Action {
			t.Errorf("Task %d Action: expected %q, got %q", i, original[i].Action, task.Action)
		}
		if task.Description != original[i].Description {
			t.Errorf("Task %d Description: expected %q, got %q", i, original[i].Description, task.Description)
		}
		if len(task.Dependencies) != len(original[i].Dependencies) {
			t.Errorf("Task %d Dependencies: expected %v, got %v", i, original[i].Dependencies, task.Dependencies)
		} else {
			for j := range task.Dependencies {
				if task.Dependencies[j] != original[i].Dependencies[j] {
					t.Errorf("Task %d Dep %d: expected %d, got %d", i, j, original[i].Dependencies[j], task.Dependencies[j])
				}
			}
		}
		if len(task.Files) != len(original[i].Files) {
			t.Errorf("Task %d Files: expected %v, got %v", i, original[i].Files, task.Files)
		} else {
			for j := range task.Files {
				if task.Files[j] != original[i].Files[j] {
					t.Errorf("Task %d File %d: expected %q, got %q", i, j, original[i].Files[j], task.Files[j])
				}
			}
		}
		if task.Status != original[i].Status {
			t.Errorf("Task %d Status: expected %q, got %q", i, original[i].Status, task.Status)
		}
	}
}

func TestPlanning_LoadTasksMissing(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	tasks, err := mgr.LoadTasks("nonexistent")
	if err != nil {
		t.Fatalf("LoadTasks on missing file should not error, got: %v", err)
	}
	if tasks == nil {
		t.Error("Expected empty slice, got nil")
	}
	if len(tasks) != 0 {
		t.Errorf("Expected 0 tasks, got %d", len(tasks))
	}
}

func TestPlanning_TasksWithDeps(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	original := []types.Task{
		{
			ID:           1,
			Action:       "Refactor",
			Description:  "Split auth package",
			Dependencies: []int{1, 2},
			Files:        []string{"a.go", "b.go"},
			Status:       types.StatusPending,
		},
	}

	if saveErr := mgr.SaveTasks(s.ID, original); saveErr != nil {
		t.Fatalf("SaveTasks failed: %v", saveErr)
	}

	loaded, err := mgr.LoadTasks(s.ID)
	if err != nil {
		t.Fatalf("LoadTasks failed: %v", err)
	}
	if len(loaded) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(loaded))
	}

	if len(loaded[0].Dependencies) != 2 {
		t.Errorf("Expected 2 dependencies, got %v", loaded[0].Dependencies)
	} else {
		if loaded[0].Dependencies[0] != 1 {
			t.Errorf("Dep[0]: expected 1, got %d", loaded[0].Dependencies[0])
		}
		if loaded[0].Dependencies[1] != 2 {
			t.Errorf("Dep[1]: expected 2, got %d", loaded[0].Dependencies[1])
		}
	}

	if len(loaded[0].Files) != 2 {
		t.Errorf("Expected 2 files, got %v", loaded[0].Files)
	} else {
		if loaded[0].Files[0] != "a.go" {
			t.Errorf("File[0]: expected 'a.go', got %q", loaded[0].Files[0])
		}
		if loaded[0].Files[1] != "b.go" {
			t.Errorf("File[1]: expected 'b.go', got %q", loaded[0].Files[1])
		}
	}
}

// ---------------------------------------------------------------------------
// STATE.md tests
// ---------------------------------------------------------------------------

func TestPlanning_SaveAndLoadState(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save state
	if saveErr := mgr.SaveState(s.ID, types.PhasePlan, "3 of 5 tasks complete", "Executed task 3"); saveErr != nil {
		t.Fatalf("SaveState failed: %v", saveErr)
	}

	// Load and verify
	phase, progress, lastAction, timestamp, err := mgr.LoadState(s.ID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != types.PhasePlan {
		t.Errorf("Phase: expected %q, got %q", types.PhasePlan, phase)
	}
	if progress != "3 of 5 tasks complete" {
		t.Errorf("Progress: expected %q, got %q", "3 of 5 tasks complete", progress)
	}
	if lastAction != "Executed task 3" {
		t.Errorf("LastAction: expected %q, got %q", "Executed task 3", lastAction)
	}
	if timestamp.IsZero() {
		t.Error("Expected non-zero timestamp")
	}
}

func TestPlanning_LoadStateMissing(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	phase, progress, lastAction, timestamp, err := mgr.LoadState("nonexistent")
	if err != nil {
		t.Fatalf("LoadState on missing file should not error, got: %v", err)
	}
	if phase != types.PhaseIdle {
		t.Errorf("Phase: expected %q, got %q", types.PhaseIdle, phase)
	}
	if progress != "" {
		t.Errorf("Progress: expected empty, got %q", progress)
	}
	if lastAction != "" {
		t.Errorf("LastAction: expected empty, got %q", lastAction)
	}
	if !timestamp.IsZero() {
		t.Error("Expected zero timestamp for missing file")
	}
}

func TestPlanning_StateTimestampFormat(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save state
	if saveErr := mgr.SaveState(s.ID, types.PhaseExecute, "1 of 1", "First task"); saveErr != nil {
		t.Fatalf("SaveState failed: %v", saveErr)
	}

	// Load and verify RFC3339 roundtrip
	_, _, _, timestamp, err := mgr.LoadState(s.ID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if timestamp.IsZero() {
		t.Fatal("Expected non-zero timestamp")
	}

	// Verify it can be formatted back to RFC3339 and re-parsed
	formatted := timestamp.Format(time.RFC3339)
	parsed, err := time.Parse(time.RFC3339, formatted)
	if err != nil {
		t.Fatalf("Timestamp does not roundtrip RFC3339: %v", err)
	}
	if !parsed.Equal(timestamp) {
		t.Errorf("Timestamp roundtrip mismatch: %v vs %v", parsed, timestamp)
	}
}

// ---------------------------------------------------------------------------
// Acceptance: atomicWrite used (verify no temp files left behind)
// ---------------------------------------------------------------------------

func TestPlanning_NoTempFilesAfterSave(t *testing.T) {
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Save all three
	_ = mgr.SaveProject(s.ID, &types.ProjectState{Goal: "x", ProjectType: "y", Framework: "z"})
	_ = mgr.SaveTasks(s.ID, []types.Task{{ID: 1, Action: "A", Description: "D"}})
	_ = mgr.SaveState(s.ID, types.PhaseIdle, "p", "a")

	// Check for leftover temp files
	planningDir := mgr.planningDirPath()
	entries, err := os.ReadDir(planningDir)
	if err != nil {
		t.Fatalf("Failed to read planning dir: %v", err)
	}
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".m31a_tmp") {
			t.Errorf("Leftover temp file: %s", e.Name())
		}
	}
}
