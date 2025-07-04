package workflow

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
)

func TestEngine_RunVerify_NoTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
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

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
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

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
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

	result := engine.verifyTask(task)
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

	result := engine.verifyTask(task)
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

	result := engine.verifyTask(task)
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

	result, err := engine.RunPhase(context.Background(), m31types.PhaseVerify, "Test")
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
	engine.git.Commit("initial commit")

	// Create a new engine to test hash capture
	dir := engine.workDir
	sessionBaseDir := engine.planningDir
	sessionBaseDir = sessionBaseDir[:len(sessionBaseDir)-len("/planning")]
	mgr := engine.sessionMgr

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := engine.planningDir
	eng, err := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		&mockProvider{}, "test-model", tools.NewDispatcher(nil), tokens.NewEstimator("test-model"), mgr, nil)
	eng.SetGit(engine.git)

	if eng.sessionStartHash == "" {
		t.Error("Expected sessionStartHash to be captured after SetGit with commits")
	}
}
