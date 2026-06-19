package tools

import (
	"context"
	"encoding/json"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestAgent_NewAgent(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false, 0)
	if a == nil {
		t.Fatal("expected non-nil agent")
	}
}

func TestAgent_IsChildField(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, true, 1)
	if !a.isChild {
		t.Error("expected isChild to be true")
	}

	b := NewAgent(nil, false, 0)
	if b.isChild {
		t.Error("expected isChild to be false")
	}
}

func TestAgent_Execute_InvalidIsolation(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false, 0)
	result, err := a.Execute(context.Background(), types.ToolInput{
		Name: "Agent",
		Params: map[string]any{
			"description": "test task",
			"prompt":      "do something",
			"isolation":   "invalid_value",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// nil manager check happens first
	if !strings.Contains(result.Error, "not configured") {
		t.Errorf("expected 'not configured' error, got %q", result.Error)
	}
}

func TestAgent_SchemaValidity(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false, 0)
	schema := a.ParameterSchema()
	var parsed map[string]any
	if err := json.Unmarshal([]byte(schema), &parsed); err != nil {
		t.Fatalf("schema is not valid JSON: %v", err)
	}
	props, ok := parsed["properties"].(map[string]any)
	if !ok {
		t.Fatal("expected properties in schema")
	}
	if _, ok := props["description"]; !ok {
		t.Error("expected 'description' property in schema")
	}
	if _, ok := props["prompt"]; !ok {
		t.Error("expected 'prompt' property in schema")
	}
}
