package provider

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/config"
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

// StreamRetryConfig holds configuration for streaming retry behavior.
// Mode controls when retries are attempted:
//   - "none": no retry, return error immediately
//   - "initial_only": retry only on initial connection failure (default)
//   - "full_resume": retry on any stream failure, restart request with exponential backoff
type StreamRetryConfig struct {
	Mode        string        // "none" | "initial_only" | "full_resume" (default "initial_only")
	MaxAttempts int           // default 3
	BaseDelay   time.Duration // default 1s
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
	Profiles      *config.ModelProfileConfig     // model profiles for parameter merging (D-09/D-10/D-11)
	RetryConfig   StreamRetryConfig              // streaming retry configuration (D-13/D-16)
}

// NewBaseClient creates a BaseClient with the given settings.
func NewBaseClient(apiKey, baseURL, version string, cacheTTL, cacheStaleTTL time.Duration, healthLiveMs, healthSlowMs int64, profiles *config.ModelProfileConfig, retryConfig StreamRetryConfig) BaseClient {
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
	// Default retry config: initial_only with 3 attempts, 1s base delay
	if retryConfig.Mode == "" {
		retryConfig.Mode = "initial_only"
	}
	if retryConfig.MaxAttempts == 0 {
		retryConfig.MaxAttempts = 3
	}
	if retryConfig.BaseDelay == 0 {
		retryConfig.BaseDelay = time.Second
	}
	transport := getSharedTransport()
	return BaseClient{
		APIKeyField:  apiKey,
		BaseURLField: baseURL,
		Version:      version,
		Profiles:     profiles,
		RetryConfig:  retryConfig,
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
	defer func() {
		if err := resp.Body.Close(); err != nil {
			slog.Debug("close response body", "error", err, "resource", "health_check")
		}
	}()
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

// MergeProfile applies model profile merging to a ChatRequest.
// Precedence (lowest to highest):
//  1. Provider defaults from Profiles.ProviderDefaults[providerName]
//  2. Model overrides from Profiles.ModelOverrides[req.Model]
//  3. Request-level values (fields explicitly set in req)
//
// If Profiles is nil, returns the request unchanged.
// If ReasoningConfigRef is set in the merged profile, it overrides
// individual reasoning params by applying the referenced reasoning config.
func (b *BaseClient) MergeProfile(req types.ChatRequest) types.ChatRequest {
	if b.Profiles == nil {
		return req
	}

	merged := req
	providerName := req.Provider
	if providerName == "" {
		// If no provider specified in request, we can't apply provider defaults
		// but we can still apply model overrides
		providerName = "nvidia" // fallback for default model profile lookup
	}

	// Build merged profile by applying in precedence order
	var providerDefaults, modelOverride *types.ModelProfile
	if b.Profiles != nil {
		if pd, ok := b.Profiles.ProviderDefaults[providerName]; ok {
			providerDefaults = &pd
		}
		if mo, ok := b.Profiles.ModelOverrides[req.Model]; ok {
			modelOverride = &mo
		}
	}

	// Apply provider defaults (lowest precedence)
	if providerDefaults != nil {
		merged = applyProfile(merged, *providerDefaults)
	}

	// Apply model overrides (medium precedence) - explicitly override fields
	if modelOverride != nil {
		if modelOverride.Temperature != nil {
			merged.Temperature = modelOverride.Temperature
		}
		if modelOverride.TopP != nil {
			merged.TopP = modelOverride.TopP
		}
		if modelOverride.MaxTokens != nil {
			merged.MaxTokens = *modelOverride.MaxTokens
		}
		if modelOverride.ReasoningEnabled != nil {
			merged.ReasoningEnabled = *modelOverride.ReasoningEnabled
		}
		if modelOverride.ReasoningConfigRef != "" {
			merged.ReasoningConfigRef = modelOverride.ReasoningConfigRef
		}
	}

	// Request values have highest precedence - overlay explicitly set values
	if req.HasTemperature() {
		merged.Temperature = req.Temperature
	}
	if req.HasTopP() {
		merged.TopP = req.TopP
	}
	if req.HasMaxTokens() {
		merged.MaxTokens = req.MaxTokens
	}
	if req.HasReasoningEnabled() {
		merged.ReasoningEnabled = req.ReasoningEnabled
	}
	if req.HasReasoningConfigRef() {
		merged.ReasoningConfigRef = req.ReasoningConfigRef
	}

	// If ReasoningConfigRef is set, apply the referenced reasoning config
	if merged.ReasoningConfigRef != "" {
		if _, ok := GetReasoningConfig(merged.ReasoningConfigRef); ok {
			// Build a temporary body to apply the reasoning config, then extract params
			body := make(map[string]any)
			ApplyReasoningParams(merged.ReasoningConfigRef, body)
			// Note: ApplyReasoningParams only handles RequestParams and ExtraBodyParams
			// The reasoning config is applied at request body build time in the provider
		}
	}

	return merged
}

// applyProfile applies non-zero/non-nil fields from profile to request.
func applyProfile(req types.ChatRequest, profile types.ModelProfile) types.ChatRequest {
	if req.Temperature == nil && profile.Temperature != nil {
		req.Temperature = profile.Temperature
	}
	if req.TopP == nil && profile.TopP != nil {
		req.TopP = profile.TopP
	}
	if req.MaxTokens == 0 && profile.MaxTokens != nil {
		req.MaxTokens = *profile.MaxTokens
	}
	if !req.ReasoningEnabled && profile.ReasoningEnabled != nil {
		req.ReasoningEnabled = *profile.ReasoningEnabled
	}
	if req.ReasoningConfigRef == "" && profile.ReasoningConfigRef != "" {
		req.ReasoningConfigRef = profile.ReasoningConfigRef
	}
	// Note: ReasoningBudget is handled via ReasoningConfigRef or provider-specific body building
	return req
}

// isInitialConnectionError checks if an error indicates a failure during
// initial connection establishment (before any stream chunks are received).
// This includes network errors, context cancellation, and retryable HTTP errors
// (gateway errors, server errors, etc.) that occur on the initial request.
func isInitialConnectionError(err error) bool {
	if err == nil {
		return false
	}
	// Context cancellation/timeout
	if errors.Is(err, context.DeadlineExceeded) || errors.Is(err, context.Canceled) {
		return true
	}
	// Network-level errors
	var netErr net.Error
	if errors.As(err, &netErr) {
		return true
	}
	// Retryable HTTP errors (includes gateway errors, server errors, etc.)
	if IsRetryable(err) {
		return true
	}
	// HTTP transport errors
	if strings.Contains(err.Error(), "connection") ||
		strings.Contains(err.Error(), "dial") ||
		strings.Contains(err.Error(), "timeout") ||
		strings.Contains(err.Error(), "EOF") {
		return true
	}
	return false
}

// RetryStream executes a streaming request with retry logic based on StreamRetryConfig.
// doStreamFunc is called to perform the actual streaming request.
// For "initial_only" mode: retries only on initial connection errors.
// For "full_resume" mode: retries on any error (restarts entire request with exponential backoff).
// For "none" mode: no retries, returns error immediately.
func (b *BaseClient) RetryStream(ctx context.Context, req types.ChatRequest, doStreamFunc func(context.Context, types.ChatRequest) (*types.StreamIterator, error)) (*types.StreamIterator, error) {
	// "none" mode: no retry
	if b.RetryConfig.Mode == "none" {
		return doStreamFunc(ctx, req)
	}

	var lastErr error
	for attempt := 0; attempt < b.RetryConfig.MaxAttempts; attempt++ {
		iter, err := doStreamFunc(ctx, req)
		if err == nil {
			return iter, nil
		}

		lastErr = err

		// Check if we should retry
		shouldRetry := false
		switch b.RetryConfig.Mode {
		case "initial_only":
			// Only retry on initial connection errors
			shouldRetry = isInitialConnectionError(err)
		case "full_resume":
			// Retry on any error (restart entire request)
			shouldRetry = true
		}

		if !shouldRetry || attempt >= b.RetryConfig.MaxAttempts-1 {
			break
		}

		// Exponential backoff
		delay := b.RetryConfig.BaseDelay * time.Duration(1<<uint(attempt))
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		case <-time.After(delay):
			continue
		}
	}

	return nil, lastErr
}
