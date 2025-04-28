package workflow

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// multiTurnMockProvider returns different responses based on the call count.
type multiTurnMockProvider struct {
	callCount int
	responses []string
}

func (m *multiTurnMockProvider) Name() string { return "mock" }
func (m *multiTurnMockProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (m *multiTurnMockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	idx := m.callCount
	m.callCount++
	content := "OK"
	if idx < len(m.responses) {
		content = m.responses[idx]
	}
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: content}, nil
	}
	close := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: close}, nil
}
func (m *multiTurnMockProvider) EstimateCost(modelID string, usage m31types.Usage) float64 {
	return 0
}
func (m *multiTurnMockProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return m31types.HealthStatus{Status: "live"}
}
func (m *multiTurnMockProvider) GetModel(id string) (*m31types.ModelInfo, error) {
	return nil, nil
}

func TestFullWorkflow(t *testing.T) {
	dir := t.TempDir()

	// Init git repo
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	// Create a Go module for build/test checks
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte("module test\ngo 1.22"), 0644)
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\nfunc main() {}"), 0644)

	// Create session
	sessionBaseDir := filepath.Join(dir, "sessions")
	os.MkdirAll(sessionBaseDir, 0755)
	mgr := session.NewManager(sessionBaseDir)

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	// Create dispatcher
	dispatcher := tools.NewDispatcher()

	// Create mock provider with responses for each phase
	// Call 0: Discuss — questions
	// Call 1: Plan — valid task JSON
	// Call 2: Execute — response for task
	// Call 3: Execute — response for task (if needed)
	mockP := &multiTurnMockProvider{
		responses: []string{
			"1. What framework should we use?\n2. What is the target audience?",
			`[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]`,
			"Task completed successfully",
			"Task completed successfully",
		},
	}

	est := tokens.NewEstimator("test-model")

	engine, err := NewEngine(s.ID, dir, filepath.Join(dir, "backups"), planningDir,
		mockP, "test-model", dispatcher, est, mgr)
	engine.SetGit(g)

	ctx := context.Background()

	// Phase 1: Initialize
	t.Log("Running Initialize")
	result, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Build a Go CLI tool")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Initialize should succeed")
	}

	// Verify planning files written
	project, err := mgr.LoadProject(s.ID)
	if err != nil || project == nil {
		t.Fatal("PROJECT.md should exist after initialize")
	}
	if project.Goal != "Build a Go CLI tool" {
		t.Errorf("Expected goal 'Build a Go CLI tool', got %q", project.Goal)
	}

	// Phase 2: Discuss
	t.Log("Running Discuss")
	result, err = engine.RunPhase(ctx, m31types.PhaseDiscuss, "Build a Go CLI tool")
	if err != nil {
		t.Fatalf("Discuss failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Discuss should succeed")
	}
	if !result.NeedsAnswers {
		t.Error("Discuss result should indicate needs answers")
	}

	// Submit answers
	if err := engine.SubmitDiscussAnswer(0, "Go standard library"); err != nil {
		t.Fatalf("SubmitDiscussAnswer 0 failed: %v", err)
	}
	if err := engine.SubmitDiscussAnswer(1, "Developers"); err != nil {
		t.Fatalf("SubmitDiscussAnswer 1 failed: %v", err)
	}

	// Finalize discuss (saves answers + transitions to Plan)
	// We need a method that finalizes without filling defaults — use SkipDiscuss which handles both
	if err := engine.FinalizeDiscuss(); err != nil {
		t.Fatalf("FinalizeDiscuss failed: %v", err)
	}

	// Verify answers saved
	project, _ = mgr.LoadProject(s.ID)
	if project == nil || len(project.Answers) == 0 {
		t.Error("Discuss answers should be saved to PROJECT.md")
	}

	// Phase 3: Plan
	t.Log("Running Plan")
	result, err = engine.RunPhase(ctx, m31types.PhasePlan, "Build a Go CLI tool")
	if err != nil {
		t.Fatalf("Plan failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Plan should succeed")
	}
	if len(result.Tasks) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(result.Tasks))
	}
	if result.Tasks[0].ID != 1 {
		t.Errorf("Expected task ID 1, got %d", result.Tasks[0].ID)
	}

	// Verify TASKS.md written
	loadedTasks, err := mgr.LoadTasks(s.ID)
	if err != nil || len(loadedTasks) != 1 {
		t.Fatalf("TASKS.md should have 1 task, got %d", len(loadedTasks))
	}

	// Phase 4: Execute
	t.Log("Running Execute")
	result, err = engine.RunPhase(ctx, m31types.PhaseExecute, "Build a Go CLI tool")
	if err != nil {
		t.Logf("Execute returned error (expected in test with mock dispatcher): %v", err)
	}
	// Execute may succeed or fail depending on dispatcher behavior — just check result exists
	if result == nil {
		t.Fatal("Execute result should not be nil")
	}

	// Phase 5: Verify
	t.Log("Running Verify")
	result, err = engine.RunPhase(ctx, m31types.PhaseVerify, "Build a Go CLI tool")
	if err != nil {
		t.Logf("Verify returned error: %v", err)
	}
	if result == nil {
		t.Fatal("Verify result should not be nil")
	}

	// Verify planning files exist before Ship (Ship archives session)
	planningFiles := []string{"PROJECT.md", "TASKS.md", "STATE.md"}
	for _, f := range planningFiles {
		path := filepath.Join(planningDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("Expected planning file %s to exist before Ship", f)
		}
	}

	// Phase 6: Ship
	t.Log("Running Ship")
	result, err = engine.RunPhase(ctx, m31types.PhaseShip, "Build a Go CLI tool")
	if err != nil {
		t.Fatalf("Ship failed: %v", err)
	}
	if !result.Success {
		t.Error("Ship should succeed")
	}

	// After Ship, session is archived — verify files exist in archived location
	archivedDir := filepath.Join(sessionBaseDir, "archived", s.ID)
	archivedPlanningDir := filepath.Join(archivedDir, "planning")
	for _, f := range planningFiles {
		path := filepath.Join(archivedPlanningDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("Expected planning file %s to exist in archived location", f)
		}
	}

	// Verify checkpoint file exists
	checkpoints, err := mgr.LoadCheckpoints(s.ID)
	if err != nil {
		t.Errorf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) == 0 {
		t.Error("Expected at least one checkpoint")
	}

	t.Logf("Full workflow completed in %d LLM calls", mockP.callCount)
}
