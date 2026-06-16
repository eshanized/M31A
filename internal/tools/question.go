package tools

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

// QuestionRequest represents a question sent from the tool to the TUI.
type QuestionRequest struct {
	ID          int64
	Question    string
	Header      string
	Options     []string
	AllowCustom bool
	TimeoutSecs int
}

// QuestionResponse represents the user's answer from the TUI.
type QuestionResponse struct {
	Answer string
}

var questionRequestIDCounter atomic.Int64

func nextQuestionRequestID() int64 {
	return questionRequestIDCounter.Add(1)
}

type AskUserQuestion struct {
	requestCh  chan QuestionRequest
	responseCh chan QuestionResponse
	pending    *sync.Map // map[int64]chan QuestionResponse — per-request routing
}

func NewAskUserQuestion(requestCh chan QuestionRequest, responseCh chan QuestionResponse, pending *sync.Map) *AskUserQuestion {
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
	return "Ask the user a question and wait for their answer. Use only in interactive sessions — NEVER in automated task execution flows."
}

func (t *AskUserQuestion) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for AskUserQuestion tool parameters.
func (t *AskUserQuestion) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"question": {"type": "string", "description": "Question to ask the user"},
			"header": {"type": "string", "description": "Short header displayed above the question"},
			"options": {
				"type": "array",
				"items": {
					"type": "object",
					"properties": {
						"label": {"type": "string"},
						"description": {"type": "string"}
					}
				},
				"description": "Pre-defined answer options shown to the user"
			},
			"allow_custom": {"type": "boolean", "description": "Allow user to type a custom answer instead of choosing an option (default true)"},
			"timeout": {"type": "integer", "description": "Seconds to wait for user answer before timing out (default 300)"}
		},
		"required": ["question"]
	}`
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
	if h, ok := input.Params["header"].(string); ok {
		header = h
	}

	var options []string
	if optsRaw, ok := input.Params["options"].([]any); ok {
		for _, o := range optsRaw {
			switch v := o.(type) {
			case string:
				options = append(options, v)
			case map[string]any:
				// Schema-compliant format: {"label": "...", "description": "..."}
				if label, ok := v["label"].(string); ok && label != "" {
					options = append(options, label)
				}
			}
		}
	}

	allowCustom := true
	if ac, ok := input.Params["allow_custom"].(bool); ok {
		allowCustom = ac
	}

	timeoutSecs := types.DefaultPermissionTimeout
	if tRaw, ok := input.Params["timeout"].(float64); ok {
		timeoutSecs = int(tRaw)
	}

	// Send question to TUI with a unique request ID
	req := QuestionRequest{
		ID:          nextQuestionRequestID(),
		Question:    question,
		Header:      header,
		Options:     options,
		AllowCustom: allowCustom,
		TimeoutSecs: timeoutSecs,
	}

	// Create per-request response channel to avoid cross-caller routing
	respCh := make(chan QuestionResponse, 1)
	if t.pending != nil {
		t.pending.Store(req.ID, respCh)
		defer t.pending.Delete(req.ID)
	}

	select {
	case t.requestCh <- req:
	case <-ctx.Done():
		return types.ToolResult{}, ctx.Err()
	default:
		return types.ToolResult{}, fmt.Errorf("%w: question channel full", m31errors.ErrToolExecution)
	}

	// Wait for response on per-request channel
	var answer string
	timer := time.NewTimer(time.Duration(timeoutSecs) * time.Second)
	defer timer.Stop()

	// Choose the response channel: per-request if available, else shared fallback
	var responseSource <-chan QuestionResponse
	if t.pending != nil {
		responseSource = respCh
	} else {
		responseSource = t.responseCh
	}

	select {
	case resp := <-responseSource:
		answer = resp.Answer
	case <-ctx.Done():
		return types.ToolResult{}, ctx.Err()
	case <-timer.C:
		return types.ToolResult{}, fmt.Errorf("%w: question timed out after %d seconds", m31errors.ErrToolExecution, timeoutSecs)
	}

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     fmt.Sprintf("User answered: %s", answer),
		DurationMs: elapsed,
	}, nil
}
