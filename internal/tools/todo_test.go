package tools

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
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
