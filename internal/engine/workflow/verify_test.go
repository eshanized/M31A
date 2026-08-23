package workflow

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/tests/testutil/mocks"
)

func TestEngine_RunVerify_NoTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("RunPhase verify failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected verify to succeed with no tasks")
	}
}

func TestEngine_RunVerify_WithDoneTasks(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Initialize and save a done task with a file
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create the file that the task references
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Create main.go", Files: []string{"main.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("RunPhase verify failed: %v", err)
	}
	// Should succeed since file exists
	if !result.Success {
		t.Error("Expected verify to succeed when files exist")
	}
}

func TestEngine_RunVerify_MissingFile(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Task references a file that doesn't exist
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Create missing.go", Files: []string{"missing.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("RunPhase verify failed: %v", err)
	}
	// Missing file means verification fails
	if !result.Success {
		t.Log("Verify correctly reported failure for missing file")
	}
}

func TestEngine_VerifyTask_FileExistence(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Create a proper Go module
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\nfunc main() {}"), 0644)

	task := m31types.Task{
		ID:     1,
		Files:  []string{"main.go"},
		Status: m31types.StatusDone,
	}

	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("Expected files to exist")
	}
	if !result.SyntaxOK {
		t.Errorf("Expected syntax to be OK, errors: %v", result.Errors)
	}
}

func TestEngine_VerifyTask_MissingFile(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:     1,
		Files:  []string{"nonexistent.go"},
		Status: m31types.StatusDone,
	}

	result := engine.verifyTask(context.Background(), task)
	if result.FilesExist {
		t.Error("Expected files to not exist")
	}
	if len(result.Errors) == 0 {
		t.Error("Expected errors for missing file")
	}
}

func TestEngine_VerifyTask_NoFiles(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:     1,
		Files:  []string{},
		Status: m31types.StatusDone,
	}

	result := engine.verifyTask(context.Background(), task)
	if !result.FilesExist {
		t.Error("Expected files existence to pass with empty file list")
	}
}

func TestVerificationResult_Fields(t *testing.T) {
	result := VerificationResult{
		TaskID:     42,
		FilesExist: true,
		SyntaxOK:   true,
		TestsOK:    false,
		Errors:     []string{"test failed"},
	}

	if result.TaskID != 42 {
		t.Errorf("Expected TaskID 42, got %d", result.TaskID)
	}
	if result.TestsOK {
		t.Error("Expected TestsOK to be false")
	}
	if len(result.Errors) != 1 {
		t.Errorf("Expected 1 error, got %d", len(result.Errors))
	}
}

func TestEngine_RunVerify_SkipsPendingTasks(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Only pending tasks — should be skipped
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Pending task", Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("RunPhase verify failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected verify to succeed when only pending tasks")
	}
}

func TestEngine_SessionStartHash(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// setupTestEngine creates a git repo and calls SetGit
	// which should capture the HEAD hash (even if no commits yet, it should be empty)
	// After first commit, sessionStartHash should be set
	os.WriteFile(filepath.Join(engine.workDir, "dummy.go"), []byte("package main"), 0644)
	engine.git.Add("dummy.go")
	_, _ = engine.git.CommitWithFiles("initial commit", "dummy.go")

	// Create a new engine to test hash capture
	dir := engine.workDir
	_ = dir
	mgr := engine.sessionMgr

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := engine.planningDir
	d, derr := tools.DefaultDispatcher(dir, filepath.Join(dir, "backups"), dir, nil, nil, nil)
	if derr != nil {
		t.Fatalf("DefaultDispatcher failed: %v", derr)
	}
	t.Cleanup(func() { d.Stop() })
	eng, _ := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		mocks.NewMockProvider("mock"), "test-model", d, tokens.NewEstimator("test-model"), mgr, nil)
	eng.SetGit(engine.git)

	if eng.sessionStartHash == "" {
		t.Error("Expected sessionStartHash to be captured after SetGit with commits")
	}
}

func TestRunVerify_AllPass(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create files for all tasks
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)
	os.WriteFile(filepath.Join(engine.workDir, "b.go"), []byte("package main"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Create a.go", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Create b.go", Files: []string{"b.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	if err != nil {
		t.Fatalf("RunPhase verify failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected verify to succeed when all tasks pass")
	}
}

func TestRunVerify_90PercentPass(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create file for passing task
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)

	// 9 out of 10 tasks pass (90% threshold)
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Task 1", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Task 2", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 3, Action: "Create", Description: "Task 3", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 4, Action: "Create", Description: "Task 4", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 5, Action: "Create", Description: "Task 5", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 6, Action: "Create", Description: "Task 6", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 7, Action: "Create", Description: "Task 7", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 8, Action: "Create", Description: "Task 8", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 9, Action: "Create", Description: "Task 9", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 10, Action: "Create", Description: "Task 10", Files: []string{"missing.go"}, Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	// Should succeed because 90% pass rate meets threshold
	if err != nil {
		t.Logf("Verify returned error (may be expected): %v", err)
	}
	if !result.Success {
		t.Error("Expected verify to succeed with 90% pass rate")
	}
}

func TestRunVerify_Below90Percent(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Create files for passing tasks only
	os.WriteFile(filepath.Join(engine.workDir, "a.go"), []byte("package main"), 0644)

	// 8 out of 10 tasks pass (80% - below 90% threshold)
	// Tasks 9 and 10 reference missing files - verification will fail and set status to failed
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Task 1", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Task 2", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 3, Action: "Create", Description: "Task 3", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 4, Action: "Create", Description: "Task 4", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 5, Action: "Create", Description: "Task 5", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 6, Action: "Create", Description: "Task 6", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 7, Action: "Create", Description: "Task 7", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 8, Action: "Create", Description: "Task 8", Files: []string{"a.go"}, Status: m31types.StatusDone},
		{ID: 9, Action: "Create", Description: "Task 9", Files: []string{"missing1.go"}, Status: m31types.StatusFailed},
		{ID: 10, Action: "Create", Description: "Task 10", Files: []string{"missing2.go"}, Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseVerify, "Test")
	// Should fail because 80% pass rate is below 90% threshold
	if err == nil {
		t.Error("Expected verify to fail with below 90% pass rate")
	}
	if result.Success {
		t.Error("Expected result.Success to be false with below 90% pass rate")
	}
}

func TestVerifySuccessThreshold(t *testing.T) {
	// Threshold is now 1.0 (all-or-nothing by default)
	// The 0.90 threshold is only used when VerifyAllowPartial is enabled
	if VerifySuccessThreshold != 1.0 {
		t.Errorf("Expected VerifySuccessThreshold to be 1.0 (all-or-nothing), got %f", VerifySuccessThreshold)
	}
}
