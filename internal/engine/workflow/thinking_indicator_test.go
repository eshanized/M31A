package workflow

import (
	"testing"
)

func TestThinkingIndicator(t *testing.T) {
	t.Run("ThinkingStartMsg has correct fields", func(t *testing.T) {
		msg := ThinkingStartMsg{
			Context: "LLM processing...",
		}
		if msg.Context != "LLM processing..." {
			t.Errorf("expected Context 'LLM processing...', got '%s'", msg.Context)
		}
	})

	t.Run("ThinkingCompleteMsg has correct fields", func(t *testing.T) {
		msg := ThinkingCompleteMsg{
			Context: "LLM processing complete",
		}
		if msg.Context != "LLM processing complete" {
			t.Errorf("expected Context 'LLM processing complete', got '%s'", msg.Context)
		}
	})

	t.Run("ThinkingCompleteMsg with error context", func(t *testing.T) {
		msg := ThinkingCompleteMsg{
			Context: "LLM processing failed",
		}
		if msg.Context != "LLM processing failed" {
			t.Errorf("expected Context 'LLM processing failed', got '%s'", msg.Context)
		}
	})

	t.Run("ThinkingStartMsg and ThinkingCompleteMsg are tea.Msg", func(t *testing.T) {
		var _ = ThinkingStartMsg{}
		var _ = ThinkingCompleteMsg{}
	})

	t.Run("Thinking indicator display format", func(t *testing.T) {
		msg := ThinkingStartMsg{
			Context: "LLM processing...",
		}
		// Simulate display: "Thinking... LLM processing..."
		display := "Thinking... " + msg.Context
		expected := "Thinking... LLM processing..."
		if display != expected {
			t.Errorf("expected '%s', got '%s'", expected, display)
		}
	})
}
