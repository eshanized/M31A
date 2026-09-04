package deps

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
)

// PendingCheckpointStore wraps an EventStore to manage dependency
// approval checkpoints. Uses existing EventCheckpointRequested/Resolved
// event pair for D-14 async resolution.
type PendingCheckpointStore struct {
	store types.EventStore
}

// NewPendingCheckpointStore creates a new checkpoint store.
func NewPendingCheckpointStore(store types.EventStore) *PendingCheckpointStore {
	return &PendingCheckpointStore{store: store}
}

// checkpointPayload is the payload for CheckpointRequested events.
type checkpointPayload struct {
	Module      string           `json:"module"`
	Version     string           `json:"version"`
	Verdict     Verdict          `json:"verdict"`
	RiskClass   string           `json:"risk_class"`
	Sources     []SourceSnapshot `json:"sources"`
	PolicyHash  string           `json:"policy_hash"`
	RequestedAt string           `json:"requested_at"`
}

// resolutionPayload is the payload for CheckpointResolved events.
type resolutionPayload struct {
	Module          string `json:"module"`
	Version         string `json:"version"`
	OriginalEventID string `json:"original_event_id"`
	Resolution      string `json:"resolution"` // approved | denied
	ResolvedAt      string `json:"resolved_at"`
}

// WritePending appends an EventCheckpointRequested event carrying the
// verdict snapshot and returns the event ID. The pending record is
// queryable immediately after this returns (persist-before-block).
func (p *PendingCheckpointStore) WritePending(ctx context.Context, verdict Verdict, policyHash string) (uuid.UUID, error) {
	if p.store == nil {
		return uuid.Nil, nil
	}

	payload := checkpointPayload{
		Module:      verdict.Module,
		Version:     verdict.Version,
		Verdict:     verdict,
		RiskClass:   verdict.RiskClass,
		Sources:     verdict.Sources,
		PolicyHash:  policyHash,
		RequestedAt: time.Now().UTC().Format(time.RFC3339),
	}

	data, err := json.Marshal(payload)
	if err != nil {
		return uuid.Nil, fmt.Errorf("marshal checkpoint: %w", err)
	}

	evt := types.Event{
		ID:        uuid.New(),
		Type:      types.EventCheckpointRequested,
		Timestamp: time.Now().UTC(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "intelligence",
		},
	}

	err = p.store.Append(ctx, evt)
	if err != nil {
		return uuid.Nil, err
	}
	return evt.ID, nil
}

// IsPending reports whether there are unresolved checkpoints for the module.
// Resolved checkpoints (approved or denied) are excluded.
func (p *PendingCheckpointStore) IsPending(ctx context.Context, module string) (bool, error) {
	if p.store == nil {
		return false, nil
	}

	// Query all CheckpointRequested events
	reqType := types.EventCheckpointRequested
	reqEvents, err := p.store.Query(ctx, types.Query{Type: &reqType, Limit: 10000})
	if err != nil {
		return false, err
	}

	// Query all CheckpointResolved events
	resType := types.EventCheckpointResolved
	resEvents, err := p.store.Query(ctx, types.Query{Type: &resType, Limit: 10000})
	if err != nil {
		return false, err
	}

	// Build set of resolved original event IDs
	resolved := make(map[string]bool)
	for _, evt := range resEvents {
		var payload resolutionPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			continue
		}
		resolved[payload.OriginalEventID] = true
	}

	// Check for any requested checkpoint for this module that isn't resolved
	for _, evt := range reqEvents {
		var payload checkpointPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			continue
		}
		if payload.Module == module && !resolved[evt.ID.String()] {
			return true, nil
		}
	}

	return false, nil
}

// ApprovePending appends an EventCheckpointResolved event referencing
// the original EventCheckpointRequested ID. Second call is a no-op
// (idempotent).
func (p *PendingCheckpointStore) ApprovePending(ctx context.Context, module string) error {
	return p.resolvePending(ctx, module, "approved")
}

// DenyPending appends an EventCheckpointResolved event with resolution "denied".
func (p *PendingCheckpointStore) DenyPending(ctx context.Context, module string) error {
	return p.resolvePending(ctx, module, "denied")
}

// resolvePending handles both approve and deny.
func (p *PendingCheckpointStore) resolvePending(ctx context.Context, module, resolution string) error {
	if p.store == nil {
		return nil
	}

	// Find the pending checkpoint for this module
	reqType := types.EventCheckpointRequested
	reqEvents, err := p.store.Query(ctx, types.Query{Type: &reqType, Limit: 10000})
	if err != nil {
		return err
	}

	// Query resolved events to check if already resolved
	resType := types.EventCheckpointResolved
	resEvents, err := p.store.Query(ctx, types.Query{Type: &resType, Limit: 10000})
	if err != nil {
		return err
	}

	resolved := make(map[string]bool)
	for _, evt := range resEvents {
		var payload resolutionPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			continue
		}
		resolved[payload.OriginalEventID] = true
	}

	// Find the first unresolved checkpoint for this module
	var targetEventID uuid.UUID
	for _, evt := range reqEvents {
		var payload checkpointPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			continue
		}
		if payload.Module == module && !resolved[evt.ID.String()] {
			targetEventID = evt.ID
			break
		}
	}

	if targetEventID == uuid.Nil {
		// No pending checkpoint to resolve
		return nil
	}

	// Check if already resolved (idempotent)
	if resolved[targetEventID.String()] {
		return nil
	}

	// Append resolution event
	resPayload := resolutionPayload{
		Module:          module,
		Version:         "", // not needed for resolution
		OriginalEventID: targetEventID.String(),
		Resolution:      resolution,
		ResolvedAt:      time.Now().UTC().Format(time.RFC3339),
	}

	data, err := json.Marshal(resPayload)
	if err != nil {
		return fmt.Errorf("marshal resolution: %w", err)
	}

	evt := types.Event{
		ID:        uuid.New(),
		Type:      types.EventCheckpointResolved,
		Timestamp: time.Now().UTC(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "intelligence",
		},
	}

	return p.store.Append(ctx, evt)
}

// GateDecision is the result of ShouldBlock.
type GateDecision struct {
	Block  bool // true = block (exit 2 headless) or prompt (TTY)
	Prompt bool // true = interactive prompt on TTY
}

// ShouldBlock is a pure function that determines the checkpoint behavior.
// High risk + TTY -> prompt (Block=true, Prompt=true)
// High risk + headless -> block (Block=true, Prompt=false)
// Medium/low risk -> proceed (Block=false, Prompt=false)
func ShouldBlock(verdict Verdict, isTTY bool) GateDecision {
	if verdict.RiskClass == "high" {
		if isTTY {
			return GateDecision{Block: true, Prompt: true}
		}
		return GateDecision{Block: true, Prompt: false}
	}
	return GateDecision{Block: false, Prompt: false}
}
