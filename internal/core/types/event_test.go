package types

import (
	"context"
	"encoding/json"
	"testing"
	"time"

	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestEventEnvelope(t *testing.T) {
	runID := uuid.New()
	sessionID := uuid.New()

	event := Event{
		ID:        uuid.New(),
		Seq:       1,
		Type:      EventProjectInitialized,
		Timestamp: time.Now(),
		RunID:     &runID,
		SessionID: &sessionID,
		Payload:   json.RawMessage(`{"name":"test"}`),
		Metadata: EventMetadata{
			SchemaVersion: 1,
			Tags:          []string{"init"},
			Source:        "migration",
		},
	}

	data, err := json.Marshal(event)
	require.NoError(t, err)

	var event2 Event
	err = json.Unmarshal(data, &event2)
	require.NoError(t, err)

	assert.Equal(t, event.ID, event2.ID)
	assert.Equal(t, event.Seq, event2.Seq)
	assert.Equal(t, event.Type, event2.Type)
	assert.Equal(t, event.RunID, event2.RunID)
	assert.Equal(t, event.SessionID, event2.SessionID)
	assert.Equal(t, string(event.Payload), string(event2.Payload))
	assert.Equal(t, event.Metadata.SchemaVersion, event2.Metadata.SchemaVersion)
	assert.Equal(t, event.Metadata.Tags, event2.Metadata.Tags)
	assert.Equal(t, event.Metadata.Source, event2.Metadata.Source)
}

func TestEventTypes(t *testing.T) {
	// Test that all event type constants are defined and unique
	eventTypes := []EventType{
		EventProjectInitialized,
		EventRepositoryIndexed,
		EventSessionCreated,
		EventRunCreated,
		EventIntentAccepted,
		EventRequirementCreated,
		EventRequirementUpdated,
		EventRequirementDeleted,
		EventDecisionLogged,
		EventDecisionUpdated,
		EventPlanCreated,
		EventPlanUpdated,
		EventTaskCreated,
		EventTaskUpdated,
		EventAgentStarted,
		EventAgentCompleted,
		EventToolCallRequested,
		EventToolCallCompleted,
		EventFileChanged,
		EventCheckpointRequested,
		EventCheckpointResolved,
		EventVerificationStarted,
		EventVerificationCompleted,
		EventTaskCompleted,
		EventRunCompleted,
		EventRunFailed,
		EventMigrationStarted,
		EventMigrationCompleted,
		EventConfigChanged,
		EventProjectUpdated,
		EventRepositoryUpdated,
		EventWorkspaceCreated,
		EventWorkspaceUpdated,
		EventArtifactCreated,
		EventArtifactUpdated,
		EventResearchCompleted,
		EventProjectionRebuilt,
		EventBackupCompleted,
	}

	// Verify all are unique
	seen := make(map[string]bool)
	for _, et := range eventTypes {
		assert.False(t, seen[string(et)], "Duplicate event type: %s", et)
		seen[string(et)] = true
		assert.NotEmpty(t, string(et), "Event type should not be empty")
	}

	// Should have 35+ types
	assert.GreaterOrEqual(t, len(eventTypes), 35)
}

func TestEventMetadata(t *testing.T) {
	// Test default values and optional fields
	meta := EventMetadata{
		SchemaVersion: 1,
	}
	assert.Equal(t, 1, meta.SchemaVersion)
	assert.Nil(t, meta.Tags)
	assert.Empty(t, meta.Source)

	// Test with all fields
	meta2 := EventMetadata{
		SchemaVersion: 2,
		Tags:          []string{"tag1", "tag2"},
		Source:        "test-source",
	}
	assert.Equal(t, 2, meta2.SchemaVersion)
	assert.Equal(t, []string{"tag1", "tag2"}, meta2.Tags)
	assert.Equal(t, "test-source", meta2.Source)

	// Test JSON serialization
	data, err := json.Marshal(meta2)
	require.NoError(t, err)
	var meta3 EventMetadata
	err = json.Unmarshal(data, &meta3)
	require.NoError(t, err)
	assert.Equal(t, meta2.SchemaVersion, meta3.SchemaVersion)
	assert.Equal(t, meta2.Tags, meta3.Tags)
	assert.Equal(t, meta2.Source, meta3.Source)
}

func TestEventStoreInterface(t *testing.T) {
	// Test that EventStore interface compiles with correct signatures
	// This is a compile-time check - we just verify the interface methods exist
	var _ EventStore = (*mockEventStore)(nil)
}

type mockEventStore struct{}

func (m *mockEventStore) Append(ctx context.Context, events ...Event) error          { return nil }
func (m *mockEventStore) Query(ctx context.Context, q Query) ([]Event, error)       { return nil, nil }
func (m *mockEventStore) Subscribe(ctx context.Context, afterSeq int64) (<-chan Event, error) { return nil, nil }
func (m *mockEventStore) Backup(ctx context.Context, dstPath string) error          { return nil }
func (m *mockEventStore) Close() error                                             { return nil }

func TestEventSerialization(t *testing.T) {
	// Test MarshalEventPayload/UnmarshalEventPayload round-trip

	// Test with Requirement
	req := Requirement{
		ID:          uuid.New(),
		Title:       "Test Requirement",
		Description: "Description",
		Phase:       1,
		Status:      RequirementStatusActive,
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}

	payload, err := MarshalEventPayload(req)
	require.NoError(t, err)

	req2, err := UnmarshalEventPayload[Requirement](payload)
	require.NoError(t, err)
	assert.Equal(t, req.ID, req2.ID)
	assert.Equal(t, req.Title, req2.Title)

	// Test with Decision
	dec := Decision{
		ID:        uuid.New(),
		Title:     "Test Decision",
		Status:    DecisionStatusAccepted,
		Rationale: "Reason",
		Timestamp: time.Now(),
	}

	payload, err = MarshalEventPayload(dec)
	require.NoError(t, err)

	dec2, err := UnmarshalEventPayload[Decision](payload)
	require.NoError(t, err)
	assert.Equal(t, dec.ID, dec2.ID)
	assert.Equal(t, dec.Title, dec2.Title)

	// Test with Run
	run := Run{
		ID:        uuid.New(),
		SessionID: uuid.New(),
		Status:    RunStatusCompleted,
		StartedAt: time.Now(),
	}

	payload, err = MarshalEventPayload(run)
	require.NoError(t, err)

	run2, err := UnmarshalEventPayload[Run](payload)
	require.NoError(t, err)
	assert.Equal(t, run.ID, run2.ID)
	assert.Equal(t, run.Status, run2.Status)

	// Test with nil payload
	var nilPayload json.RawMessage
	result, err := UnmarshalEventPayload[Requirement](nilPayload)
	require.NoError(t, err)
	assert.Equal(t, uuid.Nil, result.ID)
}

func TestQuery(t *testing.T) {
	runID := uuid.New()
	sessionID := uuid.New()
	eventType := EventRequirementCreated

	query := Query{
		RunID:     &runID,
		SessionID: &sessionID,
		Type:      &eventType,
		AfterSeq:  10,
		BeforeSeq: 100,
		Limit:     50,
		Offset:    0,
	}

	data, err := json.Marshal(query)
	require.NoError(t, err)

	var query2 Query
	err = json.Unmarshal(data, &query2)
	require.NoError(t, err)
	assert.Equal(t, query.RunID, query2.RunID)
	assert.Equal(t, query.SessionID, query2.SessionID)
	assert.Equal(t, query.Type, query2.Type)
	assert.Equal(t, query.AfterSeq, query2.AfterSeq)
	assert.Equal(t, query.BeforeSeq, query2.BeforeSeq)
	assert.Equal(t, query.Limit, query2.Limit)
	assert.Equal(t, query.Offset, query2.Offset)
}

func TestQueryDefaults(t *testing.T) {
	// Test zero-value query
	query := Query{}
	data, err := json.Marshal(query)
	require.NoError(t, err)

	var query2 Query
	err = json.Unmarshal(data, &query2)
	require.NoError(t, err)
	assert.Nil(t, query2.RunID)
	assert.Nil(t, query2.SessionID)
	assert.Nil(t, query2.Type)
	assert.Equal(t, int64(0), query2.AfterSeq)
	assert.Equal(t, int64(0), query2.BeforeSeq)
	assert.Equal(t, 0, query2.Limit)
	assert.Equal(t, 0, query2.Offset)
}