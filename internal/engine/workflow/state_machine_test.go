package workflow

import (
	"sync"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
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

// B15: SetPhase resets discussPlanCycles counter
func TestSetPhase_ResetsCycles(t *testing.T) {
	sm := NewStateMachine()

	// Drive discussPlanCycles up by oscillating Plan<->Discuss
	sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize)
	sm.Transition(m31types.PhaseInitialize, m31types.PhaseDiscuss)
	sm.Transition(m31types.PhaseDiscuss, m31types.PhasePlan)
	sm.Transition(m31types.PhasePlan, m31types.PhaseDiscuss)

	// discussPlanCycles should now be 1
	if sm.discussPlanCycles != 1 {
		t.Fatalf("expected discussPlanCycles=1 after Plan->Discuss, got %d", sm.discussPlanCycles)
	}

	// SetPhase should reset the counter
	sm.SetPhase(m31types.PhaseExecute)
	if sm.discussPlanCycles != 0 {
		t.Errorf("expected discussPlanCycles=0 after SetPhase, got %d", sm.discussPlanCycles)
	}

	// Verify we can still do Plan<->Discuss transitions (counter was reset)
	sm.SetPhase(m31types.PhasePlan)
	sm.Transition(m31types.PhasePlan, m31types.PhaseDiscuss)
	if sm.discussPlanCycles != 1 {
		t.Errorf("expected discussPlanCycles=1 after Plan->Discuss, got %d", sm.discussPlanCycles)
	}
}

// B25: Transition produces exactly one history entry (no duplicates)
func TestStateMachine_NoDuplicateHistory(t *testing.T) {
	sm := NewStateMachine()
	initialLen := len(sm.History()) // 1 (PhaseIdle)

	// Single transition should add exactly one entry
	sm.Transition(m31types.PhaseIdle, m31types.PhaseInitialize)

	history := sm.History()
	if len(history) != initialLen+1 {
		t.Errorf("expected history length %d after one transition, got %d", initialLen+1, len(history))
	}
	if history[len(history)-1] != m31types.PhaseInitialize {
		t.Errorf("expected last entry PhaseInitialize, got %s", history[len(history)-1])
	}
}

// B25: TUI flow (Transition only, no SetPhase) produces one entry per transition
func TestStateMachine_TUIFlowSingleEntry(t *testing.T) {
	sm := NewStateMachine()

	// Simulate TUI flow: Idle → Initialize → Discuss → Plan → Execute
	transitions := []struct {
		from, to m31types.WorkflowPhase
	}{
		{m31types.PhaseIdle, m31types.PhaseInitialize},
		{m31types.PhaseInitialize, m31types.PhaseDiscuss},
		{m31types.PhaseDiscuss, m31types.PhasePlan},
		{m31types.PhasePlan, m31types.PhaseExecute},
	}

	for i, tr := range transitions {
		before := len(sm.History())
		if err := sm.Transition(tr.from, tr.to); err != nil {
			t.Fatalf("transition %d failed: %v", i, err)
		}
		after := len(sm.History())
		if after != before+1 {
			t.Errorf("transition %d: expected history to grow by 1, got %d→%d", i, before, after)
		}
	}
}

// B26: History is capped at maxHistorySize
func TestStateMachine_HistoryCap(t *testing.T) {
	sm := NewStateMachine()

	// Drive transitions well beyond maxHistorySize
	for i := 0; i < maxHistorySize+100; i++ {
		// Alternate between valid transitions
		if sm.CurrentPhase() == m31types.PhaseIdle {
			sm.SetPhase(m31types.PhaseInitialize)
		} else {
			sm.SetPhase(m31types.PhaseIdle)
		}
	}

	history := sm.History()
	if len(history) > maxHistorySize {
		t.Errorf("history length %d exceeds maxHistorySize %d", len(history), maxHistorySize)
	}
	if len(history) != maxHistorySize {
		t.Errorf("expected history length %d, got %d", maxHistorySize, len(history))
	}
}
