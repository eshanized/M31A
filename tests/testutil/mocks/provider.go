// Package mocks provides centralized mock implementations of provider and
// tool interfaces for use across all test files in the M31A project.
//
// Note: Compile-time interface checks (var _ Interface = (*MockType)(nil))
// are intentionally omitted to avoid import cycles. Each consumer test file
// should verify interface compliance locally if needed.
package mocks

import (
	"context"
	"encoding/json"
	"io"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

// MockProvider implements provider.LLMProvider for testing.
type MockProvider struct {
	Name_          string
	APIKey_        string
	Response_      string
	Err_           error
	CallCount_     int
	MultiResponses []string
	HealthStatus_  types.HealthStatus
}

// NewMockProvider creates a MockProvider with sensible defaults.
func NewMockProvider(name string) *MockProvider {
	return &MockProvider{
		Name_:         name,
		HealthStatus_: types.HealthStatus{Status: "live"},
	}
}

// NewMockProviderWithResponse creates a MockProvider that returns the given response.
func NewMockProviderWithResponse(name, response string) *MockProvider {
	return &MockProvider{
		Name_:         name,
		Response_:     response,
		HealthStatus_: types.HealthStatus{Status: "live"},
	}
}

func (m *MockProvider) Name() string   { return m.Name_ }
func (m *MockProvider) APIKey() string { return m.APIKey_ }

func (m *MockProvider) FetchModels(_ context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}

func (m *MockProvider) ChatCompletionStream(_ context.Context, _ provider.ChatRequest) (*types.StreamIterator, error) {
	m.CallCount_++
	content := m.Response_
	if len(m.MultiResponses) > 0 {
		idx := m.CallCount_ - 1
		if idx < len(m.MultiResponses) {
			content = m.MultiResponses[idx]
		}
	}
	if content == "" {
		content = "OK"
	}

	// Check if content is a tool_call JSON response (OpenAI format: tool_calls array)
	var toolCallChunk *types.StreamChunk
	var toolCallJSON map[string]any
	if err := json.Unmarshal([]byte(content), &toolCallJSON); err == nil {
		// OpenAI format: {"tool_calls": [{"index": 0, "id": "call_1", "function": {"name": "Bash", "arguments": "..."}}]}
		if tcRaw, exists := toolCallJSON["tool_calls"]; exists {
			if tcArr, ok := tcRaw.([]any); ok && len(tcArr) > 0 {
				if tc, ok := tcArr[0].(map[string]any); ok {
					toolCallChunk = &types.StreamChunk{Type: "tool_call"}
					if idx, ok := tc["index"].(float64); ok {
						toolCallChunk.Index = int(idx)
					}
					if id, ok := tc["id"].(string); ok {
						toolCallChunk.ToolCallID = id
					}
					if fn, ok := tc["function"].(map[string]any); ok {
						if name, ok := fn["name"].(string); ok {
							toolCallChunk.ToolName = name
						}
						if args, ok := fn["arguments"].(string); ok {
							toolCallChunk.ToolInput = args
						}
					}
				}
			}
		}
		// Simple format: {"name": "Bash", "input": {"command": "echo test"}}
		if toolCallChunk == nil {
			if name, ok := toolCallJSON["name"].(string); ok {
				if input, ok := toolCallJSON["input"]; ok {
					inputBytes, _ := json.Marshal(input)
					toolCallChunk = &types.StreamChunk{
						Type:      "tool_call",
						Index:     0,
						ToolCallID: "call_test_1",
						ToolName:  name,
						ToolInput: string(inputBytes),
					}
				}
			}
		}
	}

	var done bool
	var sentToolCall bool
	var sentContent bool

	next := func() (*types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}

		// First, send tool_call chunk if available
		if toolCallChunk != nil && !sentToolCall {
			sentToolCall = true
			return toolCallChunk, nil
		}

		// Then send content if available
		if content != "" && !sentContent {
			sentContent = true
			return &types.StreamChunk{Type: "content", Delta: content}, nil
		}

		// Send done
		done = true
		return &types.StreamChunk{Type: "done"}, nil
	}

	closeFn := func() error { return nil }
	return &types.StreamIterator{Next: next, Close: closeFn}, m.Err_
}

func (m *MockProvider) EstimateCost(_ string, _ types.Usage) float64 { return 0 }

func (m *MockProvider) HealthCheck(_ context.Context) types.HealthStatus {
	return m.HealthStatus_
}

func (m *MockProvider) GetModel(_ string) (*types.ModelInfo, error) { return nil, nil }
func (m *MockProvider) CachedModels() []types.ModelInfo             { return nil }
