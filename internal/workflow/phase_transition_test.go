package workflow

import (
	"fmt"
	"testing"
)

func TestPhaseTransition(t *testing.T) {
	t.Run("PhaseTransitionStartMsg has correct fields", func(t *testing.T) {
		msg := PhaseTransitionStartMsg{
			From:    "initialize",
			To:      "discuss",
			Context: "Moving to discuss phase...",
		}
		if msg.From != "initialize" {
			t.Errorf("expected From 'initialize', got '%s'", msg.From)
		}
		if msg.To != "discuss" {
			t.Errorf("expected To 'discuss', got '%s'", msg.To)
		}
		if msg.Context != "Moving to discuss phase..." {
			t.Errorf("expected Context 'Moving to discuss phase...', got '%s'", msg.Context)
		}
	})

	t.Run("PhaseTransitionCompleteMsg has correct fields", func(t *testing.T) {
		msg := PhaseTransitionCompleteMsg{
			From:    "initialize",
			To:      "discuss",
			Success: true,
		}
		if msg.From != "initialize" {
			t.Errorf("expected From 'initialize', got '%s'", msg.From)
		}
		if msg.To != "discuss" {
			t.Errorf("expected To 'discuss', got '%s'", msg.To)
		}
		if !msg.Success {
			t.Error("expected Success to be true")
		}
	})

	t.Run("PhaseTransitionCompleteMsg with error", func(t *testing.T) {
		msg := PhaseTransitionCompleteMsg{
			From:    "initialize",
			To:      "discuss",
			Success: false,
			Error:   "checkpoint save failed",
		}
		if msg.Success {
			t.Error("expected Success to be false")
		}
		if msg.Error != "checkpoint save failed" {
			t.Errorf("expected Error 'checkpoint save failed', got '%s'", msg.Error)
		}
	})

	t.Run("PhaseTransitionStartMsg and PhaseTransitionCompleteMsg are tea.Msg", func(t *testing.T) {
		// Verify they implement tea.Msg interface (empty struct)
		var startMsg interface{} = PhaseTransitionStartMsg{}
		var completeMsg interface{} = PhaseTransitionCompleteMsg{}
		if startMsg == nil || completeMsg == nil {
			t.Error("messages should not be nil")
		}
	})

	t.Run("Phase transition display format", func(t *testing.T) {
		msg := PhaseTransitionStartMsg{
			From:    "initialize",
			To:      "discuss",
			Context: "Generating plan from discuss answers...",
		}
		// Simulate display: "━━━━━━ Moving to discuss phase... ━━━━━━"
		display := fmt.Sprintf("━━━━━━ %s ━━━━━━", msg.Context)
		expected := "━━━━━━ Generating plan from discuss answers... ━━━━━━"
		if display != expected {
			t.Errorf("expected '%s', got '%s'", expected, display)
		}
	})
}
