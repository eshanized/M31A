package codeintel

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

// mockEventStore implements types.EventStore for testing.
type mockEventStore struct {
	appended []types.Event
}

func (m *mockEventStore) Append(ctx context.Context, events ...types.Event) error {
	m.appended = append(m.appended, events...)
	return nil
}

func (m *mockEventStore) Query(ctx context.Context, q types.Query) ([]types.Event, error) {
	return m.appended, nil
}

func (m *mockEventStore) Subscribe(ctx context.Context, afterSeq int64) (<-chan types.Event, error) {
	ch := make(chan types.Event)
	close(ch)
	return ch, nil
}

func (m *mockEventStore) Backup(ctx context.Context, dstPath string) error {
	return nil
}

func (m *mockEventStore) Close() error {
	return nil
}

func TestEmitFileIndexed(t *testing.T) {
	store := &mockEventStore{}
	ctx := context.Background()

	payload := FileIndexedPayload{
		Path:     "internal/foo.go",
		Language: "go",
		Symbols: []SymbolInfo{
			{Name: "Foo", Kind: "func", Exported: true},
		},
		Imports: []ImportInfo{
			{Path: "fmt", ResolvedTo: ""},
		},
		Hash:    [32]byte{1, 2, 3},
		ModTime: time.Now(),
	}

	err := EmitFileIndexed(ctx, store, payload)
	require.NoError(t, err)

	require.Len(t, store.appended, 1)
	evt := store.appended[0]
	assert.Equal(t, types.EventFileIndexed, evt.Type)
	assert.Equal(t, "codeintel", evt.Metadata.Source)
	assert.Equal(t, 1, evt.Metadata.SchemaVersion)

	// Verify payload round-trip
	var decoded FileIndexedPayload
	err = json.Unmarshal(evt.Payload, &decoded)
	require.NoError(t, err)
	assert.Equal(t, payload.Path, decoded.Path)
	assert.Equal(t, payload.Language, decoded.Language)
	assert.Len(t, decoded.Symbols, 1)
	assert.Equal(t, "Foo", decoded.Symbols[0].Name)
	assert.Len(t, decoded.Imports, 1)
	assert.Equal(t, "fmt", decoded.Imports[0].Path)
}

func TestEmitNilStore(t *testing.T) {
	ctx := context.Background()

	// Should not panic and should return nil
	err := EmitFileIndexed(ctx, nil, FileIndexedPayload{Path: "test.go"})
	assert.NoError(t, err)

	err = EmitSymbolDefined(ctx, nil, SymbolDefinedPayload{Name: "Foo"})
	assert.NoError(t, err)

	err = EmitImportResolved(ctx, nil, ImportResolvedPayload{From: "a.go", To: "b.go"})
	assert.NoError(t, err)

	err = EmitCallEdgeAdded(ctx, nil, CallEdgeAddedPayload{CallerName: "Foo", CalleeName: "Bar"})
	assert.NoError(t, err)

	err = EmitInheritanceEdgeAdded(ctx, nil, InheritanceEdgeAddedPayload{Child: "Child", Parent: "Parent"})
	assert.NoError(t, err)

	err = EmitTypeHierarchyEdgeAdded(ctx, nil, TypeHierarchyEdgeAddedPayload{Subtype: "Sub", Supertype: "Super"})
	assert.NoError(t, err)

	err = EmitFileDeleted(ctx, nil, FileDeletedPayload{Path: "test.go"})
	assert.NoError(t, err)

	err = EmitSymbolRemoved(ctx, nil, SymbolRemovedPayload{Name: "Foo"})
	assert.NoError(t, err)
}

