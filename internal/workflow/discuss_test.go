package workflow

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestEngine_RunDiscuss(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// First initialize
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build a REST API")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	result, err := engine.RunPhase(context.Background(), m31types.PhaseDiscuss, "Build a REST API")
	if err != nil {
		t.Fatalf("RunPhase discuss failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected discuss to succeed")
	}
	if len(result.Messages) == 0 {
		t.Error("Expected discuss result to contain messages")
	}
}

func TestEngine_BuildDiscussContext(t *testing.T) {
	engine, _ := setupTestEngine(t)

	messages := engine.buildDiscussContext("Build a CLI tool")
	if len(messages) == 0 {
		t.Fatal("Expected non-empty messages")
	}

	// First message should be system prompt
	if messages[0].Role != "system" {
		t.Errorf("Expected system message, got %q", messages[0].Role)
	}
	if !strings.Contains(messages[0].Content, "M31A") {
		t.Error("System prompt missing M31A identity")
	}

	// Second message should contain goal
	if len(messages) < 2 {
		t.Fatal("Expected at least 2 messages")
	}
	if !strings.Contains(messages[1].Content, "Build a CLI tool") {
		t.Error("Goal not included in discuss context")
	}
}

func TestEngine_BuildDiscussContext_IncludesProjectInfo(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Save a project with type info
	project := &m31types.ProjectState{
		Goal:        "Test goal",
		ProjectType: "go",
		Framework:   "gin",
	}
	engine.sessionMgr.SaveProject(engine.sessionID, project)

	messages := engine.buildDiscussContext("Test goal")

	// Find the user message with project info
	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Project Type: go") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Project type not included in discuss context")
	}
}

func TestEngine_BuildDiscussContext_IncludesMemory(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create MEMORY.md in session dir
	sessionDir := filepath.Dir(engine.planningDir)
	os.MkdirAll(sessionDir, 0755)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	os.WriteFile(memPath, []byte("User prefers Go for backend"), 0644)

	messages := engine.buildDiscussContext("Test")

	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Memory from previous sessions") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Memory not included in discuss context")
	}
}

func TestEngine_SaveDiscussAnswers(t *testing.T) {
	engine, _ := setupTestEngine(t)

	questions := []string{"What framework?", "What language?"}
	answers := []string{"Gin", "Go"}

	err := engine.saveDiscussAnswers(nil, questions, answers)
	if err != nil {
		t.Fatalf("saveDiscussAnswers failed: %v", err)
	}

	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadProject failed: %v", err)
	}
	if project.Answers["What framework?"] != "Gin" {
		t.Errorf("Expected answer 'Gin', got %q", project.Answers["What framework?"])
	}
}

func TestEngine_SaveDiscussAnswers_WithExistingProject(t *testing.T) {
	engine, _ := setupTestEngine(t)

	existing := &m31types.ProjectState{
		Goal:    "Test",
		Answers: make(map[string]string),
	}
	engine.sessionMgr.SaveProject(engine.sessionID, existing)

	questions := []string{"Q1?"}
	answers := []string{"A1"}

	err := engine.saveDiscussAnswers(existing, questions, answers)
	if err != nil {
		t.Fatalf("saveDiscussAnswers failed: %v", err)
	}

	project, _ := engine.sessionMgr.LoadProject(engine.sessionID)
	if project.Answers["Q1?"] != "A1" {
		t.Errorf("Expected answer 'A1', got %q", project.Answers["Q1?"])
	}
}

func TestEngine_TransitionToPlan(t *testing.T) {
	engine, _ := setupTestEngine(t)

	err := engine.transitionToPlan()
	if err != nil {
		t.Fatalf("transitionToPlan failed: %v", err)
	}

	phase, _, _, _, err := engine.sessionMgr.LoadState(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != m31types.PhasePlan {
		t.Errorf("Expected phase plan, got %s", phase)
	}
}

func TestCollectAnswers(t *testing.T) {
	questions := []string{"Q1", "Q2", "Q3"}
	answers := CollectAnswers(questions, func(q string) string {
		return "Answer to: " + q
	})

	if len(answers) != 3 {
		t.Fatalf("Expected 3 answers, got %d", len(answers))
	}
	if answers[0] != "Answer to: Q1" {
		t.Errorf("Expected 'Answer to: Q1', got %q", answers[0])
	}
}

func TestDiscussResult(t *testing.T) {
	result := &DiscussResult{
		Questions: []string{"Q1", "Q2"},
		Answers:   []string{"A1", "A2"},
	}

	if len(result.Questions) != 2 {
		t.Errorf("Expected 2 questions, got %d", len(result.Questions))
	}
	if len(result.Answers) != 2 {
		t.Errorf("Expected 2 answers, got %d", len(result.Answers))
	}
}

func TestEngine_SubmitDiscussAnswer(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set up discuss state
	engine.discussState = DiscussState{
		Questions: []string{"What framework?", "What language?"},
	}

	if err := engine.SubmitDiscussAnswer(0, "Gin"); err != nil {
		t.Fatalf("SubmitDiscussAnswer failed: %v", err)
	}
	if err := engine.SubmitDiscussAnswer(1, "Go"); err != nil {
		t.Fatalf("SubmitDiscussAnswer failed: %v", err)
	}

	if engine.discussState.Answers[0] != "Gin" {
		t.Errorf("Expected answer 'Gin', got %q", engine.discussState.Answers[0])
	}
}

func TestEngine_SubmitDiscussAnswer_InvalidIndex(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.discussState = DiscussState{Questions: []string{"Q1"}}

	if err := engine.SubmitDiscussAnswer(5, "A"); err == nil {
		t.Error("Expected error for invalid index")
	}
	if err := engine.SubmitDiscussAnswer(-1, "A"); err == nil {
		t.Error("Expected error for negative index")
	}
}

func TestEngine_SubmitDiscussAnswer_NoQuestions(t *testing.T) {
	engine, _ := setupTestEngine(t)

	if err := engine.SubmitDiscussAnswer(0, "A"); err == nil {
		t.Error("Expected error when no questions set")
	}
}

func TestEngine_SkipDiscuss(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	engine.discussState = DiscussState{
		Questions: []string{"Q1", "Q2"},
	}

	if err := engine.SkipDiscuss(); err != nil {
		t.Fatalf("SkipDiscuss failed: %v", err)
	}

	// Verify answers saved with defaults
	project, err := engine.sessionMgr.LoadProject(engine.sessionID)
	if err != nil || project == nil {
		t.Fatal("PROJECT.md should exist after skip")
	}
}
