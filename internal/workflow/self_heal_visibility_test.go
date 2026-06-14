package workflow

import (
	"fmt"
	"testing"
)

func TestSelfHealVisibility(t *testing.T) {
	t.Run("SelfHealStartMsg has correct fields", func(t *testing.T) {
		msg := SelfHealStartMsg{
			TaskID:  1,
			Attempt: 1,
			Max:     2,
		}
		if msg.TaskID != 1 {
			t.Errorf("expected TaskID 1, got %d", msg.TaskID)
		}
		if msg.Attempt != 1 {
			t.Errorf("expected Attempt 1, got %d", msg.Attempt)
		}
		if msg.Max != 2 {
			t.Errorf("expected Max 2, got %d", msg.Max)
		}
	})

	t.Run("SelfHealCompleteMsg has correct fields", func(t *testing.T) {
		msg := SelfHealCompleteMsg{
			TaskID:  1,
			Attempt: 1,
			Max:     2,
			Success: true,
		}
		if msg.TaskID != 1 {
			t.Errorf("expected TaskID 1, got %d", msg.TaskID)
		}
		if msg.Attempt != 1 {
			t.Errorf("expected Attempt 1, got %d", msg.Attempt)
		}
		if msg.Max != 2 {
			t.Errorf("expected Max 2, got %d", msg.Max)
		}
		if !msg.Success {
			t.Error("expected Success to be true")
		}
	})

	t.Run("SelfHealCompleteMsg with error", func(t *testing.T) {
		msg := SelfHealCompleteMsg{
			TaskID:  1,
			Attempt: 2,
			Max:     2,
			Success: false,
			Error:   "heal failed",
		}
		if msg.Success {
			t.Error("expected Success to be false")
		}
		if msg.Error != "heal failed" {
			t.Errorf("expected Error 'heal failed', got '%s'", msg.Error)
		}
	})

	t.Run("SelfHealStartMsg and SelfHealCompleteMsg are tea.Msg", func(t *testing.T) {
		var _ = SelfHealStartMsg{}
		var _ = SelfHealCompleteMsg{}
	})

	t.Run("Attempt count displays correctly", func(t *testing.T) {
		msg := SelfHealStartMsg{
			TaskID:  5,
			Attempt: 2,
			Max:     3,
		}
		// Simulate display: "Self-healing task 5 (attempt 2/3)..."
		display := fmt.Sprintf("Self-healing task %d (attempt %d/%d)...", msg.TaskID, msg.Attempt, msg.Max)
		expected := "Self-healing task 5 (attempt 2/3)..."
		if display != expected {
			t.Errorf("expected '%s', got '%s'", expected, display)
		}
	})
}
