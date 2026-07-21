package session

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

// ---------------------------------------------------------------------------
// SavePlan / LoadPlan
// ---------------------------------------------------------------------------

func TestPlanning_SaveAndLoadPlan(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	markdown := "# Implementation Plan\n\n1. Step one\n2. Step two\n"
	if saveErr := mgr.SavePlan(s.ID, 1, markdown); saveErr != nil {
		t.Fatalf("SavePlan failed: %v", saveErr)
	}

	loaded, err := mgr.LoadPlan(s.ID)
	if err != nil {
		t.Fatalf("LoadPlan failed: %v", err)
	}
	if loaded != markdown {
		t.Errorf("Plan mismatch: got %q", loaded)
	}

	// Verify versioned file exists
	versioned := filepath.Join(mgr.planningDirPath(), "plan_v1.md")
	data, err := os.ReadFile(versioned)
	if err != nil {
		t.Fatalf("Failed to read versioned plan: %v", err)
	}
	if string(data) != markdown {
		t.Errorf("Versioned plan mismatch: got %q", string(data))
	}
}

func TestPlanning_SavePlanVersionZeroSkipsVersioned(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	if err := mgr.SavePlan(s.ID, 0, "plan content"); err != nil {
		t.Fatalf("SavePlan failed: %v", err)
	}

	// plan.md should exist
	planPath := filepath.Join(mgr.planningDirPath(), "plan.md")
	if _, err := os.Stat(planPath); os.IsNotExist(err) {
		t.Error("plan.md should exist")
	}

	// plan_v0.md should NOT exist
	versionedPath := filepath.Join(mgr.planningDirPath(), "plan_v0.md")
	if _, err := os.Stat(versionedPath); !os.IsNotExist(err) {
		t.Error("plan_v0.md should not exist when version is 0")
	}
}

func TestPlanning_LoadPlanMissing(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	plan, err := mgr.LoadPlan("nonexistent")
	if err != nil {
		t.Fatalf("LoadPlan on missing file should not error, got: %v", err)
	}
	if plan != "" {
		t.Errorf("Expected empty plan for missing file, got %q", plan)
	}
}

// ---------------------------------------------------------------------------
// SaveDemonstration / LoadDemonstration
// ---------------------------------------------------------------------------

func TestPlanning_SaveAndLoadDemonstration(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	markdown := "# Demonstration\n\nHere is the walkthrough of the feature.\n"
	if saveErr := mgr.SaveDemonstration(s.ID, markdown); saveErr != nil {
		t.Fatalf("SaveDemonstration failed: %v", saveErr)
	}

	loaded, err := mgr.LoadDemonstration(s.ID)
	if err != nil {
		t.Fatalf("LoadDemonstration failed: %v", err)
	}
	if loaded != markdown {
		t.Errorf("Demonstration mismatch: got %q", loaded)
	}
}

func TestPlanning_LoadDemonstrationMissing(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	demo, err := mgr.LoadDemonstration("nonexistent")
	if err != nil {
		t.Fatalf("LoadDemonstration on missing file should not error, got: %v", err)
	}
	if demo != "" {
		t.Errorf("Expected empty demonstration for missing file, got %q", demo)
	}
}

// ---------------------------------------------------------------------------
// SaveTasksCheckbox
// ---------------------------------------------------------------------------

func TestPlanning_SaveTasksCheckbox_GroupsByCategory(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	tasks := []types.Task{
		{ID: 1, Description: "Task A", Category: "Backend", Status: types.StatusDone},
		{ID: 2, Description: "Task B", Category: "Backend", Status: types.StatusPending},
		{ID: 3, Description: "Task C", Category: "Frontend", Status: types.StatusDone},
		{ID: 4, Description: "Task D", Status: types.StatusPending},
	}

	if saveErr := mgr.SaveTasksCheckbox(s.ID, tasks); saveErr != nil {
		t.Fatalf("SaveTasksCheckbox failed: %v", saveErr)
	}

	path := filepath.Join(mgr.planningDirPath(), "tasks.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read tasks.md: %v", err)
	}

	content := string(data)
	if !strings.Contains(content, "Backend") {
		t.Error("Expected Backend category in output")
	}
	if !strings.Contains(content, "Frontend") {
		t.Error("Expected Frontend category in output")
	}
	if !strings.Contains(content, "General") {
		t.Error("Expected General category for tasks without category")
	}
	// Backend category is NOT all done (task B is pending), so unchecked
	if strings.Contains(content, "- [x] **Backend**") {
		t.Error("Backend category should not be checked (task B is pending)")
	}
	// Frontend category IS all done
	if !strings.Contains(content, "- [x] **Frontend**") {
		t.Error("Frontend category should be checked (all tasks done)")
	}
}

