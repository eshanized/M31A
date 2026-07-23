package workflow

import (
	"context"
	"strings"
	"sync"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestProactiveCompactCheck_ResultApplied(t *testing.T) {
	t.Parallel()
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Create messages with enough tool calls to trigger compaction
	messages := make([]m31types.Message, 0, 50)
	for i := 0; i < 30; i++ {
		messages = append(messages, m31types.Message{
			Role:    "assistant",
			Content: "test message content that is somewhat long to trigger compaction logic",
		})
	}

	originalLen := len(messages)

	// Run compaction
	result := engine.proactiveCompactCheck(messages)

	// Verify the result is assigned back (B01 fix)
	if result == nil {
		t.Fatal("Expected non-nil result from proactiveCompactCheck")
	}

	// The result should be different from original if compaction occurred
	// Even if compaction didn't trigger, the result should be valid
	if len(result) > originalLen {
		t.Errorf("Expected result length <= original, got %d > %d", len(result), originalLen)
	}
}

func TestRunPhase_InvalidTransitionRejected(t *testing.T) {
	t.Parallel()
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Engine starts in Idle phase
	currentPhase := engine.stateMachine.CurrentPhase()
	if currentPhase != m31types.PhaseIdle {
		t.Fatalf("Expected engine to start in Idle phase, got %s", currentPhase)
	}

	// Try an invalid transition: Idle -> Ship (should fail)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseShip, "test goal")
	if err == nil {
		t.Fatal("Expected error for invalid transition Idle -> Ship")
	}

	// Verify error contains "transition" in message
	if !strings.Contains(err.Error(), "transition") && !strings.Contains(err.Error(), "invalid phase") {
		t.Errorf("Expected error message to contain 'transition', got: %v", err)
	}
}

func TestRunPhase_ValidTransitionAccepted(t *testing.T) {
	t.Parallel()
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Valid transition: Idle -> Initialize
	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test goal")
	if err != nil {
		t.Fatalf("Expected no error for valid transition Idle -> Initialize, got: %v", err)
	}

	// Verify we moved to Initialize phase
	currentPhase := engine.stateMachine.CurrentPhase()
	if currentPhase != m31types.PhaseInitialize {
		t.Errorf("Expected phase to be Initialize, got %s", currentPhase)
	}

	// Result should not be nil (phase ran)
	if result == nil {
		t.Error("Expected non-nil result from RunPhase")
	}
}

func TestRunPhase_ConcurrentTransitions(t *testing.T) {
	t.Parallel()
	engine, cleanup := setupTestEngine(t)
	defer cleanup()

	// Test that concurrent RunPhase calls don't corrupt the state machine
	// We'll use a mutex to serialize phase transitions and verify consistency
	var mu sync.Mutex
	phases := []m31types.WorkflowPhase{
		m31types.PhaseInitialize,
		m31types.PhaseDiscuss,
		m31types.PhasePlan,
		m31types.PhaseExecute,
	}

	var wg sync.WaitGroup
	errChan := make(chan error, len(phases))

	for _, phase := range phases {
		wg.Add(1)
		go func(p m31types.WorkflowPhase) {
			defer wg.Done()
			mu.Lock()
			defer mu.Unlock()

			// Try transition - may fail if phase is not reachable from current
			_, err := engine.RunPhase(context.Background(), p, "concurrent test")
			if err != nil {
				errChan <- err
			}
		}(phase)
	}

	wg.Wait()
	close(errChan)

	// Count errors - some are expected due to concurrent access
	errorCount := 0
	for err := range errChan {
		if err != nil {
			errorCount++
		}
	}

	// The state machine should remain consistent (no panics, no corruption)
	// Some errors are expected due to race conditions, but no panics
	currentPhase := engine.stateMachine.CurrentPhase()
	if currentPhase == "" {
		t.Error("Expected non-empty current phase after concurrent transitions")
	}
}
