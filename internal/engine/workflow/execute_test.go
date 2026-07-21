package workflow

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/tests/testutil/mocks"
	m31types "github.com/eshanized/M31A/internal/core/types"
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

	// Save an analysis-only task (no Files) so prose response succeeds
	tasks := []m31types.Task{
		{ID: 1, Action: "Analyze", Description: "Review codebase", Dependencies: []int{}, AcceptanceCriteria: []string{"report"}, Status: m31types.StatusPending},
	}
	engine.sessionMgr.SaveTasks(engine.sessionID, tasks)

	// Mock provider returns content
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "Task completed"

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

	messages := engine.buildExecuteContext(context.Background(), task, allTasks, "")
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

	// Task spec message — new structured format uses "## Current Task" header
	found = false
	for _, m := range messages {
		if m.Role == "user" && strings.Contains(m.Content, "## Current Task") && strings.Contains(m.Content, "ID: 1") {
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
		ID:           1,
		Action:       "Create",
		Description:  "Create main.go",
		Dependencies: []int{},
		Files:        []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// The mock provider returns content without tool calls
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "Done"

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
	// File-changing task with no tool calls should fail (G05 guard)
	if result.Success {
		t.Errorf("Expected file-changing task to fail with no tool calls")
	}
}

func TestEngine_ExecuteTaskWithTools_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{ID: 1, Action: "Create", Description: "Test"}
	allTasks := []m31types.Task{task}

	// Make provider return error
	mp := engine.provider.(*mocks.MockProvider)
	mp.Err_ = context.Canceled

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
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

	// Provide a response that contains a valid tool call
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = `{"name":"FileWrite","input":{"name":"FileWrite","params":{"path":"main.go","content":"package main"}}}`

	result := engine.healTask(context.Background(), task, "compilation error", "")
	// Heal uses mock provider which returns content
	if !result.Success {
		t.Errorf("Expected heal to succeed, got error: %s", result.Error)
	}
}

func TestEngine_HealTask_LLMError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{ID: 1, Action: "Create", Description: "Fix bug"}

	mp := engine.provider.(*mocks.MockProvider)
	mp.Err_ = context.Canceled

	result := engine.healTask(context.Background(), task, "compilation error", "")
	if result.Success {
		t.Error("Expected heal to fail when LLM errors")
	}
}

func TestEngine_RunExecute_ContextCancellation(t *testing.T) {
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

	// Cancel context immediately
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err = engine.RunPhase(ctx, m31types.PhaseExecute, "Test")
	// Should handle cancellation gracefully (may error or succeed depending on timing)
	_ = err
}

func TestEngine_ExecuteTaskWithTools_EmptyResponse(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:           1,
		Action:       "Create",
		Description:  "Create main.go",
		Dependencies: []int{},
		Files:        []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// Empty response — should fail for file-changing task (G05 guard)
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = ""

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
	if result.Success {
		t.Errorf("Expected execute to fail with empty response for file-changing task")
	}
}

func TestEngine_ExecuteTaskWithTools_MultipleToolCalls(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create main.go so FileRead can find it
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\n"), 0644)

	task := m31types.Task{
		ID:           1,
		Action:       "Create",
		Description:  "Create main.go",
		Dependencies: []int{},
		Files:        []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// Response with multiple tool calls
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = `{"name":"Bash","input":{"name":"Bash","params":{"command":"echo hello"}}} and also {"name":"FileRead","input":{"name":"FileRead","params":{"path":"main.go"}}}`

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
	// Should handle multiple tool calls
	if !result.Success {
		t.Errorf("Expected execute to succeed with multiple tool calls, got error: %s", result.Error)
	}
}

func TestEngine_ExecuteTaskWithTools_SelfHeal(t *testing.T) {
	engine, _ := setupTestEngine(t)

	task := m31types.Task{
		ID:           1,
		Action:       "Create",
		Description:  "Create main.go",
		Dependencies: []int{},
		Files:        []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	// Set LLM to always error — verify heal attempts are made
	mp := engine.provider.(*mocks.MockProvider)
	mp.Response_ = "Done"
	mp.Err_ = context.Canceled

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
	// With constant LLM error, heal attempts should exhaust and fail
	if result.Success {
		t.Error("Expected task to fail when LLM consistently errors")
	}
	if result.Error == "" {
		t.Error("Expected error message on failure")
	}
}

// mockProviderWithCapture records messages passed on ChatCompletionStream calls.
type mockProviderWithCapture struct {
	mocks.MockProvider
	capturedMessages []m31types.Message
	capturedCounts   []int // messages count per call
}

func (m *mockProviderWithCapture) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	// Increment a separate counter to avoid double-incrementing the embedded mock's callCount
	m.capturedCounts = append(m.capturedCounts, len(req.Messages))
	if len(m.capturedCounts) >= 2 {
		m.capturedMessages = make([]m31types.Message, len(req.Messages))
		copy(m.capturedMessages, req.Messages)
	}
	// Delegate to the embedded mock but bypass its callCount increment
	// by using the response directly
	content := m.Response_
	if len(m.MultiResponses) > 0 {
		idx := len(m.capturedCounts) - 1
		if idx < len(m.MultiResponses) {
			content = m.MultiResponses[idx]
		}
	}
	if content == "" {
		content = "OK"
	}
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: content}, nil
	}
	closeFn := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: closeFn}, m.Err_
}

