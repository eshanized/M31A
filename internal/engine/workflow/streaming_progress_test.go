package workflow

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestStreamingProgress(t *testing.T) {
	t.Run("ToolStartMsg has correct fields", func(t *testing.T) {
		msg := ToolStartMsg{
			ToolName:    "Bash",
			Description: "Executing Bash",
		}
		if msg.ToolName != "Bash" {
			t.Errorf("expected ToolName 'Bash', got '%s'", msg.ToolName)
		}
		if msg.Description != "Executing Bash" {
			t.Errorf("expected Description 'Executing Bash', got '%s'", msg.Description)
		}
	})

	t.Run("ToolCompleteMsg has correct fields", func(t *testing.T) {
		msg := ToolCompleteMsg{
			ToolName:   "Bash",
			Success:    true,
			DurationMs: 1234,
		}
		if msg.ToolName != "Bash" {
			t.Errorf("expected ToolName 'Bash', got '%s'", msg.ToolName)
		}
		if !msg.Success {
			t.Error("expected Success to be true")
		}
		if msg.DurationMs != 1234 {
			t.Errorf("expected DurationMs 1234, got %d", msg.DurationMs)
		}
	})

	t.Run("ToolCompleteMsg with error", func(t *testing.T) {
		msg := ToolCompleteMsg{
			ToolName:   "FileRead",
			Success:    false,
			DurationMs: 500,
			Error:      "file not found",
		}
		if msg.Success {
			t.Error("expected Success to be false")
		}
		if msg.Error != "file not found" {
			t.Errorf("expected Error 'file not found', got '%s'", msg.Error)
		}
	})

	t.Run("ToolStartMsg and ToolCompleteMsg are tea.Msg", func(t *testing.T) {
		var _ = ToolStartMsg{}
		var _ = ToolCompleteMsg{}
	})

	t.Run("TaskStartMsg and TaskUpdateMsg exist", func(t *testing.T) {
		task := types.Task{
			ID:          1,
			Description: "test task",
		}
		startMsg := TaskStartMsg{Task: task}
		updateMsg := TaskUpdateMsg{Task: task, Status: "running"}
		if startMsg.Task.ID != 1 {
			t.Error("TaskStartMsg should contain task")
		}
		if updateMsg.Status != "running" {
			t.Error("TaskUpdateMsg should have status")
		}
	})
}