func TestPayloadRoundTrip(t *testing.T) {
	testCases := []struct {
		name    string
		payload interface{}
	}{
		{
			name: "FileIndexedPayload",
			payload: FileIndexedPayload{
				Path:     "internal/foo.go",
				Language: "go",
				Symbols:  []SymbolInfo{{Name: "Foo", Kind: "func", Exported: true}},
				Imports:  []ImportInfo{{Path: "fmt"}},
				Hash:     [32]byte{1, 2, 3},
				ModTime:  time.Now(),
			},
		},
		{
			name: "SymbolDefinedPayload",
			payload: SymbolDefinedPayload{
				Name: "Foo", Kind: "func", File: "foo.go", Line: 10, Exported: true,
			},
		},
		{
			name: "ImportResolvedPayload",
			payload: ImportResolvedPayload{
				From: "a.go", To: "b.go", ImportPath: "pkg/b", ResolvedPath: "pkg/b.go",
			},
		},
		{
			name: "CallEdgeAddedPayload",
			payload: CallEdgeAddedPayload{
				CallerFile: "a.go", CallerLine: 10, CallerName: "Foo",
				CalleeFile: "b.go", CalleeLine: 5, CalleeName: "Bar",
			},
		},
		{
			name: "InheritanceEdgeAddedPayload",
			payload: InheritanceEdgeAddedPayload{
				Child: "Child", Parent: "Parent", ChildFile: "child.go", ParentFile: "parent.go", Kind: "extends",
			},
		},
		{
			name: "TypeHierarchyEdgeAddedPayload",
			payload: TypeHierarchyEdgeAddedPayload{
				Subtype: "Sub", Supertype: "Super", SubtypeFile: "sub.go", SupertypeFile: "super.go",
			},
		},
		{
			name: "FileDeletedPayload",
			payload: FileDeletedPayload{
				Path: "test.go", WasIndexed: true,
			},
		},
		{
			name: "SymbolRemovedPayload",
			payload: SymbolRemovedPayload{
				Name: "Foo", File: "foo.go", Kind: "func",
			},
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			data, err := json.Marshal(tc.payload)
			require.NoError(t, err)

			// Create a new instance of the same type to unmarshal into
			var decoded interface{}
			switch tc.payload.(type) {
			case FileIndexedPayload:
				decoded = &FileIndexedPayload{}
			case SymbolDefinedPayload:
				decoded = &SymbolDefinedPayload{}
			case ImportResolvedPayload:
				decoded = &ImportResolvedPayload{}
			case CallEdgeAddedPayload:
				decoded = &CallEdgeAddedPayload{}
			case InheritanceEdgeAddedPayload:
				decoded = &InheritanceEdgeAddedPayload{}
			case TypeHierarchyEdgeAddedPayload:
				decoded = &TypeHierarchyEdgeAddedPayload{}
			case FileDeletedPayload:
				decoded = &FileDeletedPayload{}
			case SymbolRemovedPayload:
				decoded = &SymbolRemovedPayload{}
			}

			err = json.Unmarshal(data, decoded)
			require.NoError(t, err)

			// Re-marshal and compare
			data2, err := json.Marshal(decoded)
			require.NoError(t, err)

			// Compare JSON representations
			var origMap, decodedMap map[string]interface{}
			require.NoError(t, json.Unmarshal(data, &origMap))
			require.NoError(t, json.Unmarshal(data2, &decodedMap))

			assert.Equal(t, origMap, decodedMap, "payload round-trip failed for %s", tc.name)
		})
	}
}

func TestEmitEventMetadata(t *testing.T) {
	store := &mockEventStore{}
	ctx := context.Background()

	// Test all event types have correct metadata
	testCases := []struct {
		name      string
		emitFunc  func()
		eventType types.EventType
	}{
		{
			name:      "FileIndexed",
			eventType: types.EventFileIndexed,
			emitFunc: func() {
				_ = EmitFileIndexed(ctx, store, FileIndexedPayload{Path: "a.go"})
			},
		},
		{
			name:      "SymbolDefined",
			eventType: types.EventSymbolDefined,
			emitFunc: func() {
				_ = EmitSymbolDefined(ctx, store, SymbolDefinedPayload{Name: "Foo"})
			},
		},
		{
			name:      "ImportResolved",
			eventType: types.EventImportResolved,
			emitFunc: func() {
				_ = EmitImportResolved(ctx, store, ImportResolvedPayload{From: "a.go", To: "b.go"})
			},
		},
		{
			name:      "CallEdgeAdded",
			eventType: types.EventCallEdgeAdded,
			emitFunc: func() {
				_ = EmitCallEdgeAdded(ctx, store, CallEdgeAddedPayload{CallerName: "Foo", CalleeName: "Bar"})
			},
		},
		{
			name:      "InheritanceEdgeAdded",
			eventType: types.EventInheritanceEdgeAdded,
			emitFunc: func() {
				_ = EmitInheritanceEdgeAdded(ctx, store, InheritanceEdgeAddedPayload{Child: "C", Parent: "P"})
			},
		},
		{
			name:      "TypeHierarchyEdgeAdded",
			eventType: types.EventTypeHierarchyEdgeAdded,
			emitFunc: func() {
				_ = EmitTypeHierarchyEdgeAdded(ctx, store, TypeHierarchyEdgeAddedPayload{Subtype: "S", Supertype: "T"})
			},
		},
		{
			name:      "FileDeleted",
			eventType: types.EventFileDeleted,
			emitFunc: func() {
				_ = EmitFileDeleted(ctx, store, FileDeletedPayload{Path: "a.go"})
			},
		},
		{
			name:      "SymbolRemoved",
			eventType: types.EventSymbolRemoved,
			emitFunc: func() {
				_ = EmitSymbolRemoved(ctx, store, SymbolRemovedPayload{Name: "Foo", File: "a.go"})
			},
		},
	}

	for _, tc := range testCases {
		store.appended = nil
		tc.emitFunc()
		require.Len(t, store.appended, 1, "test case: %s", tc.name)

		evt := store.appended[0]
		assert.Equal(t, tc.eventType, evt.Type, "event type mismatch for %s", tc.name)
		assert.Equal(t, "codeintel", evt.Metadata.Source, "source must be codeintel for %s", tc.name)
		assert.Equal(t, 1, evt.Metadata.SchemaVersion, "schema version must be 1 for %s", tc.name)
		assert.NotEqual(t, uuid.Nil, evt.ID, "event ID must be set for %s", tc.name)
		assert.False(t, evt.Timestamp.IsZero(), "timestamp must be set for %s", tc.name)
	}
}