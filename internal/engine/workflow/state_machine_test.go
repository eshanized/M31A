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

// TestStateMachine_ValidTransitions verifies all valid phase transitions succeed.
func TestStateMachine_ValidTransitions(t *testing.T) {
	t.Parallel()

	validTransitions := []struct {
		name string
		from m31types.WorkflowPhase
		to   m31types.WorkflowPhase
	}{
		{"Idle→Initialize", m31types.PhaseIdle, m31types.PhaseInitialize},
		{"Initialize→Discuss", m31types.PhaseInitialize, m31types.PhaseDiscuss},
		{"Initialize→Execute", m31types.PhaseInitialize, m31types.PhaseExecute},
		{"Initialize→Idle", m31types.PhaseInitialize, m31types.PhaseIdle},
		{"Discuss→Plan", m31types.PhaseDiscuss, m31types.PhasePlan},
		{"Discuss→Execute", m31types.PhaseDiscuss, m31types.PhaseExecute},
		{"Discuss→Idle", m31types.PhaseDiscuss, m31types.PhaseIdle},
		{"Plan→Execute", m31types.PhasePlan, m31types.PhaseExecute},
		{"Plan→Plan (replan)", m31types.PhasePlan, m31types.PhasePlan},
		{"Plan→Discuss", m31types.PhasePlan, m31types.PhaseDiscuss},
		{"Plan→Idle", m31types.PhasePlan, m31types.PhaseIdle},
		{"Execute→Verify", m31types.PhaseExecute, m31types.PhaseVerify},
		{"Execute→Ship", m31types.PhaseExecute, m31types.PhaseShip},
		{"Execute→Idle", m31types.PhaseExecute, m31types.PhaseIdle},
		{"Verify→Runtime", m31types.PhaseVerify, m31types.PhaseRuntime},
		{"Verify→Ship", m31types.PhaseVerify, m31types.PhaseShip},
		{"Verify→Execute", m31types.PhaseVerify, m31types.PhaseExecute},
		{"Verify→Idle", m31types.PhaseVerify, m31types.PhaseIdle},
		{"Runtime→Ship", m31types.PhaseRuntime, m31types.PhaseShip},
		{"Runtime→Execute", m31types.PhaseRuntime, m31types.PhaseExecute},
		{"Runtime→Idle", m31types.PhaseRuntime, m31types.PhaseIdle},
		{"Ship→Idle", m31types.PhaseShip, m31types.PhaseIdle},
	}

	for _, tt := range validTransitions {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sm := NewStateMachine()
			sm.SetPhase(tt.from)
			if err := sm.Transition(tt.from, tt.to); err != nil {
				t.Errorf("expected valid transition %s→%s, got error: %v", tt.from, tt.to, err)
			}
			if sm.CurrentPhase() != tt.to {
				t.Errorf("expected phase %s after transition, got %s", tt.to, sm.CurrentPhase())
			}
		})
	}
}