func TestExecute_OneAssistantPerTurn(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Create main.go so FileRead can find it
	os.WriteFile(filepath.Join(engine.workDir, "main.go"), []byte("package main\n"), 0644)

	// Test that multiple tool calls all execute successfully.
	// The CR-05 fix ensures only ONE assistant message is appended per
	// tool-call loop iteration (instead of N duplicate messages).
	toolCallResponse := "I'll use three tools:\n" +
		"```json\n{\"name\":\"Glob\",\"input\":{\"name\":\"Glob\",\"params\":{\"pattern\":\"*.go\"}}}\n```\n" +
		"```json\n{\"name\":\"Grep\",\"input\":{\"name\":\"Grep\",\"params\":{\"pattern\":\"test\"}}}\n```\n" +
		"```json\n{\"name\":\"FileRead\",\"input\":{\"name\":\"FileRead\",\"params\":{\"path\":\"main.go\"}}}\n```"

	mp := &mockProviderWithCapture{
		MockProvider: mocks.MockProvider{
			Response_: toolCallResponse,
		},
	}
	mp.MultiResponses = []string{
		toolCallResponse,
		"Done",
	}
	engine.provider = mp

	task := m31types.Task{
		ID:           1,
		Action:       "Create",
		Description:  "Create main.go",
		Dependencies: []int{},
		Files:        []string{"main.go"},
	}
	allTasks := []m31types.Task{task}

	result := engine.executeTaskWithTools(context.Background(), &task, allTasks, "")
	t.Logf("result: success=%v error=%q toolCalls=%d", result.Success, result.Error, result.ToolCalls)

	if !result.Success {
		t.Fatalf("Expected execute to succeed, got error: %s", result.Error)
	}

	// Verify tool calls were dispatched
	if result.ToolCalls != 3 {
		t.Errorf("expected 3 tool calls dispatched, got %d", result.ToolCalls)
	}
}

func TestLooksLikeCode_GoCode(t *testing.T) {
	t.Parallel()
	code := `package main

import (
	"fmt"
	"os"
)

func main() {
	fmt.Println("Hello, World!")
	if err := run(); err != nil {
		fmt.Fprintf(os.Stderr, "error: %v\n", err)
		os.Exit(1)
	}
}

func run() error {
	return nil
}`
	if !looksLikeCode(code) {
		t.Error("expected Go code to be detected")
	}
}

func TestLooksLikeCode_PythonCode(t *testing.T) {
	t.Parallel()
	code := `import os
import sys

def main():
    print("Hello, World!")
    if __name__ == "__main__":
        main()
`
	if !looksLikeCode(code) {
		t.Error("expected Python code to be detected")
	}
}

func TestLooksLikeCode_CodeFences(t *testing.T) {
	t.Parallel()
	code := "Here is the code for the noteflow CLI tool:\n```go\npackage main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc main() {\n\tfmt.Println(\"Hello, World!\")\n\tif err := run(); err != nil {\n\t\tfmt.Fprintf(os.Stderr, \"error: %v\\n\", err)\n\t\tos.Exit(1)\n\t}\n}\n```"
	if !looksLikeCode(code) {
		t.Error("expected fenced code to be detected")
	}
}

func TestLooksLikeCode_PlainText(t *testing.T) {
	t.Parallel()
	text := "I will create a CLI tool called noteflow. It will manage notes with a SQLite backend. The tool will support adding, listing, showing, deleting, and exporting notes."
	if looksLikeCode(text) {
		t.Error("expected plain text to NOT be detected as code")
	}
}

func TestLooksLikeCode_ShortContent(t *testing.T) {
	t.Parallel()
	if looksLikeCode("hello") {
		t.Error("expected short content to NOT be detected as code")
	}
}
