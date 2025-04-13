package workflow

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestEngine_RunInitialize(t *testing.T) {
	engine, _ := setupTestEngine(t)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build a REST API")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected initialize to succeed")
	}
	if result.Phase != m31types.PhaseInitialize {
		t.Errorf("Expected phase initialize, got %s", result.Phase)
	}
}

func TestEngine_Initialize_CreatesPlanningDir(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test goal")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}

	if _, err := os.Stat(engine.planningDir); os.IsNotExist(err) {
		t.Fatal("Planning directory should exist after initialize")
	}
}

func TestEngine_Initialize_WritesProjectFile(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build a Go web app")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project == nil {
		t.Fatal("Expected project to be saved")
	}
	if project.Goal != "Build a Go web app" {
		t.Errorf("Expected goal 'Build a Go web app', got %q", project.Goal)
	}
}

func TestEngine_Initialize_WritesState(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}

	phase, _, _, _, err := engine.sessionMgr.LoadState(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != m31types.PhaseInitialize {
		t.Errorf("Expected state phase initialize, got %s", phase)
	}
}

func TestEngine_Initialize_SavesCheckpoint(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}

	checkpoints, err := engine.sessionMgr.LoadCheckpoints(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) == 0 {
		t.Error("Expected at least one checkpoint after initialize")
	}
}

func TestEngine_Initialize_DetectsProjectType(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Create go.mod in work dir
	os.WriteFile(filepath.Join(engine.workDir, "go.mod"), []byte("module test"), 0644)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected initialize to succeed")
	}

	project, _ := engine.sessionMgr.LoadProject(engine.sessionID)
	if project.ProjectType != "go" {
		t.Errorf("Expected project type 'go', got %q", project.ProjectType)
	}
}

func TestEngine_Initialize_GitAlreadyRepo(t *testing.T) {
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// engine.setupTestEngine already inits git, so this tests the "already a repo" path
	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("RunPhase initialize failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected initialize to succeed when git is already initialized")
	}
}
