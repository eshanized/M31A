package provider

import (
	"context"

	"github.com/eshanized/M31A/pkg/types"
)

// ChatRequest is an alias for types.ChatRequest (canonical definition in pkg/types/).
type ChatRequest = types.ChatRequest

// ToolDefinition is an alias for types.ToolDefinition (canonical definition in pkg/types/).
type ToolDefinition = types.ToolDefinition

type LLMProvider interface {
	Name() string
	APIKey() string
	FetchModels(ctx context.Context) ([]types.ModelInfo, error)
	CachedModels() []types.ModelInfo
	ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
	EstimateCost(modelID string, usage types.Usage) float64
	HealthCheck(ctx context.Context) types.HealthStatus
	GetModel(id string) (*types.ModelInfo, error)
}
