package context

import (
	"context"
	"encoding/json"
	"testing"
)

func TestNewRegistry(t *testing.T) {
	r := NewRegistry()
	if r == nil {
		t.Fatal("NewRegistry() returned nil")
	}
	if len(r.sources) != 0 {
		t.Errorf("expected 0 sources, got %d", len(r.sources))
	}
}

func TestNewRegistry_WithSources(t *testing.T) {
	src1 := &mockSource{key: "src1", value: "val1"}
	src2 := &mockSource{key: "src2", value: "val2"}

	r := NewRegistry(src1, src2)
	if len(r.sources) != 2 {
		t.Errorf("expected 2 sources, got %d", len(r.sources))
	}
}

func TestRegistry_Add(t *testing.T) {
	r := NewRegistry()
	src := &mockSource{key: "src1", value: "val1"}

	r.Add(src)
	if len(r.sources) != 1 {
		t.Errorf("expected 1 source, got %d", len(r.sources))
	}
}

func TestRegistry_LoadAll(t *testing.T) {
	src1 := &mockSource{key: "src1", value: "val1"}
	src2 := &mockSource{key: "src2", value: "val2"}

	r := NewRegistry(src1, src2)
	snapshot := r.LoadAll(context.Background())

	if len(snapshot) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(snapshot))
	}
	if snapshot["src1"] != "val1" {
		t.Errorf("src1 = %q, want %q", snapshot["src1"], "val1")
	}
	if snapshot["src2"] != "val2" {
		t.Errorf("src2 = %q, want %q", snapshot["src2"], "val2")
	}
}

func TestRegistry_LoadAll_Error(t *testing.T) {
	src := &mockSource{key: "src1", err: context.DeadlineExceeded}

	r := NewRegistry(src)
	snapshot := r.LoadAll(context.Background())

	if len(snapshot) != 0 {
		t.Errorf("expected 0 entries on error, got %d", len(snapshot))
	}
}

func TestRegistry_Reconcile_Added(t *testing.T) {
	src := &mockSource{key: "src1", value: "val1", rendered: "rendered val1"}

	r := NewRegistry(src)
	changes := r.Reconcile(context.Background(), map[string]string{})

	if len(changes) != 1 {
		t.Fatalf("expected 1 change, got %d", len(changes))
	}
	if changes[0].Key != "src1" {
		t.Errorf("change Key = %q, want %q", changes[0].Key, "src1")
	}
	if changes[0].Type != ChangeAdded {
		t.Errorf("change Type = %d, want %d", changes[0].Type, ChangeAdded)
	}
	if changes[0].Content != "rendered val1" {
		t.Errorf("change Content = %q, want %q", changes[0].Content, "rendered val1")
	}
}

func TestRegistry_Reconcile_Updated(t *testing.T) {
	src := &mockSource{key: "src1", value: "newval", renderUpdate: "updated val"}

	r := NewRegistry(src)
	previous := map[string]string{"src1": "oldval"}
	changes := r.Reconcile(context.Background(), previous)

	if len(changes) != 1 {
		t.Fatalf("expected 1 change, got %d", len(changes))
	}
	if changes[0].Type != ChangeUpdated {
		t.Errorf("change Type = %d, want %d", changes[0].Type, ChangeUpdated)
	}
	if changes[0].Content != "updated val" {
		t.Errorf("change Content = %q, want %q", changes[0].Content, "updated val")
	}
}

func TestRegistry_Reconcile_Removed(t *testing.T) {
	// Test removal behavior: src2 was in previous but not in current
	src := &mockSource{key: "src1", value: "val1", renderRemoval: "removed val"}

	r := NewRegistry(src)
	previous := map[string]string{"src1": "val1", "src2": "gone"}
	changes := r.Reconcile(context.Background(), previous)

	// src1 same value -> no change
	// src2 removed but has no source in registry -> renderRemoval returns "" -> no change
	if len(changes) != 0 {
		t.Errorf("expected 0 changes, got %d", len(changes))
	}
}