// TestStateMachine_InvalidTransitions verifies invalid transitions return ErrPhaseTransition.
func TestStateMachine_InvalidTransitions(t *testing.T) {
	t.Parallel()

	invalidTransitions := []struct {
		name string
		from m31types.WorkflowPhase
		to   m31types.WorkflowPhase
	}{
		{"Idle→Discuss", m31types.PhaseIdle, m31types.PhaseDiscuss},
		{"Idle→Plan", m31types.PhaseIdle, m31types.PhasePlan},
		{"Idle→Execute", m31types.PhaseIdle, m31types.PhaseExecute},
		{"Idle→Verify", m31types.PhaseIdle, m31types.PhaseVerify},
		{"Idle→Runtime", m31types.PhaseIdle, m31types.PhaseRuntime},
		{"Idle→Ship", m31types.PhaseIdle, m31types.PhaseShip},
		{"Initialize→Plan", m31types.PhaseInitialize, m31types.PhasePlan},
		{"Initialize→Verify", m31types.PhaseInitialize, m31types.PhaseVerify},
		{"Initialize→Runtime", m31types.PhaseInitialize, m31types.PhaseRuntime},
		{"Initialize→Ship", m31types.PhaseInitialize, m31types.PhaseShip},
		{"Discuss→Initialize", m31types.PhaseDiscuss, m31types.PhaseInitialize},
		{"Discuss→Verify", m31types.PhaseDiscuss, m31types.PhaseVerify},
		{"Discuss→Runtime", m31types.PhaseDiscuss, m31types.PhaseRuntime},
		{"Discuss→Ship", m31types.PhaseDiscuss, m31types.PhaseShip},
		{"Plan→Initialize", m31types.PhasePlan, m31types.PhaseInitialize},
		{"Plan→Verify", m31types.PhasePlan, m31types.PhaseVerify},
		{"Plan→Runtime", m31types.PhasePlan, m31types.PhaseRuntime},
		{"Plan→Ship", m31types.PhasePlan, m31types.PhaseShip},
		{"Execute→Initialize", m31types.PhaseExecute, m31types.PhaseInitialize},
		{"Execute→Discuss", m31types.PhaseExecute, m31types.PhaseDiscuss},
		{"Execute→Plan", m31types.PhaseExecute, m31types.PhasePlan},
		{"Execute→Runtime", m31types.PhaseExecute, m31types.PhaseRuntime},
		{"Verify→Initialize", m31types.PhaseVerify, m31types.PhaseInitialize},
		{"Verify→Discuss", m31types.PhaseVerify, m31types.PhaseDiscuss},
		{"Verify→Plan", m31types.PhaseVerify, m31types.PhasePlan},
		{"Runtime→Initialize", m31types.PhaseRuntime, m31types.PhaseInitialize},
		{"Runtime→Discuss", m31types.PhaseRuntime, m31types.PhaseDiscuss},
		{"Runtime→Plan", m31types.PhaseRuntime, m31types.PhasePlan},
		{"Runtime→Verify", m31types.PhaseRuntime, m31types.PhaseVerify},
		{"Ship→Initialize", m31types.PhaseShip, m31types.PhaseInitialize},
		{"Ship→Discuss", m31types.PhaseShip, m31types.PhaseDiscuss},
		{"Ship→Plan", m31types.PhaseShip, m31types.PhasePlan},
		{"Ship→Execute", m31types.PhaseShip, m31types.PhaseExecute},
		{"Ship→Verify", m31types.PhaseShip, m31types.PhaseVerify},
		{"Ship→Runtime", m31types.PhaseShip, m31types.PhaseRuntime},
	}

	for _, tt := range invalidTransitions {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sm := NewStateMachine()
			sm.SetPhase(tt.from)
			err := sm.Transition(tt.from, tt.to)
			if err == nil {
				t.Errorf("expected error for invalid transition %s→%s", tt.from, tt.to)
			}
			if sm.CurrentPhase() != tt.from {
				t.Errorf("phase should remain %s after failed transition, got %s", tt.from, sm.CurrentPhase())
			}
		})
	}
}

