package workflow

import (
	"testing"
)

func TestIntermediateProgress(t *testing.T) {
	t.Run("IntermediateProgressMsg has correct fields", func(t *testing.T) {
		msg := IntermediateProgressMsg{
			Phase:   "discuss",
			Message: "Generating clarifying questions...",
		}
		if msg.Phase != "discuss" {
			t.Errorf("expected Phase 'discuss', got '%s'", msg.Phase)
		}
		if msg.Message != "Generating clarifying questions..." {
			t.Errorf("expected Message 'Generating clarifying questions...', got '%s'", msg.Message)
		}
	})

	t.Run("IntermediateProgressMsg for plan phase", func(t *testing.T) {
		msg := IntermediateProgressMsg{
			Phase:   "plan",
			Message: "Creating task list... (attempt 1)",
		}
		if msg.Phase != "plan" {
			t.Errorf("expected Phase 'plan', got '%s'", msg.Phase)
		}
		if msg.Message != "Creating task list... (attempt 1)" {
			t.Errorf("expected Message 'Creating task list... (attempt 1)', got '%s'", msg.Message)
		}
	})

	t.Run("IntermediateProgressMsg for verify phase", func(t *testing.T) {
		msg := IntermediateProgressMsg{
			Phase:   "verify",
			Message: "Running acceptance checks...",
		}
		if msg.Phase != "verify" {
			t.Errorf("expected Phase 'verify', got '%s'", msg.Phase)
		}
		if msg.Message != "Running acceptance checks..." {
			t.Errorf("expected Message 'Running acceptance checks...', got '%s'", msg.Message)
		}
	})

	t.Run("IntermediateProgressMsg for ship phase", func(t *testing.T) {
		msg := IntermediateProgressMsg{
			Phase:   "ship",
			Message: "Creating final commit...",
		}
		if msg.Phase != "ship" {
			t.Errorf("expected Phase 'ship', got '%s'", msg.Phase)
		}
		if msg.Message != "Creating final commit..." {
			t.Errorf("expected Message 'Creating final commit...', got '%s'", msg.Message)
		}
	})

	t.Run("IntermediateProgressMsg is tea.Msg", func(t *testing.T) {
		// Verify it implements tea.Msg interface (empty struct)
		var msg interface{} = IntermediateProgressMsg{}
		if msg == nil {
			t.Error("message should not be nil")
		}
	})
}
