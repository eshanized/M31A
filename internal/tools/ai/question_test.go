package ai

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestAskUserQuestion_Name(t *testing.T) {
	t.Parallel()
	// Create with nil channels - just testing properties
	q := NewAskUserQuestion(nil, nil, nil)
	if q.Name() != "AskUserQuestion" {
		t.Errorf("expected 'AskUserQuestion', got %q", q.Name())
	}
}

func TestAskUserQuestion_RiskLevel(t *testing.T) {
	t.Parallel()
	q := NewAskUserQuestion(nil, nil, nil)
	if q.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %v", q.RiskLevel())
	}
}

func TestAskUserQuestion_Description(t *testing.T) {
	t.Parallel()
	q := NewAskUserQuestion(nil, nil, nil)
	desc := q.Description()
	if desc == "" {
		t.Errorf("expected non-empty description")
	}
}

func TestAskUserQuestion_ParameterSchema(t *testing.T) {
	t.Parallel()
	q := NewAskUserQuestion(nil, nil, nil)
	schema := q.ParameterSchema()
	if schema == "" {
		t.Errorf("expected non-empty schema")
	}
}

func TestAskUserQuestion_ExecuteMissingQuestion(t *testing.T) {
	t.Parallel()
	q := NewAskUserQuestion(nil, nil, nil)
	_, err := q.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{},
	})
	if err == nil {
		t.Errorf("expected error for missing question")
	}
}

func TestNextQuestionRequestID(t *testing.T) {
	t.Parallel()
	id1 := NextQuestionRequestID()
	id2 := NextQuestionRequestID()
	if id2 <= id1 {
		t.Errorf("expected increasing IDs, got %d then %d", id1, id2)
	}
}
