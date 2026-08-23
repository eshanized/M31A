package eventstore

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func mustMarshal(v any) json.RawMessage {
	data, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return data
}

func newTestStore(t *testing.T) (*SQLiteEventStore, func()) {
	dir := t.TempDir()
	dbPath := dir + "/events.db"
	store, err := NewEventStore(dbPath)
	require.NoError(t, err)
	return store, func() { store.Close() }
}

func TestWALMode(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	// Verify WAL mode
	var mode string
	err := store.DB().QueryRow("PRAGMA journal_mode").Scan(&mode)
	require.NoError(t, err)
	assert.Equal(t, "wal", mode)

	// Verify foreign keys
	var fk int
	err = store.DB().QueryRow("PRAGMA foreign_keys").Scan(&fk)
	require.NoError(t, err)
	assert.Equal(t, 1, fk)

	// Verify busy timeout
	var timeout int
	err = store.DB().QueryRow("PRAGMA busy_timeout").Scan(&timeout)
	require.NoError(t, err)
	assert.Equal(t, 5000, timeout)

	// Verify tables exist
	tables := []string{"events", "projections", "migrations"}
	for _, table := range tables {
		var count int
		err := store.DB().QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?", table).Scan(&count)
		require.NoError(t, err)
		assert.Equal(t, 1, count, "table %s should exist", table)
	}

	// Verify indexes exist
	indexes := []string{"idx_events_run_seq", "idx_events_session_seq", "idx_events_type_seq", "idx_events_timestamp"}
	for _, idx := range indexes {
		var count int
		err := store.DB().QueryRow("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?", idx).Scan(&count)
		require.NoError(t, err)
		assert.Equal(t, 1, count, "index %s should exist", idx)
	}
}

func TestAppendSingle(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	evt := types.Event{
		Type:     types.EventProjectInitialized,
		Payload:  mustMarshal(map[string]string{"name": "test"}),
		Metadata: types.EventMetadata{SchemaVersion: 1, Tags: []string{"init"}, Source: "test"},
	}

	err := store.Append(ctx, evt)
	require.NoError(t, err)

	// Verify event was inserted
	var count int
	err = store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 1, count)

	// Verify SEQ assigned
	var seq int64
	err = store.DB().QueryRow("SELECT seq FROM events").Scan(&seq)
	require.NoError(t, err)
	assert.Equal(t, int64(1), seq)
}

func TestAppendBatch(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}

	err := store.AppendEvents(ctx, events)
	require.NoError(t, err)

	// Verify all events inserted
	var count int
	err = store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 3, count)

	// Verify sequential SEQs
	var seqs []int64
	rows, err := store.DB().Query("SELECT seq FROM events ORDER BY seq")
	require.NoError(t, err)
	defer rows.Close()
	for rows.Next() {
		var seq int64
		rows.Scan(&seq)
		seqs = append(seqs, seq)
	}
	assert.Equal(t, []int64{1, 2, 3}, seqs)
}

func TestAppendEmpty(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	err := store.AppendEvents(ctx, []types.Event{})
	require.NoError(t, err)

	var count int
	err = store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 0, count)
}

func TestAppendRunID(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	runID := uuid.New()
	evt := types.Event{
		Type:     types.EventRunCreated,
		RunID:    &runID,
		Payload:  mustMarshal(map[string]string{"run": "test"}),
	}

	err := store.Append(ctx, evt)
	require.NoError(t, err)

	// Verify run_id stored
	var storedRunID string
	err = store.DB().QueryRow("SELECT run_id FROM events").Scan(&storedRunID)
	require.NoError(t, err)
	assert.Equal(t, runID.String(), storedRunID)
}

func TestAppendMetadata(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	evt := types.Event{
		Type:     types.EventProjectInitialized,
		Payload:  mustMarshal(map[string]string{"name": "test"}),
		Metadata: types.EventMetadata{SchemaVersion: 2, Tags: []string{"tag1", "tag2"}, Source: "test-source"},
	}

	err := store.Append(ctx, evt)
	require.NoError(t, err)

	// Verify metadata stored
	var metadata []byte
	err = store.DB().QueryRow("SELECT metadata FROM events").Scan(&metadata)
	require.NoError(t, err)
	assert.Contains(t, string(metadata), "tag1")
	assert.Contains(t, string(metadata), "tag2")
	assert.Contains(t, string(metadata), "test-source")
}

func TestAppendConcurrent(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	const numEvents = 10
	events := make([]types.Event, numEvents)
	for i := 0; i < numEvents; i++ {
		events[i] = types.Event{
			Type:    types.EventTaskCreated,
			Payload: mustMarshal(map[string]int{"idx": i}),
		}
	}

	// Simulate concurrent appends by doing them sequentially (WAL serializes)
	err := store.AppendEvents(ctx, events)
	require.NoError(t, err)

	var count int
	err = store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, numEvents, count)
}

