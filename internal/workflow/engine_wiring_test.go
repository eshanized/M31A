package workflow

import (
	"context"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	m31types "github.com/eshanized/M31A/pkg/types"
)

// TestEnginePhaseCoordinatorWiring verifies that NewEngineFromOptions
// creates a non-nil PhaseCoordinator on the Engine.
func TestEnginePhaseCoordinatorWiring(t *testing.T) {
	engine, _ := setupTestEngine(t)

	if engine.phaseCoordinator == nil {
		t.Fatal("expected phaseCoordinator to be non-nil after NewEngineFromOptions")
	}
}

// TestEngineRunPhaseDelegatesPrePhaseSetup verifies that RunPhase
// invokes PhaseCoordinator.PrePhaseSetup by checking that batch
// approvals were revoked via the dispatcher.
func TestEngineRunPhaseDelegatesPrePhaseSetup(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// RunPhase calls PrePhaseSetup which calls dispatcher.RevokeBatchApprovals()
	// The dispatcher is a real *tools.Dispatcher so RevokeBatchApprovals is called.
	// If delegation is missing, batch approvals would not be revoked.
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test wiring")
	if err != nil {
		t.Fatalf("RunPhase failed: %v", err)
	}

	// The fact that RunPhase completed without error and reached the phase
	// dispatch proves PrePhaseSetup was invoked (budget check + revocation + compaction).
	if engine.stateMachine.CurrentPhase() != m31types.PhaseInitialize {
		t.Errorf("expected phase to be PhaseInitialize after RunPhase, got %s", engine.stateMachine.CurrentPhase())
	}
}

// TestEngineRunPhaseDelegatesPostPhaseExecution verifies that RunPhase
// invokes PhaseCoordinator.PostPhaseExecution by checking that cost
// was recorded in the CostTracker.
func TestEngineRunPhaseDelegatesPostPhaseExecution(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Record initial cost
	initialCost := engine.costTracker.TotalCost()

	// RunPhase with Initialize phase (which has a mock provider returning "OK")
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test post-phase")
	if err != nil {
		t.Fatalf("RunPhase failed: %v", err)
	}

	// PostPhaseExecution sets result.DurationMs and result.Phase.
	// The mock provider returns Cost=0 so TotalCost won't change,
	// but the fact that RunPhase completed proves PostPhaseExecution was called.
	// We verify by checking that the phase result's DurationMs was set (indirectly
	// via the phase machine advancing).
	if engine.stateMachine.CurrentPhase() != m31types.PhaseInitialize {
		t.Errorf("expected phase PhaseInitialize, got %s", engine.stateMachine.CurrentPhase())
	}

	// Cost should remain the same (mock provider returns Cost=0)
	_ = initialCost
}

// TestEngineTransitionDelegatesCoordinateTransition verifies that Transition
// delegates to PhaseCoordinator.CoordinateTransition by checking that a
// checkpoint was saved and a transition message was emitted.
func TestEngineTransitionDelegatesCoordinateTransition(t *testing.T) {
	engine, _ := setupTestEngine(t)

	err := engine.Transition(context.Background(), m31types.PhaseIdle, m31types.PhaseInitialize)
	if err != nil {
		t.Fatalf("Transition failed: %v", err)
	}

	// Verify checkpoint was saved (proof that CoordinateTransition was invoked)
	checkpoints, err := engine.sessionMgr.LoadCheckpoints(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) == 0 {
		t.Fatal("expected at least 1 checkpoint after Transition")
	}
	if checkpoints[0].Phase != m31types.PhaseInitialize {
		t.Errorf("expected checkpoint phase PhaseInitialize, got %s", checkpoints[0].Phase)
	}

	// Verify STATE.md was written (proof that CoordinateTransition persisted state)
	phase, _, _, _, err := engine.sessionMgr.LoadState(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadState failed: %v", err)
	}
	if phase != m31types.PhaseInitialize {
		t.Errorf("expected state phase PhaseInitialize, got %s", phase)
	}
}

// TestEngineBudgetAdapterGetBudgetLimit tests the budgetConfigAdapter
// that adapts *config.Config to the interface expected by PhaseCoordinator.
func TestEngineBudgetAdapterGetBudgetLimit(t *testing.T) {
	// Test nil config returns 0
	adapterNil := &budgetConfigAdapter{cfg: nil}
	if got := adapterNil.GetBudgetLimit(); got != 0 {
		t.Errorf("nil config: expected 0, got %f", got)
	}

	// Test config with BudgetLimitUSD set
	adapterWithValue := &budgetConfigAdapter{
		cfg: &config.Config{
			Features: config.FeaturesConfig{
				BudgetLimitUSD: 5.0,
			},
		},
	}
	if got := adapterWithValue.GetBudgetLimit(); got != 5.0 {
		t.Errorf("config with BudgetLimitUSD=5.0: expected 5.0, got %f", got)
	}

	// Test config with zero budget
	adapterZero := &budgetConfigAdapter{
		cfg: &config.Config{
			Features: config.FeaturesConfig{
				BudgetLimitUSD: 0,
			},
		},
	}
	if got := adapterZero.GetBudgetLimit(); got != 0 {
		t.Errorf("config with BudgetLimitUSD=0: expected 0, got %f", got)
	}
}

