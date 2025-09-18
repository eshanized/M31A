package tools

import (
	"context"
	"fmt"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// QuestionRequest represents a question sent from the tool to the TUI.
type QuestionRequest struct {
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

type AskUserQuestion struct {
	requestCh  chan QuestionRequest
	responseCh chan QuestionResponse
}

func NewAskUserQuestion(requestCh chan QuestionRequest, responseCh chan QuestionResponse) *AskUserQuestion {
	return &AskUserQuestion{
		requestCh:  requestCh,
		responseCh: responseCh,
	}
}

func (t *AskUserQuestion) Name() string {
	return "AskUserQuestion"
}

func (t *AskUserQuestion) Description() string {
	return "Ask the user a question and wait for their answer."
}

func (t *AskUserQuestion) RiskLevel() types.RiskLevel {
	return types.RiskSafe
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
			if s, ok := o.(string); ok {
				options = append(options, s)
			}
		}
	}

	allowCustom := true
	if ac, ok := input.Params["allow_custom"].(bool); ok {
		allowCustom = ac
	}

	timeoutSecs := 300
	if tRaw, ok := input.Params["timeout"].(float64); ok {
		timeoutSecs = int(tRaw)
	}

	// Send question to TUI
	req := QuestionRequest{
		Question:    question,
		Header:      header,
		Options:     options,
		AllowCustom: allowCustom,
		TimeoutSecs: timeoutSecs,
	}

	select {
	case t.requestCh <- req:
	case <-ctx.Done():
		return types.ToolResult{}, ctx.Err()
	default:
		return types.ToolResult{}, fmt.Errorf("%w: question channel full", m31errors.ErrToolExecution)
	}

	// Wait for response
	var answer string
	timer := time.NewTimer(time.Duration(timeoutSecs) * time.Second)
	defer timer.Stop()
	select {
	case resp := <-t.responseCh:
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