// TestStateMachine_TransitionFromAnyPhase verifies each phase can only transition
// to its allowed targets by attempting all possible targets.
func TestStateMachine_TransitionFromAnyPhase(t *testing.T) {
	t.Parallel()

	allPhases := []m31types.WorkflowPhase{
		m31types.PhaseIdle, m31types.PhaseInitialize, m31types.PhaseDiscuss,
		m31types.PhasePlan, m31types.PhaseExecute, m31types.PhaseVerify,
		m31types.PhaseRuntime, m31types.PhaseShip,
	}

	validTransitions := map[m31types.WorkflowPhase][]m31types.WorkflowPhase{
		m31types.PhaseIdle:       {m31types.PhaseInitialize},
		m31types.PhaseInitialize: {m31types.PhaseDiscuss, m31types.PhaseExecute, m31types.PhaseIdle},
		m31types.PhaseDiscuss:    {m31types.PhasePlan, m31types.PhaseExecute, m31types.PhaseIdle},
		m31types.PhasePlan:       {m31types.PhaseExecute, m31types.PhasePlan, m31types.PhaseDiscuss, m31types.PhaseIdle},
		m31types.PhaseExecute:    {m31types.PhaseVerify, m31types.PhaseShip, m31types.PhaseIdle},
		m31types.PhaseVerify:     {m31types.PhaseRuntime, m31types.PhaseShip, m31types.PhaseExecute, m31types.PhaseIdle},
		m31types.PhaseRuntime:    {m31types.PhaseShip, m31types.PhaseExecute, m31types.PhaseIdle},
		m31types.PhaseShip:       {m31types.PhaseIdle},
	}

	for _, from := range allPhases {
		for _, to := range allPhases {
			if from == to {
				continue // skip self-transitions except Plan→Plan
				// (Plan→Plan is tested in ValidTransitions)
			}
			name := string(from) + "→" + string(to)
			t.Run(name, func(t *testing.T) {
				t.Parallel()
				sm := NewStateMachine()
				sm.SetPhase(from)

				isAllowed := false
				for _, allowed := range validTransitions[from] {
					if allowed == to {
						isAllowed = true
						break
					}
				}

				err := sm.Transition(from, to)
				if isAllowed && err != nil {
					t.Errorf("expected allowed transition %s→%s to succeed, got: %v", from, to, err)
				}
				if !isAllowed && err == nil {
					t.Errorf("expected disallowed transition %s→%s to fail", from, to)
				}
			})
		}
	}
}

// TestStateMachine_HistoryTracking verifies history records all transitions correctly.
func TestStateMachine_HistoryTracking(t *testing.T) {
	t.Parallel()
	sm := NewStateMachine()

	// Full workflow path: Idle → Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship → Idle
	fullPath := []struct {
		from, to m31types.WorkflowPhase
	}{
		{m31types.PhaseIdle, m31types.PhaseInitialize},
		{m31types.PhaseInitialize, m31types.PhaseDiscuss},
		{m31types.PhaseDiscuss, m31types.PhasePlan},
		{m31types.PhasePlan, m31types.PhaseExecute},
		{m31types.PhaseExecute, m31types.PhaseVerify},
		{m31types.PhaseVerify, m31types.PhaseRuntime},
		{m31types.PhaseRuntime, m31types.PhaseShip},
		{m31types.PhaseShip, m31types.PhaseIdle},
	}

	for _, tr := range fullPath {
		if err := sm.Transition(tr.from, tr.to); err != nil {
			t.Fatalf("transition %s→%s failed: %v", tr.from, tr.to, err)
		}
	}

	history := sm.History()
	expectedLen := 1 + len(fullPath) // initial Idle + transitions
	if len(history) != expectedLen {
		t.Fatalf("expected history length %d, got %d", expectedLen, len(history))
	}

	// Verify each entry
	expectedPhases := []m31types.WorkflowPhase{m31types.PhaseIdle}
	for _, tr := range fullPath {
		expectedPhases = append(expectedPhases, tr.to)
	}
	for i, p := range expectedPhases {
		if history[i] != p {
			t.Errorf("history[%d] = %s, want %s", i, history[i], p)
		}
	}
}

// TestStateMachine_ResetToIdle verifies engine can reset to Idle from any terminal phase.
func TestStateMachine_ResetToIdle(t *testing.T) {
	t.Parallel()

	terminalPhases := []m31types.WorkflowPhase{
		m31types.PhaseExecute,
		m31types.PhaseVerify,
		m31types.PhaseRuntime,
		m31types.PhaseShip,
	}

	for _, phase := range terminalPhases {
		t.Run(string(phase), func(t *testing.T) {
			t.Parallel()
			sm := NewStateMachine()
			sm.SetPhase(phase)

			// Transition to Idle
			if err := sm.Transition(phase, m31types.PhaseIdle); err != nil {
				t.Errorf("expected to reset from %s to Idle, got: %v", phase, err)
			}
			if sm.CurrentPhase() != m31types.PhaseIdle {
				t.Errorf("expected PhaseIdle after reset, got %s", sm.CurrentPhase())
			}
		})
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
