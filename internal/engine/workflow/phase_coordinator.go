package workflow

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	"github.com/eshanized/M31A/internal/metrics"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/engine/session"
	m31types "github.com/eshanized/M31A/internal/types"
)

// PhaseCoordinator orchestrates phase execution lifecycle including
// pre-phase setup, metrics recording, and post-phase cleanup.
// It coordinates between StateMachine, WorkflowCache, and Engine
// for phase transitions and execution.
type PhaseCoordinator struct {
	stateMachine *StateMachine
	cache        *WorkflowCache
	sessionMgr   *session.Manager
	sessionID    string
	costTracker  *CostTracker
	collector    *metrics.Collector
	dispatcher   Dispatcher
	logger       *slog.Logger
	emitFn       func(msg any)
}

// Dispatcher is an interface for tool dispatcher operations needed by PhaseCoordinator.
type Dispatcher interface {
	RevokeBatchApprovals()
}

// NewPhaseCoordinator creates a PhaseCoordinator with the given dependencies.
func NewPhaseCoordinator(
	sm *StateMachine,
	cache *WorkflowCache,
	sessionMgr *session.Manager,
	sessionID string,
	costTracker *CostTracker,
	collector *metrics.Collector,
	dispatcher Dispatcher,
	logger *slog.Logger,
	emitFn func(msg any),
) *PhaseCoordinator {
	return &PhaseCoordinator{
		stateMachine: sm,
		cache:        cache,
		sessionMgr:   sessionMgr,
		sessionID:    sessionID,
		costTracker:  costTracker,
		collector:    collector,
		dispatcher:   dispatcher,
		logger:       logger,
		emitFn:       emitFn,
	}
}

// PrePhaseSetup performs pre-phase setup including budget checks, batch approval
// revocation, and proactive compaction. Returns true if execution should proceed.
func (pc *PhaseCoordinator) PrePhaseSetup(ctx context.Context, phase m31types.WorkflowPhase, cfg interface{ GetBudgetLimit() float64 }, messages []m31types.Message, compactFn func([]m31types.Message) []m31types.Message) ([]m31types.Message, error) {
	// Budget guardrail: check cumulative cost before each phase
	if cfg != nil && cfg.GetBudgetLimit() > 0 {
		cost := pc.costTracker.TotalCost()
		if cost >= cfg.GetBudgetLimit() {
			return messages, fmt.Errorf("budget limit exceeded: $%.4f of $%.4f", cost, cfg.GetBudgetLimit())
		}
	}

	// Revoke batch approvals on phase transition to prevent stale approvals
	if pc.dispatcher != nil {
		pc.dispatcher.RevokeBatchApprovals()
	}

	// Proactive compaction: check if context is already heavy before entering a new phase
	if compactFn != nil && len(messages) > 0 {
		messages = compactFn(messages)
	}

	// Record phase transition
	if pc.collector != nil {
		pc.collector.RecordPhaseTransition(phase)
	}

	return messages, nil
}

// PostPhaseExecution records metrics after phase execution completes.
func (pc *PhaseCoordinator) PostPhaseExecution(phase m31types.WorkflowPhase, result *PhaseResult, start time.Time) {
	if result == nil {
		return
	}

	result.DurationMs = time.Since(start).Milliseconds()
	result.Phase = phase

	// Accumulate cost for budget tracking
	if result.Cost > 0 {
		pc.costTracker.RecordCost(result.Cost)
	}

	// Record metrics
	if pc.collector != nil {
		pc.collector.RecordPhaseDuration(phase, result.DurationMs, result.Success)
		if result.Usage != nil {
			pc.collector.RecordLLMInteraction(phase, result.Usage, result.Cost)
		}
	}
}

// CoordinateTransition handles transition side effects including checkpoint saving,
// state persistence, and event emission.
func (pc *PhaseCoordinator) CoordinateTransition(ctx context.Context, from, to m31types.WorkflowPhase) error {
	// Emit phase transition start message
	pc.emitFn(PhaseTransitionStartMsg{
		From:    string(from),
		To:      string(to),
		Context: fmt.Sprintf("Moving to %s phase...", to),
	})

	// Save checkpoint
	cp := session.Checkpoint{
		Phase:     to,
		Timestamp: time.Now(),
	}
	if err := pc.sessionMgr.SaveCheckpoint(pc.sessionID, cp); err != nil {
		pc.emitFn(PhaseTransitionCompleteMsg{
			From:    string(from),
			To:      string(to),
			Success: false,
			Error:   err.Error(),
		})
		return fmt.Errorf("save checkpoint: %w", err)
	}

	// Write STATE.md
	if err := pc.sessionMgr.SaveState(pc.sessionID, to, "transitioning", string(to)); err != nil {
		pc.emitFn(PhaseTransitionCompleteMsg{
			From:    string(from),
			To:      string(to),
			Success: false,
			Error:   err.Error(),
		})
		return fmt.Errorf("save state: %w", err)
	}

	// Emit phase transition complete message
	pc.emitFn(PhaseTransitionCompleteMsg{
		From:    string(from),
		To:      string(to),
		Success: true,
	})

	return nil
}

// ModelForPhase returns the model ID for the given phase.
// This is a placeholder that can be extended with per-phase model resolution.
func (pc *PhaseCoordinator) ModelForPhase(phase m31types.WorkflowPhase) string {
	// Default implementation - can be extended with per-phase model resolution
	return ""
}

// ProviderForPhase returns the provider for the given phase.
// This is a placeholder that can be extended with per-phase provider resolution.
func (pc *PhaseCoordinator) ProviderForPhase(phase m31types.WorkflowPhase) provider.LLMProvider {
	// Default implementation - can be extended with per-phase provider resolution
	return nil
}

// Emit sends a message to the TUI if an emitter is configured.
func (pc *PhaseCoordinator) Emit(msg any) {
	if pc.emitFn != nil {
		pc.emitFn(msg)
	}
}

// Logger returns the logger for the phase coordinator.
func (pc *PhaseCoordinator) Logger() *slog.Logger {
	return pc.logger
}
