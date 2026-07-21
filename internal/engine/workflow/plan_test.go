package workflow

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/tests/testutil/mocks"
)

func TestEngine_RunPlan_Success(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Initialize first
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Build a REST API")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Mock provider returns valid JSON tasks
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = `[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]`

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Build a REST API")
	if err != nil {
		t.Fatalf("RunPhase plan failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected plan to succeed")
	}
	if len(result.Tasks) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(result.Tasks))
	}
	if result.Tasks[0].ID != 1 {
		t.Errorf("Expected task ID 1, got %d", result.Tasks[0].ID)
	}
	if result.Tasks[0].Status != m31types.StatusPending {
		t.Errorf("Expected task status pending, got %s", result.Tasks[0].Status)
	}
}

func TestEngine_RunPlan_ParsesJSONFromMarkdown(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "```json\n[{\"id\":1,\"action\":\"Create\",\"description\":\"Task\",\"dependencies\":[],\"files\":[],\"acceptance_criteria\":[\"task completes\"]}]\n```"

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err != nil {
		t.Fatalf("RunPhase plan failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected plan to succeed with markdown-wrapped JSON")
	}
	if len(result.Tasks) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(result.Tasks))
	}
}

func TestEngine_RunPlan_FailsOnInvalidJSON(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "this is not json"

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil {
		t.Fatal("Expected error for invalid JSON")
	}
	if result != nil && result.Success {
		t.Error("Expected plan to fail for invalid JSON")
	}
}

func TestEngine_RunPlan_FailsOnValidationErrors(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Task with missing description fails validation
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = `[{"id":1,"action":"Create","dependencies":[],"files":[],"acceptance_criteria":[]}]`

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil {
		t.Fatal("Expected error for invalid task")
	}
	if result != nil && result.Success {
		t.Error("Expected plan to fail validation")
	}
}

func TestEngine_BuildPlanContext(t *testing.T) {
	engine, _ := setupTestEngine(t)

	messages := engine.buildPlanContext(context.Background(), "Build a tool", nil, nil, "")
	if len(messages) == 0 {
		t.Fatal("Expected non-empty messages")
	}

	// System prompt should include tool-use and plan-format
	if messages[0].Role != "system" {
		t.Errorf("Expected system message, got %q", messages[0].Role)
	}
	if !strings.Contains(messages[0].Content, "M31A") {
		t.Error("System prompt missing M31A")
	}

	// User message should include goal
	if len(messages) < 2 {
		t.Fatal("Expected at least 2 messages")
	}
	if !strings.Contains(messages[1].Content, "Build a tool") {
		t.Error("Goal not in plan context")
	}
}

func TestEngine_BuildPlanContext_IncludesProjectAnswers(t *testing.T) {
	engine, _ := setupTestEngine(t)

	project := &m31types.ProjectState{
		Goal:        "Test",
		ProjectType: "go",
		Framework:   "gin",
		Answers:     map[string]string{"Q1?": "A1"},
	}
	engine.sessionMgr.SaveProject(engine.sessionID, project)

	messages := engine.buildPlanContext(context.Background(), "Test", nil, nil, "")

	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "User answers from Discuss phase") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Discuss answers not included in plan context")
	}
}

func TestEngine_BuildPlanContext_IncludesPreviousErrors(t *testing.T) {
	engine, _ := setupTestEngine(t)

	existingTasks := []m31types.Task{{ID: 1, Action: "Create", Description: "test"}}
	valErrs := []string{"task 1: missing description"}
	raw := `[{"id":1,"action":"Create"}]`
	messages := engine.buildPlanContext(context.Background(), "Test", existingTasks, valErrs, raw)

	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Previous Attempt Failed") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Previous errors not included in plan context")
	}
	if !strings.Contains(messages[len(messages)-1].Content, "missing description") {
		t.Error("Validation error not included in context")
	}
	if !strings.Contains(messages[len(messages)-1].Content, "Previous LLM response") {
		t.Error("Raw response not included in context")
	}
}

func TestEngine_Plan_SavesTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = `[{"id":1,"action":"Create","description":"Task","dependencies":[],"files":["main.go"],"acceptance_criteria":["works"]}]`

	_, err = engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err != nil {
		t.Fatalf("RunPhase plan failed: %v", err)
	}

	loadedTasks, err := engine.sessionMgr.LoadTasks(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadTasks failed: %v", err)
	}
	if len(loadedTasks) != 1 {
		t.Errorf("Expected 1 task saved, got %d", len(loadedTasks))
	}
}

func TestEngine_RunPlan_RetryWithErrorFeedback(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	mp := engine.provider.(*mocks.MockProvider)
	// First call: invalid JSON (missing description), second call: valid JSON
	mp.MultiResponses = []string{
		`[{"id":1,"action":"Create","dependencies":[],"files":[],"acceptance_criteria":[]}]`,
		`[{"id":1,"action":"Create","description":"Fixed task","dependencies":[],"files":["main.go"],"acceptance_criteria":["works"]}]`,
	}

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err != nil {
		t.Fatalf("Plan failed: %v", err)
	}
	if !result.Success {
		t.Fatal("Plan should succeed after retry")
	}
	if mp.CallCount_ != 2 {
		t.Errorf("Expected 2 LLM calls (invalid + retry), got %d", mp.CallCount_)
	}
	if len(result.Tasks) != 1 {
		t.Fatalf("Expected 1 task, got %d", len(result.Tasks))
	}
	if result.Tasks[0].Description != "Fixed task" {
		t.Errorf("Expected 'Fixed task', got %q", result.Tasks[0].Description)
	}
}

func TestEngine_RunPlan_ManualFallback(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Always return invalid JSON — should fail after MaxPlanRetries
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "not json at all"

	result, err := engine.RunPhase(context.Background(), m31types.PhasePlan, "Test")
	if err == nil {
		t.Fatal("Expected error after 3 retries")
	}
	if result == nil {
		t.Fatal("Expected non-nil result")
	}
	if !result.RequiresManualInput {
		t.Error("Expected RequiresManualInput to be true after 3 failed retries")
	}
}

func TestEngine_BuildPlanContext_IncludesMemory(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create MEMORY.md in session dir
	sessionDir := filepath.Dir(engine.planningDir)
	os.MkdirAll(sessionDir, 0755)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	os.WriteFile(memPath, []byte("User prefers Go for backend"), 0644)

	messages := engine.buildPlanContext(context.Background(), "Test", nil, nil, "")

	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Cross-Session Memory") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Memory not included in plan context")
	}
}
