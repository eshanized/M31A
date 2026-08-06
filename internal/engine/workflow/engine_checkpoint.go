package workflow

// Checkpoint save/load, recovery, and rollback support for crash-safe workflow persistence.

import (
	"errors"
	"fmt"
	"os"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/engine/session"
)

// CheckpointData holds data that can be saved/restored across checkpoints.
type CheckpointData struct {
	Phase       m31types.WorkflowPhase     `json:"phase"`
	Goal        string                     `json:"goal"`
	PlanVersion int                        `json:"plan_version"`
	Decisions   []decision.DecisionReceipt `json:"decisions,omitempty"`
	Timestamp   time.Time                  `json:"timestamp"`
}

// SaveCheckpointData saves current workflow state for checkpoint resume.
// It persists to both in-memory state and disk via the session manager.
func (e *Engine) SaveCheckpointData(goal string) {
	decisions := e.SnapshotDecisions()
	planVersion := e.state.PlanVersion()
	cp := &CheckpointData{
		Phase:       e.stateMachine.CurrentPhase(),
		Goal:        goal,
		PlanVersion: planVersion,
		Decisions:   decisions,
		Timestamp:   time.Now(),
	}
	e.state.SetCheckpointData(cp)
	e.logger.Info("checkpoint saved", "phase", cp.Phase, "goal", truncateForLog(cp.Goal, 100))

	// Persist to disk so checkpoint data survives process crashes.
	sessCheckpoint := session.Checkpoint{
		Phase:       cp.Phase,
		Timestamp:   cp.Timestamp,
		Goal:        cp.Goal,
		PlanVersion: cp.PlanVersion,
	}
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, sessCheckpoint); err != nil {
		e.logger.Warn("failed to persist checkpoint to disk", "error", err)
	}
}

// LoadCheckpointData restores workflow state from a checkpoint.
// If data is nil, it attempts to load from disk via the session manager.
func (e *Engine) LoadCheckpointData(data *CheckpointData) {
	if data == nil {
		// Attempt to load from disk if no in-memory checkpoint exists.
		checkpoints, err := e.sessionMgr.LoadCheckpoints(e.sessionID)
		if err != nil {
			e.logger.Warn("failed to load checkpoints from disk", "error", err)
			return
		}
		if len(checkpoints) == 0 {
			return
		}
		// Use the most recent checkpoint (first element, sorted newest-first).
		cp := checkpoints[0]
		data = &CheckpointData{
			Phase:       cp.Phase,
			Goal:        cp.Goal,
			PlanVersion: cp.PlanVersion,
			Timestamp:   cp.Timestamp,
		}
	}
	e.state.SetCheckpointData(data)
	e.state.SetPlanVersion(data.PlanVersion)
	e.stateMachine.SetPhase(data.Phase)
	e.logger.Info("checkpoint loaded", "phase", data.Phase, "timestamp", data.Timestamp)
	// Restore decisions to the log
	dl := e.state.DecisionLog()
	if data.Decisions != nil && dl != nil {
		for _, d := range data.Decisions {
			dl.Log(d)
		}
	}
}

// GetCheckpointData returns the current checkpoint data, or nil if none.
func (e *Engine) GetCheckpointData() *CheckpointData {
	return e.state.CheckpointData()
}

// Recover attempts to restore engine state from a persisted recovery file.
// Returns nil on successful recovery or if no recovery file exists (clean start).
// Returns an error only when a recovery file exists but is corrupted or invalid.
func (e *Engine) Recover() error {
	if e.recoveryPath == "" {
		return nil
	}

	state, err := LoadRecoveryState(e.recoveryPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			// No recovery file — clean start, not an error
			return nil
		}
		return fmt.Errorf("load recovery state: %w", err)
	}

	// Restore engine state from recovery
	e.state.SetPlanContent(state.PlanMarkdown)
	e.state.SetPlanVersion(state.PlanVersion)
	e.state.SetMessages(state.Messages)
	e.state.SetCurrentGoal(state.Goal)
	e.state.SetCheckpointData(state.Checkpoint)
	e.stateMachine.SetPhase(state.CurrentPhase)

	e.logger.Info("recovery restored",
		"phase", state.CurrentPhase,
		"messages", len(state.Messages),
	)

	return nil
}

// ClearRecovery removes the recovery file. Called after successful phase completion
// to prevent stale recovery data from being loaded on the next session start.
func (e *Engine) ClearRecovery() error {
	if e.recoveryPath == "" {
		return nil
	}
	if err := os.Remove(e.recoveryPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("remove recovery file: %w", err)
	}
	return nil
}

// persistRecovery saves the current engine state to the recovery file.
// This is called at key persistence points (before phase transitions, after
// plan content changes, after significant message batches).
func (e *Engine) persistRecovery() {
	if e.recoveryPath == "" {
		return
	}
	if err := SaveRecoveryState(e, e.recoveryPath); err != nil {
		e.logger.Warn("failed to persist recovery state", "error", err)
	}
}

// RollbackCurrentPhase saves current state as a recovery checkpoint and
// transitions the state machine back to the previous phase in history.
// Restores plan and messages from recovery state.
// Returns error if rollback is not possible (e.g., already at Idle).
// Rollback is idempotent — calling twice does not corrupt state.
func (e *Engine) RollbackCurrentPhase() error {
	if e.recoveryPath == "" {
		return fmt.Errorf("recovery path not configured")
	}

	state, err := LoadRecoveryState(e.recoveryPath)
	if err != nil {
		return fmt.Errorf("load recovery state for rollback: %w", err)
	}

	// Verify we have enough history to rollback
	if len(state.PhaseHistory) < 2 {
		return fmt.Errorf("cannot rollback: recovery history has only %d entries", len(state.PhaseHistory))
	}

	// Save current state before rolling back (idempotency: if we crash mid-rollback,
	// the recovery file still has the pre-rollback state)
	e.persistRecovery()

	// The previous phase is the second-to-last in history
	previousPhase := state.PhaseHistory[len(state.PhaseHistory)-2]

	// Restore engine state from recovery
	e.state.SetPlanContent(state.PlanMarkdown)
	e.state.SetPlanVersion(state.PlanVersion)
	e.state.SetMessages(state.Messages)
	e.state.SetCurrentGoal(state.Goal)
	e.state.SetCheckpointData(state.Checkpoint)

	// Transition state machine to the previous phase
	e.phaseCoordinator.PostPhaseExecution(state.CurrentPhase, nil, time.Now())
	e.stateMachine.SetPhase(previousPhase)

	e.logger.Info("rolled back",
		"from", state.CurrentPhase,
		"to", previousPhase,
	)

	return nil
}
