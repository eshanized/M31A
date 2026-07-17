package ai

import (
	"context"
	"fmt"
	"sync"
	"sync/atomic"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*AskUserQuestion)(nil)

// AskUserQuestion prompts the user for input with a question and optional multiple choice options.
// This tool blocks until the user responds in the TUI.
// Use for disambiguation, confirmation, or getting user preferences.
type AskUserQuestion struct {
	requestCh  chan types.QuestionRequest
	responseCh chan types.QuestionResponse
	pending    *sync.Map // map[int64]chan types.QuestionResponse — per-request routing
}

// NewAskUserQuestion creates a new AskUserQuestion tool instance.
func NewAskUserQuestion(requestCh chan types.QuestionRequest, responseCh chan types.QuestionResponse, pending *sync.Map) *AskUserQuestion {
	return &AskUserQuestion{
		requestCh:  requestCh,
		responseCh: responseCh,
		pending:    pending,
	}
}

func (t *AskUserQuestion) Name() string {
	return "AskUserQuestion"
}

func (t *AskUserQuestion) Description() string {
	return `Prompt the user for input with a question and optional multiple choice options.
This tool blocks until the user responds in the TUI.
Use for disambiguation, confirmation, or getting user preferences.`
}

func (t *AskUserQuestion) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

func (t *AskUserQuestion) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"question": {"type": "string", "description": "The question to ask the user"},
			"header": {"type": "string", "description": "Optional header/title for the prompt"},
			"options": {"type": "array", "items": {"type": "string"}, "description": "Predefined answer options (if empty, free-text input)"},
			"allow_custom": {"type": "boolean", "description": "Allow custom answer when options provided (default true)"},
			"timeout": {"type": "integer", "description": "Timeout in seconds (default 60, max 300)"}
		},
		"required": ["question"]
	}`
}

var questionRequestIDCounter atomic.Int64

func nextQuestionRequestID() int64 {
	return questionRequestIDCounter.Add(1)
}

func (t *AskUserQuestion) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	questionRaw, ok := input.Params["question"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: question", m31errors.ErrToolExecution)
	}
	question, ok := questionRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter question must be a string", m31errors.ErrToolExecution)
	}

	header := ""
	if hRaw, ok := input.Params["header"]; ok {
		if hStr, ok := hRaw.(string); ok {
			header = hStr
		}
	}

	var options []string
	if optRaw, ok := input.Params["options"]; ok {
		if optSlice, ok := optRaw.([]any); ok {
			for _, v := range optSlice {
				if s, ok := v.(string); ok {
					options = append(options, s)
				}
			}
		}
	}

	allowCustom := true
	if acRaw, ok := input.Params["allow_custom"]; ok {
		if acBool, ok := acRaw.(bool); ok {
			allowCustom = acBool
		}
	}

	timeoutSecs := 60
	if tRaw, ok := input.Params["timeout"]; ok {
		if tFloat, ok := tRaw.(float64); ok {
			timeoutSecs = int(tFloat)
		}
	}
	if timeoutSecs <= 0 {
		timeoutSecs = 60
	}
	if timeoutSecs > 300 {
		timeoutSecs = 300
	}

	reqID := nextQuestionRequestID()
	respCh := make(chan types.QuestionResponse, 1)
	t.pending.Store(reqID, respCh)

	req := types.QuestionRequest{
		ID:          reqID,
		Question:    question,
		Header:      header,
		Options:     options,
		AllowCustom: allowCustom,
		TimeoutSecs: timeoutSecs,
	}

	select {
	case t.requestCh <- req:
	case <-ctx.Done():
		t.pending.Delete(reqID)
		return types.ToolResult{}, ctx.Err()
	}

	select {
	case resp := <-respCh:
		elapsed := time.Since(start).Milliseconds()
		return types.ToolResult{
			Output:     resp.Answer,
			DurationMs: elapsed,
		}, nil
	case <-time.After(time.Duration(timeoutSecs) * time.Second):
		t.pending.Delete(reqID)
		return types.ToolResult{
			Error:      fmt.Sprintf("question timed out after %d seconds", timeoutSecs),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	case <-ctx.Done():
		t.pending.Delete(reqID)
		return types.ToolResult{}, ctx.Err()
	}
}
