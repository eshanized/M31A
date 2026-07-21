package provider

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
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
			DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
			TLSHandshakeTimeout:   10 * time.Second,
			ResponseHeaderTimeout: 10 * time.Second,
			MaxIdleConns:          100,
			MaxIdleConnsPerHost:   10,
			IdleConnTimeout:       90 * time.Second,
		}
	})
	return sharedTransport
}

// BaseClient holds fields and methods shared by all provider implementations.
// Provider-specific clients embed BaseClient and override only Name(),
// FetchModels(), ChatCompletionStream(), and HealthCheck().
//
// Two HTTP clients are maintained:
//   - HTTPClient: no Timeout, used for long-running SSE streaming requests
//     where the connection stays open for the entire generation.
//   - CatalogClient: hard Timeout (FetchModelsTimeout), used for short
//     catalog and health-check requests that must never block indefinitely.
type BaseClient struct {
	APIKeyField   string
	BaseURLField  string
	HTTPClient    *http.Client // streaming — no hard Timeout
	CatalogClient *http.Client // catalog/health — hard Timeout
	Cache         *ModelCache
	HealthLiveMs  int64
	HealthSlowMs  int64
	Version       string
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
	transport := getSharedTransport()
	return BaseClient{
		APIKeyField:  apiKey,
		BaseURLField: baseURL,
		Version:      version,
		// HTTPClient has no hard Timeout so SSE streams can run indefinitely.
		HTTPClient: &http.Client{
			Transport: transport,
		},
		// CatalogClient has a hard wall-clock Timeout for model list / health check
		// requests. This is the primary defence against the hanging-fetch bug:
		// even if the server stalls mid-response the request will be cancelled.
		CatalogClient: &http.Client{
			Transport: transport,
			Timeout:   types.FetchModelsTimeout,
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

// EvictModel removes a model from the cache by ID. Used for self-healing
// when a chat completion request fails because the model is unavailable,
// deprecated, or incompatible — preventing it from appearing in the selector.
func (b *BaseClient) EvictModel(id string) {
	b.Cache.Remove(id)
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

// HealthCheck performs a GET request to the given endpoint and returns a
// HealthStatus based on latency thresholds. Shared by all provider clients.
func (b *BaseClient) HealthCheck(ctx context.Context, endpoint string) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, b.BaseURLField+endpoint, nil)
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, Error: err.Error()}
	}
	SetCommonHeaders(req, b.APIKeyField, b.Version)

	resp, err := b.CatalogClient.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: err.Error()}
	}
	defer resp.Body.Close() //nolint:errcheck
	_, _ = ReadBodyLimited(resp, types.MaxLLMResponseBytes)

	if resp.StatusCode != http.StatusOK {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: fmt.Sprintf("status %d", resp.StatusCode)}
	}

	switch {
	case latency < b.HealthLiveMs:
		return types.HealthStatus{Status: types.HealthStatusLive, LatencyMs: latency}
	case latency < b.HealthSlowMs:
		return types.HealthStatus{Status: types.HealthStatusSlow, LatencyMs: latency}
	default:
		return types.HealthStatus{Status: types.HealthStatusDegraded, LatencyMs: latency}
	}
}

// HandleChatHTTPError interprets a non-200 HTTP response from a chat completion
// request and returns the appropriate sentinel error. Shared by all provider
// clients. Provider-specific error handling (e.g. model eviction) should be
// done by the caller after this function returns.
func (b *BaseClient) HandleChatHTTPError(resp *http.Response, providerName string, extraHandling func(statusCode int, bodyStr string)) error {
	bodyBytes, _ := ReadBodyLimited(resp, types.MaxLLMResponseBytes)
	retryAfter := GetRetryAfter(resp)
	_ = resp.Body.Close()
	bodyStr := string(bodyBytes)

	switch resp.StatusCode {
	case http.StatusTooManyRequests:
		if retryAfter != "" {
			return fmt.Errorf("%w (retry-after: %s)", m31errors.ErrRateLimited, retryAfter)
		}
		return m31errors.ErrRateLimited
	case http.StatusUnauthorized:
		return m31errors.ErrInvalidKey
	case http.StatusServiceUnavailable:
		return m31errors.ErrProviderUnreachable
	default:
		if extraHandling != nil {
			extraHandling(resp.StatusCode, bodyStr)
		}
		if IsContextExceeded(resp.StatusCode, bodyStr) {
			return m31errors.ErrContextExceeded
		}
		msg := SanitizeProviderError(resp.StatusCode, bodyStr, providerName)
		return &HTTPStatusError{StatusCode: resp.StatusCode, Message: msg}
	}
}

// HandleChatHTTPErrorWithCredits is like HandleChatHTTPError but also handles
// StatusPaymentRequired by mapping it to ErrNoCredits with a provider-specific
// message. Use this for providers (OpenRouter, NVIDIA) that return 402 for
// insufficient credits.
func (b *BaseClient) HandleChatHTTPErrorWithCredits(resp *http.Response, providerName string, extraHandling func(statusCode int, bodyStr string)) error {
	bodyBytes, _ := ReadBodyLimited(resp, types.MaxLLMResponseBytes)
	retryAfter := GetRetryAfter(resp)
	_ = resp.Body.Close()
	bodyStr := string(bodyBytes)

	switch resp.StatusCode {
	case http.StatusTooManyRequests:
		if retryAfter != "" {
			return fmt.Errorf("%w (retry-after: %s)", m31errors.ErrRateLimited, retryAfter)
		}
		return m31errors.ErrRateLimited
	case http.StatusUnauthorized:
		return m31errors.ErrInvalidKey
	case http.StatusPaymentRequired:
		return fmt.Errorf("%w: insufficient credits on %s", m31errors.ErrNoCredits, providerName)
	case http.StatusServiceUnavailable:
		return m31errors.ErrProviderUnreachable
	default:
		if extraHandling != nil {
			extraHandling(resp.StatusCode, bodyStr)
		}
		if IsContextExceeded(resp.StatusCode, bodyStr) {
			return m31errors.ErrContextExceeded
		}
		msg := SanitizeProviderError(resp.StatusCode, bodyStr, providerName)
		return &HTTPStatusError{StatusCode: resp.StatusCode, Message: msg}
	}
}