func TestPlanning_SaveTasksCheckbox_AllDoneCategories(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	tasks := []types.Task{
		{ID: 1, Description: "Task A", Category: "Deploy", Status: types.StatusDone},
		{ID: 2, Description: "Task B", Category: "Deploy", Status: types.StatusSkipped},
	}

	if saveErr := mgr.SaveTasksCheckbox(s.ID, tasks); saveErr != nil {
		t.Fatalf("SaveTasksCheckbox failed: %v", saveErr)
	}

	path := filepath.Join(mgr.planningDirPath(), "tasks.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read tasks.md: %v", err)
	}

	content := string(data)
	if !strings.Contains(content, "- [x] **Deploy**") {
		t.Error("Deploy category should be checked (all tasks done/skipped)")
	}
	// Task checkboxes should also be checked
	if !strings.Contains(content, "- [x] Task A") {
		t.Error("Task A should be checked")
	}
	if !strings.Contains(content, "- [x] Task B") {
		t.Error("Task B (skipped) should be checked")
	}
}

func TestPlanning_SaveTasksCheckbox_EmptyTasks(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("gpt-4o", "openrouter")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	if saveErr := mgr.SaveTasksCheckbox(s.ID, nil); saveErr != nil {
		t.Fatalf("SaveTasksCheckbox with nil tasks failed: %v", saveErr)
	}

	path := filepath.Join(mgr.planningDirPath(), "tasks.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read tasks.md: %v", err)
	}

	content := string(data)
	if !strings.Contains(content, "# Task List") {
		t.Error("Expected Task List header even for empty tasks")
	}
}

// ---------------------------------------------------------------------------
// readFileLimited
// ---------------------------------------------------------------------------

func TestReadFileLimited_NormalFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "test.txt")
	content := []byte("hello world")
	if err := os.WriteFile(path, content, 0644); err != nil {
		t.Fatal(err)
	}

	data, err := readFileLimited(path, 1024)
	if err != nil {
		t.Fatalf("readFileLimited failed: %v", err)
	}
	if string(data) != "hello world" {
		t.Errorf("Expected 'hello world', got %q", string(data))
	}
}

func TestReadFileLimited_FileExceedsLimit(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "big.txt")
	content := []byte("this is a file that exceeds the limit")
	if err := os.WriteFile(path, content, 0644); err != nil {
		t.Fatal(err)
	}

	_, err := readFileLimited(path, 10) // limit is 10 bytes
	if err == nil {
		t.Error("Expected error for file exceeding size limit")
	}
	if !strings.Contains(err.Error(), "exceeds maximum size limit") {
		t.Errorf("Expected size limit error, got: %v", err)
	}
}

func TestReadFileLimited_MissingFile(t *testing.T) {
	t.Parallel()
	_, err := readFileLimited("/nonexistent/path/file.txt", 1024)
	if err == nil {
		t.Error("Expected error for missing file")
	}
	if !os.IsNotExist(err) {
		t.Errorf("Expected os.IsNotExist error, got: %v", err)
	}
}

func TestReadFileLimited_EmptyFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "empty.txt")
	if err := os.WriteFile(path, []byte{}, 0644); err != nil {
		t.Fatal(err)
	}

	data, err := readFileLimited(path, 1024)
	if err != nil {
		t.Fatalf("readFileLimited on empty file failed: %v", err)
	}
	if len(data) != 0 {
		t.Errorf("Expected empty data, got %d bytes", len(data))
	}
}

func TestReadFileLimited_ExactLimit(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "exact.txt")
	content := []byte("12345")
	if err := os.WriteFile(path, content, 0644); err != nil {
		t.Fatal(err)
	}

	data, err := readFileLimited(path, 5) // exactly 5 bytes
	if err != nil {
		t.Fatalf("readFileLimited at exact limit failed: %v", err)
	}
	if string(data) != "12345" {
		t.Errorf("Expected '12345', got %q", string(data))
	}
}

// ---------------------------------------------------------------------------
// generateID
// ---------------------------------------------------------------------------

func TestGenerateID_DifferentLengths(t *testing.T) {
	t.Parallel()
	for _, numBytes := range []int{1, 2, 4, 8} {
		id, err := generateID(numBytes)
		if err != nil {
			t.Fatalf("generateID(%d) failed: %v", numBytes, err)
		}
		expectedLen := numBytes * 2
		if len(id) != expectedLen {
			t.Errorf("generateID(%d): expected %d hex chars, got %d (%q)", numBytes, expectedLen, len(id), id)
		}
	}
}

