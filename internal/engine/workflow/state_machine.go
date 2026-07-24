package workflow

import (
	"fmt"
	"sync"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
)

// StateMachine manages workflow phase transitions with thread-safe state tracking.
// It validates transitions against a predefined map and maintains a history
// of visited phases.
type StateMachine struct {
	currentPhase      m31types.WorkflowPhase
	mu                sync.RWMutex
	history           []m31types.WorkflowPhase
	validTransitions  map[m31types.WorkflowPhase][]m31types.WorkflowPhase
	discussPlanCycles int
}

// NewStateMachine creates a StateMachine initialized at PhaseIdle
// with the standard M31A phase transition graph.
func NewStateMachine() *StateMachine {
	return &StateMachine{
		currentPhase: m31types.PhaseIdle,
		history:      []m31types.WorkflowPhase{m31types.PhaseIdle},
		validTransitions: map[m31types.WorkflowPhase][]m31types.WorkflowPhase{
			m31types.PhaseIdle:       {m31types.PhaseInitialize},
			m31types.PhaseInitialize: {m31types.PhaseDiscuss, m31types.PhaseExecute, m31types.PhaseIdle},
			m31types.PhaseDiscuss:    {m31types.PhasePlan, m31types.PhaseExecute, m31types.PhaseIdle},
			m31types.PhasePlan:       {m31types.PhaseExecute, m31types.PhasePlan, m31types.PhaseDiscuss, m31types.PhaseIdle},
			m31types.PhaseExecute:    {m31types.PhaseVerify, m31types.PhaseShip, m31types.PhaseIdle},
			m31types.PhaseVerify:     {m31types.PhaseRuntime, m31types.PhaseShip, m31types.PhaseExecute, m31types.PhaseIdle},
			m31types.PhaseRuntime:    {m31types.PhaseShip, m31types.PhaseExecute, m31types.PhaseIdle},
			m31types.PhaseShip:       {m31types.PhaseIdle},
		},
	}
}

// CurrentPhase returns the current workflow phase in a thread-safe manner.
func (sm *StateMachine) CurrentPhase() m31types.WorkflowPhase {
	sm.mu.RLock()
	defer sm.mu.RUnlock()
	return sm.currentPhase
}

// History returns a copy of the phase transition history.
func (sm *StateMachine) History() []m31types.WorkflowPhase {
	sm.mu.RLock()
	defer sm.mu.RUnlock()
	history := make([]m31types.WorkflowPhase, len(sm.history))
	copy(history, sm.history)
	return history
}

// Transition validates and applies a phase transition from the current phase.
// Returns an error if the transition is not allowed.
func (sm *StateMachine) Transition(from, to m31types.WorkflowPhase) error {
	sm.mu.Lock()
	defer sm.mu.Unlock()

	if sm.currentPhase != from {
		return fmt.Errorf("state machine in phase %s, expected %s: %w", sm.currentPhase, from, m31errors.ErrPhaseTransition)
	}

	allowed, ok := sm.validTransitions[from]
	if !ok {
		return fmt.Errorf("invalid phase transition from %s to %s: %w", from, to, m31errors.ErrPhaseTransition)
	}

	valid := false
	for _, a := range allowed {
		if a == to {
			valid = true
			break
		}
	}
	if !valid {
		return fmt.Errorf("invalid phase transition from %s to %s: %w", from, to, m31errors.ErrPhaseTransition)
	}

	// Plan<->Discuss oscillation guard
	if from == m31types.PhasePlan && to == m31types.PhaseDiscuss {
		sm.discussPlanCycles++
		if sm.discussPlanCycles > maxDiscussPlanCycles {
			return fmt.Errorf("plan<->discuss cycle limit exceeded (%d): %w",
				maxDiscussPlanCycles, m31errors.ErrPhaseTransition)
		}
	}
	if to == m31types.PhaseExecute || to == m31types.PhaseShip || to == m31types.PhaseIdle {
		sm.discussPlanCycles = 0
	}

	sm.currentPhase = to
	sm.history = append(sm.history, to)
	return nil
}

// SetPhase directly sets the current phase without validation (for checkpoint restore).
func (sm *StateMachine) SetPhase(phase m31types.WorkflowPhase) {
	sm.mu.Lock()
	defer sm.mu.Unlock()
	sm.currentPhase = phase
	sm.discussPlanCycles = 0
	sm.history = append(sm.history, phase)
}