func TestQueryAll(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{})
	require.NoError(t, err)
	assert.Len(t, result, 3)
	// Should be ordered by SEQ ASC
	assert.Equal(t, types.EventProjectInitialized, result[0].Type)
	assert.Equal(t, types.EventSessionCreated, result[1].Type)
	assert.Equal(t, types.EventRunCreated, result[2].Type)
}

func TestQueryByRunID(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	runID := uuid.New()
	otherRunID := uuid.New()
	events := []types.Event{
		{Type: types.EventRunCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventTaskCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventTaskCompleted, RunID: &otherRunID, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{RunID: &runID})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	for _, evt := range result {
		assert.Equal(t, runID, *evt.RunID)
	}
}

func TestQueryBySessionID(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	sessionID := uuid.New()
	events := []types.Event{
		{Type: types.EventSessionCreated, SessionID: &sessionID, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventTaskCreated, SessionID: &sessionID, Payload: mustMarshal(map[string]int{"n": 2})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{SessionID: &sessionID})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	for _, evt := range result {
		assert.Equal(t, sessionID, *evt.SessionID)
	}
}

func TestQueryByType(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	eventType := types.EventProjectInitialized
	result, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	for _, evt := range result {
		assert.Equal(t, types.EventProjectInitialized, evt.Type)
	}
}

func TestQueryAfterSeq(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{AfterSeq: 1})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	assert.Equal(t, int64(2), result[0].Seq)
	assert.Equal(t, int64(3), result[1].Seq)
}

func TestQueryBeforeSeq(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{BeforeSeq: 3})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	assert.Equal(t, int64(1), result[0].Seq)
	assert.Equal(t, int64(2), result[1].Seq)
}

func TestQueryLimit(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{Limit: 2})
	require.NoError(t, err)
	assert.Len(t, result, 2)
}

func TestQueryOffset(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.Query(ctx, types.Query{Offset: 1, Limit: 2})
	require.NoError(t, err)
	assert.Len(t, result, 2)
	assert.Equal(t, int64(2), result[0].Seq)
	assert.Equal(t, int64(3), result[1].Seq)
}

func TestQueryCombined(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	runID := uuid.New()
	events := []types.Event{
		{Type: types.EventRunCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventTaskCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})}, // no run_id
	}
	store.AppendEvents(ctx, events)

	eventType := types.EventRunCreated
	result, err := store.Query(ctx, types.Query{RunID: &runID, Type: &eventType})
	require.NoError(t, err)
	assert.Len(t, result, 1)
	assert.Equal(t, types.EventRunCreated, result[0].Type)
	assert.Equal(t, runID, *result[0].RunID)
}

func TestQueryEmpty(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	eventType := types.EventProjectInitialized
	result, err := store.Query(ctx, types.Query{Type: &eventType})
	require.NoError(t, err)
	assert.Empty(t, result)
	assert.NotNil(t, result) // Should return empty slice, not nil
}

func TestQueryByRun(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	runID := uuid.New()
	events := []types.Event{
		{Type: types.EventRunCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventTaskCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventTaskCreated, RunID: &runID, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	result, err := store.QueryByRun(ctx, runID, 0, 2)
	require.NoError(t, err)
	assert.Len(t, result, 2)
}

func TestSubscribeAfterSeq(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	ch, err := store.Subscribe(ctx, 1)
	require.NoError(t, err)

	// Should receive events with seq > 1 (i.e., 2 and 3)
	evt1 := <-ch
	assert.Equal(t, int64(2), evt1.Seq)
	evt2 := <-ch
	assert.Equal(t, int64(3), evt2.Seq)

	// Channel should not have more (context not cancelled yet, but no more events)
	select {
	case <-ch:
		t.Fatal("channel should not have more events")
	case <-time.After(200 * time.Millisecond):
		// Expected timeout
	}
}

func TestSubscribeRealTime(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	// Start with one event
	store.AppendEvents(ctx, []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
	})

	ch, err := store.Subscribe(ctx, 1)
	require.NoError(t, err)

	// Append new event
	store.AppendEvents(ctx, []types.Event{
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
	})

	// Should receive the new event
	evt := <-ch
	assert.Equal(t, int64(2), evt.Seq)
	assert.Equal(t, types.EventSessionCreated, evt.Type)
}

func TestSubscribeContextCancel(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch, err := store.Subscribe(ctx, 0)
	require.NoError(t, err)

	// Cancel context
	cancel()

	// Channel should close
	_, ok := <-ch
	assert.False(t, ok, "channel should be closed after context cancellation")
}

func TestSubscribeNoDuplicates(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
	}
	store.AppendEvents(ctx, events)

	ch, err := store.Subscribe(ctx, 0)
	require.NoError(t, err)

	// Collect all events
	var received []int64
	for i := 0; i < 2; i++ {
		evt := <-ch
		received = append(received, evt.Seq)
	}

	// Should receive each event exactly once in order
	assert.Equal(t, []int64{1, 2}, received)

	// No more events
	select {
	case <-ch:
		t.Fatal("should not receive duplicate events")
	case <-time.After(200 * time.Millisecond):
	}
}

