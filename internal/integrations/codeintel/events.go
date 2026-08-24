package codeintel

import (
	"context"
	"encoding/json"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/google/uuid"
)

// FileIndexedPayload represents a file that has been indexed.
type FileIndexedPayload struct {
	Path     string       `json:"path"`
	Language string       `json:"language"`
	Symbols  []SymbolInfo `json:"symbols"`
	Imports  []ImportInfo `json:"imports"`
	Hash     [32]byte     `json:"hash"`
	ModTime  time.Time    `json:"mod_time"`
}

// SymbolDefinedPayload represents a symbol that has been defined.
type SymbolDefinedPayload struct {
	Name     string `json:"name"`
	Kind     string `json:"kind"`
	File     string `json:"file"`
	Line     int    `json:"line"`
	Exported bool   `json:"exported"`
}

// ImportResolvedPayload represents an import that has been resolved.
type ImportResolvedPayload struct {
	From         string `json:"from"`
	To           string `json:"to"`
	ImportPath   string `json:"import_path"`
	ResolvedPath string `json:"resolved_path"`
}

// CallEdgeAddedPayload represents a call edge added to the graph.
type CallEdgeAddedPayload struct {
	CallerFile  string `json:"caller_file"`
	CallerLine  int    `json:"caller_line"`
	CallerName  string `json:"caller_name"`
	CalleeFile  string `json:"callee_file"`
	CalleeLine  int    `json:"callee_line"`
	CalleeName  string `json:"callee_name"`
}

// InheritanceEdgeAddedPayload represents an inheritance edge added to the graph.
type InheritanceEdgeAddedPayload struct {
	Child       string `json:"child"`
	Parent      string `json:"parent"`
	ChildFile   string `json:"child_file"`
	ParentFile  string `json:"parent_file"`
	Kind        string `json:"kind"` // "extends" or "implements"
}

// TypeHierarchyEdgeAddedPayload represents a type hierarchy edge added to the graph.
type TypeHierarchyEdgeAddedPayload struct {
	Subtype       string `json:"subtype"`
	Supertype     string `json:"supertype"`
	SubtypeFile   string `json:"subtype_file"`
	SupertypeFile string `json:"supertype_file"`
}

// FileDeletedPayload represents a file that has been deleted.
type FileDeletedPayload struct {
	Path      string `json:"path"`
	WasIndexed bool   `json:"was_indexed"`
}

// SymbolRemovedPayload represents a symbol that has been removed.
type SymbolRemovedPayload struct {
	Name string `json:"name"`
	File string `json:"file"`
	Kind string `json:"kind"`
}

// CallSiteInfo represents a call site extracted during parsing.
type CallSiteInfo struct {
	CallerName string `json:"caller_name"`
	CalleeName string `json:"callee_name"`
	Line       int    `json:"line"`
}

// emitEvent creates an event with the given type and payload, tags it with source "codeintel",
// and appends it to the EventStore. If store is nil, returns nil (nil-safe).
func emitEvent(ctx context.Context, store types.EventStore, eventType types.EventType, payload interface{}) error {
	if store == nil {
		return nil
	}

	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}

	evt := types.Event{
		ID:        uuid.New(),
		Type:      eventType,
		Timestamp: time.Now(),
		Payload:   data,
		Metadata: types.EventMetadata{
			SchemaVersion: 1,
			Source:        "codeintel",
		},
	}

	return store.Append(ctx, evt)
}

// EmitFileIndexed emits a FileIndexed event.
func EmitFileIndexed(ctx context.Context, store types.EventStore, payload FileIndexedPayload) error {
	return emitEvent(ctx, store, types.EventFileIndexed, payload)
}

// EmitSymbolDefined emits a SymbolDefined event.
func EmitSymbolDefined(ctx context.Context, store types.EventStore, payload SymbolDefinedPayload) error {
	return emitEvent(ctx, store, types.EventSymbolDefined, payload)
}

// EmitImportResolved emits an ImportResolved event.
func EmitImportResolved(ctx context.Context, store types.EventStore, payload ImportResolvedPayload) error {
	return emitEvent(ctx, store, types.EventImportResolved, payload)
}

// EmitCallEdgeAdded emits a CallEdgeAdded event.
func EmitCallEdgeAdded(ctx context.Context, store types.EventStore, payload CallEdgeAddedPayload) error {
	return emitEvent(ctx, store, types.EventCallEdgeAdded, payload)
}

// EmitInheritanceEdgeAdded emits an InheritanceEdgeAdded event.
func EmitInheritanceEdgeAdded(ctx context.Context, store types.EventStore, payload InheritanceEdgeAddedPayload) error {
	return emitEvent(ctx, store, types.EventInheritanceEdgeAdded, payload)
}

// EmitTypeHierarchyEdgeAdded emits a TypeHierarchyEdgeAdded event.
func EmitTypeHierarchyEdgeAdded(ctx context.Context, store types.EventStore, payload TypeHierarchyEdgeAddedPayload) error {
	return emitEvent(ctx, store, types.EventTypeHierarchyEdgeAdded, payload)
}

// EmitFileDeleted emits a FileDeleted event.
func EmitFileDeleted(ctx context.Context, store types.EventStore, payload FileDeletedPayload) error {
	return emitEvent(ctx, store, types.EventFileDeleted, payload)
}

// EmitSymbolRemoved emits a SymbolRemoved event.
func EmitSymbolRemoved(ctx context.Context, store types.EventStore, payload SymbolRemovedPayload) error {
	return emitEvent(ctx, store, types.EventSymbolRemoved, payload)
}