func TestRegistry_Reconcile_NoChanges(t *testing.T) {
	src := &mockSource{key: "src1", value: "val1"}

	r := NewRegistry(src)
	previous := map[string]string{"src1": "val1"}
	changes := r.Reconcile(context.Background(), previous)

	if len(changes) != 0 {
		t.Errorf("expected 0 changes, got %d", len(changes))
	}
}

func TestRegistry_Reconcile_SortedKeys(t *testing.T) {
	src1 := &mockSource{key: "z-source", value: "z", rendered: "z rendered"}
	src2 := &mockSource{key: "a-source", value: "a", rendered: "a rendered"}

	r := NewRegistry(src1, src2)
	changes := r.Reconcile(context.Background(), map[string]string{})

	if len(changes) != 2 {
		t.Fatalf("expected 2 changes, got %d", len(changes))
	}
	if changes[0].Key != "a-source" {
		t.Errorf("first change Key = %q, want %q (sorted)", changes[0].Key, "a-source")
	}
	if changes[1].Key != "z-source" {
		t.Errorf("second change Key = %q, want %q (sorted)", changes[1].Key, "z-source")
	}
}

func TestRegistry_Reconcile_EmptyRender(t *testing.T) {
	// When Render returns the value itself (not empty), a change IS added
	src := &mockSource{key: "src1", value: "val1", rendered: ""}

	r := NewRegistry(src)
	changes := r.Reconcile(context.Background(), map[string]string{})

	// Render("") fallback returns val1, so change is added
	if len(changes) != 1 {
		t.Errorf("expected 1 change (Render returns value when rendered is empty), got %d", len(changes))
	}
}

func TestChangeType_Constants(t *testing.T) {
	if ChangeAdded != 0 {
		t.Errorf("ChangeAdded = %d, want 0", ChangeAdded)
	}
	if ChangeUpdated != 1 {
		t.Errorf("ChangeUpdated = %d, want 1", ChangeUpdated)
	}
	if ChangeRemoved != 2 {
		t.Errorf("ChangeRemoved = %d, want 2", ChangeRemoved)
	}
}

func TestNewSnapshot(t *testing.T) {
	state := map[string]string{"key": "value"}
	snap := NewSnapshot(state)

	if snap == nil {
		t.Fatal("NewSnapshot() returned nil")
	}
	if snap.State["key"] != "value" {
		t.Errorf("State[key] = %q, want %q", snap.State["key"], "value")
	}
}

func TestSnapshot_Encode(t *testing.T) {
	state := map[string]string{"key": "value", "key2": "value2"}
	snap := NewSnapshot(state)

	data, err := snap.Encode()
	if err != nil {
		t.Fatalf("Encode() error = %v", err)
	}

	// Verify it's valid JSON
	var decoded map[string]string
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("encoded data is not valid JSON: %v", err)
	}
	if decoded["key"] != "value" {
		t.Errorf("decoded key = %q, want %q", decoded["key"], "value")
	}
}

func TestSnapshot_Encode_Empty(t *testing.T) {
	snap := NewSnapshot(map[string]string{})

	data, err := snap.Encode()
	if err != nil {
		t.Fatalf("Encode() error = %v", err)
	}
	if string(data) != "{}" {
		t.Errorf("encoded empty map = %q, want %q", string(data), "{}")
	}
}

func TestDecodeSnapshot(t *testing.T) {
	data := []byte(`{"key":"value","key2":"value2"}`)

	state, err := DecodeSnapshot(data)
	if err != nil {
		t.Fatalf("DecodeSnapshot() error = %v", err)
	}
	if state["key"] != "value" {
		t.Errorf("state[key] = %q, want %q", state["key"], "value")
	}
	if state["key2"] != "value2" {
		t.Errorf("state[key2] = %q, want %q", state["key2"], "value2")
	}
}

