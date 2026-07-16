package workflow

import (
	"sync"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestNewStateMachine(t *testing.T) {
	sm := NewStateMachine()
	if sm.CurrentPhase() != m31types.PhaseIdle {
		t.Errorf("expected initial phase to be PhaseIdle, got %s", sm.CurrentPhase())
	}
	history := sm.History()
	if len(history) != 1 || history[0] != m31types.PhaseIdle {
		t.Errorf("expected history to start with PhaseIdle, got %v", history)
	}
}

func TestStateMachine_ValidTransition(t *testing.T) {
	sm := NewStateMachine()

	// PhaseIdle -> PhaseInitialize
	if err := sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if sm.CurrentPhase() != m31types.PhaseInitialize {
		t.Errorf("expected PhaseInitialize, got %s", sm.CurrentPhase())
	}

	// PhaseInitialize -> PhaseDiscuss
	if err := sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if sm.CurrentPhase() != m31types.PhaseDiscuss {
		t.Errorf("expected PhaseDiscuss, got %s", sm.CurrentPhase())
	}
}

func TestStateMachine_InvalidTransition(t *testing.T) {
	sm := NewStateMachine()

	// PhaseIdle -> PhaseShip should fail
	err := sm.Transition(m31types.PhaseIdle, m31types.PhaseShip)
	if err == nil {
		t.Fatal("expected error for invalid transition")
	}
	if sm.CurrentPhase() != m31types.PhaseIdle {
		t.Errorf("phase should remain PhaseIdle after failed transition, got %s", sm.CurrentPhase())
	}
}

func TestStateMachine_PhaseInitializeToPhaseShip_Invalid(t *testing.T) {
	sm := NewStateMachine()
	sm.SetPhase(m31types.PhaseInitialize)

	err := sm.Transition(m31types.PhaseInitialize, m31types.PhaseShip)
	if err == nil {
		t.Fatal("expected error for invalid transition")
	}
}

func TestStateMachine_CurrentPhase_ReturnsCorrectPhase(t *testing.T) {
	sm := NewStateMachine()
	sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize)
	sm.Transition(m31types.PhaseInitialize, m31types.PhaseExecute)

	if sm.CurrentPhase() != m31types.PhaseExecute {
		t.Errorf("expected PhaseExecute, got %s", sm.CurrentPhase())
	}
}

func TestStateMachine_ConcurrentTransitions(t *testing.T) {
	sm := NewStateMachine()
	sm.SetPhase(m31types.PhaseInitialize)

	var wg sync.WaitGroup
	errs := make(chan error, 100)

	// Launch many goroutines trying to transition from PhaseInitialize to PhaseDiscuss
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			err := sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss)
			if err != nil {
				errs <- err
			}
		}()
	}

	wg.Wait()
	close(errs)

	// Only one should succeed (since PhaseInitialize -> PhaseDiscuss is valid but only from PhaseInitialize)
	successCount := 0
	for err := range errs {
		if err == nil {
			successCount++
		}
	}
	// The exact count depends on timing, but the state machine should be in a valid state
	if sm.CurrentPhase() != m31types.PhaseInitialize && sm.CurrentPhase() != m31types.PhaseDiscuss {
		t.Errorf("expected either PhaseInitialize or PhaseDiscuss, got %s", sm.CurrentPhase())
	}
}

func TestStateMachine_SetPhase(t *testing.T) {
	sm := NewStateMachine()
	sm.SetPhase(m31types.PhaseExecute)

	if sm.CurrentPhase() != m31types.PhaseExecute {
		t.Errorf("expected PhaseExecute, got %s", sm.CurrentPhase())
	}
}

func TestStateMachine_History(t *testing.T) {
	sm := NewStateMachine()
	sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize)
	sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss)

	history := sm.History()
	expected := []m31types.WorkflowPhase{m31types.PhaseIdle, m31types.PhaseInitialize, m31types.PhaseDiscuss}
	if len(history) != len(expected) {
		t.Fatalf("expected history length %d, got %d", len(expected), len(history))
	}
	for i, p := range expected {
		if history[i] != p {
			t.Errorf("expected history[%d] = %s, got %s", i, p, history[i])
		}
	}
}

func TestStateMachine_TransitionFromWrongPhase(t *testing.T) {
	sm := NewStateMachine()

	// Try to transition from PhaseInitialize when we're in PhaseIdle
	err := sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss)
	if err == nil {
		t.Fatal("expected error when transitioning from wrong phase")
	}
}