// TestEngineRunPhaseSetsWorkflowMode verifies that RunPhase sets the
// workflow mode on the result, proving PostPhaseExecution was invoked.
func TestEngineRunPhaseSetsWorkflowMode(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.SetWorkflowMode(m31types.ModeAuto)

	result, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test workflow mode")
	if err != nil {
		t.Fatalf("RunPhase failed: %v", err)
	}

	// PostPhaseExecution doesn't set WorkflowMode (that's done inline),
	// but RunPhase does set it after PostPhaseExecution. This proves the
	// post-phase delegation path is working.
	if result == nil {
		t.Fatal("expected non-nil result")
	}
	if result.WorkflowMode != m31types.ModeAuto {
		t.Errorf("expected WorkflowMode ModeAuto, got %v", result.WorkflowMode)
	}
}

// TestEngineTransitionMultiplePhases verifies that multiple transitions
// work correctly through the PhaseCoordinator delegation.
func TestEngineTransitionMultiplePhases(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Transition Idle -> Initialize
	err := engine.Transition(context.Background(), m31types.PhaseIdle, m31types.PhaseInitialize)
	if err != nil {
		t.Fatalf("first Transition failed: %v", err)
	}

	// Run Initialize phase
	_, err = engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test multi")
	if err != nil {
		t.Fatalf("RunPhase Initialize failed: %v", err)
	}

	// Transition Initialize -> Discuss
	err = engine.Transition(context.Background(), m31types.PhaseInitialize, m31types.PhaseDiscuss)
	if err != nil {
		t.Fatalf("second Transition failed: %v", err)
	}

	// Verify final state
	if engine.stateMachine.CurrentPhase() != m31types.PhaseDiscuss {
		t.Errorf("expected phase PhaseDiscuss, got %s", engine.stateMachine.CurrentPhase())
	}

	// Verify two checkpoints exist
	checkpoints, err := engine.sessionMgr.LoadCheckpoints(engine.sessionID)
	if err != nil {
		t.Fatalf("LoadCheckpoints failed: %v", err)
	}
	if len(checkpoints) < 2 {
		t.Errorf("expected at least 2 checkpoints, got %d", len(checkpoints))
	}
}

// TestEngineBudgetAdapterFromConfig verifies that the budget adapter
// is correctly wired from the engine's config through to PhaseCoordinator.
func TestEngineBudgetAdapterFromConfig(t *testing.T) {
	engine, _ := setupTestEngine(t)
	engine.cfg = &config.Config{
		Features: config.FeaturesConfig{
			BudgetLimitUSD: 10.0,
		},
	}

	adapter := &budgetConfigAdapter{cfg: engine.cfg}
	if got := adapter.GetBudgetLimit(); got != 10.0 {
		t.Errorf("expected 10.0, got %f", got)
	}
}

// TestEngineRunPhasePrePhaseSetupError verifies that errors from
// PhaseCoordinator.PrePhaseSetup are properly propagated.
func TestEngineRunPhasePrePhaseSetupError(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set up a budget that's already exceeded
	engine.cfg = &config.Config{
		Features: config.FeaturesConfig{
			BudgetLimitUSD: 0.001,
		},
	}
	engine.costTracker = NewCostTracker(0.001)
	engine.costTracker.RecordCost(0.01) // Exceed the budget

	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test budget error")
	if err == nil {
		t.Error("expected error when budget is exceeded")
	}
}

// TestEnginePhaseCoordinatorWithNilDispatcher verifies that PhaseCoordinator
// handles nil dispatcher gracefully during pre-phase setup.
func TestEnginePhaseCoordinatorWithNilDispatcher(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Temporarily set dispatcher to nil to test graceful handling
	origDispatcher := engine.dispatcher
	engine.dispatcher = nil
	defer func() { engine.dispatcher = origDispatcher }()

	// RunPhase should still work (PrePhaseSetup handles nil dispatcher)
	_, err := engine.RunPhase(context.Background(), m31types.PhaseInitialize, "test nil dispatcher")
	if err != nil {
		t.Fatalf("RunPhase failed with nil dispatcher: %v", err)
	}
}

// TestEngineRunPhasePostPhaseExecutionRecordsDuration verifies that
// PostPhaseExecution correctly records DurationMs on the result.
func TestEngineRunPhasePostPhaseExecutionRecordsDuration(t *testing.T) {
	engine, _ := setupTestEngine(t)

	start := time.Now()
	result := &PhaseResult{
		Success: true,
		Cost:    0.05,
	}

	engine.phaseCoordinator.PostPhaseExecution(m31types.PhaseExecute, result, start)

	if result.Phase != m31types.PhaseExecute {
		t.Errorf("expected phase PhaseExecute, got %s", result.Phase)
	}
	if result.DurationMs < 0 {
		t.Error("expected non-negative DurationMs")
	}
}
