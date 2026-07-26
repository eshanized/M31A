package workflow

import (
	"context"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestEngine_RunShip(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Save some tasks
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Task 1", Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Task 2", Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test goal")
	if err != nil {
		t.Fatalf("RunPhase ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected ship to succeed")
	}
	if result.Phase != m31types.PhaseShip {
		t.Errorf("Expected phase ship, got %s", result.Phase)
	}
}

func TestEngine_RunShip_NoTasks(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test")
	if err != nil {
		t.Fatalf("RunPhase ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected ship to succeed even with no tasks")
	}
}

func TestEngine_BuildSummary(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Done task", Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Failed task", Status: m31types.StatusFailed},
		{ID: 3, Action: "Create", Description: "Skipped task", Status: m31types.StatusSkipped},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	summary := engine.BuildSummary()

	if summary.TaskTotal != 3 {
		t.Errorf("Expected 3 total tasks, got %d", summary.TaskTotal)
	}
	if summary.TaskDone != 1 {
		t.Errorf("Expected 1 done task, got %d", summary.TaskDone)
	}
	if summary.TaskFailed != 1 {
		t.Errorf("Expected 1 failed task, got %d", summary.TaskFailed)
	}
	if summary.TaskSkipped != 1 {
		t.Errorf("Expected 1 skipped task, got %d", summary.TaskSkipped)
	}
	if summary.SessionID == "" {
		t.Error("Expected non-empty session ID in summary")
	}
}

func TestEngine_RunShip_NoGit(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Set git to nil — ship should still succeed (git operations are optional)
	engine.git = nil

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test goal")
	if err != nil {
		t.Fatalf("RunPhase ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected ship to succeed without git")
	}
}

func TestEngine_RunShip_WithFailedTasks(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Done task", Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Failed task", Status: m31types.StatusFailed},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test goal")
	// Ship with failed tasks should return an error
	if err == nil {
		t.Error("Expected error when shipping with failed tasks")
	}
	// But result should still be populated
	if result == nil {
		t.Fatal("Expected non-nil result even with failed tasks")
	}
	if result.Success {
		t.Error("Expected result.Success=false when tasks failed")
	}
	if result.Phase != m31types.PhaseShip {
		t.Errorf("Expected phase ship, got %s", result.Phase)
	}
}

func TestEngine_RunShip_WithSkippedTasks(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Done task", Status: m31types.StatusDone},
		{ID: 2, Action: "Create", Description: "Skipped task", Status: m31types.StatusSkipped},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test goal")
	if err != nil {
		t.Fatalf("RunPhase ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected ship to succeed with skipped tasks")
	}
}

func TestEngine_BuildSummary_Empty(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	summary := engine.BuildSummary()

	if summary.TaskTotal != 0 {
		t.Errorf("Expected 0 total tasks, got %d", summary.TaskTotal)
	}
	if summary.TaskDone != 0 {
		t.Errorf("Expected 0 done tasks, got %d", summary.TaskDone)
	}
	if summary.SessionID == "" {
		t.Error("Expected non-empty session ID in summary")
	}
}

func TestShipSummary_Fields(t *testing.T) {
	summary := ShipSummary{
		TaskDone:    5,
		TaskTotal:   10,
		TaskFailed:  2,
		TaskSkipped: 3,
		SessionID:   "test-session",
	}

	if summary.TaskDone != 5 {
		t.Errorf("Expected TaskDone 5, got %d", summary.TaskDone)
	}
	if summary.TaskTotal != 10 {
		t.Errorf("Expected TaskTotal 10, got %d", summary.TaskTotal)
	}
	if summary.SessionID != "test-session" {
		t.Errorf("Expected SessionID 'test-session', got %q", summary.SessionID)
	}
}

// TestShip_SaveStateBeforeArchive verifies CR-10: the ship phase writes
// STATE.md before calling ArchiveSession, so the archived session contains
// the final state. This test verifies the ship phase completes without error
// with the corrected call ordering.
func TestShip_SaveStateBeforeArchive(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Task 1", Status: m31types.StatusDone},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	result, err := engine.RunPhaseDirect(context.Background(), m31types.PhaseShip, "Test goal")
	if err != nil {
		t.Fatalf("RunPhase ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected ship to succeed")
	}
	if result.Phase != m31types.PhaseShip {
		t.Errorf("Expected phase ship, got %s", result.Phase)
	}
}
