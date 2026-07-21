package session

import (
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// SessionInfo holds display metadata for session listings.
type SessionInfo struct {
	ID            string              `json:"id"`
	ParentID      string              `json:"parent_id,omitempty"`
	ChildrenIDs   []string            `json:"children_ids,omitempty"`
	Label         string              `json:"label,omitempty"`
	Model         string              `json:"model"`
	Provider      string              `json:"provider"`
	StartedAt     time.Time           `json:"started_at"`
	LastModified  time.Time           `json:"last_modified"`
	MessageCount  int                 `json:"message_count"`
	Corrupted     bool                `json:"corrupted"`
	WorkflowPhase types.WorkflowPhase `json:"workflow_phase,omitempty"`
}
