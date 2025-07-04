package provider

import (
	"context"

	"github.com/eshanized/M31A/internal/types"
)

type LLMProvider interface {
	Name() string
	APIKey() string
	FetchModels(ctx context.Context) ([]types.ModelInfo, error)
	ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
	EstimateCost(modelID string, usage types.Usage) float64
	HealthCheck(ctx context.Context) types.HealthStatus
	GetModel(id string) (*types.ModelInfo, error)
}

type ChatRequest struct {
	Model            string           `json:"model"`
	Messages         []types.Message  `json:"messages"`
	MaxTokens        int              `json:"max_tokens,omitempty"`
	Stream           bool             `json:"stream"`
	Tools            []ToolDefinition `json:"tools,omitempty"`
	ReasoningEnabled bool             `json:"reasoning_enabled,omitempty"`
}

type ToolDefinition struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Parameters  string `json:"parameters"`
}
