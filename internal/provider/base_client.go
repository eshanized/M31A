package provider

import (
	"net"
	"net/http"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// sharedTransport is a shared HTTP transport across all provider clients.
// This reuses connection pools, idle goroutines, and TLS session caches
// instead of creating separate ones per provider (PERF-30).
var (
	sharedTransport     *http.Transport
	sharedTransportOnce sync.Once
)

func getSharedTransport() *http.Transport {
	sharedTransportOnce.Do(func() {
		sharedTransport = &http.Transport{
			DialContext: (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
			MaxIdleConns:        100,
			MaxIdleConnsPerHost: 10,
			IdleConnTimeout:     90 * time.Second,
		}
	})
	return sharedTransport
}

// BaseClient holds fields and methods shared by all provider implementations.
// Provider-specific clients embed BaseClient and override only Name(),
// FetchModels(), ChatCompletionStream(), and HealthCheck().
type BaseClient struct {
	APIKeyField  string
	BaseURLField string
	HTTPClient   *http.Client
	Cache        *ModelCache
	HealthLiveMs int64
	HealthSlowMs int64
	Version      string
}

// NewBaseClient creates a BaseClient with the given settings.
func NewBaseClient(apiKey, baseURL, version string, cacheTTL, cacheStaleTTL time.Duration, healthLiveMs, healthSlowMs int64) BaseClient {
	if cacheTTL == 0 {
		cacheTTL = types.ModelCacheTTL
	}
	if cacheStaleTTL == 0 {
		cacheStaleTTL = types.StaleCacheTTL
	}
	if healthLiveMs == 0 {
		healthLiveMs = types.DefaultHealthLiveMs
	}
	if healthSlowMs == 0 {
		healthSlowMs = types.DefaultHealthSlowMs
	}
	return BaseClient{
		APIKeyField:  apiKey,
		BaseURLField: baseURL,
		Version:      version,
		HTTPClient: &http.Client{
			Transport: getSharedTransport(),
		},
		Cache:        NewModelCacheWithStale(cacheTTL, cacheStaleTTL),
		HealthLiveMs: healthLiveMs,
		HealthSlowMs: healthSlowMs,
	}
}

// APIKey returns a masked version of the API key for display.
func (b *BaseClient) APIKey() string {
	if len(b.APIKeyField) <= 4 {
		return "****"
	}
	return "****" + b.APIKeyField[len(b.APIKeyField)-4:]
}

// EstimateCost calculates cost for a usage sample against the model cache.
func (b *BaseClient) EstimateCost(modelID string, usage types.Usage) float64 {
	return EstimateCost(modelID, usage, b.Cache)
}

// GetModel retrieves a model from the cache by ID.
func (b *BaseClient) GetModel(id string) (*types.ModelInfo, error) {
	return GetModel(id, b.Cache)
}

// CachedModels returns all models from the cache without a network call.
func (b *BaseClient) CachedModels() []types.ModelInfo {
	return CachedModels(b.Cache)
}

// MakeIterator wraps an SSEParser into a StreamIterator.
func (b *BaseClient) MakeIterator(sse *SSEParser, modelID string) *types.StreamIterator {
	return &types.StreamIterator{
		Next: func() (*types.StreamChunk, error) {
			_, data, err := sse.Next()
			if err != nil {
				return nil, err
			}
			if data == "" {
				return nil, nil
			}
			chunk, err := ParseSSEChunk(data, modelID)
			if err != nil {
				return nil, err
			}
			return chunk, nil
		},
		Close: func() error {
			return sse.Close()
		},
	}
}
