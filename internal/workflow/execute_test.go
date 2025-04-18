package workflow

import (
	"context"
	"strings"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestEngine_RunExecute_NoTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// No tasks saved — execute should succeed with empty result
	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	if err != nil {
		t.Fatalf("RunPhase execute failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected execute to succeed with no tasks")
	}
}

func TestEngine_RunExecute_WithTasks(t *testing.T) {
	engine, _ := setupTestEngine(t)

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "Test")
	if err != nil {
		t.Fatalf("Initialize failed: %v", err)
	}

	// Save a task
	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Create main.go", Dependencies: []int{}, Files: []string{"main.go"}, AcceptanceCriteria: []string{"compiles"}, Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	// Mock provider returns content
	mp := engine.provider.(*mockProvider)
	mp.response = "Task completed"

	result, err := engine.RunPhase(context.Background(), m31types.PhaseExecute, "Test")
	if err != nil {
		t.Fatalf("RunPhase execute failed: %v", err)
	}
	if !result.Success {
		t.Error("Expected execute to succeed")
	}
}

func TestEngine_BuildExecuteContext(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:                 1,
		Action:             "Create",
		Description:        "Create main.go",
		Dependencies:       []int{},
		Files:              []string{"main.go"},
		AcceptanceCriteria: []string{"compiles"},
	}
	allTasks := []m31types.Task{task}

	messages := engine.buildExecuteContext(task, allTasks)
	if len(messages) == 0 {
		t.Fatal("Expected non-empty messages")
	}

	// System prompt
	if messages[0].Role != "system" {
		t.Errorf("Expected system message, got %q", messages[0].Role)
	}

	// Task list message
	found := false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Task list") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Task list not included in execute context")
	}

	// Task spec message
	found = false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "Execute task 1") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Task spec not included in execute context")
	}
}

func TestEngine_ExecuteTaskWithTools_ToolDispatch(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "Create main.go",
		Dependencies: []int{},
		Files:       []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// The mock provider returns content without tool calls
	mp := engine.provider.(*mockProvider)
	mp.response = "Done"

	result := engine.executeTaskWithTools(context.Background(), task, allTasks)
	// With no tool calls parsed, the task should still succeed
	if !result.Success {
		t.Errorf("Expected execute to succeed, got error: %s", result.Error)
	}
}

func TestEngine_ExecuteTaskWithTools_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{ID: 1, Action: "Create", Description: "Test"}
	allTasks := []m31types.Task{task}

	// Make provider return error
	mp := engine.provider.(*mockProvider)
	mp.err = context.Canceled

	result := engine.executeTaskWithTools(context.Background(), task, allTasks)
	if result.Success {
		t.Error("Expected execute to fail when LLM errors")
	}
	if result.Error == "" {
		t.Error("Expected error message")
	}
}

func TestEngine_HealTask(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "Fix bug",
		Files:       []string{"main.go"},
	}

	mp := engine.provider.(*mockProvider)
	mp.response = "Fixed the bug"

	result := engine.healTask(context.Background(), task, "compilation error")
	// Heal uses mock provider which returns content
	if !result.Success {
		t.Errorf("Expected heal to succeed, got error: %s", result.Error)
	}
}

func TestEngine_HealTask_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{ID: 1, Action: "Create", Description: "Fix bug"}

	mp := engine.provider.(*mockProvider)
	mp.err = context.Canceled

	result := engine.healTask(context.Background(), task, "compilation error")
	if result.Success {
		t.Error("Expected heal to fail when LLM errors")
	}
}

func TestEngine_ExecuteTaskWithTools_SelfHeal(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:          1,
		Action:      "Create",
		Description: "Create main.go",
		Dependencies: []int{},
		Files:       []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// Set LLM to always error — verify heal attempts are made
	mp := engine.provider.(*mockProvider)
	mp.response = "Done"
	mp.err = context.Canceled

	result := engine.executeTaskWithTools(context.Background(), task, allTasks)
	// With constant LLM error, heal attempts should exhaust and fail
	if result.Success {
		t.Error("Expected task to fail when LLM consistently errors")
	}
	if result.Error == "" {
		t.Error("Expected error message on failure")
	}
}
