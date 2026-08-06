package extensions

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// ExternalProviderAdapter wraps a SubprocessManager to implement ExternalProvider.
type ExternalProviderAdapter struct {
	name        string
	procManager *SubprocessManager
	models      []types.ModelInfo
	modelsMu    sync.RWMutex
	initialized bool
	initMu      sync.Mutex
}

var (
	_ ExternalProvider = (*ExternalProviderAdapter)(nil)
)

// NewExternalProviderAdapter creates a new ExternalProviderAdapter.
func NewExternalProviderAdapter(name string, proc *SubprocessManager) *ExternalProviderAdapter {
	return &ExternalProviderAdapter{
		name:        name,
		procManager: proc,
	}
}

// Name returns the provider name.
func (a *ExternalProviderAdapter) Name() string {
	return a.name
}

// FetchModels returns the list of available models.
func (a *ExternalProviderAdapter) FetchModels() ([]types.ModelInfo, error) {
	a.modelsMu.RLock()
	if a.models != nil {
		models := make([]types.ModelInfo, len(a.models))
		copy(models, a.models)
		a.modelsMu.RUnlock()
		return models, nil
	}
	a.modelsMu.RUnlock()

	// Fetch models if not cached
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	models, err := a.fetchModels(ctx)
	if err != nil {
		return nil, err
	}

	a.modelsMu.Lock()
	a.models = models
	a.modelsMu.Unlock()

	result := make([]types.ModelInfo, len(models))
	copy(result, models)
	return result, nil
}

// ChatCompletionStream performs a streaming chat completion.
func (a *ExternalProviderAdapter) ChatCompletionStream(req types.ChatRequest) (types.StreamIterator, error) {
	a.ensureInitialized()

	ctx, cancel := context.WithCancel(context.Background())

	reqMsg := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodProviderChatCompletion,
	}

	params := ProviderChatCompletionParams{Request: req}
	paramsData, err := json.Marshal(params)
	if err != nil {
		cancel()
		return types.StreamIterator{}, fmt.Errorf("marshal params: %w", err)
	}
	reqMsg.Params = paramsData

	resp, err := a.procManager.Call(ctx, reqMsg, 60*time.Second)
	if err != nil {
		cancel()
		return types.StreamIterator{}, err
	}

	if resp.Error != nil {
		cancel()
		return types.StreamIterator{}, fmt.Errorf("chat_completion error: %s", resp.Error.Message)
	}

	var result ProviderChatCompletionResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		cancel()
		return types.StreamIterator{}, fmt.Errorf("unmarshal result: %w", err)
	}

	// Create a stream iterator that reads notifications from the subprocess
	// For now, we'll implement a basic version that polls for notifications
	// A full implementation would use a dedicated notification channel
	return types.StreamIterator{
		Next: func() (*types.StreamChunk, error) {
			// This is a simplified implementation
			// In a real implementation, we'd have a notification listener
			return nil, context.Canceled
		},
		Close: func() error {
			cancel()
			return nil
		},
	}, nil
}

// EstimateCost estimates the cost of a request.
func (a *ExternalProviderAdapter) EstimateCost(req types.ChatRequest) float64 {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	reqMsg := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodProviderEstimateCost,
	}

	paramsData, err := json.Marshal(req)
	if err != nil {
		return 0
	}
	reqMsg.Params = paramsData

	resp, err := a.procManager.Call(ctx, reqMsg, 10*time.Second)
	if err != nil {
		return 0
	}

	if resp.Error != nil {
		return 0
	}

	var result ProviderEstimateCostResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return 0
	}

	return result.CostUSD
}

// HealthCheck checks provider health.
func (a *ExternalProviderAdapter) HealthCheck() error {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodProviderHealthCheck,
	}

	resp, err := a.procManager.Call(ctx, req, 10*time.Second)
	if err != nil {
		return err
	}

	if resp.Error != nil {
		return fmt.Errorf("health_check error: %s", resp.Error.Message)
	}

	var result ProviderHealthCheckResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return err
	}

	if result.Status != "healthy" {
		return fmt.Errorf("provider unhealthy: %s", result.Error)
	}

	return nil
}

// ensureInitialized fetches provider metadata on first access.
func (a *ExternalProviderAdapter) ensureInitialized() {
	a.initMu.Lock()
	defer a.initMu.Unlock()

	if a.initialized {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Fetch name
	if name, err := a.fetchName(ctx); err == nil {
		a.name = name
	}

	// Fetch models
	if models, err := a.fetchModels(ctx); err == nil {
		a.modelsMu.Lock()
		a.models = models
		a.modelsMu.Unlock()
	}

	a.initialized = true
	slog.Debug("external provider adapter initialized", "name", a.name, "models", len(a.models))
}

// fetchName calls provider.name method.
func (a *ExternalProviderAdapter) fetchName(ctx context.Context) (string, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodProviderName,
	}

	resp, err := a.procManager.Call(ctx, req, 5*time.Second)
	if err != nil {
		return "", err
	}

	if resp.Error != nil {
		return "", fmt.Errorf("provider.name error: %s", resp.Error.Message)
	}

	var result ProviderNameResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return "", err
	}

	return result.Name, nil
}

// fetchModels calls provider.fetch_models method.
func (a *ExternalProviderAdapter) fetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	req := JSONRPCRequest{
		JSONRPC: "2.0",
		ID:      a.procManager.nextRequestID(),
		Method:  MethodProviderFetchModels,
	}

	resp, err := a.procManager.Call(ctx, req, 30*time.Second)
	if err != nil {
		return nil, err
	}

	if resp.Error != nil {
		return nil, fmt.Errorf("provider.fetch_models error: %s", resp.Error.Message)
	}

	var result ProviderFetchModelsResult
	if err := json.Unmarshal(resp.Result, &result); err != nil {
		return nil, err
	}

	return result.Models, nil
}