// Package mocks provides centralized mock implementations of provider and
// tool interfaces for use across all test files in the M31A project.
//
// Note: Compile-time interface checks (var _ Interface = (*MockType)(nil))
// are intentionally omitted to avoid import cycles. Each consumer test file
// should verify interface compliance locally if needed.
package mocks

import (
	"context"
	"io"

	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/types"
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
	done := false
	next := func() (*types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &types.StreamChunk{Delta: content}, nil
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