func TestDecodeSnapshot_InvalidJSON(t *testing.T) {
	_, err := DecodeSnapshot([]byte("invalid"))
	if err == nil {
		t.Error("expected error for invalid JSON")
	}
}

func TestDecodeSnapshot_EmptyObject(t *testing.T) {
	state, err := DecodeSnapshot([]byte("{}"))
	if err != nil {
		t.Fatalf("DecodeSnapshot() error = %v", err)
	}
	if len(state) != 0 {
		t.Errorf("expected empty map, got %d entries", len(state))
	}
}

func TestDateTimeSource(t *testing.T) {
	src := DateTimeSource{}

	if src.Key() != "core/datetime" {
		t.Errorf("Key() = %q, want %q", src.Key(), "core/datetime")
	}

	val, err := src.Load(context.Background())
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if val == "" {
		t.Error("Load() returned empty string")
	}

	rendered := src.Render(val)
	if rendered == "" {
		t.Error("Render() returned empty string")
	}

	updated := src.RenderUpdate("old", "new")
	if updated == "" {
		t.Error("RenderUpdate() returned empty string")
	}

	removal := src.RenderRemoval("value")
	if removal != "" {
		t.Errorf("RenderRemoval() = %q, want empty", removal)
	}
}

func TestEnvironmentSource(t *testing.T) {
	src := EnvironmentSource{WorkDir: "/tmp/test"}

	if src.Key() != "core/environment" {
		t.Errorf("Key() = %q, want %q", src.Key(), "core/environment")
	}

	val, err := src.Load(context.Background())
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if val == "" {
		t.Error("Load() returned empty string")
	}

	rendered := src.Render(val)
	if rendered == "" {
		t.Error("Render() returned empty string")
	}

	updated := src.RenderUpdate("old", "new")
	if updated == "" {
		t.Error("RenderUpdate() returned empty string")
	}

	removal := src.RenderRemoval("value")
	if removal != "" {
		t.Errorf("RenderRemoval() = %q, want empty", removal)
	}
}

func TestInstructionsSource_Key(t *testing.T) {
	src := InstructionsSource{}
	if src.Key() != "core/instructions" {
		t.Errorf("Key() = %q, want %q", src.Key(), "core/instructions")
	}
}

func TestInstructionsSource_Render(t *testing.T) {
	src := InstructionsSource{}
	rendered := src.Render("test content")
	if rendered == "" {
		t.Error("Render() returned empty string")
	}
}

func TestInstructionsSource_RenderUpdate(t *testing.T) {
	src := InstructionsSource{}
	updated := src.RenderUpdate("old", "new")
	if updated == "" {
		t.Error("RenderUpdate() returned empty string")
	}
}

func TestInstructionsSource_RenderRemoval(t *testing.T) {
	src := InstructionsSource{}
	removal := src.RenderRemoval("value")
	if removal == "" {
		t.Error("RenderRemoval() returned empty string")
	}
}

func TestInstructionsSource_Load_NoFiles(t *testing.T) {
	src := InstructionsSource{
		ProjectRoot: "/nonexistent/root",
		WorkDir:     "/nonexistent/workdir",
	}

	_, err := src.Load(context.Background())
	if err == nil {
		t.Error("expected error when no AGENTS.md files found")
	}
}

// mockSource is a test helper that implements ContextSource.
type mockSource struct {
	key           string
	value         string
	err           error
	rendered      string
	renderUpdate  string
	renderRemoval string
}

func (m *mockSource) Key() string                            { return m.key }
func (m *mockSource) Load(_ context.Context) (string, error) { return m.value, m.err }
func (m *mockSource) Render(value string) string {
	if m.rendered != "" {
		return m.rendered
	}
	return value
}
func (m *mockSource) RenderUpdate(_, newVal string) string {
	if m.renderUpdate != "" {
		return m.renderUpdate
	}
	return newVal
}
func (m *mockSource) RenderRemoval(value string) string {
	return m.renderRemoval
}