func TestGenerateID_Uniqueness(t *testing.T) {
	t.Parallel()
	ids := make(map[string]bool)
	for i := 0; i < 1000; i++ {
		id, err := generateID(4)
		if err != nil {
			t.Fatalf("generateID failed: %v", err)
		}
		if ids[id] {
			t.Fatalf("Duplicate ID generated: %s", id)
		}
		ids[id] = true
	}
}

func TestGenerateID_HexOnly(t *testing.T) {
	t.Parallel()
	for i := 0; i < 100; i++ {
		id, err := generateID(4)
		if err != nil {
			t.Fatalf("generateID failed: %v", err)
		}
		for _, c := range id {
			if (c < '0' || c > '9') && (c < 'a' || c > 'f') {
				t.Errorf("Non-hex char %c in ID %q", c, id)
				break
			}
		}
	}
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

func TestManager_ProjectDir(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	expected := filepath.Join(dir, ".m31a")
	got := mgr.projectDir()
	if got != expected {
		t.Errorf("projectDir: expected %q, got %q", expected, got)
	}
}

func TestManager_SessionJSONPath(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_MessagesJSONPath(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_PlanningDirPath(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_RecentModelsPath(t *testing.T) {
	t.Skip("removed: project-local sessions")
}

func TestManager_BaseDir(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	if got := mgr.BaseDir(); got != dir {
		t.Errorf("BaseDir(): expected %q, got %q", dir, got)
	}
}

// ---------------------------------------------------------------------------
// NewManager options
// ---------------------------------------------------------------------------

func TestManager_NewManager_DefaultOpts(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{})
	if mgr.sessionIDBytes != 4 {
		t.Errorf("Expected default sessionIDBytes=4, got %d", mgr.sessionIDBytes)
	}
	if mgr.maxRecentModels != types.DefaultMaxRecentModels {
		t.Errorf("Expected default maxRecentModels=%d, got %d", types.DefaultMaxRecentModels, mgr.maxRecentModels)
	}
	if mgr.sessionCacheTTL != types.DefaultSessionCacheTTL {
		t.Errorf("Expected default sessionCacheTTL=%v, got %v", types.DefaultSessionCacheTTL, mgr.sessionCacheTTL)
	}
}

func TestManager_NewManager_CustomOpts(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	mgr := NewManager(dir, dir, ManagerOpts{
		SessionIDBytes:  8,
		MaxRecentModels: 5,
		SessionCacheTTL: 10,
	})
	if mgr.sessionIDBytes != 8 {
		t.Errorf("Expected sessionIDBytes=8, got %d", mgr.sessionIDBytes)
	}
	if mgr.maxRecentModels != 5 {
		t.Errorf("Expected maxRecentModels=5, got %d", mgr.maxRecentModels)
	}
	if mgr.sessionCacheTTL != 10 {
		t.Errorf("Expected sessionCacheTTL=10, got %v", mgr.sessionCacheTTL)
	}
}

// ---------------------------------------------------------------------------
// SaveTasks edge cases: empty deps, empty files
// ---------------------------------------------------------------------------

func TestPlanning_TasksWithEmptyDepsAndFiles(t *testing.T) {
	t.Parallel()
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
			Description:  "A simple task",
			Dependencies: nil,
			Files:        nil,
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
	if loaded[0].Dependencies != nil {
		t.Errorf("Expected nil deps, got %v", loaded[0].Dependencies)
	}
	if loaded[0].Files != nil {
		t.Errorf("Expected nil files, got %v", loaded[0].Files)
	}
}

// ---------------------------------------------------------------------------
// LoadState with empty phase defaults to PhaseIdle
// ---------------------------------------------------------------------------

func TestPlanning_LoadStateEmptyPhaseDefaultsToIdle(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Manually write STATE.md with no Phase line
	content := `# State

**Progress:** some progress
**Last Action:** did something
**Timestamp:** 2025-01-01T00:00:00Z
`
	statePath := filepath.Join(mgr.planningDirPath(), "STATE.md")
	if writeErr := os.WriteFile(statePath, []byte(content), 0644); writeErr != nil {
		t.Fatalf("Failed to write STATE.md: %v", writeErr)
	}

	phase, _, _, _, err := mgr.LoadState(s.ID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != types.PhaseIdle {
		t.Errorf("Expected PhaseIdle for missing phase, got %s", phase)
	}
}

// ---------------------------------------------------------------------------
// LoadState with malformed timestamp
// ---------------------------------------------------------------------------

func TestPlanning_LoadStateMalformedTimestamp(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	content := `# State

**Phase:** plan
**Progress:** half done
**Last Action:** wrote tests
**Timestamp:** not-a-valid-timestamp
`
	statePath := filepath.Join(mgr.planningDirPath(), "STATE.md")
	if writeErr := os.WriteFile(statePath, []byte(content), 0644); writeErr != nil {
		t.Fatalf("Failed to write STATE.md: %v", writeErr)
	}

	phase, progress, lastAction, timestamp, err := mgr.LoadState(s.ID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != types.PhasePlan {
		t.Errorf("Expected PhasePlan, got %s", phase)
	}
	if progress != "half done" {
		t.Errorf("Expected 'half done', got %q", progress)
	}
	if lastAction != "wrote tests" {
		t.Errorf("Expected 'wrote tests', got %q", lastAction)
	}
	if !timestamp.IsZero() {
		t.Errorf("Expected zero timestamp for malformed input, got %v", timestamp)
	}
}

// ---------------------------------------------------------------------------
// LoadProject with malformed Q&A line (no → separator)
// ---------------------------------------------------------------------------

func TestPlanning_LoadProjectMalformedQA(t *testing.T) {
	t.Parallel()
	mgr, dir := newTestManager(t)
	defer os.RemoveAll(dir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	content := `# Project

**Goal:** test goal
**Type:** go
**Framework:** fiber

## Questions

- **Q:** valid question → **A:** valid answer
- **Q:** malformed question without arrow
`
	projectPath := filepath.Join(mgr.planningDirPath(), "PROJECT.md")
	if writeErr := os.WriteFile(projectPath, []byte(content), 0644); writeErr != nil {
		t.Fatalf("Failed to write PROJECT.md: %v", writeErr)
	}

	project, err := mgr.LoadProject(s.ID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project.Goal != "test goal" {
		t.Errorf("Expected goal 'test goal', got %q", project.Goal)
	}
	// The malformed line should be skipped, only valid one parsed
	if len(project.Answers) != 1 {
		t.Errorf("Expected 1 answer (malformed skipped), got %d", len(project.Answers))
	}
	if project.Answers["valid question"] != "valid answer" {
		t.Errorf("Expected answer 'valid answer', got %q", project.Answers["valid question"])
	}
}

// ---------------------------------------------------------------------------
// parseTableRow / parseDeps / parseFileList unit tests
// ---------------------------------------------------------------------------

func TestParseTableRow(t *testing.T) {
	t.Parallel()
	cells := parseTableRow("| a | b | c |")
	if len(cells) != 3 {
		t.Fatalf("Expected 3 cells, got %d", len(cells))
	}
	if cells[0] != "a" || cells[1] != "b" || cells[2] != "c" {
		t.Errorf("Unexpected cells: %v", cells)
	}
}

func TestParseTableRow_WithSpaces(t *testing.T) {
	t.Parallel()
	cells := parseTableRow("|  hello   |  world  |")
	if len(cells) != 2 {
		t.Fatalf("Expected 2 cells, got %d", len(cells))
	}
	if cells[0] != "hello" || cells[1] != "world" {
		t.Errorf("Unexpected cells: %v", cells)
	}
}

func TestParseDeps(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected []int
	}{
		{"-", nil},
		{"", nil},
		{"1", []int{1}},
		{"1, 2, 3", []int{1, 2, 3}},
		{" 1 , 2 ", []int{1, 2}},
		{"abc", nil},
		{"1, abc, 3", []int{1, 3}},
	}
	for _, tt := range tests {
		got := parseDeps(tt.input)
		if len(got) != len(tt.expected) {
			t.Errorf("parseDeps(%q): expected %v, got %v", tt.input, tt.expected, got)
			continue
		}
		for i := range got {
			if got[i] != tt.expected[i] {
				t.Errorf("parseDeps(%q)[%d]: expected %d, got %d", tt.input, i, tt.expected[i], got[i])
			}
		}
	}
}

func TestParseFileList(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected []string
	}{
		{"-", nil},
		{"", nil},
		{"a.go", []string{"a.go"}},
		{"a.go, b.go", []string{"a.go", "b.go"}},
		{"  a.go , b.go ", []string{"a.go", "b.go"}},
		{"a.go,,b.go", []string{"a.go", "b.go"}},
	}
	for _, tt := range tests {
		got := parseFileList(tt.input)
		if len(got) != len(tt.expected) {
			t.Errorf("parseFileList(%q): expected %v, got %v", tt.input, tt.expected, got)
			continue
		}
		for i := range got {
			if got[i] != tt.expected[i] {
				t.Errorf("parseFileList(%q)[%d]: expected %q, got %q", tt.input, i, tt.expected[i], got[i])
			}
		}
	}
}
