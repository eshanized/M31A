package workflow

import (
	"context"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
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

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test goal")
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

	result, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "Test")
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
