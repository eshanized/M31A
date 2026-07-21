package tools

import (
	"context"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/tools/ai"
)

func TestAskUserQuestion_Execute_ContextCancel(t *testing.T) {
	t.Parallel()
	reqCh := make(chan types.QuestionRequest, 1)
	respCh := make(chan types.QuestionResponse, 1)
	pending := &sync.Map{}
	q := NewAskUserQuestion(reqCh, respCh, pending)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	_, err := q.Execute(ctx, types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
		},
	})
	if err == nil {
		t.Fatal("expected context error")
	}
}

func TestAskUserQuestion_Execute_TimeoutWaiting(t *testing.T) {
	t.Parallel()
	reqCh := make(chan types.QuestionRequest, 1)
	respCh := make(chan types.QuestionResponse, 1)
	pending := &sync.Map{}
	q := NewAskUserQuestion(reqCh, respCh, pending)

	// Drain request but don't respond
	go func() {
		<-reqCh
	}()

	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	_, err := q.Execute(ctx, types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
			"timeout":  1,
		},
	})
	if err == nil {
		t.Fatal("expected timeout error")
	}
}

func TestAskUserQuestion_Execute_SuccessResponse(t *testing.T) {
	t.Parallel()
	reqCh := make(chan types.QuestionRequest, 1)
	respCh := make(chan types.QuestionResponse, 1)
	pending := &sync.Map{}
	q := NewAskUserQuestion(reqCh, respCh, pending)

	go func() {
		req := <-reqCh
		if ch, ok := pending.Load(req.ID); ok {
			ch.(chan types.QuestionResponse) <- types.QuestionResponse{Answer: "Bob"}
		}
	}()

	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What is your name?",
			"header":   "Name",
			"options":  []any{"Alice", "Bob"},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Bob") {
		t.Errorf("expected 'Bob' in output, got: %s", result.Output)
	}
}

func TestAskUserQuestion_Execute_WithCustomAnswer(t *testing.T) {
	t.Parallel()
	reqCh := make(chan types.QuestionRequest, 1)
	respCh := make(chan types.QuestionResponse, 1)
	pending := &sync.Map{}
	q := NewAskUserQuestion(reqCh, respCh, pending)

	go func() {
		req := <-reqCh
		if ch, ok := pending.Load(req.ID); ok {
			ch.(chan types.QuestionResponse) <- types.QuestionResponse{Answer: "custom answer"}
		}
	}()

	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question":     "Choose or type:",
			"allow_custom": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "custom answer") {
		t.Errorf("expected 'custom answer' in output, got: %s", result.Output)
	}
}

func TestNextQuestionRequestID_Unique(t *testing.T) {
	t.Parallel()
	ids := make(map[int64]bool)
	for i := 0; i < 100; i++ {
		id := ai.NextQuestionRequestID()
		if ids[id] {
			t.Fatalf("duplicate ID: %d", id)
		}
		ids[id] = true
	}
}
