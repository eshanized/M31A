package session

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Checkpoint represents a point-in-time snapshot of workflow state, used by
// the /undo command to restore previous workflow phases.
type Checkpoint struct {
	Phase        types.WorkflowPhase `json:"phase"`
	Timestamp    time.Time           `json:"timestamp"`
	MessageCount int                 `json:"message_count"`
	TaskCount    int                 `json:"task_count"`
}

// SaveCheckpoint appends a checkpoint to checkpoint.json for the given session.
// If more than 2 checkpoints exist, the oldest are trimmed (only last 2 retained).
func (m *Manager) SaveCheckpoint(sessionID string, cp Checkpoint) error {
	path := filepath.Join(m.basePathFor(sessionID), "checkpoint.json")

	// Read existing checkpoints
	checkpoints, err := m.LoadCheckpoints(sessionID)
	if err != nil {
		return err
	}

	// Append new checkpoint
	checkpoints = append(checkpoints, cp)

	// Trim to max 2 (keep newest)
	if len(checkpoints) > 2 {
		checkpoints = checkpoints[len(checkpoints)-2:]
	}

	// Marshal and write atomically
	data, err := json.Marshal(checkpoints)
	if err != nil {
		return fmt.Errorf("cannot marshal checkpoints: %w", err)
	}

	return m.atomicWrite(path, data)
}

// LoadCheckpoints reads all checkpoints from checkpoint.json for the given session.
// Returns an empty slice without error if the file does not exist.
func (m *Manager) LoadCheckpoints(sessionID string) ([]Checkpoint, error) {
	path := filepath.Join(m.basePathFor(sessionID), "checkpoint.json")
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return []Checkpoint{}, nil
		}
		return nil, fmt.Errorf("cannot read checkpoint.json: %w", err)
	}

	var checkpoints []Checkpoint
	if err := json.Unmarshal(data, &checkpoints); err != nil {
		return nil, fmt.Errorf("cannot unmarshal checkpoints: %w", err)
	}

	return checkpoints, nil
}

// LatestCheckpoint returns the most recently saved checkpoint.
// Returns ErrCheckpointNotFound if no checkpoints exist.
func (m *Manager) LatestCheckpoint(sessionID string) (*Checkpoint, error) {
	checkpoints, err := m.LoadCheckpoints(sessionID)
	if err != nil {
		return nil, err
	}

	if len(checkpoints) == 0 {
		return nil, m31errors.ErrCheckpointNotFound
	}

	// Last element is the most recent
	return &checkpoints[len(checkpoints)-1], nil
}