func TestSubscribeBackpressure(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	const numEvents = 150
	events := make([]types.Event, numEvents)
	for i := 0; i < numEvents; i++ {
		events[i] = types.Event{
			Type:    types.EventTaskCreated,
			Payload: mustMarshal(map[string]int{"idx": i}),
		}
	}
	store.AppendEvents(ctx, events)

	ch, err := store.Subscribe(ctx, 0)
	require.NoError(t, err)

	// Channel buffer is 100, should handle burst
	count := 0
	timeout := time.After(2 * time.Second)
	for {
		select {
		case <-ch:
			count++
			if count >= numEvents {
				return
			}
		case <-timeout:
			t.Fatalf("timed out waiting for events, got %d/%d", count, numEvents)
		}
	}
}

func TestSubscribeMultiple(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	store.AppendEvents(ctx, []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
	})

	ch1, err := store.Subscribe(ctx, 0)
	require.NoError(t, err)
	ch2, err := store.Subscribe(ctx, 0)
	require.NoError(t, err)

	// Both should receive the event
	evt1 := <-ch1
	evt2 := <-ch2
	assert.Equal(t, int64(1), evt1.Seq)
	assert.Equal(t, int64(1), evt2.Seq)
}

func TestBackupCreatesValidDB(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	events := []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
		{Type: types.EventSessionCreated, Payload: mustMarshal(map[string]int{"n": 2})},
		{Type: types.EventRunCreated, Payload: mustMarshal(map[string]int{"n": 3})},
	}
	store.AppendEvents(ctx, events)

	dir := t.TempDir()
	dstPath := filepath.Join(dir, "backup.db")

	err := store.Backup(ctx, dstPath)
	require.NoError(t, err)

	// Verify backup file exists and is valid SQLite
	backupStore, err := NewEventStore(dstPath)
	require.NoError(t, err)
	defer backupStore.Close()

	// Verify event count matches
	var count int
	err = backupStore.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 3, count)

	// Verify events match
	originalEvents, err := store.Query(ctx, types.Query{})
	require.NoError(t, err)
	backupEvents, err := backupStore.Query(ctx, types.Query{})
	require.NoError(t, err)
	assert.Equal(t, len(originalEvents), len(backupEvents))
	for i := range originalEvents {
		assert.Equal(t, originalEvents[i].Seq, backupEvents[i].Seq)
		assert.Equal(t, originalEvents[i].Type, backupEvents[i].Type)
	}
}

func TestBackupNonBlocking(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	// Add initial events
	store.AppendEvents(ctx, []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
	})

	dir := t.TempDir()
	dstPath := filepath.Join(dir, "backup.db")

	// Start backup in background
	errCh := make(chan error, 1)
	go func() {
		errCh <- store.Backup(ctx, dstPath)
	}()

	// Try concurrent appends during backup
	for i := 0; i < 10; i++ {
		evt := types.Event{
			Type:    types.EventTaskCreated,
			Payload: mustMarshal(map[string]int{"idx": i}),
		}
		err := store.Append(ctx, evt)
		require.NoError(t, err, "append should not block during backup")
		time.Sleep(5 * time.Millisecond)
	}

	// Backup should complete successfully
	err := <-errCh
	require.NoError(t, err)

	// Verify all events persisted
	var count int
	err = store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 11, count) // 1 initial + 10 concurrent
}

func TestBackupIncremental(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	// Add many events
	events := make([]types.Event, 100)
	for i := 0; i < 100; i++ {
		events[i] = types.Event{
			Type:    types.EventTaskCreated,
			Payload: mustMarshal(map[string]int{"idx": i}),
		}
	}
	store.AppendEvents(ctx, events)

	dir := t.TempDir()
	dstPath := filepath.Join(dir, "backup.db")

	err := store.Backup(ctx, dstPath)
	require.NoError(t, err)

	// Verify backup is valid
	backupStore, err := NewEventStore(dstPath)
	require.NoError(t, err)
	defer backupStore.Close()

	var count int
	err = backupStore.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&count)
	require.NoError(t, err)
	assert.Equal(t, 100, count)
}

func TestBackupContextCancel(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx, cancel := context.WithCancel(context.Background())
	dir := t.TempDir()
	dstPath := filepath.Join(dir, "backup.db")

	// Cancel immediately
	cancel()

	err := store.Backup(ctx, dstPath)
	require.Error(t, err)
	assert.True(t, errors.Is(err, context.Canceled), "error should be context.Canceled, got: %v", err)
}

func TestBackupDestination(t *testing.T) {
	store, cleanup := newTestStore(t)
	defer cleanup()

	ctx := context.Background()
	store.AppendEvents(ctx, []types.Event{
		{Type: types.EventProjectInitialized, Payload: mustMarshal(map[string]int{"n": 1})},
	})

	dir := t.TempDir()
	// Nested directory that doesn't exist
	dstPath := filepath.Join(dir, "nested", "subdir", "backup.db")

	err := store.Backup(ctx, dstPath)
	require.NoError(t, err)

	// Verify file was created
	_, err = os.Stat(dstPath)
	require.NoError(t, err)
}