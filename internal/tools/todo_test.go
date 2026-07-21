package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestTodoWrite_Execute_WithTasks(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "sess-1")

	result, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "Task A", "status": "completed"},
				map[string]any{"content": "Task B", "status": "pending"},
				map[string]any{"content": "Task C", "status": "in_progress"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "TODO") {
		t.Errorf("expected 'TODO' in output, got: %s", result.Output)
	}

	// Verify file content
	sessionDir := filepath.Join(sessionsDir, "sess-1")
	data, err := os.ReadFile(filepath.Join(sessionDir, "TODO.md"))
	if err != nil {
		t.Fatalf("TODO.md should exist: %v", err)
	}
	content := string(data)
	if !strings.Contains(content, "Task A") {
		t.Error("expected 'Task A' in TODO.md")
	}
	if !strings.Contains(content, "Task B") {
		t.Error("expected 'Task B' in TODO.md")
	}
}

func TestTodoWrite_Execute_SingleTask(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "single")

	result, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "Only task", "status": "completed"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
}

func TestTodoWrite_SetAndGetSessionID(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "")
	if tw.getSessionID() != "" {
		t.Errorf("expected empty, got %q", tw.getSessionID())
	}

	tw.SetSessionID("new-id")
	if tw.getSessionID() != "new-id" {
		t.Errorf("expected 'new-id', got %q", tw.getSessionID())
	}
}

func TestTodoWrite_Execute_InvalidEntry(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "sess")

	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{"not a map"},
		},
	})
	if err == nil {
		t.Fatal("expected error for invalid entry")
	}
}

func TestTodoWrite_Execute_MissingContent(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "sess")

	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"status": "pending"}, // missing content
			},
		},
	})
	if err == nil {
		t.Fatal("expected error for missing content")
	}
	if !strings.Contains(err.Error(), "content") {
		t.Errorf("expected content error, got: %v", err)
	}
}

func TestTodoWrite_Execute_InvalidStatus(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "sess")

	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "Task", "status": "invalid_status"},
			},
		},
	})
	if err == nil {
		t.Fatal("expected error for invalid status")
	}
}

func TestTodoWrite_Execute_WithPriority(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "priority-test")

	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "High priority", "status": "pending", "priority": "high"},
				map[string]any{"content": "Low priority", "status": "pending", "priority": "low"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sessionDir := filepath.Join(sessionsDir, "priority-test")
	data, err := os.ReadFile(filepath.Join(sessionDir, "TODO.md"))
	if err != nil {
		t.Fatalf("TODO.md should exist: %v", err)
	}
	content := string(data)
	if !strings.Contains(content, "High priority") {
		t.Error("expected 'High priority' in TODO.md")
	}
}

func TestTodoWrite_SyncTodoFromTasks(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "sync-test")

	tasks := []types.Task{
		{ID: 1, Description: "Setup project", Action: "Create", Status: types.StatusDone, Dependencies: []int{}},
		{ID: 2, Description: "Implement auth", Action: "Add", Status: types.StatusRunning, Dependencies: []int{1}},
		{ID: 3, Description: "Write tests", Action: "Add", Status: types.StatusPending, Dependencies: []int{2}},
		{ID: 4, Description: "Fix bug", Action: "Modify", Status: types.StatusFailed, Dependencies: []int{}},
	}

	err := tw.SyncTodoFromTasks(tasks)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sessionDir := filepath.Join(sessionsDir, "sync-test")
	data, err := os.ReadFile(filepath.Join(sessionDir, "TODO.md"))
	if err != nil {
		t.Fatalf("TODO.md should exist: %v", err)
	}
	content := string(data)

	// Verify all tasks appear
	if !strings.Contains(content, "Setup project") {
		t.Error("expected 'Setup project' in TODO.md")
	}
	if !strings.Contains(content, "Implement auth") {
		t.Error("expected 'Implement auth' in TODO.md")
	}
	if !strings.Contains(content, "Write tests") {
		t.Error("expected 'Write tests' in TODO.md")
	}
	if !strings.Contains(content, "Fix bug") {
		t.Error("expected 'Fix bug' in TODO.md")
	}

	// Verify status mapping
	if !strings.Contains(content, "[x]") {
		t.Error("expected completed icon [x] for done task")
	}
	if !strings.Contains(content, "[~]") {
		t.Error("expected in_progress icon [~] for running task")
	}
	if !strings.Contains(content, "[-]") {
		t.Error("expected cancelled icon [-] for failed task")
	}

	// Verify task ID prefix
	if !strings.Contains(content, "[Task 1]") {
		t.Error("expected '[Task 1]' prefix")
	}
}

func TestTodoWrite_SyncTodoFromTasks_Empty(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "empty-sync")

	err := tw.SyncTodoFromTasks([]types.Task{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	sessionDir := filepath.Join(sessionsDir, "empty-sync")
	data, err := os.ReadFile(filepath.Join(sessionDir, "TODO.md"))
	if err != nil {
		t.Fatalf("TODO.md should exist: %v", err)
	}
	content := string(data)
	if !strings.Contains(content, "# TODO") {
		t.Error("expected '# TODO' header even for empty list")
	}
}

func TestTodoWrite_SyncTodoFromTasks_NilSessionID(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "")

	err := tw.SyncTodoFromTasks([]types.Task{{ID: 1, Description: "test"}})
	if err == nil {
		t.Fatal("expected error for empty session ID")
	}
}

func TestTodoWrite_SyncTodoFromTasks_CallsOnUpdate(t *testing.T) {
	t.Parallel()
	sessionsDir := t.TempDir()
	tw := NewTodoWrite(sessionsDir, "callback-test")

	var callbackItems []TodoItem
	tw.SetOnUpdate(func(items []TodoItem) {
		callbackItems = items
	})

	tasks := []types.Task{
		{ID: 1, Description: "Task A", Status: types.StatusDone},
		{ID: 2, Description: "Task B", Status: types.StatusPending},
	}

	err := tw.SyncTodoFromTasks(tasks)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if len(callbackItems) != 2 {
		t.Fatalf("expected 2 callback items, got %d", len(callbackItems))
	}
	if callbackItems[0].Content != "[Task 1] Task A" {
		t.Errorf("expected '[Task 1] Task A', got %q", callbackItems[0].Content)
	}
	if callbackItems[0].Status != "completed" {
		t.Errorf("expected status 'completed', got %q", callbackItems[0].Status)
	}
	if callbackItems[1].Status != "pending" {
		t.Errorf("expected status 'pending', got %q", callbackItems[1].Status)
	}
}
