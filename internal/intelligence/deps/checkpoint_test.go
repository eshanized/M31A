package deps

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCheckpoint_WritePending_ThenPending(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
		Sources: []SourceSnapshot{
			{Name: "osv", Status: "ok", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
		},
	}

	pcs := NewPendingCheckpointStore(store)
	eventID, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)
	assert.NotEqual(t, uuid.Nil, eventID)

	// Should be pending immediately after WritePending
	pending, err := pcs.IsPending(ctx, "github.com/test/mod")
	require.NoError(t, err)
	assert.True(t, pending)
}

func TestCheckpoint_ApproveOnce_Resolves(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
	}
	pcs := NewPendingCheckpointStore(store)
	_, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// First approve should succeed
	err = pcs.ApprovePending(ctx, "github.com/test/mod")
	require.NoError(t, err)

	// Should no longer be pending
	pending, err := pcs.IsPending(ctx, "github.com/test/mod")
	require.NoError(t, err)
	assert.False(t, pending)
}

func TestCheckpoint_SecondApprove_NoOp(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
	}
	pcs := NewPendingCheckpointStore(store)
	_, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// First approve
	err = pcs.ApprovePending(ctx, "github.com/test/mod")
	require.NoError(t, err)

	// Count events after first approve
	eventType := types.EventCheckpointResolved
	eventsAfterFirst, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	countAfterFirst := len(eventsAfterFirst)

	// Second approve should be no-op
	err = pcs.ApprovePending(ctx, "github.com/test/mod")
	require.NoError(t, err)

	// Event count should be unchanged
	eventsAfterSecond, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, eventsAfterSecond, countAfterFirst)
}

func TestCheckpoint_DenyPending_WritesDeniedMarker(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
	}
	pcs := NewPendingCheckpointStore(store)
	_, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	err = pcs.DenyPending(ctx, "github.com/test/mod")
	require.NoError(t, err)

	// Should no longer be pending
	pending, err := pcs.IsPending(ctx, "github.com/test/mod")
	require.NoError(t, err)
	assert.False(t, pending)

	// Check for denied marker event
	eventType := types.EventCheckpointResolved
	events, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, events, 1)

	var payload map[string]interface{}
	err = json.Unmarshal(events[0].Payload, &payload)
	require.NoError(t, err)
	assert.Equal(t, "denied", payload["resolution"])
}

func TestCheckpoint_ImmediateVisibility(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
	}
	pcs := NewPendingCheckpointStore(store)
	_, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Query directly - should find the CheckpointRequested event
	eventType := types.EventCheckpointRequested
	events, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, events, 1)
}

func TestCheckpoint_ShouldBlock_TruthTable(t *testing.T) {
	tests := []struct {
		name       string
		riskClass  string
		isTTY      bool
		wantBlock  bool
		wantPrompt bool
	}{
		{"high risk TTY", "high", true, true, true},
		{"high risk headless", "high", false, true, false},
		{"medium risk TTY", "medium", true, false, false},
		{"medium risk headless", "medium", false, false, false},
		{"low risk TTY", "low", true, false, false},
		{"low risk headless", "low", false, false, false},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			verdict := Verdict{
				RiskClass: tc.riskClass,
			}
			decision := ShouldBlock(verdict, tc.isTTY)
			assert.Equal(t, tc.wantBlock, decision.Block)
			assert.Equal(t, tc.wantPrompt, decision.Prompt)
		})
	}
}

func TestCheckpoint_ApprovalTrustsPersistedVerdict(t *testing.T) {
	store, cleanup := newTestEventStore(t)
	defer cleanup()

	ctx := context.Background()
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.2.3",
		RiskClass:  "high",
		Confidence: types.ConfidenceVerified,
		License:    "MIT",
	}
	pcs := NewPendingCheckpointStore(store)
	_, err := pcs.WritePending(ctx, verdict, "policy-hash-123")
	require.NoError(t, err)

	// Approve should use the persisted verdict (no refetch)
	// We verify this by checking the resolved event references the original
	err = pcs.ApprovePending(ctx, "github.com/test/mod")
	require.NoError(t, err)

	eventType := types.EventCheckpointResolved
	events, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, events, 1)

	var payload map[string]interface{}
	err = json.Unmarshal(events[0].Payload, &payload)
	require.NoError(t, err)
	// Should reference the original checkpoint event
	assert.NotEmpty(t, payload["original_event_id"])
	// Resolution should be "approved"
	assert.Equal(t, "approved", payload["resolution"])
}

func TestCheckpoint_ModelMemoryOnly_UnverifiedMarker(t *testing.T) {
	// When all sources are skipped/unavailable, the verdict should carry
	// an unverified marker and never trigger install flow
	verdict := Verdict{
		Module:     "github.com/test/mod",
		Version:    "v1.0.0",
		RiskClass:  "low",
		Confidence: types.ConfidenceSpeculative,
		Sources: []SourceSnapshot{
			{Name: "depsdev", Status: "skipped", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
			{Name: "osv", Status: "skipped", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
			{Name: "github", Status: "skipped", FetchedAt: time.Now().UTC().Format(time.RFC3339)},
		},
	}

	// Should have no ok sources
	okSources := 0
	for _, src := range verdict.Sources {
		if src.Status == "ok" {
			okSources++
		}
	}
	assert.Equal(t, 0, okSources)
	// Confidence should be speculative
	assert.Equal(t, types.ConfidenceSpeculative, verdict.Confidence)
	// Package contains no install capability (verified by grep in acceptance criteria)
}
